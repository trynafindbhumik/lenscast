/// GNOME Adwaita theme colors for light and dark modes.
pub const GNOME_COLORS: &str = r#"
.toast-text {
    margin: 0;
}

window {
    background-color: @theme_bg_color;
    color: @theme_fg_color;
}

.sidebar {
    background-color: @view_bg_color;
    border-right: 1px solid @borders;
}

.content-area {
    background-color: @theme_bg_color;
}

.sidebar-separator {
    background-color: @borders;
    min-width: 1px;
}

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

button.add-device-btn.suggested-action:hover  { opacity: 0.95; }
button.add-device-btn.suggested-action:active { opacity: 0.85; }
button.add-device-btn.suggested-action:disabled { opacity: 0.5; }

button.add-device-btn image { margin-right: 4px; }

headerbar {
    background-color: @theme_surface_bg_color;
    border-bottom: 1px solid @borders;
}

.dim-label {
    opacity: 0.7;
    color: @theme_fg_color;
}

.device-listbox {
    background: transparent;
}

.device-listbox > row {
    background: transparent;
    border-radius: 8px;
    margin: 1px 6px;
    padding: 0;
    min-height: 36px;
}

.device-listbox > row:hover {
    background-color: alpha(@theme_fg_color, 0.07);
}

.device-listbox > row:active {
    background-color: alpha(@theme_fg_color, 0.12);
}

.device-row-icon {
    color: @accent_color;
    opacity: 0.80;
}

.device-row-label {
    font-size: 13px;
    font-weight: 500;
    color: @theme_fg_color;
}

.device-connected-indicator {
    color: #2ec27e;
    font-size: 16px;
    font-weight: bold;
    margin-right: 6px;
}

button.device-menu-btn {
    min-width:  26px;
    min-height: 26px;
    padding:    2px;
    border-radius: 6px;
    opacity: 0.0;
    transition: opacity 150ms ease;
}

row:hover button.device-menu-btn {
    opacity: 0.55;
}

button.device-menu-btn:hover {
    opacity: 1.0;
    background-color: alpha(@theme_fg_color, 0.10);
}

button.device-menu-btn:active {
    opacity: 1.0;
    background-color: alpha(@theme_fg_color, 0.18);
}

.device-popover > contents {
    padding: 4px;
    min-width: 160px;
}

button.device-popover-item {
    border-radius: 6px;
    padding: 7px 10px;
    font-size: 13px;
    min-height: 0;
}

button.device-popover-item:hover {
    background-color: alpha(@theme_fg_color, 0.07);
}

button.device-popover-item.destructive-action {
    color: @error_color;
}

button.device-popover-item.destructive-action:hover {
    background-color: alpha(@error_color, 0.10);
}

.modal-icon-outer {
    border-radius: 66px;
    padding: 12px;
    background-color: alpha(@accent_bg_color, 0.08);
    animation: modal-ring-pulse 3s ease-in-out infinite;
}

.modal-icon-inner {
    border-radius: 54px;
    padding: 20px;
    background-color: alpha(@accent_bg_color, 0.20);
}

.modal-phone-icon {
    color: @accent_color;
}

.modal-wifi-badge {
    background-color: @accent_bg_color;
    color: @accent_fg_color;
    border-radius: 10px;
    padding: 2px 3px;
    margin: 0;
}

@keyframes modal-ring-pulse {
    0%,  100% { background-color: alpha(@accent_bg_color, 0.06); }
    50%        { background-color: alpha(@accent_bg_color, 0.16); }
}

.modal-step-label {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.6px;
    color: @accent_color;
    opacity: 0.9;
}

.modal-heading {
    font-size: 17px;
    font-weight: 700;
    color: @theme_fg_color;
}

.modal-field-label {
    font-size: 13px;
    font-weight: 500;
    color: @theme_fg_color;
    opacity: 0.85;
}

button.modal-info-btn {
    min-width:  18px;
    min-height: 18px;
    padding:    1px;
    opacity:    0.50;
    transition: opacity 150ms ease;
}

button.modal-info-btn:hover { opacity: 1.0; }

button.modal-info-btn image {
    -gtk-icon-size: 13px;
}

entry.modal-entry {
    min-height:  24px;
    padding:     8px 10px;
    border-radius: 8px;
    font-size:   13px;
}

entry.modal-entry.error,
entry.modal-entry.error:focus {
    border-color: @error_color;
    box-shadow:   0 0 0 1px @error_color;
}

button.modal-next-btn {
    min-width:    28px;
    min-height:   28px;
    padding:      5px;
    border-radius: 50%;
}

button.modal-next-btn image {
    -gtk-icon-size: 16px;
}

.modal-error-label {
    font-size:   12px;
    font-weight: 500;
    color: @error_color;
}

.step2-title {
    font-size: 18px;
    font-weight: 700;
    color: @theme_fg_color;
}

.step2-subtitle {
    font-size: 13px;
    color: @theme_fg_color;
    opacity: 0.65;
}

.step2-panel-heading {
    font-size: 12px;
    font-weight: 700;
    letter-spacing: 0.5px;
    color: @accent_color;
    text-transform: uppercase;
}

.step2-field-label {
    font-size: 12px;
    font-weight: 500;
    color: @theme_fg_color;
    opacity: 0.80;
}

entry.step2-field-entry {
    min-height: 24px;
    padding: 7px 10px;
    border-radius: 8px;
    font-size: 13px;
}

entry.step2-field-entry.error,
entry.step2-field-entry.error:focus {
    border-color: @error_color;
    box-shadow: 0 0 0 1px @error_color;
}

.step2-error-label {
    font-size: 12px;
    font-weight: 500;
    color: @error_color;
}

.step2-panel-divider {
    background-color: @borders;
    min-width: 1px;
    margin-top: 0;
    margin-bottom: 0;
}

.step2-qr-card {
    background-color: @view_bg_color;
    border-radius: 16px;
    border: 1px solid @borders;
    padding: 20px;
}

.qr-ascii {
    font-family: "JetBrains Mono", "Fira Code", "SF Mono", "Cascadia Code", "Courier New", monospace;
    font-size: 10px;
    line-height: 1.1;
    letter-spacing: 0px;
    background-color: @view_bg_color;
    color: @theme_fg_color;
    border: 1px solid @borders;
    border-radius: 8px;
    padding: 16px;
}

.step2-qr-card textview.qr-ascii {
    background-color: @view_bg_color;
    outline: none;
}

.step2-qr-card > scrolledwindow {
    background-color: transparent;
    border: none;
    box-shadow: none;
}

.step2-qr-card > scrolledwindow > viewport {
    background-color: transparent;
}

.step2-qr-card > scrolledwindow > scrollbar {
    opacity: 0;
    min-width: 0;
    min-height: 0;
}

.step2-qr-image {
    border-radius: 6px;
    border: 1px solid alpha(@borders, 0.5);
    background-color: white;
    padding: 6px;
}

.step2-qr-hint {
    font-size: 11.5px;
    color: @theme_fg_color;
    opacity: 0.60;
    line-height: 1.55;
}

button.step2-connect-btn {
    border-radius: 10px;
    padding: 10px 24px;
    font-size: 14px;
    font-weight: 600;
    min-height: 20px;
}

.step2-spinner {
    -gtk-icon-size: 52px;
    color: @accent_color;
}

.connecting-title {
    font-size: 17px;
    font-weight: 700;
    color: @theme_fg_color;
}

.connecting-device-label {
    font-size: 13px;
    color: @theme_fg_color;
    opacity: 0.75;
}

.connecting-hint {
    font-size: 12px;
    color: @theme_fg_color;
    opacity: 0.55;
}

.code-icon-wrap {
    background-color: alpha(@accent_bg_color, 0.15);
    border-radius: 50px;
    padding: 18px;
}

.code-icon {
    color: @accent_color;
}

.code-title {
    font-size: 17px;
    font-weight: 700;
    color: @theme_fg_color;
}

.code-subtitle {
    font-size: 13px;
    color: @theme_fg_color;
    opacity: 0.65;
    line-height: 1.6;
}

entry.code-entry {
    font-size: 26px;
    font-weight: 700;
    letter-spacing: 12px;
    min-height: 32px;
    padding: 10px 20px;
    border-radius: 12px;
    color: @theme_fg_color;
}

button.code-submit-btn {
    border-radius: 8px;
    padding: 8px 24px;
    font-size: 14px;
    font-weight: 600;
}

button.code-cancel-btn {
    border-radius: 8px;
    padding: 8px 18px;
    font-size: 14px;
}
"#;