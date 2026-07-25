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

/// Processes tray messages on the GTK main thread.
pub fn start_tray_message_handler(
    app: AdwApplication,
    window_ref: Rc<RefCell<Option<adw::ApplicationWindow>>>,
    device_store: crate::ui::devices::DeviceStore,
    pipeline_store: crate::video::PipelineStore,
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
                    // Step 1: Explicitly stop every video pipeline.
                    //
                    // Each pipeline owns a scrcpy child process and a GStreamer relay
                    // child process.  VideoPipeline::stop() kills both and waits for
                    // them to exit, so no orphan processes are left behind.
                    //
                    // We call stop() before clearing the store because other Arc clones
                    // held by closures in run.rs keep the refcount above 1, so
                    // clearing the store alone would NOT trigger Drop on the pipeline.
                    let pipelines_to_stop: Vec<_> = pipeline_store
                        .lock()
                        .unwrap()
                        .values()
                        .cloned()
                        .collect();

                    for pipeline in &pipelines_to_stop {
                        log::info!("[tray] stopping pipeline before quit");
                        pipeline.stop();
                    }
                    drop(pipelines_to_stop);
                    pipeline_store.lock().unwrap().clear();

                    // Step 2: Disconnect ADB from every paired device.
                    //
                    // Use output() (blocking) instead of spawn() so each disconnect
                    // completes before we call app.quit().  adb disconnect is fast
                    // (<100 ms), so briefly blocking the GTK main loop is acceptable
                    // during an application shutdown sequence.
                    for device in device_store.borrow().iter() {
                        let addr = format!("{}:{}", device.address, device.port);
                        log::info!("[tray] adb disconnect {}", addr);
                        let _ = std::process::Command::new("adb")
                            .args(["disconnect", &addr])
                            .output();
                    }

                    // Step 3: Hide the window then quit.
                    if let Some(window) = window_ref.borrow().as_ref() {
                        window.set_visible(false);
                    }
                    app.quit();
                }
            }
        }
    });
}