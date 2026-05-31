use adw::prelude::*;
use std::rc::Rc;

use crate::ui::connect_modal::show_connect_modal;

/// Opens the "Add Device" modal — Step 1 (device name).
/// `on_success` is called after the device is successfully paired; the caller
/// receives the chosen device name, address, and port so it can add it to the sidebar.
pub fn show_add_device_modal(
    parent: &adw::ApplicationWindow,
    on_success: Box<dyn Fn(String, String, u16)>,
) {
    let modal = adw::Window::builder()
        .modal(true)
        .transient_for(parent)
        .title("Add Device")
        .default_width(400)
        .default_height(460)
        .resizable(false)
        .deletable(true)
        .build();

    let toolbar_view = adw::ToolbarView::new();

    let header = adw::HeaderBar::builder()
        .show_end_title_buttons(true)
        .build();
    header.add_css_class("flat");
    toolbar_view.add_top_bar(&header);

    //  Outer wrapper — two springs keep content vertically centered 
    let outer = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .vexpand(true)
        .build();

    let top_spring = gtk::Box::builder().vexpand(true).build();
    let bottom_spring = gtk::Box::builder().vexpand(true).build();

    let content = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(0)
        .margin_start(36)
        .margin_end(36)
        .build();

    let icon_outer = gtk::Box::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .margin_bottom(20)
        .build();
    icon_outer.add_css_class("modal-icon-outer");

    let icon_inner = gtk::Box::builder()
        .halign(gtk::Align::Center)
        .valign(gtk::Align::Center)
        .build();
    icon_inner.add_css_class("modal-icon-inner");

    let icon_stack = gtk::Overlay::new();

    let phone_icon = gtk::Image::from_icon_name("phone-symbolic");
    phone_icon.set_pixel_size(48);
    phone_icon.add_css_class("modal-phone-icon");
    icon_stack.set_child(Some(&phone_icon));

    let wifi_badge_wrap = gtk::Box::builder()
        .halign(gtk::Align::End)
        .valign(gtk::Align::End)
        .build();
    wifi_badge_wrap.add_css_class("modal-wifi-badge");
    let wifi_icon = gtk::Image::from_icon_name("network-wireless-symbolic");
    wifi_icon.set_pixel_size(13);
    wifi_badge_wrap.append(&wifi_icon);
    icon_stack.add_overlay(&wifi_badge_wrap);

    icon_inner.append(&icon_stack);
    icon_outer.append(&icon_inner);
    content.append(&icon_outer);

    let step_label = gtk::Label::builder()
        .label("Step 1 of 2")
        .halign(gtk::Align::Center)
        .margin_bottom(4)
        .build();
    step_label.add_css_class("modal-step-label");
    content.append(&step_label);

    let heading = gtk::Label::builder()
        .label("Name your device")
        .halign(gtk::Align::Center)
        .margin_bottom(20)
        .build();
    heading.add_css_class("modal-heading");
    content.append(&heading);

    let label_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(4)
        .halign(gtk::Align::Start)
        .margin_bottom(6)
        .build();

    let field_label = gtk::Label::builder()
        .label("Enter your device name")
        .halign(gtk::Align::Start)
        .build();
    field_label.add_css_class("modal-field-label");

    let info_btn = gtk::Button::from_icon_name("dialog-information-symbolic");
    info_btn.add_css_class("flat");
    info_btn.add_css_class("circular");
    info_btn.add_css_class("modal-info-btn");
    info_btn.set_valign(gtk::Align::Center);
    info_btn.set_can_focus(false);
    info_btn.set_tooltip_text(Some(
        "Assign a custom label to identify this device within LensCast. \
         This name is for your reference only and has no effect on connectivity.",
    ));

    label_row.append(&field_label);
    label_row.append(&info_btn);
    content.append(&label_row);

    let input_row = gtk::Box::builder()
        .orientation(gtk::Orientation::Horizontal)
        .spacing(8)
        .valign(gtk::Align::Center)
        .build();

    let entry = gtk::Entry::builder()
        .placeholder_text("e.g. My Phone")
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    entry.add_css_class("modal-entry");

    let next_btn = gtk::Button::from_icon_name("go-next-symbolic");
    next_btn.add_css_class("suggested-action");
    next_btn.add_css_class("modal-next-btn");
    next_btn.set_valign(gtk::Align::Center);
    next_btn.set_tooltip_text(Some("Continue"));

    input_row.append(&entry);
    input_row.append(&next_btn);
    content.append(&input_row);

    let error_label = gtk::Label::builder()
        .label("Please enter a name for your device.")
        .halign(gtk::Align::Start)
        .margin_top(5)
        .xalign(0.0)
        .build();
    error_label.add_css_class("modal-error-label");
    error_label.set_visible(false);
    content.append(&error_label);

    outer.append(&top_spring);
    outer.append(&content);
    outer.append(&bottom_spring);

    toolbar_view.set_content(Some(&outer));
    modal.set_content(Some(&toolbar_view));

    // Wrap on_success in Rc for multi-closure sharing
    let on_success = Rc::new(on_success);
    let parent_ref = parent.clone();

    //  Validation + Step 2 transition 
    let validate: Rc<dyn Fn()> = Rc::new({
        let entry = entry.clone();
        let error_label = error_label.clone();
        let modal = modal.clone();
        let parent = parent_ref;
        let on_success = on_success.clone();
        move || {
            let text = entry.text();
            if text.trim().is_empty() {
                entry.add_css_class("error");
                error_label.set_visible(true);
                entry.grab_focus();
            } else {
                entry.remove_css_class("error");
                error_label.set_visible(false);
                let name = text.trim().to_string();
                modal.close();
                // Build the step-2 callback: calls on_success with the device name
                let on_s = on_success.clone();
                let name_clone = name.clone();
                show_connect_modal(
                    &parent,
                    name,
                    Box::new(move |address, port| on_s(name_clone.clone(), address, port)),
                );
            }
        }
    });

    next_btn.connect_clicked({ let v = validate.clone(); move |_| v() });
    entry.connect_activate({ let v = validate.clone(); move |_| v() });
    entry.connect_changed({
        let error_label = error_label.clone();
        move |e| {
            if !e.text().is_empty() {
                e.remove_css_class("error");
                error_label.set_visible(false);
            }
        }
    });

    modal.present();
    entry.grab_focus();
}