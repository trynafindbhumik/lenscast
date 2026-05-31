use gtk::prelude::*;
use gtk::{Label, Orientation};

/// Creates the main content area shown when no device is selected.
pub fn create_content_area() -> gtk::Box {
    let content_area = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .hexpand(true)
        .build();

    let welcome = Label::new(None);
    welcome.set_markup(
        "<b><big>Welcome to LensCast</big></b>\n\nConnect your Android device to use it as a webcam"
    );
    welcome.add_css_class("dim-label");
    welcome.set_justify(gtk::Justification::Center);
    content_area.append(&welcome);

    content_area
}