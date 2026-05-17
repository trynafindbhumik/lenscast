use gtk::prelude::*;
use gtk::{Application, ApplicationWindow, Label};

pub fn run() {
    let app = Application::builder()
        .application_id("com.lenscast.app")
        .build();

    app.connect_activate(|app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("LensCast")
            .default_width(800)
            .default_height(600)
            .build();

        let label = Label::new(Some("Hello from LensCast!"));
        window.set_child(Some(&label));
        window.show();
    });

    app.run();
}