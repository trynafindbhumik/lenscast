use adw::prelude::*;
use gtk::{Label, Orientation};
use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use crate::ui::devices::{save_devices, Device, DeviceStore};
use crate::video::PipelineStore;

/// Shared callback type for refreshing the device list.
pub(crate) type RefreshFn = Rc<dyn Fn()>;

/// Bundles all dependencies needed to render and edit the device list.
/// Keeps function signatures short and groups related wiring together.
#[derive(Clone)]
pub(crate) struct DeviceListContext {
    pub listbox: gtk::ListBox,
    pub store: DeviceStore,
    pub toast_overlay: adw::ToastOverlay,
    pub window: adw::ApplicationWindow,
    pub refresh_fn: RefreshFn,
    pub on_select: Rc<dyn Fn(Option<u32>)>,
    pub pipelines: PipelineStore,
    pub selected_id: Rc<RefCell<Option<u32>>>,
    pub on_deselect_fn: Rc<dyn Fn()>,
}

/// Returns (sidebar_box, device_listbox, selected_device_id_cell).
pub fn create_sidebar() -> (gtk::Box, gtk::ListBox, Rc<RefCell<Option<u32>>>) {
    let sidebar = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .build();
    sidebar.set_size_request(240, -1);
    sidebar.add_css_class("sidebar");

    let sidebar_header = Label::new(None);
    sidebar_header.set_markup("<b>Devices</b>");
    sidebar_header.set_halign(gtk::Align::Start);
    sidebar_header.set_margin_start(16);
    sidebar_header.set_margin_top(16);
    sidebar_header.set_margin_bottom(8);
    sidebar.append(&sidebar_header);

    let scroll = gtk::ScrolledWindow::builder()
        .vexpand(true)
        .hscrollbar_policy(gtk::PolicyType::Never)
        .build();

    let device_listbox = gtk::ListBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .show_separators(false)
        .build();
    device_listbox.add_css_class("device-listbox");

    scroll.set_child(Some(&device_listbox));
    sidebar.append(&scroll);

    let selected_device_id: Rc<RefCell<Option<u32>>> = Rc::new(RefCell::new(None));

    (sidebar, device_listbox, selected_device_id)
}

/// Repopulates the device list after add/edit/delete.
pub fn rebuild_device_list(ctx: &DeviceListContext) {
    // First clear all existing children
    while let Some(child) = ctx.listbox.first_child() {
        ctx.listbox.remove(&child);
    }

    let devices = ctx.store.borrow().clone();

    if devices.is_empty() {
        let empty_row = gtk::ListBoxRow::builder()
            .activatable(false)
            .selectable(false)
            .build();
        let lbl = Label::new(Some("No devices added"));
        lbl.add_css_class("dim-label");
        lbl.set_halign(gtk::Align::Start);
        lbl.set_margin_start(16);
        lbl.set_margin_top(12);
        lbl.set_margin_bottom(12);
        empty_row.set_child(Some(&lbl));
        ctx.listbox.append(&empty_row);
    } else {
        for device in &devices {
            let row = build_device_row(device, ctx);
            ctx.listbox.append(&row);
        }
    }
}

fn build_device_row(device: &Device, ctx: &DeviceListContext) -> gtk::ListBoxRow {
    let row_box = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(0)
        .margin_start(8)
        .margin_end(4)
        .margin_top(2)
        .margin_bottom(2)
        .valign(gtk::Align::Center)
        .build();

    let icon = gtk::Image::from_icon_name("phone-symbolic");
    icon.set_pixel_size(16);
    icon.set_margin_end(10);
    icon.add_css_class("device-row-icon");

    let name_lbl = Label::builder()
        .label(&device.name)
        .halign(gtk::Align::Start)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    name_lbl.add_css_class("device-row-label");

    let connected_indicator = Label::new(Some("✓"));
    connected_indicator.add_css_class("device-connected-indicator");
    connected_indicator.set_visible(device.connected);

    let spacer = gtk::Box::builder().hexpand(true).build();

    let menu_btn = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .has_frame(false)
        .valign(gtk::Align::Center)
        .build();
    menu_btn.add_css_class("device-menu-btn");

    let popover = build_device_popover(device, ctx, &name_lbl, &connected_indicator);
    menu_btn.set_popover(Some(&popover));

    row_box.append(&icon);
    row_box.append(&name_lbl);
    row_box.append(&spacer);
    row_box.append(&connected_indicator);
    row_box.append(&menu_btn);

    let row = gtk::ListBoxRow::builder()
        .activatable(true)
        .selectable(false)
        .build();
    row.add_css_class("device-row");
    row.set_child(Some(&row_box));

    // Check if this row is selected and apply style
    let is_selected = *ctx.selected_id.borrow() == Some(device.id);
    if is_selected {
        row.add_css_class("device-row-selected");
    }

    // Track for immediate style update - need listbox to update other rows too
    let row_clone = row.clone();
    let listbox_for_update = ctx.listbox.clone();

    {
        let device_id = device.id;
        let selected_id = ctx.selected_id.clone();
        let on_select = ctx.on_select.clone();

        // Use click controller for reliable click handling
        let click = gtk::GestureClick::builder()
            .button(gtk::gdk::BUTTON_PRIMARY)
            .build();

        click.connect_pressed(move |_, _, _, _| {
            *selected_id.borrow_mut() = Some(device_id);
            on_select(Some(device_id));

            // Remove selected state from ALL rows first
            let mut child = listbox_for_update.first_child();
            while let Some(child_widget) = child {
                if let Some(row) = child_widget.downcast_ref::<gtk::ListBoxRow>() {
                    row.remove_css_class("device-row-selected");
                }
                child = child_widget.next_sibling();
            }

            // Then add selected state to this row
            row_clone.add_css_class("device-row-selected");
        });

        row.add_controller(click);
    }

    row
}

fn build_device_popover(
    device: &Device,
    ctx: &DeviceListContext,
    name_lbl: &Label,
    connected_indicator: &Label,
) -> gtk::Popover {
    let popover = gtk::Popover::new();
    popover.set_has_arrow(false);
    popover.add_css_class("device-popover");

    let vbox = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .spacing(2)
        .margin_start(4)
        .margin_end(4)
        .margin_top(4)
        .margin_bottom(4)
        .build();

    // Get current connection status from store dynamically
    let current_connected = ctx
        .store
        .borrow()
        .iter()
        .find(|d| d.id == device.id)
        .map(|d| d.connected)
        .unwrap_or(false);

    let (btn_label, btn_icon, is_connected) = if current_connected {
        ("Disconnect", "network-offline-symbolic", true)
    } else {
        ("Connect", "network-wired-symbolic", false)
    };

    let connect_btn = make_popover_btn(btn_label, btn_icon, false);
    {
        let device_id = device.id;
        let store = ctx.store.clone();
        let pipelines = ctx.pipelines.clone();
        let toast_overlay = ctx.toast_overlay.clone();
        let refresh = ctx.refresh_fn.clone();
        let p = popover.clone();
        let conn_indicator = connected_indicator.clone();
        let on_select = ctx.on_select.clone();
        let selected_id = ctx.selected_id.clone();

        connect_btn.connect_clicked(move |_| {
            p.popdown();

            log::info!(
                "[sidebar] connect_btn clicked for device_id={} is_connected={}",
                device_id,
                is_connected
            );

            let (success, device_name) = if is_connected {
                crate::ui::devices::try_disconnect_device(&store, &pipelines, device_id)
            } else {
                crate::ui::devices::try_connect_device(&store, &pipelines, device_id)
            };

            log::info!(
                "[sidebar] connect result: success={} device={}",
                success,
                device_name
            );

            if success {
                conn_indicator.set_visible(!is_connected);
                let title = if is_connected {
                    format!("Disconnected from {}", device_name)
                } else {
                    format!("Connected to {}", device_name)
                };
                let t = adw::Toast::builder().title(title).timeout(3).build();
                t.set_priority(adw::ToastPriority::Normal);
                toast_overlay.add_toast(t);

                if *selected_id.borrow() == Some(device_id) {
                    on_select(Some(device_id));
                }
            } else {
                let action = if is_connected {
                    "disconnect from"
                } else {
                    "connect to"
                };
                let t = adw::Toast::builder()
                    .title(format!("Failed to {} {}", action, device_name))
                    .timeout(3)
                    .build();
                t.set_priority(adw::ToastPriority::Normal);
                toast_overlay.add_toast(t);
            }

            refresh();
        });
    }
    vbox.append(&connect_btn);

    let edit_btn = make_popover_btn("Edit Name", "document-edit-symbolic", false);
    {
        let device_id = device.id;
        let name_lbl = name_lbl.clone();
        let ctx = ctx.clone();
        let p = popover.clone();
        edit_btn.connect_clicked(move |_| {
            p.popdown();
            show_edit_dialog(device_id, &ctx, &name_lbl);
        });
    }
    vbox.append(&edit_btn);

    let sep = gtk::Separator::builder()
        .orientation(Orientation::Horizontal)
        .margin_top(4)
        .margin_bottom(4)
        .build();
    vbox.append(&sep);

    let delete_btn = make_popover_btn("Delete", "user-trash-symbolic", true);
    {
        let device_id = device.id;
        let ctx = ctx.clone();
        let p = popover.clone();
        delete_btn.connect_clicked(move |_| {
            p.popdown();
            delete_device(device_id, &ctx);
        });
    }
    vbox.append(&delete_btn);

    popover.set_child(Some(&vbox));
    popover
}

fn make_popover_btn(label: &str, icon: &str, destructive: bool) -> gtk::Button {
    let row = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();

    let img = gtk::Image::from_icon_name(icon);
    img.set_pixel_size(14);
    row.append(&img);

    let lbl = Label::builder()
        .label(label)
        .halign(gtk::Align::Start)
        .hexpand(true)
        .build();
    row.append(&lbl);

    let btn = gtk::Button::builder().child(&row).build();
    btn.add_css_class("flat");
    btn.add_css_class("device-popover-item");
    if destructive {
        btn.add_css_class("destructive-action");
    }
    btn
}

fn show_edit_dialog(device_id: u32, ctx: &DeviceListContext, name_lbl: &Label) {
    let current_name = ctx
        .store
        .borrow()
        .iter()
        .find(|d| d.id == device_id)
        .map(|d| d.name.clone())
        .unwrap_or_default();

    let dialog = adw::Window::builder()
        .modal(true)
        .transient_for(&ctx.window)
        .title("Edit Device Name")
        .default_width(340)
        .default_height(180)
        .resizable(false)
        .deletable(true)
        .build();

    let tv = adw::ToolbarView::new();

    let hdr = adw::HeaderBar::builder()
        .show_end_title_buttons(false)
        .build();
    hdr.add_css_class("flat");

    let cancel_btn = gtk::Button::builder().label("Cancel").build();
    let save_btn = gtk::Button::builder().label("Save").build();
    save_btn.add_css_class("suggested-action");

    hdr.pack_start(&cancel_btn);
    hdr.pack_end(&save_btn);
    tv.add_top_bar(&hdr);

    let body = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .vexpand(true)
        .spacing(8)
        .margin_start(24)
        .margin_end(24)
        .build();

    let lbl = Label::builder()
        .label("Device name")
        .halign(gtk::Align::Start)
        .build();
    lbl.add_css_class("modal-field-label");

    let entry = gtk::Entry::builder()
        .text(&current_name)
        .hexpand(true)
        .activates_default(true)
        .build();
    entry.add_css_class("modal-entry");

    let err_lbl = Label::builder()
        .label("Name cannot be empty.")
        .halign(gtk::Align::Start)
        .build();
    err_lbl.add_css_class("modal-error-label");
    err_lbl.set_visible(false);

    body.append(&lbl);
    body.append(&entry);
    body.append(&err_lbl);
    tv.set_content(Some(&body));
    dialog.set_content(Some(&tv));

    let do_save: Rc<dyn Fn()> = Rc::new({
        let entry = entry.clone();
        let err_lbl = err_lbl.clone();
        let store = ctx.store.clone();
        let toast_overlay_ref = ctx.toast_overlay.clone();
        let refresh = ctx.refresh_fn.clone();
        let dialog = dialog.clone();
        let name_lbl = name_lbl.clone();
        let on_select = ctx.on_select.clone();
        let selected_id = ctx.selected_id.clone();
        move || {
            let text = entry.text();
            let trimmed = text.trim();
            if trimmed.is_empty() {
                entry.add_css_class("error");
                err_lbl.set_visible(true);
                return;
            }
            let new_name = trimmed.to_string();
            if let Some(d) = store.borrow_mut().iter_mut().find(|d| d.id == device_id) {
                d.name = new_name.clone();
            }
            name_lbl.set_label(&new_name);

            save_devices(&store.borrow());

            // If this device is currently shown in the content area,
            // re-render it so the new name appears immediately.
            if *selected_id.borrow() == Some(device_id) {
                on_select(Some(device_id));
            }

            refresh();

            let toast = adw::Toast::builder()
                .title(format!("Device renamed to \"{}\"", new_name))
                .timeout(3)
                .build();
            toast.set_priority(adw::ToastPriority::Normal);
            toast_overlay_ref.add_toast(toast);

            dialog.close();
        }
    });

    save_btn.connect_clicked({
        let f = do_save.clone();
        move |_| f()
    });
    entry.connect_activate({
        let f = do_save.clone();
        move |_| f()
    });
    entry.connect_changed({
        let err = err_lbl.clone();
        move |e| {
            if !e.text().is_empty() {
                e.remove_css_class("error");
                err.set_visible(false);
            }
        }
    });
    cancel_btn.connect_clicked({
        let d = dialog.clone();
        move |_| d.close()
    });

    dialog.present();
    entry.grab_focus();
    entry.select_region(0, -1);
}

fn delete_device(device_id: u32, ctx: &DeviceListContext) {
    let device_address = {
        let devices = ctx.store.borrow();
        devices
            .iter()
            .find(|d| d.id == device_id)
            .map(|d| format!("{}:{}", d.address, d.port))
    };

    if let Some(addr) = &device_address {
        let _ = Command::new("adb").args(["disconnect", addr]).spawn();
    }

    crate::ui::devices::despawn_pipeline(&ctx.pipelines, device_id);

    let removed: Option<Device> = {
        let mut v = ctx.store.borrow_mut();
        let pos = v.iter().position(|d| d.id == device_id);
        pos.map(|p| v.remove(p))
    };

    let Some(device) = removed else { return };

    if *ctx.selected_id.borrow() == Some(device_id) {
        *ctx.selected_id.borrow_mut() = None;
        (ctx.on_deselect_fn)();
    }

    (ctx.refresh_fn)();

    let toast = adw::Toast::builder()
        .title(format!("\"{}\" removed", device.name))
        .button_label("Undo")
        .timeout(5)
        .build();
    toast.set_priority(adw::ToastPriority::Normal);

    save_devices(&ctx.store.borrow());

    toast.connect_button_clicked({
        let store = ctx.store.clone();
        let refresh = ctx.refresh_fn.clone();
        let device = device.clone();
        move |_| {
            store.borrow_mut().push(device.clone());
            store.borrow_mut().sort_by_key(|d| d.id);
            save_devices(&store.borrow());
            refresh();
        }
    });

    ctx.toast_overlay.add_toast(toast);
}
