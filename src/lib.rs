use gtk::prelude::*;
use gtk::{CssProvider, StyleContext};
use gdk::Display;
use adw::prelude::*;
use adw::Application as AdwApplication;

mod css;

const APP_ID: &str = "com.lenscast.app";

pub fn run() {
    let app = AdwApplication::builder()
        .application_id(APP_ID)
        .build();

    app.connect_activate(|app| {
        // Setup CSS provider with GNOME theme colors
        setup_css_provider();

        // Create the main window
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .default_width(900)
            .default_height(600)
            .build();

        window.show();
    });

    app.run();
}

fn setup_css_provider() {
    let provider = CssProvider::new();
    provider.load_from_data(css::GNOME_COLORS);

    // For GTK4, add provider to the default display
    if let Some(display) = Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
}