use adw::prelude::*;
use qrcode::{ QrCode, Color };
use std::cell::RefCell;
use std::rc::Rc;
use std::thread;

// ADB interaction types
#[derive(Debug, Clone)]
pub enum PairMethod {
    /// QR Code flow: generate pairing string directly
    QrCode,
    /// Manual flow: `adb pair <ip>:<port>`
    Manual {
        ip: String,
        port: String,
    },
}

#[derive(Debug)]
pub enum PairEvent {
    /// Generated pairing string
    DecodedString(String),
    /// Manual pairing: waiting for 6-digit code from user
    NeedsCode,
    PairSuccess,
    PairFailed(String),
    #[allow(dead_code)] StatusUpdate(String),
}

#[derive(Debug, Clone)]
pub enum CodeSubmit {
    Submit(String),
    Cancel,
}

// Native ADB wireless pairing via mDNS
fn run_native_pairing_qr(event_tx: async_channel::Sender<PairEvent>) {
    let service = match crate::adb::pair_service::PairService::new() {
        Ok(s) => s,
        Err(e) => {
            let _ = event_tx.try_send(
                PairEvent::PairFailed(format!("Failed to initialize pairing service: {}", e))
            );
            return;
        }
    };

    let qr_payload = service.qr_text();
    let _ = event_tx.try_send(PairEvent::DecodedString(qr_payload.clone()));

    if let Err(e) = service.start_discovery() {
        let _ = event_tx.try_send(
            PairEvent::PairFailed(format!("mDNS registration failed: {}", e))
        );
        return;
    }

    let tx = event_tx.clone();
    let password = service.password.clone();

    std::thread::spawn(move || {
        match service.wait_for_pairing() {
            Ok(device) => {
                match
                    crate::adb::pair_service::PairService::execute_pair_and_connect(
                        &device,
                        &password
                    )
                {
                    Ok(()) => {
                        let _ = tx.try_send(PairEvent::PairSuccess);
                    }
                    Err(e) => {
                        let _ = tx.try_send(PairEvent::PairFailed(e));
                    }
                }
            }
            Err(e) => {
                let _ = event_tx.try_send(
                    PairEvent::PairFailed(format!("Device discovery failed: {}", e))
                );
            }
        }
    });
}

// Manual pairing via `adb pair <ip>:<port>`
fn run_adb_pair_manual(
    ip: String,
    port: String,
    event_tx: async_channel::Sender<PairEvent>,
    code_rx: async_channel::Receiver<CodeSubmit>
) {
    let addr = format!("{}:{}", ip.trim(), port.trim());
    
    let mut child = match
        std::process::Command
            ::new("adb")
            .args(["pair", &addr])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
    {
        Ok(c) => c,
        Err(e) => {
            let _ = event_tx.try_send(PairEvent::PairFailed(format!("Cannot run adb: {}", e)));
            return;
        }
    };

    let stdin = child.stdin.take().expect("stdin piped");
    let stdout = child.stdout.take().expect("stdout piped");
    let stderr = child.stderr.take().expect("stderr piped");

    let (stream_tx, stream_rx) = std::sync::mpsc::channel();

    let stream_tx_read = stream_tx.clone();
    let reader_thread = thread::spawn(move || {
        use std::io::Read;
        
        // Read stdout in chunks
        {
            let mut reader = stdout;
            let mut chunk_buf = vec![0u8; 4096];
            let mut accumulated = String::new();
            
            loop {
                match reader.read(&mut chunk_buf) {
                    Ok(0) => {
                        if !accumulated.trim().is_empty() {
                            let _ = stream_tx_read.send(("stdout", accumulated.trim().to_string()));
                        }
                        break;
                    }
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&chunk_buf[..n]);
                        accumulated.push_str(&chunk);
                        
                        while let Some(newline_pos) = accumulated.find('\n') {
                            let line = accumulated[..newline_pos].to_string();
                            accumulated = accumulated[newline_pos + 1..].to_string();
                            
                            if !line.trim().is_empty() {
                                let _ = stream_tx_read.send(("stdout", line.trim().to_string()));
                            }
                        }
                        
                        if accumulated.contains("Enter pairing code:") ||
                           accumulated.to_lowercase().contains("enter code") ||
                           accumulated.to_lowercase().contains("code:") {
                            let _ = stream_tx_read.send(("stdout_prompt", accumulated.trim().to_string()));
                            accumulated.clear();
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        }
        
        // Read stderr in chunks
        {
            let mut reader = stderr;
            let mut chunk_buf = vec![0u8; 4096];
            let mut accumulated = String::new();
            
            loop {
                match reader.read(&mut chunk_buf) {
                    Ok(0) => {
                        if !accumulated.trim().is_empty() {
                            let _ = stream_tx_read.send(("stderr", accumulated.trim().to_string()));
                        }
                        break;
                    }
                    Ok(n) => {
                        let chunk = String::from_utf8_lossy(&chunk_buf[..n]);
                        accumulated.push_str(&chunk);
                        
                        while let Some(newline_pos) = accumulated.find('\n') {
                            let line = accumulated[..newline_pos].to_string();
                            accumulated = accumulated[newline_pos + 1..].to_string();
                            
                            if !line.trim().is_empty() {
                                let _ = stream_tx_read.send(("stderr", line.trim().to_string()));
                            }
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        }
    });
    
    drop(stream_tx);

    // Handle events and code submission
    let event_tx_main = event_tx.clone();
    let code_rx_main = code_rx.clone();
    let main_thread = thread::spawn(move || {
        use std::io::Write;
        let mut stdin = stdin;
        let mut code_prompt_detected = false;
        let mut attempt_count = 0;
        let max_attempts = 300;
        
        loop {
            attempt_count += 1;
            if attempt_count > max_attempts {
                let _ = event_tx_main.try_send(
                    PairEvent::PairFailed("Pairing timeout - no response from device".to_string())
                );
                break;
            }
            
            match stream_rx.recv_timeout(std::time::Duration::from_millis(50)) {
                Ok((source, line)) => {
                    let trimmed = line.trim();
                    let lower_trimmed = trimmed.to_lowercase();
                    
                    // Detect pairing code prompt
                    if !code_prompt_detected &&
                       (source == "stdout_prompt" ||
                        lower_trimmed.contains("enter pairing code") ||
                        lower_trimmed.contains("enter code") ||
                        lower_trimmed.contains("pairing code:") ||
                        lower_trimmed.ends_with("code:")) {
                        let _ = event_tx_main.try_send(PairEvent::NeedsCode);
                        code_prompt_detected = true;
                        // Don't reset attempt_count here - let the timeout handle it
                    }
                    
                    // Detect success
                    if lower_trimmed.contains("successfully paired") ||
                       lower_trimmed.contains("pairing successful") ||
                       lower_trimmed.contains("pairing established") ||
                       (lower_trimmed.contains("paired") && lower_trimmed.contains("success")) {
                        let _ = event_tx_main.try_send(PairEvent::PairSuccess);
                        break;
                    }
                    
                    // Detect errors (only if haven't seen prompt yet)
                    if !code_prompt_detected &&
                       (lower_trimmed.contains("error:") ||
                        lower_trimmed.contains("failed") ||
                        lower_trimmed.contains("refused") ||
                        lower_trimmed.contains("connection refused")) {
                        let _ = event_tx_main.try_send(
                            PairEvent::PairFailed(trimmed.to_string())
                        );
                        break;
                    }
                    
                    attempt_count = 0;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Check if user submitted a code
                    if let Ok(CodeSubmit::Submit(code)) = code_rx_main.try_recv() {
                        if writeln!(stdin, "{}", code.trim()).is_ok() {
                            let _ = stdin.flush();
                            code_prompt_detected = false;
                        }
                    } else if let Ok(CodeSubmit::Cancel) = code_rx_main.try_recv() {
                        let _ = event_tx_main.try_send(
                            PairEvent::PairFailed("User cancelled".to_string())
                        );
                        break;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    break;
                }
            }
        }
    });

    let _ = main_thread.join();
    let _ = reader_thread.join();
    let _ = child.wait();
}

// GTK orchestration
struct PairingConfig {
    method: PairMethod,
    stack: gtk::Stack,
    modal: adw::Window,
    code_sender: Rc<RefCell<Option<async_channel::Sender<CodeSubmit>>>>,
    error_lbl: gtk::Label,
    qr_image: gtk::Image,
    spinner: gtk::Spinner,
    loading_lbl: gtk::Label,
    device_name: String,
    on_success: Rc<dyn Fn()>,
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max { format!("{}…", &s[..max]) } else { s.to_string() }
}

fn start_pairing(config: PairingConfig) {
    let method = config.method.clone();
    if matches!(method, PairMethod::QrCode) {
        config.spinner.set_visible(true);
        config.spinner.start();
        config.loading_lbl.set_visible(true);
    }

    let (pair_tx, pair_rx) = async_channel::bounded::<PairEvent>(8);
    let (code_tx, code_rx) = async_channel::bounded::<CodeSubmit>(1);
    *config.code_sender.borrow_mut() = Some(code_tx);

    match method {
        PairMethod::QrCode => {
            let tx = pair_tx.clone();
            thread::spawn(move || {
                run_native_pairing_qr(tx);
            });
        }
        PairMethod::Manual { ip, port } => {
            let tx = pair_tx.clone();
            let rx = code_rx.clone();
            thread::spawn(move || {
                run_adb_pair_manual(ip, port, tx, rx);
            });
        }
    }

    let stack_c = config.stack.clone();
    let modal_c = config.modal.clone();
    let err_c = config.error_lbl.clone();
    let spinner_c = config.spinner.clone();
    let loading_lbl_c = config.loading_lbl.clone();
    let qr_image_c = config.qr_image.clone();
    let _dn = config.device_name.clone();
    let on_success = config.on_success.clone();

    glib::MainContext::default().spawn_local(async move {
        while let Ok(event) = pair_rx.recv().await {
            match event {
                PairEvent::DecodedString(decoded) => {
                    spinner_c.stop();
                    spinner_c.set_visible(false);
                    loading_lbl_c.set_visible(false);

                    if let Ok(code) = QrCode::new(decoded.as_bytes()) {
                        let size = code.width();
                        let display_size = 300;
                        let module_px = display_size / size;
                        let actual_size = module_px * size;

                        let mut pixels = Vec::with_capacity(actual_size * actual_size * 4);

                        for my in 0..size {
                            for _ in 0..module_px {
                                for mx in 0..size {
                                    let color = match code[(mx, my)] {
                                        Color::Dark => [0u8, 0, 0, 255],
                                        Color::Light => [255u8, 255, 255, 255],
                                    };
                                    for _ in 0..module_px {
                                        pixels.extend_from_slice(&color);
                                    }
                                }
                            }
                        }

                        let bytes = glib::Bytes::from(&pixels[..]);
                        let stride = actual_size * 4;

                        let texture = gdk::MemoryTexture::new(
                            actual_size as i32,
                            actual_size as i32,
                            gdk::MemoryFormat::R8g8b8a8,
                            &bytes,
                            stride
                        );

                        qr_image_c.set_paintable(Some(&texture));
                        qr_image_c.set_visible(true);
                    }
                }

                PairEvent::NeedsCode => {
                    stack_c.set_visible_child_name("enter_code");
                }

                PairEvent::PairSuccess => {
                    modal_c.close();
                    on_success();
                    break;
                }

                PairEvent::PairFailed(msg) => {
                    err_c.set_label(&format!("Pairing failed: {}", truncate(&msg, 130)));
                    err_c.set_visible(true);
                    stack_c.set_visible_child_name("connect");
                    break;
                }

                #[allow(unused_variables)]
                PairEvent::StatusUpdate(_status) => {
                    // Optional: log status updates for debugging
                }
            }
        }
    });
}

// Public entry point
pub fn show_connect_modal(
    parent: &adw::ApplicationWindow,
    device_name: String,
    on_success: Box<dyn Fn()>
) {
    let modal = adw::Window
        ::builder()
        .modal(true)
        .transient_for(parent)
        .title("Add Device")
        .default_width(780)
        .default_height(520)
        .resizable(false)
        .deletable(true)
        .build();

    let toast_overlay = adw::ToastOverlay::new();
    let stack = gtk::Stack
        ::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(220)
        .build();
    toast_overlay.set_child(Some(&stack));

    // Shared channel sender
    let code_sender: Rc<RefCell<Option<async_channel::Sender<CodeSubmit>>>> = Rc::new(
        RefCell::new(None)
    );

    let on_success: Rc<dyn Fn()> = Rc::from(on_success);

    // Two-panel connect page
    let (connect_page, error_lbl, qr_image, spinner, loading_lbl) = build_connect_page(
        &device_name,
        stack.clone(),
        modal.clone(),
        code_sender.clone(),
        on_success.clone()
    );
    stack.add_named(&connect_page, Some("connect"));

    // Pairing spinner page
    let (pairing_page, pair_spinner) = build_status_page(
        "Pairing Device…",
        &format!("Running pairing for \"{}\" ", device_name),
        "Keep your phone unlocked with Wireless debugging enabled"
    );
    stack.add_named(&pairing_page, Some("pairing"));

    // Enter 6-digit pairing code page
    let code_page = build_enter_code_page(&device_name, &stack, code_sender.clone(), &error_lbl);
    stack.add_named(&code_page, Some("enter_code"));

    stack.set_visible_child_name("connect");

    // Spinner auto-start/stop
    stack.connect_visible_child_notify({
        let sp = pair_spinner.clone();
        move |s| {
            match s.visible_child_name().as_deref() {
                Some("pairing") => sp.start(),
                _ => sp.stop(),
            }
        }
    });

    let hdr = adw::HeaderBar::builder().show_end_title_buttons(true).build();
    hdr.add_css_class("flat");

    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&hdr);
    tv.set_content(Some(&toast_overlay));

    modal.set_content(Some(&tv));
    modal.present();

    // Start QR pairing flow
    start_pairing(PairingConfig {
        method: PairMethod::QrCode,
        stack: stack.clone(),
        modal: modal.clone(),
        code_sender: code_sender.clone(),
        error_lbl: error_lbl.clone(),
        qr_image: qr_image.clone(),
        spinner: spinner.clone(),
        loading_lbl: loading_lbl.clone(),
        device_name: device_name.clone(),
        on_success: on_success.clone(),
    });
}

// Page builders
fn build_connect_page(
    device_name: &str,
    stack: gtk::Stack,
    modal: adw::Window,
    code_sender: Rc<RefCell<Option<async_channel::Sender<CodeSubmit>>>>,
    on_success: Rc<dyn Fn()>
) -> (gtk::Box, gtk::Label, gtk::Image, gtk::Spinner, gtk::Label) {
    let root = gtk::Box
        ::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_start(32)
        .margin_end(32)
        .margin_top(20)
        .margin_bottom(28)
        .build();
    let title = gtk::Label
        ::builder()
        .label("Connect Your Android Device")
        .halign(gtk::Align::Center)
        .margin_bottom(6)
        .build();
    title.add_css_class("step2-title");

    let subtitle = gtk::Label
        ::builder()
        .label("Pair your phone wirelessly via QR code or IP address")
        .halign(gtk::Align::Center)
        .margin_bottom(20)
        .build();
    subtitle.add_css_class("step2-subtitle");

    root.append(&title);
    root.append(&subtitle);

    // Two panels side-by-side
    let panels = gtk::Box
        ::builder()
        .orientation(gtk::Orientation::Horizontal)
        .vexpand(true)
        .build();

    let (left, ip_e, port_e) = build_manual_panel();
    let divider = gtk::Separator::new(gtk::Orientation::Vertical);
    divider.add_css_class("step2-panel-divider");
    let (right, qr_image, spinner, loading_lbl) = build_qr_panel();

    panels.append(&left);
    panels.append(&divider);
    panels.append(&right);
    root.append(&panels);

    // Shared error label
    let error_lbl = gtk::Label
        ::builder()
        .label(" ")
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .wrap(true)
        .margin_top(8)
        .build();
    error_lbl.add_css_class("step2-error-label");
    error_lbl.set_visible(false);
    root.append(&error_lbl);

    // "Pair Device" button for manual flow
    let pair_btn = gtk::Button::builder().label("Pair via IP/Port").margin_top(4).build();
    pair_btn.add_css_class("suggested-action");
    pair_btn.add_css_class("step2-connect-btn");
    root.append(&pair_btn);

    // Wire up button handler
    let dn = device_name.to_string();
    pair_btn.connect_clicked({
        let ip_e = ip_e.clone();
        let port_e = port_e.clone();
        let err = error_lbl.clone();
        let stack = stack.clone();
        let modal = modal.clone();
        let cs = code_sender.clone();
        let dn = dn.clone();
        let qr_image = qr_image.clone();
        let spinner = spinner.clone();
        let loading_lbl = loading_lbl.clone();
        let on_success = on_success.clone();

        move |_| {
            let ip = ip_e.text().to_string();
            let port = port_e.text().to_string();

            if ip.trim().is_empty() {
                err.set_label("Please enter the device IP address.");
                err.set_visible(true);
                ip_e.add_css_class("error");
                ip_e.grab_focus();
                return;
            }
            if port.trim().is_empty() {
                err.set_label("Please enter the pairing port.");
                err.set_visible(true);
                port_e.add_css_class("error");
                port_e.grab_focus();
                return;
            }
            err.set_visible(false);
            ip_e.remove_css_class("error");
            port_e.remove_css_class("error");

            stack.set_visible_child_name("pairing");

            // Start manual pairing flow
            start_pairing(PairingConfig {
                method: PairMethod::Manual { ip, port },
                stack: stack.clone(),
                modal: modal.clone(),
                code_sender: cs.clone(),
                error_lbl: err.clone(),
                qr_image: qr_image.clone(),
                spinner: spinner.clone(),
                loading_lbl: loading_lbl.clone(),
                device_name: dn.clone(),
                on_success: on_success.clone(),
            });
        }
    });

    (root, error_lbl, qr_image, spinner, loading_lbl)
}

fn build_manual_panel() -> (gtk::Box, gtk::Entry, gtk::Entry) {
    let panel = gtk::Box
        ::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .hexpand(true)
        .margin_end(24)
        .build();
    let heading = gtk::Label
        ::builder()
        .label("Manual Connection")
        .halign(gtk::Align::Start)
        .margin_bottom(14)
        .build();
    heading.add_css_class("step2-panel-heading");
    panel.append(&heading);

    // Device IP
    let ip_lbl = gtk::Label
        ::builder()
        .label("Device IP")
        .halign(gtk::Align::Start)
        .margin_bottom(4)
        .build();
    ip_lbl.add_css_class("step2-field-label");
    panel.append(&ip_lbl);

    let ip_e = gtk::Entry::builder().placeholder_text("192.168.1.x").hexpand(true).build();
    ip_e.add_css_class("step2-field-entry");
    panel.append(&ip_e);

    // Port
    let port_lbl = gtk::Label
        ::builder()
        .label("Pairing Port")
        .halign(gtk::Align::Start)
        .margin_top(10)
        .margin_bottom(4)
        .build();
    port_lbl.add_css_class("step2-field-label");
    panel.append(&port_lbl);

    let port_e = gtk::Entry::builder().placeholder_text("37845").hexpand(true).build();
    port_e.add_css_class("step2-field-entry");
    panel.append(&port_e);

    let hint = gtk::Label
        ::builder()
        .label("Use the pairing port shown under\nWireless Debugging on your phone.")
        .halign(gtk::Align::Start)
        .wrap(true)
        .margin_top(12)
        .build();
    hint.add_css_class("step2-qr-hint");
    panel.append(&hint);

    // Clear error on change
    ip_e.connect_changed({
        let e = ip_e.clone();
        move |_| e.remove_css_class("error")
    });
    port_e.connect_changed({
        let e = port_e.clone();
        move |_| e.remove_css_class("error")
    });

    (panel, ip_e, port_e)
}

/// Right panel: shows loading spinner, then the generated QR code.
fn build_qr_panel() -> (gtk::Box, gtk::Image, gtk::Spinner, gtk::Label) {
    let panel = gtk::Box
        ::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .margin_start(24)
        .spacing(12)
        .build();

    let heading = gtk::Label
        ::builder()
        .label("Scan QR with Phone")
        .halign(gtk::Align::Start)
        .build();
    heading.add_css_class("step2-panel-heading");
    panel.append(&heading);

    let qr_container = gtk::Box
        ::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .css_classes(["card", "qr-container"])
        .build();

    qr_container.connect_realize(|w| {
        let display = w.display();

        let provider = gtk::CssProvider::new();
        provider.load_from_string(
            ".qr-container { 
            background-color: white; 
            padding: 16px; 
            border-radius: 8px; 
        }"
        );

        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION
        );
    });
    let qr_image = gtk::Image
        ::builder()
        .pixel_size(300)
        .visible(false)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    qr_image.add_css_class("qr-image");
    qr_container.append(&qr_image);

    panel.append(&qr_container);

    // Loading spinner (centered in same container area)
    let spinner = gtk::Spinner
        ::builder()
        .width_request(32)
        .height_request(32)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    panel.append(&spinner);

    let loading_lbl = gtk::Label
        ::builder()
        .label("Generating pairing credentials...")
        .halign(gtk::Align::Center)
        .css_classes(["dim-label"])
        .build();
    panel.append(&loading_lbl);

    let hint = gtk::Label
        ::builder()
        .label("Scan this QR code with your phone's\nWireless Debugging QR scanner.")
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    hint.add_css_class("step2-qr-hint");
    panel.append(&hint);

    (panel, qr_image, spinner, loading_lbl)
}

fn build_enter_code_page(
    device_name: &str,
    stack: &gtk::Stack,
    code_sender: Rc<RefCell<Option<async_channel::Sender<CodeSubmit>>>>,
    connect_error_lbl: &gtk::Label
) -> gtk::Box {
    let page = gtk::Box
        ::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .vexpand(true)
        .spacing(0)
        .margin_start(48)
        .margin_end(48)
        .build();

    let icon_wrap = gtk::Box::builder().halign(gtk::Align::Center).margin_bottom(20).build();
    icon_wrap.add_css_class("code-icon-wrap");
    let lock_icon = gtk::Image::from_icon_name("system-lock-screen-symbolic");
    lock_icon.set_pixel_size(40);
    lock_icon.add_css_class("code-icon");
    icon_wrap.append(&lock_icon);
    page.append(&icon_wrap);

    let title = gtk::Label
        ::builder()
        .label("Enter Pairing Code")
        .halign(gtk::Align::Center)
        .margin_bottom(6)
        .build();
    title.add_css_class("code-title");
    page.append(&title);

    let subtitle = gtk::Label
        ::builder()
        .label(
            format!("Your phone shows a 6-digit code for \"{}\"\nEnter it below to complete pairing", device_name)
        )
        .halign(gtk::Align::Center)
        .justify(gtk::Justification::Center)
        .wrap(true)
        .margin_bottom(22)
        .build();
    subtitle.add_css_class("code-subtitle");
    page.append(&subtitle);

    // 6-digit entry
    let entry = gtk::Entry
        ::builder()
        .placeholder_text("000000")
        .max_length(6)
        .input_purpose(gtk::InputPurpose::Digits)
        .halign(gtk::Align::Center)
        .width_chars(10)
        .build();
    entry.add_css_class("code-entry");
    page.append(&entry);

    // Inline error
    let err_lbl = gtk::Label::builder().label(" ").halign(gtk::Align::Center).margin_top(6).build();
    err_lbl.add_css_class("step2-error-label");
    err_lbl.set_visible(false);
    page.append(&err_lbl);

    // Buttons
    let btn_row = gtk::Box
        ::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(10)
        .halign(gtk::Align::Center)
        .margin_top(18)
        .build();

    let cancel_btn = gtk::Button::builder().label("Cancel").build();
    cancel_btn.add_css_class("code-cancel-btn");

    let submit_btn = gtk::Button::builder().label("Confirm Pairing").build();
    submit_btn.add_css_class("suggested-action");
    submit_btn.add_css_class("code-submit-btn");

    btn_row.append(&cancel_btn);
    btn_row.append(&submit_btn);
    page.append(&btn_row);

    // Submit logic
    let do_submit: Rc<dyn Fn()> = Rc::new({
        let entry = entry.clone();
        let err_lbl = err_lbl.clone();
        let code_sender = code_sender.clone();
        let stack = stack.clone();
        let conn_error_lbl = connect_error_lbl.clone();

        move || {
            let code = entry.text().to_string();
            let trimmed = code.trim().to_string();

            if trimmed.len() != 6 || !trimmed.chars().all(|c| c.is_ascii_digit()) {
                err_lbl.set_label("Please enter the exact 6-digit code.");
                err_lbl.set_visible(true);
                return;
            }
            err_lbl.set_visible(false);

            match code_sender.borrow().as_ref() {
                Some(tx) => {
                    let _ = tx.try_send(CodeSubmit::Submit(trimmed));
                    stack.set_visible_child_name("pairing");
                }
                None => {
                    conn_error_lbl.set_label("Internal error: no active pairing session.");
                    conn_error_lbl.set_visible(true);
                    stack.set_visible_child_name("connect");
                }
            }
        }
    });

    submit_btn.connect_clicked({
        let f = do_submit.clone();
        move |_| f()
    });
    entry.connect_activate({
        let f = do_submit.clone();
        move |_| f()
    });
    entry.connect_changed({
        let err_lbl = err_lbl.clone();
        move |_| err_lbl.set_visible(false)
    });

    // Cancel
    cancel_btn.connect_clicked({
        let stack = stack.clone();
        let code_sender = code_sender.clone();
        move |_| {
            if let Some(tx) = code_sender.borrow().as_ref() {
                let _ = tx.try_send(CodeSubmit::Cancel);
            }
            stack.set_visible_child_name("connect");
        }
    });

    page
}

fn build_status_page(
    title_text: &str,
    subtitle_text: &str,
    hint_text: &str
) -> (gtk::Box, gtk::Spinner) {
    let page = gtk::Box
        ::builder()
        .orientation(gtk::Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .vexpand(true)
        .spacing(12)
        .build();
    let spinner = gtk::Spinner::new();
    spinner.set_size_request(52, 52);
    spinner.add_css_class("step2-spinner");
    page.append(&spinner);

    let title = gtk::Label::builder().label(title_text).margin_top(4).build();
    title.add_css_class("connecting-title");
    page.append(&title);

    let subtitle = gtk::Label
        ::builder()
        .label(subtitle_text)
        .halign(gtk::Align::Center)
        .justify(gtk::Justification::Center)
        .wrap(true)
        .max_width_chars(50)
        .build();
    subtitle.add_css_class("connecting-device-label");
    page.append(&subtitle);

    let hint = gtk::Label
        ::builder()
        .label(hint_text)
        .halign(gtk::Align::Center)
        .justify(gtk::Justification::Center)
        .wrap(true)
        .max_width_chars(48)
        .build();
    hint.add_css_class("connecting-hint");
    page.append(&hint);

    (page, spinner)
}
