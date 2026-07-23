use gtk::prelude::*;
use std::rc::Rc;

pub struct TransformCallbacks {
    pub on_rotation: Rc<dyn Fn(u8)>, // 0=Original 1=90 2=180 3=270
    pub on_h_flip: Rc<dyn Fn(bool)>,
    pub on_v_flip: Rc<dyn Fn(bool)>,
}

/// "Video Transformations" section per FLOW.MD UI requirements.
pub fn build_transform_section(cb: TransformCallbacks) -> gtk::Box {
    let section = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(10)
        .margin_top(20)
        .build();

    let heading = gtk::Label::new(None);
    heading.set_markup("<b>Video Transformations</b>");
    heading.set_halign(gtk::Align::Start);
    section.append(&heading);

    let rot_row = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(8).build();
    let rot_lbl = gtk::Label::new(Some("Rotation"));
    rot_lbl.set_halign(gtk::Align::Start);
    rot_lbl.set_hexpand(true);
    let rot_dropdown = gtk::DropDown::from_strings(&["Original", "90°", "180°", "270°"]);
    rot_dropdown.connect_selected_notify({
        let f = cb.on_rotation.clone();
        move |dd| f(dd.selected() as u8)
    });
    rot_row.append(&rot_lbl);
    rot_row.append(&rot_dropdown);
    section.append(&rot_row);

    let h_row = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(8).build();
    let h_lbl = gtk::Label::new(Some("Horizontal Flip"));
    h_lbl.set_halign(gtk::Align::Start);
    h_lbl.set_hexpand(true);
    let h_switch = gtk::Switch::new();
    h_switch.connect_state_set({
        let f = cb.on_h_flip.clone();
        move |_, state| { f(state); gtk::glib::Propagation::Proceed }
    });
    h_row.append(&h_lbl);
    h_row.append(&h_switch);
    section.append(&h_row);

    let v_row = gtk::Box::builder().orientation(gtk::Orientation::Horizontal).spacing(8).build();
    let v_lbl = gtk::Label::new(Some("Vertical Flip"));
    v_lbl.set_halign(gtk::Align::Start);
    v_lbl.set_hexpand(true);
    let v_switch = gtk::Switch::new();
    v_switch.connect_state_set({
        let f = cb.on_v_flip.clone();
        move |_, state| { f(state); gtk::glib::Propagation::Proceed }
    });
    v_row.append(&v_lbl);
    v_row.append(&v_switch);
    section.append(&v_row);

    section
}