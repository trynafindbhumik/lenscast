use adw::prelude::*;
use adw::Application as AdwApplication;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::theme::setup_theme;
use crate::ui;
use crate::ui::devices::{refresh_connected_status, next_device_id, new_device_store, save_devices, Device};
use crate::ui::sidebar::{rebuild_device_list, DeviceListContext, RefreshFn};

const APP_ID: &str = "com.lenscast.app";

#[cfg(not(any(target_os = "macos", windows)))]
use crate::tray::{start_tray_message_handler, LensCastTray};
#[cfg(not(any(target_os = "macos", windows)))]
use ksni::TrayMethods;

pub fn run() {
    let app = AdwApplication::builder().application_id(APP_ID).build();

    let window_ref: Rc<RefCell<Option<adw::ApplicationWindow>>> = Rc::new(RefCell::new(None));
    let window_ref_clone = window_ref.clone();

    let hold_guards = Rc::new(RefCell::new(Vec::new()));
    let hold_guards_clone = hold_guards.clone();

    app.connect_activate(move |app| {
        if window_ref_clone.borrow().as_ref().map(|w| w.is_visible()).unwrap_or(false) {
            window_ref_clone.borrow().as_ref().unwrap().present();
            return;
        }

        hold_guards_clone.borrow_mut().push(app.hold());

        let device_store = new_device_store();
        refresh_connected_status(&device_store);

        let add_device_btn = ui::create_add_device_button();
        let header_bar = ui::create_header_bar(&add_device_btn);

        let (sidebar, device_listbox, selected_device_id) = ui::create_sidebar();
        let separator = ui::create_separator();
        let (content_container, content_area) = ui::create_content_area();
        let content_box = ui::create_content_box(&sidebar, &separator, &content_container);

        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&content_box));

        let window = ui::create_window(app, &header_bar, &toast_overlay);
        setup_theme(&window);

        // Clone for use in closures
        let toast_overlay_clone = toast_overlay.clone();
        let selected_device_id_clone = selected_device_id.clone();

        // Refresh holder for cross-referencing
        let refresh_holder: Rc<RefCell<Option<RefreshFn>>> = Rc::new(RefCell::new(None));

        // Deselect callback - resets content area to welcome
        let on_deselect: Rc<dyn Fn()> = {
            let content_area = content_area.clone();
            let selected_id = selected_device_id_clone.clone();
            Rc::new(move || {
                *selected_id.borrow_mut() = None;
                ui::reset_content_to_welcome(&content_area);
            })
        };

        // Device selection callback - shows device info in content area
        let on_device_select: Rc<dyn Fn(Option<u32>)> = {
            let store = device_store.clone();
            let content_area = content_area.clone();
            let selected_id = selected_device_id_clone.clone();
            let toast = toast_overlay_clone.clone();
            let refresh_h = refresh_holder.clone();

            Rc::new(move |device_id: Option<u32>| {
                *selected_id.borrow_mut() = device_id;

                match device_id {
                    Some(id) => {
                        let devices = store.borrow();
                        if let Some(device) = devices.iter().find(|d| d.id == id) {
                            // Create connect function for the content area's Connect button
                            let actual_connect_fn: Rc<dyn Fn()> = {
                                let store = store.clone();
                                let content_area = content_area.clone();
                                let selected_id_inner = selected_id.clone();
                                let toast = toast.clone();
                                let rh = refresh_h.clone();

                                Rc::new(move || {
                                    let addr = {
                                        let devices = store.borrow();
                                        devices.iter()
                                            .find(|d| d.id == id)
                                            .map(|d| (d.name.clone(), format!("{}:{}", d.address, d.port)))
                                            .unwrap_or_default()
                                    };

                                    if addr.1.is_empty() { return; }

                                    let device_name = addr.0.clone();
                                    let address = addr.1.clone();

                                    eprintln!("[Content] Connecting to: {}", address);
                                    
                                    let _ = std::process::Command::new("adb")
                                        .args(["connect", &address])
                                        .output();

                                    std::thread::sleep(std::time::Duration::from_millis(500));

                                    let verify_result = std::process::Command::new("adb")
                                        .args(["devices"])
                                        .output();

                                    let is_connected = match verify_result {
                                        Ok(out) => {
                                            let stdout = String::from_utf8_lossy(&out.stdout);
                                            stdout.lines().any(|line| {
                                                line.starts_with(&address) && line.contains("device")
                                            })
                                        }
                                        Err(_) => false,
                                    };

                                    if is_connected {
                                        if let Some(d) = store.borrow_mut().iter_mut().find(|d| d.id == id) {
                                            d.connected = true;
                                        }
                                        save_devices(&store.borrow());

                                        let t = adw::Toast::builder()
                                            .title(format!("Connected to {}", device_name))
                                            .timeout(3)
                                            .build();
                                        toast.add_toast(t);

                                        // Update content area
                                        if *selected_id_inner.borrow() == Some(id) {
                                            ui::update_content_for_device(
                                                &content_area,
                                                &device_name,
                                                true,
                                                Rc::new(|| {}),
                                            );
                                        }
                                    } else {
                                        let t = adw::Toast::builder()
                                            .title(format!("Failed to connect to {}", device_name))
                                            .timeout(3)
                                            .build();
                                        toast.add_toast(t);
                                    }

                                    // Refresh the device list
                                    if let Some(r) = rh.borrow().clone() {
                                        r();
                                    }
                                })
                            };

                            ui::update_content_for_device(
                                &content_area,
                                &device.name,
                                device.connected,
                                actual_connect_fn,
                            );
                        }
                    }
                    None => {
                        ui::reset_content_to_welcome(&content_area);
                    }
                }
            })
        };

        let on_select_for_refresh = on_device_select.clone();
        let on_deselect_for_refresh = on_deselect.clone();

        // Inner rebuild: performs the actual rebuild, passing the real
        // refresh (looked up via refresh_holder) so newly built rows
        // capture a working refresh closure in their buttons. Breaks the
        // previous no-op-dummy that broke delete-after-rebuild/timer.
        let inner_rebuild: Rc<dyn Fn()> = {
            let listbox = device_listbox.clone();
            let store = device_store.clone();
            let toast = toast_overlay.clone();
            let window = window.clone();
            let on_select = on_select_for_refresh.clone();
            let on_deselect = on_deselect_for_refresh.clone();
            let selected_id = selected_device_id.clone();
            let refresh_h = refresh_holder.clone();
            Rc::new(move || {
                if let Some(r) = refresh_h.borrow().clone() {
                    let ctx = DeviceListContext {
                        listbox: listbox.clone(),
                        store: store.clone(),
                        toast_overlay: toast.clone(),
                        window: window.clone(),
                        refresh_fn: r,
                        on_select: on_select.clone(),
                        selected_id: selected_id.clone(),
                        on_deselect_fn: on_deselect.clone(),
                    };
                    rebuild_device_list(&ctx);
                }
            })
        };

        // Public refresh: delegates to inner_rebuild. Captured by row
        // button handlers and by the periodic connection-status timer.
        let refresh: Rc<dyn Fn()> = {
            let ir = inner_rebuild.clone();
            Rc::new(move || ir())
        };
        *refresh_holder.borrow_mut() = Some(refresh.clone());

        // Initial rebuild
        let initial_ctx = DeviceListContext {
            listbox: device_listbox.clone(),
            store: device_store.clone(),
            toast_overlay: toast_overlay.clone(),
            window: window.clone(),
            refresh_fn: refresh.clone(),
            on_select: on_device_select.clone(),
            selected_id: selected_device_id.clone(),
            on_deselect_fn: on_deselect.clone(),
        };
        rebuild_device_list(&initial_ctx);

        // Periodic connection status check
        let refresh_store = device_store.clone();
        let refresh_toast = toast_overlay.clone();
        let content_area_for_timer = content_area.clone();
        let refresh_h = refresh_holder.clone();

        let source_id: Rc<RefCell<Option<glib::source::SourceId>>> = Rc::new(RefCell::new(None));
        let source_id_clone = source_id.clone();

        *source_id.borrow_mut() = Some(gtk::glib::timeout_add_local(Duration::from_secs(5), move || {
            if refresh_connected_status(&refresh_store) {
                // Use the real refresh so newly built rows capture a working
                // refresh closure in their delete/edit/connect buttons.
                if let Some(r) = refresh_h.borrow().as_ref() {
                    r();
                }
                
                // Update content if a device is selected
                if let Some(id) = *selected_device_id.borrow() {
                    let devices = refresh_store.borrow();
                    if let Some(device) = devices.iter().find(|d| d.id == id) {
                        let connect_fn: Rc<dyn Fn()> = {
                            let store = refresh_store.clone();
                            let _content_area = content_area_for_timer.clone();
                            let _selected_id = selected_device_id.clone();
                            let toast = refresh_toast.clone();
                            let rh = refresh_holder.clone();

                            Rc::new(move || {
                                let addr = {
                                    let devices = store.borrow();
                                    devices.iter()
                                        .find(|d| d.id == id)
                                        .map(|d| (d.name.clone(), format!("{}:{}", d.address, d.port)))
                                        .unwrap_or_default()
                                };

                                if addr.1.is_empty() { return; }

                                let device_name = addr.0.clone();
                                let address = addr.1.clone();

                                let _ = std::process::Command::new("adb")
                                    .args(["connect", &address])
                                    .output();

                                std::thread::sleep(std::time::Duration::from_millis(500));

                                let verify_result = std::process::Command::new("adb")
                                    .args(["devices"])
                                    .output();

                                let is_connected = match verify_result {
                                    Ok(out) => {
                                        let stdout = String::from_utf8_lossy(&out.stdout);
                                        stdout.lines().any(|line| {
                                            line.starts_with(&address) && line.contains("device")
                                        })
                                    }
                                    Err(_) => false,
                                };

                                if is_connected {
                                    if let Some(d) = store.borrow_mut().iter_mut().find(|d| d.id == id) {
                                        d.connected = true;
                                    }
                                    save_devices(&store.borrow());

                                    let t = adw::Toast::builder()
                                        .title(format!("Connected to {}", device_name))
                                        .timeout(3)
                                        .build();
                                    toast.add_toast(t);
                                } else {
                                    let t = adw::Toast::builder()
                                        .title(format!("Failed to connect to {}", device_name))
                                        .timeout(3)
                                        .build();
                                    toast.add_toast(t);
                                }

                                if let Some(r) = rh.borrow().clone() {
                                    r();
                                }
                            })
                        };
                        ui::update_content_for_device(&content_area_for_timer, &device.name, device.connected, connect_fn);
                    } else {
                        *selected_device_id.borrow_mut() = None;
                        ui::reset_content_to_welcome(&content_area_for_timer);
                    }
                }
            }
            gtk::glib::ControlFlow::Continue
        }));

        add_device_btn.connect_clicked({
            let window = window.clone();
            let store = device_store.clone();
            let toast = toast_overlay.clone();
            let refresh = refresh.clone();
            move |_| {
                let store = store.clone();
                let toast = toast.clone();
                let refresh = refresh.clone();

                ui::show_add_device_modal(
                    &window,
                    Box::new(move |name: String, address: String, port: u16| {
                        eprintln!("[APP] Device added: name='{}', address='{}', port={}", name, address, port);
                        let device = Device {
                            id: next_device_id(),
                            name: name.clone(),
                            address,
                            port,
                            connected: false,
                        };
                        store.borrow_mut().push(device.clone());
                        save_devices(&store.borrow());
                        refresh_connected_status(&store);
                        refresh();

                        let t = adw::Toast::builder()
                            .title(format!("\"{}\" paired successfully", name))
                            .timeout(4)
                            .build();
                        toast.add_toast(t);
                    }),
                );
            }
        });

        // Hide on close instead of quitting
        window.connect_close_request({
            let window = window.clone();
            let source_id_clone = source_id_clone.clone();
            move |_| {
                if let Some(id) = source_id_clone.borrow_mut().take() {
                    id.remove();
                }
                window.set_visible(false);
                gtk::glib::Propagation::Stop
            }
        });

        *window_ref.borrow_mut() = Some(window.clone());
        window.present();

        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let (tx, rx) = async_channel::unbounded::<crate::tray::TrayMessage>();
            let tray = LensCastTray::new(tx);

            start_tray_message_handler(app.clone(), window_ref.clone(), device_store.clone(), rx);

            std::thread::spawn(move || {
                let rt = match tokio::runtime::Runtime::new() {
                    Ok(rt) => rt,
                    Err(e) => {
                        eprintln!("Failed to create tokio runtime: {}", e);
                        return;
                    }
                };
                rt.block_on(async {
                    let _tray_handle = tray.spawn().await;
                    std::future::pending::<()>().await;
                });
            });
        }
    });

    app.run();
}