use adw::prelude::*;
use qrcode::{Color, QrCode};
use std::rc::Rc;
use std::thread;

/// Pairing method selection.
#[derive(Debug, Clone)]
pub enum PairMethod {
    /// QR code flow using mDNS service discovery.
    QrCode,
    /// Manual flow via `adb pair <ip>:<port> <code>`.
    Manual {
        ip: String,
        port: String,
        code: String,
    },
}

/// Events emitted during the pairing process.
#[derive(Debug)]
pub enum PairEvent {
    /// QR code payload generated.
    DecodedString(String),
    /// Pairing succeeded with device address and port.
    PairSuccess(String, u16),
    PairFailed(String),
}

/// Runs QR-based pairing via mDNS discovery.
fn run_native_pairing_qr(event_tx: async_channel::Sender<PairEvent>) {
    let service = match crate::adb::pair_service::PairService::new() {
        Ok(s) => s,
        Err(e) => {
            let _ = event_tx.try_send(PairEvent::PairFailed(format!(
                "Failed to initialize pairing service: {}",
                e
            )));
            return;
        }
    };

    let qr_payload = service.qr_text();
    let _ = event_tx.try_send(PairEvent::DecodedString(qr_payload.clone()));

    if let Err(e) = service.start_discovery() {
        let _ = event_tx.try_send(PairEvent::PairFailed(format!(
            "mDNS registration failed: {}",
            e
        )));
        return;
    }

    let tx = event_tx.clone();
    let password = service.password.clone();

    std::thread::spawn(move || match service.wait_for_pairing() {
        Ok(device) => {
            match crate::adb::pair_service::PairService::execute_pair_and_connect(&device, &password) {
                Ok(info) => {
                    let _ = tx.try_send(PairEvent::PairSuccess(
                        info.address.to_string(),
                        info.debugging_port,
                    ));
                }
                Err(e) => {
                    let _ = tx.try_send(PairEvent::PairFailed(e));
                }
            }
        }
        Err(e) => {
            let _ = event_tx.try_send(PairEvent::PairFailed(format!(
                "Device discovery failed: {}",
                e
            )));
        }
    });
}

/// Queries adb for the debug port of a recently-paired device at the given IP.
/// Parses `adb devices -l` output to find the device's actual connection address.
fn query_debug_port(ip: &str) -> Option<(String, u16)> {
    let output = std::process::Command::new("adb")
        .args(["devices", "-l"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.len() >= 2 && parts[1] == "device" {
            let addr = parts[0];
            if let Some(ip_part) = addr.strip_suffix(":5555") {
                if ip_part == ip {
                    return Some((ip_part.to_string(), 5555));
                }
            }
            // Try stripping any port suffix to compare IPs
            let addr_ip = addr.split(':').next()?;
            if addr_ip == ip {
                let addr_port: u16 = addr.split(':').nth(1)?.parse().ok()?;
                return Some((addr_ip.to_string(), addr_port));
            }
        }
    }
    None
}

/// Runs manual pairing via `adb pair <ip>:<port> <code>`.
fn run_adb_pair_manual(
    ip: String,
    port: String,
    code: String,
    event_tx: async_channel::Sender<PairEvent>,
) {
    let addr = format!("{}:{}", ip.trim(), port.trim());

    let mut child = match std::process::Command::new("adb")
        .args(["pair", &addr, &code])
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

    let mut stdout = child.stdout.take().expect("stdout piped");
    let mut stderr = child.stderr.take().expect("stderr piped");

    let event_tx_reader = event_tx.clone();
    let ip_clone = ip.clone();
    thread::spawn(move || {
        use std::io::Read;

        let mut chunk_buf = vec![0u8; 4096];
        let mut found_success = false;

        loop {
            let mut active = false;

            match stdout.read(&mut chunk_buf) {
                Ok(0) => {}
                Ok(n) => {
                    active = true;
                    let chunk = String::from_utf8_lossy(&chunk_buf[..n]);
                    for line in chunk.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let lower = trimmed.to_lowercase();
                            if !found_success
                                && (lower.contains("successfully paired")
                                    || lower.contains("pairing successful")
                                    || lower.contains("pairing established")
                                    || (lower.contains("paired") && lower.contains("success")))
                            {
                                found_success = true;
                                // Query adb devices to get the actual debug port after pairing,
                                // instead of returning the pairing port the user entered.
                                if let Some((real_ip, real_port)) = query_debug_port(&ip_clone) {
                                    let _ = event_tx_reader.try_send(PairEvent::PairSuccess(
                                        real_ip, real_port,
                                    ));
                                } else {
                                    // Fallback: assume default wireless debug port 5555
                                    let _ = event_tx_reader.try_send(PairEvent::PairSuccess(
                                        ip_clone.clone(),
                                        5555,
                                    ));
                                }
                            }
                            if lower.contains("error:")
                                || lower.contains("failed")
                                || lower.contains("refused")
                                || lower.contains("connection refused")
                            {
                                let _ = event_tx_reader
                                    .try_send(PairEvent::PairFailed(trimmed.to_string()));
                                return;
                            }
                        }
                    }
                }
                Err(_) => {}
            }

            match stderr.read(&mut chunk_buf) {
                Ok(0) => {}
                Ok(n) => {
                    active = true;
                    let chunk = String::from_utf8_lossy(&chunk_buf[..n]);
                    for line in chunk.lines() {
                        let trimmed = line.trim();
                        if !trimmed.is_empty() {
                            let lower = trimmed.to_lowercase();
                            if !found_success
                                && (lower.contains("successfully paired")
                                    || lower.contains("pairing successful")
                                    || lower.contains("pairing established")
                                    || (lower.contains("paired") && lower.contains("success")))
                            {
                                found_success = true;
                                if let Some((real_ip, real_port)) = query_debug_port(&ip_clone) {
                                    let _ = event_tx_reader.try_send(PairEvent::PairSuccess(
                                        real_ip, real_port,
                                    ));
                                } else {
                                    let _ = event_tx_reader.try_send(PairEvent::PairSuccess(
                                        ip_clone.clone(),
                                        5555,
                                    ));
                                }
                            }
                            if lower.contains("error:")
                                || lower.contains("failed")
                                || lower.contains("refused")
                                || lower.contains("connection refused")
                            {
                                let _ = event_tx_reader
                                    .try_send(PairEvent::PairFailed(trimmed.to_string()));
                                return;
                            }
                        }
                    }
                }
                Err(_) => {}
            }

            if !active {
                if !found_success {
                    let _ = event_tx_reader.try_send(PairEvent::PairFailed(
                        "Pairing process ended unexpectedly".to_string(),
                    ));
                }
                break;
            }

            thread::sleep(std::time::Duration::from_millis(10));
        }
    });

    let _ = child.wait();
}

/// Configuration for the pairing process.
struct PairingConfig {
    method: PairMethod,
    stack: gtk::Stack,
    modal: adw::Window,
    error_lbl: gtk::Label,
    qr_image: gtk::Image,
    spinner: gtk::Spinner,
    loading_lbl: gtk::Label,
    device_name: String,
    on_success: Rc<dyn Fn(String, u16)>,
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}…", &s[..max])
    } else {
        s.to_string()
    }
}

fn start_pairing(config: PairingConfig) {
    let method = config.method.clone();
    if matches!(method, PairMethod::QrCode) {
        config.spinner.set_visible(true);
        config.spinner.start();
        config.loading_lbl.set_visible(true);
    }

    let (pair_tx, pair_rx) = async_channel::bounded::<PairEvent>(8);

    match method {
        PairMethod::QrCode => {
            let tx = pair_tx.clone();
            thread::spawn(move || {
                run_native_pairing_qr(tx);
            });
        }
        PairMethod::Manual { ip, port, code } => {
            let tx = pair_tx.clone();
            thread::spawn(move || {
                run_adb_pair_manual(ip, port, code, tx);
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
                        let display_size = 200;
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
                            stride,
                        );

                        qr_image_c.set_paintable(Some(&texture));
                        qr_image_c.set_visible(true);
                    }
                }

                PairEvent::PairSuccess(address, port) => {
                    modal_c.close();
                    on_success(address, port);
                    break;
                }

                PairEvent::PairFailed(msg) => {
                    err_c.set_label(&format!("Pairing failed: {}", truncate(&msg, 130)));
                    err_c.set_visible(true);
                    stack_c.set_visible_child_name("connect");
                    break;
                }
            }
        }
    });
}

/// Opens the connection modal (Step 2) for device pairing.
pub fn show_connect_modal(
    parent: &adw::ApplicationWindow,
    device_name: String,
    on_success: Box<dyn Fn(String, u16)>,
) {
    let modal = adw::Window::builder()
        .modal(true)
        .transient_for(parent)
        .title("Add Device")
        .default_width(780)
        .default_height(520)
        .resizable(false)
        .deletable(true)
        .build();

    let toast_overlay = adw::ToastOverlay::new();
    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .transition_duration(220)
        .build();
    toast_overlay.set_child(Some(&stack));

    let on_success: Rc<dyn Fn(String, u16)> = Rc::from(on_success);

    let (connect_page, error_lbl, qr_image, spinner, loading_lbl) = build_connect_page(
        &device_name,
        stack.clone(),
        modal.clone(),
        on_success.clone(),
    );
    stack.add_named(&connect_page, Some("connect"));

    let (pairing_page, pair_spinner) = build_status_page(
        "Pairing Device…",
        &format!("Running pairing for \"{}\" ", device_name),
        "Keep your phone unlocked with Wireless debugging enabled",
    );
    stack.add_named(&pairing_page, Some("pairing"));

    stack.set_visible_child_name("connect");

    stack.connect_visible_child_notify({
        let sp = pair_spinner.clone();
        move |s| match s.visible_child_name().as_deref() {
            Some("pairing") => sp.start(),
            _ => sp.stop(),
        }
    });

    let hdr = adw::HeaderBar::builder()
        .show_end_title_buttons(true)
        .build();
    hdr.add_css_class("flat");

    let tv = adw::ToolbarView::new();
    tv.add_top_bar(&hdr);
    tv.set_content(Some(&toast_overlay));

    modal.set_content(Some(&tv));
    modal.present();

    start_pairing(PairingConfig {
        method: PairMethod::QrCode,
        stack: stack.clone(),
        modal: modal.clone(),
        error_lbl: error_lbl.clone(),
        qr_image: qr_image.clone(),
        spinner: spinner.clone(),
        loading_lbl: loading_lbl.clone(),
        device_name: device_name.clone(),
        on_success: on_success.clone(),
    });
}

/// Builds the main connection page with QR and manual panels.
fn build_connect_page(
    device_name: &str,
    stack: gtk::Stack,
    modal: adw::Window,
    on_success: Rc<dyn Fn(String, u16)>,
) -> (gtk::Box, gtk::Label, gtk::Image, gtk::Spinner, gtk::Label) {
    let root = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .margin_start(32)
        .margin_end(32)
        .margin_top(20)
        .margin_bottom(28)
        .build();
    let title = gtk::Label::builder()
        .label("Connect Your Android Device")
        .halign(gtk::Align::Center)
        .margin_bottom(6)
        .build();
    title.add_css_class("step2-title");

    let subtitle = gtk::Label::builder()
        .label("Pair your phone wirelessly via QR code or IP address")
        .halign(gtk::Align::Center)
        .margin_bottom(20)
        .build();
    subtitle.add_css_class("step2-subtitle");

    root.append(&title);
    root.append(&subtitle);

    let panels = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .vexpand(true)
        .build();

    let (left, ip_e, port_e, code_e) = build_manual_panel();
    let divider = gtk::Separator::new(gtk::Orientation::Vertical);
    divider.add_css_class("step2-panel-divider");
    let (right, qr_image, spinner, loading_lbl) = build_qr_panel();

    panels.append(&left);
    panels.append(&divider);
    panels.append(&right);
    root.append(&panels);

    let error_lbl = gtk::Label::builder()
        .label(" ")
        .halign(gtk::Align::Start)
        .xalign(0.0)
        .wrap(true)
        .margin_top(8)
        .build();
    error_lbl.add_css_class("step2-error-label");
    error_lbl.set_visible(false);
    root.append(&error_lbl);

    let pair_btn = gtk::Button::builder()
        .label("Pair via IP/Port")
        .margin_top(4)
        .build();
    pair_btn.add_css_class("suggested-action");
    pair_btn.add_css_class("step2-connect-btn");
    root.append(&pair_btn);

    let dn = device_name.to_string();
    pair_btn.connect_clicked({
        let ip_e = ip_e.clone();
        let port_e = port_e.clone();
        let code_e = code_e.clone();
        let err = error_lbl.clone();
        let stack = stack.clone();
        let modal = modal.clone();
        let dn = dn.clone();
        let qr_image = qr_image.clone();
        let spinner = spinner.clone();
        let loading_lbl = loading_lbl.clone();
        let on_success = on_success.clone();

        move |_| {
            let ip = ip_e.text().to_string();
            let port = port_e.text().to_string();
            let code = code_e.text().to_string();

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
            if code.trim().is_empty()
                || code.trim().len() != 6
                || !code.trim().chars().all(|c| c.is_ascii_digit())
            {
                err.set_label("Please enter the 6-digit pairing code.");
                err.set_visible(true);
                code_e.add_css_class("error");
                code_e.grab_focus();
                return;
            }
            err.set_visible(false);
            ip_e.remove_css_class("error");
            port_e.remove_css_class("error");
            code_e.remove_css_class("error");

            stack.set_visible_child_name("pairing");

            start_pairing(PairingConfig {
                method: PairMethod::Manual { ip, port, code },
                stack: stack.clone(),
                modal: modal.clone(),
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

/// Builds the manual connection panel.
fn build_manual_panel() -> (gtk::Box, gtk::Entry, gtk::Entry, gtk::Entry) {
    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .hexpand(true)
        .margin_end(24)
        .build();
    let heading = gtk::Label::builder()
        .label("Manual Connection")
        .halign(gtk::Align::Start)
        .margin_bottom(14)
        .build();
    heading.add_css_class("step2-panel-heading");
    panel.append(&heading);

    let ip_lbl = gtk::Label::builder()
        .label("Device IP")
        .halign(gtk::Align::Start)
        .margin_bottom(4)
        .build();
    ip_lbl.add_css_class("step2-field-label");
    panel.append(&ip_lbl);

    let ip_e = gtk::Entry::builder()
        .placeholder_text("192.168.1.x")
        .hexpand(true)
        .build();
    ip_e.add_css_class("step2-field-entry");
    panel.append(&ip_e);

    let port_lbl = gtk::Label::builder()
        .label("Pairing Port")
        .halign(gtk::Align::Start)
        .margin_top(10)
        .margin_bottom(4)
        .build();
    port_lbl.add_css_class("step2-field-label");
    panel.append(&port_lbl);

    let port_e = gtk::Entry::builder()
        .placeholder_text("37845")
        .hexpand(true)
        .build();
    port_e.add_css_class("step2-field-entry");
    panel.append(&port_e);

    let code_lbl = gtk::Label::builder()
        .label("Pairing Code")
        .halign(gtk::Align::Start)
        .margin_top(10)
        .margin_bottom(4)
        .build();
    code_lbl.add_css_class("step2-field-label");
    panel.append(&code_lbl);

    let code_e = gtk::Entry::builder()
        .placeholder_text("000000")
        .hexpand(true)
        .build();
    code_e.add_css_class("step2-field-entry");
    code_e.set_input_purpose(gtk::InputPurpose::Digits);
    code_e.set_max_length(6);
    panel.append(&code_e);

    let hint = gtk::Label::builder()
        .label("Use the pairing port and code shown under\nWireless Debugging on your phone.")
        .halign(gtk::Align::Start)
        .wrap(true)
        .margin_top(12)
        .build();
    hint.add_css_class("step2-qr-hint");
    panel.append(&hint);

    ip_e.connect_changed({
        let e = ip_e.clone();
        move |_| e.remove_css_class("error")
    });
    port_e.connect_changed({
        let e = port_e.clone();
        move |_| e.remove_css_class("error")
    });
    code_e.connect_changed({
        let e = code_e.clone();
        move |_| e.remove_css_class("error")
    });

    (panel, ip_e, port_e, code_e)
}

/// Builds the QR code display panel.
fn build_qr_panel() -> (gtk::Box, gtk::Image, gtk::Spinner, gtk::Label) {
    let panel = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .margin_start(24)
        .spacing(12)
        .build();

    let heading = gtk::Label::builder()
        .label("Scan QR with Phone")
        .halign(gtk::Align::Start)
        .build();
    heading.add_css_class("step2-panel-heading");
    panel.append(&heading);

    let qr_container = gtk::Box::builder()
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
        }",
        );

        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    });
    let qr_image = gtk::Image::builder()
        .pixel_size(200)
        .visible(false)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    qr_image.add_css_class("qr-image");
    qr_container.append(&qr_image);

    panel.append(&qr_container);

    let spinner = gtk::Spinner::builder()
        .width_request(32)
        .height_request(32)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    panel.append(&spinner);

    let loading_lbl = gtk::Label::builder()
        .label("Generating pairing credentials...")
        .halign(gtk::Align::Center)
        .css_classes(["dim-label"])
        .build();
    panel.append(&loading_lbl);

    let hint = gtk::Label::builder()
        .label("Scan this QR code with your phone's\nWireless Debugging QR scanner.")
        .halign(gtk::Align::Start)
        .wrap(true)
        .build();
    hint.add_css_class("step2-qr-hint");
    panel.append(&hint);

    (panel, qr_image, spinner, loading_lbl)
}

/// Builds the pairing-in-progress status page.
fn build_status_page(
    title_text: &str,
    subtitle_text: &str,
    hint_text: &str,
) -> (gtk::Box, gtk::Spinner) {
    let page = gtk::Box::builder()
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

    let title = gtk::Label::builder()
        .label(title_text)
        .margin_top(4)
        .build();
    title.add_css_class("connecting-title");
    page.append(&title);

    let subtitle = gtk::Label::builder()
        .label(subtitle_text)
        .halign(gtk::Align::Center)
        .justify(gtk::Justification::Center)
        .wrap(true)
        .max_width_chars(50)
        .build();
    subtitle.add_css_class("connecting-device-label");
    page.append(&subtitle);

    let hint = gtk::Label::builder()
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
