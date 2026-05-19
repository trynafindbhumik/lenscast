mod css;

pub use css::GNOME_COLORS;

use adw::prelude::*;
use adw::{ApplicationWindow, StyleManager};
use gtk::CssProvider;

pub fn setup_theme(window: &ApplicationWindow) {
    let provider = CssProvider::new();
    provider.load_from_string(GNOME_COLORS);

    let display: gdk::Display = RootExt::display(window);
    gtk::style_context_add_provider_for_display(
        &display,
        &provider,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let style_manager = StyleManager::default();
    style_manager.set_color_scheme(adw::ColorScheme::Default);
}
