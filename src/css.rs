/// GNOME Fedora Desktop Theme CSS
/// Used by GTK4 + libadwaita

pub const GNOME_COLORS: &str = r#"
/* GNOME Color Variables - Light Mode */
@define-color bg-primary #fafafa;
@define-color bg-secondary #f6f5f4;
@define-color bg-sidebar #f6f5f4;
@define-color bg-card #ffffff;
@define-color text-primary #1e1e1e;
@define-color text-secondary #5e5c64;
@define-color text-muted #8b8b8b;
@define-color border-color #deddda;
@define-color accent #9141ac;
@define-color accent-hover #7c3aad;
@define-color accent-active #613889;
@define-color success #2ec27e;
@define-color warning #e5a50a;
@define-color error #e01b24;

/* GNOME Color Variables - Dark Mode */
@define-color dark-bg-primary #1e1e1e;
@define-color dark-bg-secondary #242424;
@define-color dark-bg-sidebar #242424;
@define-color dark-bg-card #303030;
@define-color dark-text-primary #ffffff;
@define-color dark-text-secondary #9a9996;
@define-color dark-text-muted #77767b;
@define-color dark-border-color #3d3d3d;
@define-color dark-accent #c061cb;
@define-color dark-accent-hover #d17ed9;
@define-color dark-accent-active #9141ac;

/* Typography */
@define-color font-size-xs 12px;
@define-color font-size-sm 14px;
@define-color font-size-base 16px;
@define-color font-size-lg 18px;
@define-color font-size-xl 24px;
@define-color font-size-2xl 30px;

/* Spacing */
@define-color space-xs 4px;
@define-color space-sm 8px;
@define-color space-md 12px;
@define-color space-lg 16px;
@define-color space-xl 24px;
@define-color space-2xl 32px;

/* Border Radius */
@define-color radius-sm 4px;
@define-color radius-md 8px;
@define-color radius-lg 12px;
@define-color radius-xl 16px;
@define-color radius-full 9999px;

/* Transitions */
@define-color transition-fast 150ms;
@define-color transition-base 200ms;
@define-color transition-slow 300ms;

/* Easing */
@define-color ease-out cubic-bezier(0.16, 1, 0.3, 1);
@define-color ease-in-out cubic-bezier(0.4, 0, 0.2, 1);

/* Base window styling */
window {
    background-color: @bg-primary;
    color: @text-primary;
}

window.dark {
    background-color: @dark-bg-primary;
    color: @dark-text-primary;
}

/* Typography base */
label, text {
    font-family: "Cantarell", sans-serif;
    font-size: @font-size-base;
}

/* Header bar styling */
headerbar {
    background-color: @bg-secondary;
    color: @text-primary;
    border-bottom: 1px solid @border-color;
}

headerbar.dark {
    background-color: @dark-bg-secondary;
    color: @dark-text-primary;
    border-bottom: 1px solid @dark-border-color;
}

/* Sidebar styling */
.sidebar {
    background-color: @bg-sidebar;
    border-right: 1px solid @border-color;
}

.sidebar.dark {
    background-color: @dark-bg-sidebar;
    border-right: 1px solid @dark-border-color;
}

/* Card styling */
card {
    background-color: @bg-card;
    border-radius: @radius-lg;
    border: 1px solid @border-color;
}

card.dark {
    background-color: @dark-bg-card;
    border: 1px solid @dark-border-color;
}

/* Button styling */
button.primary {
    background-color: @accent;
    color: white;
    border-radius: @radius-md;
    border: none;
    padding: @space-sm @space-lg;
}

button.primary:hover {
    background-color: @accent-hover;
}

button.primary:active {
    background-color: @accent-active;
}

button.primary.dark {
    background-color: @dark-accent;
}

button.primary.dark:hover {
    background-color: @dark-accent-hover;
}

/* Secondary button */
button.secondary {
    background-color: transparent;
    color: @text-primary;
    border-radius: @radius-md;
    border: 1px solid @border-color;
    padding: @space-sm @space-lg;
}

button.secondary.dark {
    color: @dark-text-primary;
    border: 1px solid @dark-border-color;
}

/* Icon button */
iconbutton {
    border-radius: @radius-md;
    padding: @space-sm;
}

/* List row styling */
listrow {
    padding: @space-md @space-lg;
    border-radius: @radius-sm;
}

listrow:hover {
    background-color: rgba(0, 0, 0, 0.05);
}

listrow:selected {
    background-color: @accent;
    color: white;
}

/* Input styling */
entry {
    background-color: @bg-card;
    border: 1px solid @border-color;
    border-radius: @radius-md;
    padding: @space-sm @space-md;
    color: @text-primary;
}

entry.dark {
    background-color: @dark-bg-card;
    border: 1px solid @dark-border-color;
    color: @dark-text-primary;
}

/* Toggle/Switch styling */
switch {
    border-radius: @radius-full;
}

/* Status page styling */
statuspage {
    padding: @space-2xl;
}

/* Animations */
* {
    transition: all @transition-fast @ease-out;
}
"#;