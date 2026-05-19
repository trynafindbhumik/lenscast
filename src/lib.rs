mod app;
mod theme;
mod ui;

#[cfg(not(any(target_os = "macos", windows)))]
mod tray;

pub use app::run;
