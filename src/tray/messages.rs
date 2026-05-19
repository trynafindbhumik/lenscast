/// Messages sent from the system tray to the main application thread.
/// Used to request showing the window or quitting the application.
#[cfg(not(any(target_os = "macos", windows)))]
pub enum TrayMessage {
    Show,
    Quit,
}
