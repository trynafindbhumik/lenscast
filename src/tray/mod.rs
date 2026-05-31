mod messages;

use adw::prelude::*;
use adw::Application as AdwApplication;
use std::cell::RefCell;
use std::rc::Rc;

pub use messages::TrayMessage;

pub struct LensCastTray {
    sender: async_channel::Sender<TrayMessage>,
}

impl LensCastTray {
    pub fn new(sender: async_channel::Sender<TrayMessage>) -> Self {
        Self { sender }
    }
}

impl ksni::Tray for LensCastTray {
    fn id(&self) -> String {
        "com.lenscast.app".into()
    }

    fn icon_name(&self) -> String {
        "camera-web".into()
    }

    fn title(&self) -> String {
        "LensCast".into()
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::*;
        vec![
            StandardItem {
                label: "Show".into(),
                activate: std::boxed::Box::new(|tray: &mut Self| {
                    // try_send is fine here — channel is unbounded and never blocks
                    let _ = tray.sender.try_send(TrayMessage::Show);
                }),
                ..Default::default()
            }
            .into(),
            StandardItem {
                label: "Quit".into(),
                activate: std::boxed::Box::new(|tray: &mut Self| {
                    let _ = tray.sender.try_send(TrayMessage::QuitAndDisconnect);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// Processes tray messages on the GTK main thread using async_channel.
/// recv().await is non-blocking — it yields to the GLib event loop while waiting.
pub fn start_tray_message_handler(
    app: AdwApplication,
    window_ref: Rc<RefCell<Option<adw::ApplicationWindow>>>,
    device_store: crate::ui::devices::DeviceStore,
    rx: async_channel::Receiver<TrayMessage>,
) {
    let ctx = glib::MainContext::default();
    ctx.spawn_local(async move {
        while let Ok(msg) = rx.recv().await {
            match msg {
                TrayMessage::Show => {
                    if let Some(window) = window_ref.borrow().as_ref() {
                        window.set_visible(true);
                        window.present();
                    }
                }
                TrayMessage::QuitAndDisconnect => {
                    eprintln!("[Tray] Quit with disconnect requested");
                    
                    // Disconnect all devices
                    let devices = device_store.borrow();
                    for device in devices.iter() {
                        let addr = format!("{}:{}", device.address, device.port);
                        eprintln!("[Tray] Disconnecting: adb disconnect {}", addr);
                        let _ = std::process::Command::new("adb")
                            .args(["disconnect", &addr])
                            .spawn();
                    }
                    drop(devices); // Release borrow before quitting
                    
                    if let Some(window) = window_ref.borrow().as_ref() {
                        window.set_visible(false);
                    }
                    app.quit();
                }
            }
        }
    });
}
