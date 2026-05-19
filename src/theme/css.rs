/// GNOME Adwaita theme colors supporting both light and dark modes.
pub const GNOME_COLORS: &str = r#"
/* Adwaita CSS variables for automatic light/dark theme support */

window {
    background-color: @theme_bg_color;
    color: @theme_fg_color;
}

/* Sidebar background */
.sidebar {
    background-color: @view_bg_color;
    border-right: 1px solid @borders;
}

/* Content area */
.content-area {
    background-color: @theme_bg_color;
}

/* Sidebar separator */
.sidebar-separator {
    background-color: @borders;
    min-width: 1px;
}

/* Add Device button */
button.add-device-btn {
    border-radius: 8px;
    padding: 4px 16px 4px 20px;
    font-weight: 500;
    font-size: 13px;
    min-width: 100px;
    min-height: 24px;
    transition: all 200ms ease-in-out;
}

button.add-device-btn.suggested-action {
    background-color: @accent_bg_color;
    color: @accent_fg_color;
}

button.add-device-btn.suggested-action:hover {
    opacity: 0.95;
}

button.add-device-btn.suggested-action:active {
    opacity: 0.85;
}

button.add-device-btn.suggested-action:disabled {
    opacity: 0.5;
}

button.add-device-btn image {
    margin-right: 4px;
}

/* Header bar */
headerbar {
    background-color: @theme_surface_bg_color;
    border-bottom: 1px solid @borders;
}

/* Secondary text */
.dim-label {
    opacity: 0.7;
    color: @theme_fg_color;
}
"#;
