use adw::prelude::*;
use gtk::{Label, Orientation};
use std::rc::Rc;

use crate::ui::devices::{Device, DeviceStore};

/// Returns `(sidebar_box, device_listbox)`.
/// `device_listbox` is populated/repopulated by `rebuild_device_list`.
pub fn create_sidebar() -> (gtk::Box, gtk::ListBox) {
    let sidebar = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .build();
    sidebar.set_size_request(240, -1);
    sidebar.add_css_class("sidebar");

    // Header
    let sidebar_header = Label::new(None);
    sidebar_header.set_markup("<b>Devices</b>");
    sidebar_header.set_halign(gtk::Align::Start);
    sidebar_header.set_margin_start(16);
    sidebar_header.set_margin_top(16);
    sidebar_header.set_margin_bottom(8);
    sidebar.append(&sidebar_header);

    // Scrollable device list
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

    (sidebar, device_listbox)
}

//  Rebuild — called after every add / edit / delete
pub fn rebuild_device_list(
    listbox: &gtk::ListBox,
    store: &DeviceStore,
    toast_overlay: &adw::ToastOverlay,
    window: &adw::ApplicationWindow,
    refresh: &Rc<dyn Fn()>,
) {
    // Remove all existing rows
    while let Some(child) = listbox.first_child() {
        listbox.remove(&child);
    }

    let devices = store.borrow().clone();

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
        listbox.append(&empty_row);
    } else {
        for device in &devices {
            let row = build_device_row(device, store, toast_overlay, window, refresh);
            listbox.append(&row);
        }
    }
}

//  Individual device row
fn build_device_row(
    device: &Device,
    store: &DeviceStore,
    toast_overlay: &adw::ToastOverlay,
    window: &adw::ApplicationWindow,
    refresh: &Rc<dyn Fn()>,
) -> gtk::ListBoxRow {
    let row_box = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(0)
        .margin_start(8)
        .margin_end(4)
        .margin_top(2)
        .margin_bottom(2)
        .valign(gtk::Align::Center)
        .build();

    // Phone icon
    let icon = gtk::Image::from_icon_name("phone-symbolic");
    icon.set_pixel_size(16);
    icon.set_margin_end(10);
    icon.add_css_class("device-row-icon");

    // Device name
    let name_lbl = Label::builder()
        .label(&device.name)
        .halign(gtk::Align::Start)
        .hexpand(true)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .build();
    name_lbl.add_css_class("device-row-label");

    // Three-dot menu button
    let menu_btn = gtk::MenuButton::builder()
        .icon_name("view-more-symbolic")
        .has_frame(false)
        .valign(gtk::Align::Center)
        .build();
    menu_btn.add_css_class("device-menu-btn");

    let popover = build_device_popover(
        device,
        store,
        toast_overlay,
        window,
        refresh,
        &name_lbl,
    );
    menu_btn.set_popover(Some(&popover));

    row_box.append(&icon);
    row_box.append(&name_lbl);
    row_box.append(&menu_btn);

    let row = gtk::ListBoxRow::builder()
        .activatable(false)
        .selectable(false)
        .build();
    row.add_css_class("device-row");
    row.set_child(Some(&row_box));

    row
}

fn build_device_popover(
    device: &Device,
    store: &DeviceStore,
    toast_overlay: &adw::ToastOverlay,
    window: &adw::ApplicationWindow,
    refresh: &Rc<dyn Fn()>,
    name_lbl: &Label,
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

    let connect_btn = make_popover_btn("Connect", "network-wired-symbolic", false);
    connect_btn.connect_clicked({
        let p = popover.clone();
        move |_| { p.popdown(); }
    });
    vbox.append(&connect_btn);

    let edit_btn = make_popover_btn("Edit Name", "document-edit-symbolic", false);
    {
        let device_id = device.id;
        let store = store.clone();
        let toast = toast_overlay.clone();
        let refresh = refresh.clone();
        let name_lbl = name_lbl.clone();
        let window = window.clone();
        let p = popover.clone();
        edit_btn.connect_clicked(move |_| {
            p.popdown();
            show_edit_dialog(device_id, &store, &toast, &window, &refresh, &name_lbl);
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
        let store = store.clone();
        let toast = toast_overlay.clone();
        let refresh = refresh.clone();
        let p = popover.clone();
        delete_btn.connect_clicked(move |_| {
            p.popdown();
            delete_device(device_id, &store, &toast, &refresh);
        });
    }
    vbox.append(&delete_btn);

    popover.set_child(Some(&vbox));
    popover
}

/// Builds a flat button with icon + label for use inside the popover.
fn make_popover_btn(label: &str, icon: &str, destructive: bool) -> gtk::Button {
    let row = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(10)
        .build();

    let img = gtk::Image::from_icon_name(icon);
    img.set_pixel_size(14);
    row.append(&img);

    let lbl = Label::builder().label(label).halign(gtk::Align::Start).hexpand(true).build();
    row.append(&lbl);

    let btn = gtk::Button::builder().child(&row).build();
    btn.add_css_class("flat");
    btn.add_css_class("device-popover-item");
    if destructive {
        btn.add_css_class("destructive-action");
    }
    btn
}

fn show_edit_dialog(
    device_id: u32,
    store: &DeviceStore,
    toast_overlay: &adw::ToastOverlay,
    parent: &adw::ApplicationWindow,
    refresh: &Rc<dyn Fn()>,
    name_lbl: &Label,
) {
    // Find current name
    let current_name = store
        .borrow()
        .iter()
        .find(|d| d.id == device_id)
        .map(|d| d.name.clone())
        .unwrap_or_default();

    let dialog = adw::Window::builder()
        .modal(true)
        .transient_for(parent)
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
        let store = store.clone();
        let toast_overlay_ref = toast_overlay.clone();
        let refresh = refresh.clone();
        let dialog = dialog.clone();
        let name_lbl = name_lbl.clone();
        move || {
            let text = entry.text();
            let trimmed = text.trim();
            if trimmed.is_empty() {
                entry.add_css_class("error");
                err_lbl.set_visible(true);
                return;
            }
            // Update store
            let new_name = trimmed.to_string();
            if let Some(d) = store.borrow_mut().iter_mut().find(|d| d.id == device_id) {
                d.name = new_name.clone();
            }
            name_lbl.set_label(&new_name);
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

    save_btn.connect_clicked({ let f = do_save.clone(); move |_| f() });
    entry.connect_activate({ let f = do_save.clone(); move |_| f() });
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
    // Select all so the user can type immediately
    entry.select_region(0, -1);
}

fn delete_device(
    device_id: u32,
    store: &DeviceStore,
    toast_overlay: &adw::ToastOverlay,
    refresh: &Rc<dyn Fn()>,
) {
    // Pull the device out of the store
    let removed: Option<Device> = {
        let mut v = store.borrow_mut();
        v.iter().position(|d| d.id == device_id).map(|pos| v.remove(pos))
    };

    let Some(device) = removed else { return };

    refresh();

    // Toast with Undo
    let toast = adw::Toast::builder()
        .title(format!("\"{}\" removed", device.name))
        .button_label("Undo")
        .timeout(5)
        .build();
    toast.set_priority(adw::ToastPriority::Normal);

    // On Undo: put the device back, refresh
    toast.connect_button_clicked({
        let store = store.clone();
        let refresh = refresh.clone();
        let device = device.clone();
        move |_| {
            store.borrow_mut().push(device.clone());
            // Sort by id so it re-appears in original order
            store.borrow_mut().sort_by_key(|d| d.id);
            refresh();
        }
    });

    toast_overlay.add_toast(toast);
}