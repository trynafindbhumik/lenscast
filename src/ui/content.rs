use gtk::prelude::*;
use gtk::{Box as GtkBox, Button, Image, Label, Orientation, Spinner};
use std::rc::Rc;

/// Creates the main content area shown when no device is selected.
pub fn create_content_area() -> (gtk::Box, gtk::Box) {
    let content_container = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .vexpand(true)
        .hexpand(true)
        .build();

    let content_area = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .vexpand(true)
        .hexpand(true)
        .build();

    let welcome = Label::new(None);
    welcome.set_markup(
        "<b><big>Welcome to LensCast</big></b>\n\nSelect a device from the sidebar to get started",
    );
    welcome.add_css_class("dim-label");
    welcome.set_justify(gtk::Justification::Center);
    welcome.set_halign(gtk::Align::Center);
    welcome.set_valign(gtk::Align::Center);
    content_area.append(&welcome);

    content_container.append(&content_area);

    (content_container, content_area)
}

/// Updates the content area based on device selection
pub fn update_content_for_device(
    content_area: &gtk::Box,
    device_name: &str,
    is_connected: bool,
    on_connect_clicked: Rc<dyn Fn()>,
    transform_cb: Option<crate::ui::TransformCallbacks>,
    on_camera_changed: Option<Rc<dyn Fn(String)>>,
    current_camera_selection: Option<String>,
) {
    while let Some(child) = content_area.first_child() {
        content_area.remove(&child);
    }

    if is_connected {
        let icon = Image::from_icon_name("emblem-ok-symbolic");
        icon.set_pixel_size(64);
        icon.add_css_class("success-icon");

        let title = Label::new(None);
        title.set_markup(&format!("<b><big>Hello, {}!</big></b>", device_name));
        title.set_justify(gtk::Justification::Center);
        title.set_halign(gtk::Align::Center);
        title.set_valign(gtk::Align::Center);

        let subtitle = Label::new(Some("Your device is connected and ready to use"));
        subtitle.add_css_class("dim-label");
        subtitle.set_justify(gtk::Justification::Center);
        subtitle.set_halign(gtk::Align::Center);
        subtitle.set_valign(gtk::Align::Center);

        let vbox = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .spacing(16)
            .build();

        vbox.append(&icon);
        vbox.append(&title);
        vbox.append(&subtitle);

        if let Some(cb) = transform_cb {
            vbox.append(&crate::ui::build_transform_section(cb));
        }

        let camera_row = gtk::Box::builder()
            .orientation(Orientation::Horizontal)
            .spacing(8)
            .build();
        let camera_label = Label::new(Some("Camera"));
        camera_label.set_halign(gtk::Align::Start);
        camera_label.set_hexpand(true);
        let camera_dropdown = gtk::DropDown::from_strings(&["Back", "Front"]);
        if let Some(selection) = current_camera_selection {
            camera_dropdown.set_selected(if selection.to_lowercase() == "front" {
                1
            } else {
                0
            });
        }
        if let Some(on_camera_changed) = on_camera_changed {
            camera_dropdown.connect_selected_notify(move |dropdown| {
                let label = match dropdown.selected() {
                    1 => "front".to_string(),
                    _ => "back".to_string(),
                };
                on_camera_changed(label.clone());
            });
        }
        camera_row.append(&camera_label);
        camera_row.append(&camera_dropdown);
        vbox.append(&camera_row);

        content_area.append(&vbox);
    } else {
        let icon = Image::from_icon_name("network-wireless-symbolic");
        icon.set_pixel_size(80);
        icon.add_css_class("disconnected-icon");

        let title = Label::new(None);
        title.set_markup(&format!("<b><big>{}</big></b>", device_name));
        title.set_justify(gtk::Justification::Center);
        title.set_halign(gtk::Align::Center);
        title.set_valign(gtk::Align::Center);

        let subtitle = Label::new(Some("Device is not connected"));
        subtitle.add_css_class("dim-label");
        subtitle.set_justify(gtk::Justification::Center);
        subtitle.set_halign(gtk::Align::Center);
        subtitle.set_valign(gtk::Align::Center);

        let connect_btn = Button::builder()
            .label("Connect Now")
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .build();
        connect_btn.add_css_class("suggested-action");
        connect_btn.add_css_class("connect-btn");

        let on_click = on_connect_clicked.clone();
        connect_btn.connect_clicked(move |_| {
            on_click();
        });

        let vbox = GtkBox::builder()
            .orientation(Orientation::Vertical)
            .halign(gtk::Align::Center)
            .valign(gtk::Align::Center)
            .spacing(16)
            .build();

        vbox.append(&icon);
        vbox.append(&title);
        vbox.append(&subtitle);
        vbox.append(&connect_btn);

        content_area.append(&vbox);
    }
}

/// Resets content area to default welcome state
pub fn reset_content_to_welcome(content_area: &gtk::Box) {
    while let Some(child) = content_area.first_child() {
        content_area.remove(&child);
    }

    let welcome = Label::new(None);
    welcome.set_markup(
        "<b><big>Welcome to LensCast</big></b>\n\nSelect a device from the sidebar to get started",
    );
    welcome.add_css_class("dim-label");
    welcome.set_justify(gtk::Justification::Center);
    welcome.set_halign(gtk::Align::Center);
    welcome.set_valign(gtk::Align::Center);
    content_area.append(&welcome);
}

/// Shows a loading spinner with "Connecting to {device_name}..." text
pub fn show_connecting_state(content_area: &gtk::Box, device_name: &str) {
    while let Some(child) = content_area.first_child() {
        content_area.remove(&child);
    }

    let vbox = GtkBox::builder()
        .orientation(Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .spacing(24)
        .build();

    let spinner = Spinner::new();
    spinner.set_size_request(48, 48);
    spinner.set_halign(gtk::Align::Center);
    spinner.set_valign(gtk::Align::Center);
    spinner.start();

    let label = Label::new(None);
    label.set_markup(&format!(
        "<b><big>Connecting to {}...</big></b>\n\nPlease wait while we establish the connection",
        device_name
    ));
    label.add_css_class("dim-label");
    label.set_justify(gtk::Justification::Center);
    label.set_halign(gtk::Align::Center);
    label.set_valign(gtk::Align::Center);

    vbox.append(&spinner);
    vbox.append(&label);

    content_area.append(&vbox);
    content_area.queue_draw();
}
