/// Messages sent from the system tray to the main application thread.
#[cfg(not(any(target_os = "macos", windows)))]
pub enum TrayMessage {
    Show,
    QuitAndDisconnect,
}
