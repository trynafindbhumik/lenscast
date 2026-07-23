mod app;
mod theme;
mod ui;
mod adb;
mod video;

#[cfg(not(any(target_os = "macos", windows)))]
mod tray;

pub use app::run;