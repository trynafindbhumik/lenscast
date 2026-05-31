use adw::prelude::*;
use adw::{ApplicationWindow as AdwApplicationWindow, HeaderBar, ToolbarView};
use gtk::Orientation;

/// Creates a vertical separator between sidebar and content area.
pub fn create_separator() -> gtk::Box {
    let separator = gtk::Box::builder()
        .orientation(Orientation::Vertical)
        .vexpand(true)
        .build();
    separator.set_size_request(1, -1);
    separator.add_css_class("sidebar-separator");
    separator
}

/// Combines sidebar, separator, and content area into a single box.
pub fn create_content_box(
    sidebar: &gtk::Box,
    separator: &gtk::Box,
    content_area: &gtk::Box,
) -> gtk::Box {
    let content_box = gtk::Box::builder()
        .orientation(Orientation::Horizontal)
        .vexpand(true)
        .build();
    content_box.add_css_class("content-area");

    content_box.append(sidebar);
    content_box.append(separator);
    content_box.append(content_area);

    content_box
}

/// Builds the main application window.
pub fn create_window(
    app: &adw::Application,
    header_bar: &HeaderBar,
    content: &impl gtk::prelude::IsA<gtk::Widget>,
) -> AdwApplicationWindow {
    let window = AdwApplicationWindow::builder()
        .application(app)
        .default_width(900)
        .default_height(600)
        .deletable(true)
        .build();

    let toolbar_view = ToolbarView::builder().vexpand(true).build();
    toolbar_view.add_top_bar(header_bar);
    toolbar_view.set_content(Some(content));

    window.set_content(Some(&toolbar_view));
    window.set_resizable(true);

    window
}