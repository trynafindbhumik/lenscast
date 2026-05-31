use adw::prelude::*;
use adw::Application as AdwApplication;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::theme::setup_theme;
use crate::ui;
use crate::ui::devices::{refresh_connected_status, next_device_id, new_device_store, save_devices, Device};
use crate::ui::sidebar::rebuild_device_list;

const APP_ID: &str = "com.lenscast.app";

type RefreshFn = dyn Fn();
type RefreshHolder = Rc<RefCell<Option<Rc<RefreshFn>>>>;

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

        let (sidebar, device_listbox) = ui::create_sidebar();
        let separator = ui::create_separator();
        let content_area = ui::create_content_area();
        let content_box = ui::create_content_box(&sidebar, &separator, &content_area);

        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&content_box));

        let window = ui::create_window(app, &header_bar, &toast_overlay);
        setup_theme(&window);

        // Refresh function that can call itself recursively
        let refresh_holder: RefreshHolder = Rc::new(RefCell::new(None));

        let refresh: Rc<dyn Fn()> = {
            let listbox = device_listbox.clone();
            let store = device_store.clone();
            let toast = toast_overlay.clone();
            let window = window.clone();
            let rh = refresh_holder.clone();
            Rc::new(move || {
                let self_ref = rh.borrow().clone();
                if let Some(r) = self_ref {
                    rebuild_device_list(&listbox, &store, &toast, &window, &r);
                }
            })
        };
        *refresh_holder.borrow_mut() = Some(refresh.clone());

        // Periodic connection status check
        let refresh_store = device_store.clone();
        let refresh_toast = toast_overlay.clone();
        let refresh_window = window.clone();
        let refresh_listbox = device_listbox.clone();
        let refresh_holder_for_timer = refresh_holder.clone();

        let source_id: Rc<RefCell<Option<glib::source::SourceId>>> = Rc::new(RefCell::new(None));
        let source_id_clone = source_id.clone();

        *source_id.borrow_mut() = Some(gtk::glib::timeout_add_local(Duration::from_secs(5), move || {
            if refresh_connected_status(&refresh_store) {
                if let Some(r) = refresh_holder_for_timer.borrow().clone() {
                    rebuild_device_list(&refresh_listbox, &refresh_store, &refresh_toast, &refresh_window, &r);
                }
            }
            gtk::glib::ControlFlow::Continue
        }));

        refresh();

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

        // Hide on close instead of quitting (keep running in tray)
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