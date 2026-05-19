use gtk::prelude::*;
use gtk::{Label, Orientation};

pub fn create_sidebar() -> gtk::Box {
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
    sidebar_header.set_margin_bottom(12);
    sidebar.append(&sidebar_header);

    let empty_state = Label::new(Some("No devices added"));
    empty_state.add_css_class("dim-label");
    empty_state.set_halign(gtk::Align::Start);
    empty_state.set_margin_start(16);
    empty_state.set_margin_top(32);
    sidebar.append(&empty_state);

    sidebar
}
