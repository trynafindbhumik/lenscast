use gtk::prelude::*;
use gtk::{Label, Orientation, Box as GtkBox, Image, Button};
use std::rc::Rc;

/// Creates the main content area shown when no device is selected.
pub fn create_content_area() -> (gtk::Box, gtk::Box) {
    let content_container = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .vexpand(true)
        .hexpand(true)
        .build();

    // Content area that changes based on selection
    let content_area = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .vexpand(true)
        .hexpand(true)
        .build();

    let welcome = Label::new(None);
    welcome.set_markup(
        "<b><big>Welcome to LensCast</big></b>\n\nSelect a device from the sidebar to get started"
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
) {
    // Clear existing children
    while let Some(child) = content_area.first_child() {
        content_area.remove(&child);
    }

    if is_connected {
        // Show hello message when connected
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

        content_area.append(&vbox);
    } else {
        // Show disconnect image and connect button
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
        "<b><big>Welcome to LensCast</big></b>\n\nSelect a device from the sidebar to get started"
    );
    welcome.add_css_class("dim-label");
    welcome.set_justify(gtk::Justification::Center);
    welcome.set_halign(gtk::Align::Center);
    welcome.set_valign(gtk::Align::Center);
    content_area.append(&welcome);
}