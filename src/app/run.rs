use adw::prelude::*;
use adw::Application as AdwApplication;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::theme::setup_theme;
use crate::ui;
use crate::ui::devices::{
    new_device_store, next_device_id, refresh_connected_status, save_devices,
    transform_callbacks_for, try_connect_device, update_camera_selection, Device,
};
use crate::ui::sidebar::{rebuild_device_list, DeviceListContext, RefreshFn};
use crate::video::new_pipeline_store;

const APP_ID: &str = "com.lenscast.app";

#[cfg(not(any(target_os = "macos", windows)))]
use crate::tray::{start_tray_message_handler, LensCastTray};
#[cfg(not(any(target_os = "macos", windows)))]
use ksni::TrayMethods;

pub fn run() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug"))
        .format_timestamp_millis()
        .init();
    log::info!("LensCast starting");

    let app = AdwApplication::builder().application_id(APP_ID).build();

    let window_ref: Rc<RefCell<Option<adw::ApplicationWindow>>> = Rc::new(RefCell::new(None));
    let window_ref_clone = window_ref.clone();

    let hold_guards = Rc::new(RefCell::new(Vec::new()));
    let hold_guards_clone = hold_guards.clone();

    app.connect_activate(move |app| {
        if window_ref_clone
            .borrow()
            .as_ref()
            .map(|w| w.is_visible())
            .unwrap_or(false)
        {
            window_ref_clone.borrow().as_ref().unwrap().present();
            return;
        }

        hold_guards_clone.borrow_mut().push(app.hold());

        let device_store = new_device_store();
        let pipelines = new_pipeline_store(); // NEW — device_id -> VideoPipeline
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

        let toast_overlay_clone = toast_overlay.clone();
        let selected_device_id_clone = selected_device_id.clone();

        let refresh_holder: Rc<RefCell<Option<RefreshFn>>> = Rc::new(RefCell::new(None));

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
            let pipelines = pipelines.clone();
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
                            let device_name = device.name.clone();
                            let device_connected = device.connected;
                            let device_camera_selection = device.camera_selection.clone();
                            let device_camera_selection_for_connect =
                                device_camera_selection.clone();

                            let refresh_ui: Rc<dyn Fn()> = {
                                let rh = refresh_h.clone();
                                Rc::new(move || {
                                    if let Some(r) = rh.borrow().clone() {
                                        r();
                                    }
                                })
                            };

                            let actual_connect_fn: Rc<dyn Fn()> = {
                                let store = store.clone();
                                let pipelines = pipelines.clone();
                                let content_area = content_area.clone();
                                let selected_id_inner = selected_id.clone();
                                let toast = toast.clone();
                                let refresh_ui = refresh_ui.clone();
                                let device_name_for_connect = device_name.clone();

                                Rc::new(move || {
                                    ui::show_connecting_state(&content_area, &device_name_for_connect);

                                    // Flush GTK events so spinner actually renders
                                    while gtk::glib::MainContext::default().iteration(false) {}

                                    // Run connect in a thread, send raw results back via channel
                                    let (tx, rx) = std::sync::mpsc::channel();
                                    ui::devices::try_connect_device_background(&store, pipelines.clone(), id, tx);

                                    // Poll channel on main thread with timeout
                                    let store_clone = store.clone();
                                    let pipelines_clone = pipelines.clone();
                                    let content_area_clone = content_area.clone();
                                    let toast_clone = toast.clone();
                                    let refresh_ui_clone = refresh_ui.clone();
                                    let selected_id_clone = selected_id_inner.clone();
                                    let device_camera_selection_clone = device_camera_selection_for_connect.clone();
                                    let device_id = id;

                                    gtk::glib::timeout_add_local(std::time::Duration::from_millis(100), move || {
                                        if let Ok((is_connected, device_name, mdns_update)) = rx.try_recv() {
                                            // Apply result to store on main thread (non-blocking, pipeline already spawned in thread)
                                            ui::devices::apply_connect_result(&store_clone, device_id, is_connected, &device_name, mdns_update);

                                            if is_connected {
                                                let t = adw::Toast::builder()
                                                    .title(format!("Connected to {}", device_name))
                                                    .timeout(3)
                                                    .build();
                                                toast_clone.add_toast(t);

                                                if *selected_id_clone.borrow() == Some(device_id) {
                                                    // Pipeline already spawned in background thread, show connected UI
                                                    let content_area_d = content_area_clone.clone();
                                                    let device_name_d = device_name.clone();
                                                    let pipelines_d = pipelines_clone.clone();
                                                    let camera_sel_d = device_camera_selection_clone.clone();
                                                    let refresh_ui_d = refresh_ui_clone.clone();

                                                    let camera_change_fn: Rc<dyn Fn(String)> = {
                                                        let store = store_clone.clone();
                                                        let pipelines = pipelines_d.clone();
                                                        let refresh_ui = refresh_ui_d.clone();
                                                        Rc::new(move |selection| {
                                                            if update_camera_selection(&store, &pipelines, device_id, &selection) {
                                                                refresh_ui();
                                                            }
                                                        })
                                                    };

                                                    ui::update_content_for_device(
                                                        &content_area_d,
                                                        &device_name_d,
                                                        true,
                                                        Rc::new(|| {}),
                                                        transform_callbacks_for(&pipelines_d, device_id),
                                                        Some(camera_change_fn),
                                                        Some(camera_sel_d.clone()),
                                                    );
                                                    refresh_ui_d();
                                                }
                                                glib::ControlFlow::Break
                                            } else {
                                                let t = adw::Toast::builder()
                                                    .title(format!("Failed to connect to {}", device_name))
                                                    .timeout(3)
                                                    .build();
                                                toast_clone.add_toast(t);
                                                refresh_ui_clone();
                                                glib::ControlFlow::Break
                                            }
                                        } else {
                                            glib::ControlFlow::Continue
                                        }
                                    });
                                })
                            };

                            let camera_change_fn: Rc<dyn Fn(String)> = {
                                let store = store.clone();
                                let pipelines = pipelines.clone();
                                let refresh_ui = refresh_ui.clone();
                                Rc::new(move |selection| {
                                    if update_camera_selection(&store, &pipelines, id, &selection) {
                                        refresh_ui();
                                    }
                                })
                            };

                            ui::update_content_for_device(
                                &content_area,
                                &device_name,
                                device_connected,
                                actual_connect_fn,
                                transform_callbacks_for(&pipelines, id),
                                Some(camera_change_fn),
                                Some(device_camera_selection.clone()),
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

        let inner_rebuild: Rc<dyn Fn()> = {
            let listbox = device_listbox.clone();
            let store = device_store.clone();
            let pipelines = pipelines.clone();
            let toast = toast_overlay.clone();
            let window = window.clone();
            let on_select = on_select_for_refresh.clone();
            let on_deselect = on_deselect_for_refresh.clone();
            let selected_id = selected_device_id.clone();
            let refresh_h = refresh_holder.clone();
            let content_area_for_rebuild = content_area.clone();
            Rc::new(move || {
                if let Some(r) = refresh_h.borrow().clone() {
                    let ctx = DeviceListContext {
                        listbox: listbox.clone(),
                        store: store.clone(),
                        pipelines: pipelines.clone(),
                        toast_overlay: toast.clone(),
                        window: window.clone(),
                        refresh_fn: r,
                        on_select: on_select.clone(),
                        selected_id: selected_id.clone(),
                        on_deselect_fn: on_deselect.clone(),
                        content_area: content_area_for_rebuild.clone(),
                    };
                    rebuild_device_list(&ctx);
                }
            })
        };

        let refresh: Rc<dyn Fn()> = {
            let ir = inner_rebuild.clone();
            Rc::new(move || ir())
        };
        *refresh_holder.borrow_mut() = Some(refresh.clone());

        let initial_ctx = DeviceListContext {
            listbox: device_listbox.clone(),
            store: device_store.clone(),
            pipelines: pipelines.clone(),
            toast_overlay: toast_overlay.clone(),
            window: window.clone(),
            refresh_fn: refresh.clone(),
            on_select: on_device_select.clone(),
            selected_id: selected_device_id.clone(),
            on_deselect_fn: on_deselect.clone(),
            content_area: content_area.clone(),
        };
        rebuild_device_list(&initial_ctx);

        // Periodic connection status check
        let refresh_store = device_store.clone();
        let refresh_pipelines = pipelines.clone();
        let refresh_toast = toast_overlay.clone();
        let content_area_for_timer = content_area.clone();
        let refresh_h = refresh_holder.clone();

        let source_id: Rc<RefCell<Option<glib::source::SourceId>>> = Rc::new(RefCell::new(None));
        let source_id_clone = source_id.clone();

        *source_id.borrow_mut() = Some(gtk::glib::timeout_add_local(
            Duration::from_secs(5),
            move || {
                let status_changed = refresh_connected_status(&refresh_store);
                log::info!("[run] periodic timer: status_changed={}", status_changed);
                if status_changed {
                    if let Some(r) = refresh_h.borrow().as_ref() {
                        r();
                    }

                    if let Some(id) = *selected_device_id.borrow() {
                        let devices = refresh_store.borrow();
                        if let Some(device) = devices.iter().find(|d| d.id == id) {
                            // If pipeline exists for this device, treat as connected regardless of ADB state
                            let has_pipeline = refresh_pipelines.lock().unwrap().contains_key(&id);
                            let effective_connected = device.connected || has_pipeline;
                            log::info!("[run] periodic timer: device_id={} connected={} has_pipeline={} effective={}",
                                id, device.connected, has_pipeline, effective_connected);
                            let device_camera_selection = device.camera_selection.clone();
                            let connect_fn: Rc<dyn Fn()> = {
                                let store = refresh_store.clone();
                                let pipelines = refresh_pipelines.clone();
                                let toast = refresh_toast.clone();
                                let rh = refresh_h.clone();

                                Rc::new(move || {
                                    let (is_connected, device_name) =
                                        try_connect_device(&store, &pipelines, id);

                                    let t = if is_connected {
                                        adw::Toast::builder()
                                            .title(format!("Connected to {}", device_name))
                                            .timeout(3)
                                            .build()
                                    } else {
                                        adw::Toast::builder()
                                            .title(format!("Failed to connect to {}", device_name))
                                            .timeout(3)
                                            .build()
                                    };
                                    toast.add_toast(t);

                                    if let Some(r) = rh.borrow().clone() {
                                        r();
                                    }
                                })
                            };
                            let camera_change_fn: Rc<dyn Fn(String)> = {
                                let store = refresh_store.clone();
                                let pipelines = refresh_pipelines.clone();
                                let rh = refresh_h.clone();
                                Rc::new(move |selection| {
                                    if update_camera_selection(&store, &pipelines, id, &selection) {
                                        if let Some(r) = rh.borrow().clone() {
                                            r();
                                        }
                                    }
                                })
                            };

                            ui::update_content_for_device(
                                &content_area_for_timer,
                                &device.name,
                                effective_connected,
                                connect_fn,
                                transform_callbacks_for(&refresh_pipelines, id),
                                Some(camera_change_fn),
                                Some(device_camera_selection.clone()),
                            );
                        } else {
                            *selected_device_id.borrow_mut() = None;
                            ui::reset_content_to_welcome(&content_area_for_timer);
                        }
                    }
                }
                gtk::glib::ControlFlow::Continue
            },
        ));

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
                        let device = Device {
                            id: next_device_id(),
                            name: name.clone(),
                            address,
                            port,
                            connected: false,
                            camera_selection: "back".to_string(),
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
                    Err(_e) => {
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
