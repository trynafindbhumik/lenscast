mod adb;
mod app;
mod theme;
mod ui;
mod video;

#[cfg(not(any(target_os = "macos", windows)))]
mod tray;

pub use app::run;
