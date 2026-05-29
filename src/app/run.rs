use adw::prelude::*;
use adw::Application as AdwApplication;
use std::cell::RefCell;
use std::rc::Rc;

use crate::theme::setup_theme;
use crate::ui;
use crate::ui::devices::{next_device_id, new_device_store, Device};
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
        // If window already exists, just show it
        if let Some(window) = window_ref_clone.borrow().as_ref() {
            if !window.is_visible() {
                window.set_visible(true);
            }
            window.present();
            return;
        }

        hold_guards_clone.borrow_mut().push(app.hold());

        // ── Shared device store ───────────────────────────────────────────────
        let device_store = new_device_store();

        // ── Build UI skeleton ─────────────────────────────────────────────────
        let add_device_btn = ui::create_add_device_button();
        let header_bar = ui::create_header_bar(&add_device_btn);

        let (sidebar, device_listbox) = ui::create_sidebar();
        let separator = ui::create_separator();
        let content_area = ui::create_content_area();
        let content_box = ui::create_content_box(&sidebar, &separator, &content_area);

        // ── Toast overlay wraps the entire content area ───────────────────────
        let toast_overlay = adw::ToastOverlay::new();
        toast_overlay.set_child(Some(&content_box));

        let window = ui::create_window(app, &header_bar, &toast_overlay);
        setup_theme(&window);

        // ── Recursive refresh closure ─────────────────────────────────────────
        // We need the refresh fn to pass itself to row builders (for edit/delete).
        // Use an Rc<RefCell<Option<Rc<dyn Fn()>>>> to allow self-reference.
        let refresh_holder: RefreshHolder = Rc::new(RefCell::new(None));

        let refresh: Rc<dyn Fn()> = {
            let listbox = device_listbox.clone();
            let store = device_store.clone();
            let toast = toast_overlay.clone();
            let window = window.clone();
            let rh = refresh_holder.clone();
            Rc::new(move || {
                // Pull out the stored Rc before passing it in (avoids borrow clash)
                let self_ref = rh.borrow().clone();
                if let Some(r) = self_ref {
                    rebuild_device_list(&listbox, &store, &toast, &window, &r);
                }
            })
        };
        *refresh_holder.borrow_mut() = Some(refresh.clone());

        // Populate sidebar with initial (empty) state
        refresh();

        // ── "Add Device" button handler ───────────────────────────────────────
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
                    Box::new(move |name: String| {
                        // Add the newly paired device to the store
                        let device = Device {
                            id: next_device_id(),
                            name: name.clone(),
                        };
                        store.borrow_mut().push(device);

                        // Rebuild sidebar
                        refresh();

                        // Celebrate with a GNOME-style success toast
                        let t = adw::Toast::builder()
                            .title(format!("\"{}\" paired successfully", name))
                            .timeout(4)
                            .build();
                        toast.add_toast(t);
                    }),
                );
            }
        });

        // ── Hide on close (keep running in tray) ──────────────────────────────
        window.connect_close_request({
            let window = window.clone();
            move |_| {
                window.set_visible(false);
                gtk::glib::Propagation::Stop
            }
        });

        *window_ref.borrow_mut() = Some(window.clone());
        window.present();

        // ── System tray (Linux only) ──────────────────────────────────────────
        #[cfg(not(any(target_os = "macos", windows)))]
        {
            let (tx, rx) = async_channel::unbounded::<crate::tray::TrayMessage>();
            let tray = LensCastTray::new(tx);

            start_tray_message_handler(app.clone(), window_ref.clone(), rx);

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