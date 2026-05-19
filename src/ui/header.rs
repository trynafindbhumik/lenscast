use gtk::prelude::*;
use gtk::{Button, Image, Label, Orientation};

pub fn create_add_device_button() -> Button {
    let add_device_btn = Button::builder().label("Add Device").build();
    let btn_box = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .spacing(5)
        .homogeneous(false)
        .build();
    let plus_icon = Image::from_icon_name("list-add-symbolic");
    plus_icon.set_icon_size(gtk::IconSize::Normal);
    btn_box.append(&plus_icon);
    btn_box.append(&Label::new(Some("Add Device")));
    add_device_btn.set_child(Some(&btn_box));
    add_device_btn.add_css_class("add-device-btn");
    add_device_btn.add_css_class("suggested-action");
    add_device_btn.set_margin_start(12);
    add_device_btn
}

pub fn create_header_bar(add_device_btn: &Button) -> adw::HeaderBar {
    let title_label = Label::new(None);
    title_label.set_markup("<b>LensCast</b>");
    let header_bar = adw::HeaderBar::new();
    header_bar.set_title_widget(Some(&title_label));
    header_bar.pack_start(add_device_btn);
    header_bar
}
