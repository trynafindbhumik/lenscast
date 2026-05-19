use adw::prelude::*;
use adw::Application as AdwApplication;
use std::cell::RefCell;
use std::rc::Rc;

use crate::theme::setup_theme;
use crate::ui;

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
        if let Some(window) = window_ref_clone.borrow().as_ref() {
            if !window.is_visible() {
                window.set_visible(true);
            }
            window.present();
            return;
        }

        hold_guards_clone.borrow_mut().push(app.hold());

        let add_device_btn = ui::create_add_device_button();
        let header_bar = ui::create_header_bar(&add_device_btn);
        let sidebar = ui::create_sidebar();
        let separator = ui::create_separator();
        let content_area = ui::create_content_area();
        let content_box = ui::create_content_box(&sidebar, &separator, &content_area);

        let window = ui::create_window(app, &header_bar, &content_box);

        setup_theme(&window);

        // Hide instead of close so the app stays in the tray.
        window.connect_close_request({
            let window = window.clone();
            move |_| {
                window.set_visible(false);
                gtk::glib::Propagation::Stop
            }
        });

        *window_ref.borrow_mut() = Some(window.clone());

        window.present();

        #[cfg(not(any(target_os = "macos", windows)))]
        {
            // unbounded so try_send() from the tray thread never blocks/fails
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
