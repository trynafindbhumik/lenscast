// scrcpy → /dev/video8 → [gst relay] → /dev/video7 ← Chrome

use super::types::Rotation;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::Mutex;

pub fn camera_id_for_selection(selection: &str) -> u32 {
    match selection.trim().to_lowercase().as_str() {
        "front" => 1,
        _ => 0,
    }
}

/// Returns scrcpy arguments for camera capture with v4l2sink output.
/// scrcpy writes to the intermediate device (/dev/video8); the relay forwards
/// frames to the Chrome-facing device (/dev/video7).
pub fn scrcpy_command_args(sink_device: &str, camera_id: u32) -> Vec<String> {
    vec![
        "--video-source=camera".to_string(),
        format!("--camera-id={camera_id}"),
        "--camera-size=1280x720".to_string(),
        "--camera-fps=30".to_string(),
        "--video-bit-rate=6M".to_string(),
        "--video-codec=h264".to_string(),
        format!("--v4l2-sink={sink_device}"),
        "--no-playback".to_string(),
        "--no-window".to_string(),
    ]
}

// Transform order: rotation → h_flip → v_flip.
fn build_relay_args(
    input_device: &str,
    output_device: &str,
    rotation: Rotation,
    h_flip: bool,
    v_flip: bool,
) -> Vec<String> {
    let mut args = vec![
        "-e".to_string(),
        "v4l2src".to_string(),
        format!("device={}", input_device),
        "!".to_string(),
        "videoconvert".to_string(),
        "!".to_string(),
    ];

    if rotation != Rotation::Original {
        args.push("videoflip".to_string());
        args.push(format!("method={}", rotation.method()));
        args.push("!".to_string());
    }

    if h_flip {
        args.push("videoflip".to_string());
        args.push("method=horizontal-flip".to_string());
        args.push("!".to_string());
    }

    if v_flip {
        args.push("videoflip".to_string());
        args.push("method=vertical-flip".to_string());
        args.push("!".to_string());
    }

    args.push("v4l2sink".to_string());
    args.push(format!("device={}", output_device));
    args.push("sync=false".to_string());

    args
}

// Keeps /dev/video7 fed via relay (sink→output). sustain_framerate=1 handles scrcpy gaps.
fn start_relay_process(
    input_device: &str,
    output_device: &str,
    rotation: Rotation,
    h_flip: bool,
    v_flip: bool,
) -> Result<Child, String> {
    let args = build_relay_args(input_device, output_device, rotation, h_flip, v_flip);
    Command::new("gst-launch-1.0")
        .args(&args)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to start GStreamer relay: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relay_args_no_transforms_has_no_videoflip() {
        let args = build_relay_args(
            "/dev/video8",
            "/dev/video7",
            Rotation::Original,
            false,
            false,
        );
        assert!(!args.iter().any(|a| a == "videoflip"));
        assert!(args.iter().any(|a| a == "v4l2src"));
        assert!(args.iter().any(|a| a == "v4l2sink"));
        assert!(args.iter().any(|a| a == "videoconvert"));
    }

    #[test]
    fn relay_args_rotation_inserts_videoflip_before_sink() {
        let args = build_relay_args("/dev/video8", "/dev/video7", Rotation::Rot90, false, false);
        assert!(args.iter().any(|a| a == "videoflip"));
        assert!(args.iter().any(|a| a == "method=clockwise"));
        // videoflip must appear before v4l2sink
        let flip_pos = args.iter().position(|a| a == "videoflip").unwrap();
        let sink_pos = args.iter().position(|a| a == "v4l2sink").unwrap();
        assert!(flip_pos < sink_pos);
    }

    #[test]
    fn relay_args_h_flip_only() {
        let args = build_relay_args(
            "/dev/video8",
            "/dev/video7",
            Rotation::Original,
            true,
            false,
        );
        assert!(args.iter().any(|a| a == "method=horizontal-flip"));
        assert!(!args.iter().any(|a| a == "method=vertical-flip"));
    }

    #[test]
    fn relay_args_v_flip_only() {
        let args = build_relay_args(
            "/dev/video8",
            "/dev/video7",
            Rotation::Original,
            false,
            true,
        );
        assert!(args.iter().any(|a| a == "method=vertical-flip"));
        assert!(!args.iter().any(|a| a == "method=horizontal-flip"));
    }

    #[test]
    fn relay_args_rotation_plus_both_flips_has_three_videoflip_elements() {
        let args = build_relay_args("/dev/video8", "/dev/video7", Rotation::Rot180, true, true);
        let flip_count = args.iter().filter(|a| a.as_str() == "videoflip").count();
        assert_eq!(flip_count, 3);
        assert!(args.iter().any(|a| a == "method=rotate-180"));
        assert!(args.iter().any(|a| a == "method=horizontal-flip"));
        assert!(args.iter().any(|a| a == "method=vertical-flip"));
    }

    #[test]
    fn relay_args_no_trailing_spaces() {
        let args = build_relay_args("/dev/video8", "/dev/video7", Rotation::Rot270, true, true);
        assert!(args.iter().all(|a| !a.ends_with(' ')));
    }

    #[test]
    fn scrcpy_args_include_camera_source_and_v4l2_sink() {
        // scrcpy now writes to the intermediate device /dev/video8, not the
        // Chrome-facing /dev/video7.  The relay bridges the two.
        let args = scrcpy_command_args("/dev/video8", 0);
        assert!(args.contains(&"--video-source=camera".to_string()));
        assert!(args.contains(&"--v4l2-sink=/dev/video8".to_string()));
        assert!(args.contains(&"--camera-id=0".to_string()));
        assert!(args.iter().all(|arg| !arg.ends_with(' ')));
    }

    #[test]
    fn camera_selection_defaults_to_back_and_front() {
        assert_eq!(camera_id_for_selection("back"), 0);
        assert_eq!(camera_id_for_selection("front"), 1);
        assert_eq!(camera_id_for_selection("unknown"), 0);
    }

    #[test]
    fn camera_selection_is_case_insensitive_and_trims_whitespace() {
        assert_eq!(camera_id_for_selection("Front"), 1);
        assert_eq!(camera_id_for_selection("FRONT"), 1);
        assert_eq!(camera_id_for_selection("  front  "), 1);
        assert_eq!(camera_id_for_selection("Back"), 0);
        assert_eq!(camera_id_for_selection("\tback\n"), 0);
    }
}

pub struct VideoPipeline {
    sink_device: String,   // /dev/video8 — scrcpy writes, relay reads.
    output_device: String, // /dev/video7 — relay writes, Chrome reads.
    camera_id: AtomicU32,
    rotation_idx: AtomicU8, // Written atomically so transform setters run on any thread.
    h_flip: AtomicBool,
    v_flip: AtomicBool,
    scrcpy_process: Mutex<Option<Child>>,
    // Restarted on transform change; keeps running during camera switch so Chrome's track stays alive.
    relay_process: Mutex<Option<Child>>,
}

impl VideoPipeline {
    pub fn new(input_device: &str, output_device: &str, camera_id: u32) -> Result<Self, String> {
        log::info!(
            "[pipeline] VideoPipeline::new: input={} output={} camera_id={}",
            input_device,
            output_device,
            camera_id
        );

        let scrcpy_device = input_device.to_string();
        let chrome_device = if output_device.is_empty() {
            input_device.to_string()
        } else {
            output_device.to_string()
        };

        log::info!(
            "[pipeline] scrcpy_device={} chrome_device={}",
            scrcpy_device,
            chrome_device
        );

        // NOTE: relay is started in start() after scrcpy is writing.
        let relay = None;

        Ok(Self {
            sink_device: scrcpy_device,
            output_device: chrome_device,
            camera_id: AtomicU32::new(camera_id),
            rotation_idx: AtomicU8::new(0),
            h_flip: AtomicBool::new(false),
            v_flip: AtomicBool::new(false),
            scrcpy_process: Mutex::new(None),
            relay_process: Mutex::new(relay),
        })
    }

    pub fn start(&self) -> Result<(), String> {
        log::info!(
            "[pipeline] start: sink={} output={}",
            self.sink_device,
            self.output_device
        );

        // Start scrcpy first, wait for it to open the v4l2sink before relay reads.
        log::info!("[pipeline] calling start_scrcpy()");
        self.start_scrcpy()?;

        log::info!("[pipeline] waiting 2000ms for scrcpy to start writing frames");
        std::thread::sleep(std::time::Duration::from_millis(2000));

        // Now start the relay — it should find /dev/video8 with an active writer.
        log::info!("[pipeline] calling start_relay()");
        self.start_relay();

        Ok(())
    }

    fn start_relay(&self) {
        if self.output_device.is_empty() || self.output_device == self.sink_device {
            log::info!("[pipeline] start_relay: skipped (single-device mode)");
            return;
        }

        let mut relay = self.relay_process.lock().unwrap();
        if relay.is_some() {
            log::info!("[pipeline] start_relay: already running");
            return;
        }

        // Read current transform state atomically so the relay is always
        // spawned with the settings that were active at call time.
        let rotation = Rotation::from_index(self.rotation_idx.load(Ordering::SeqCst));
        let h_flip = self.h_flip.load(Ordering::SeqCst);
        let v_flip = self.v_flip.load(Ordering::SeqCst);

        log::info!(
            "[pipeline] start_relay: spawning gst-launch v4l2src={} v4l2sink={} \
             rotation={:?} h_flip={} v_flip={}",
            self.sink_device,
            self.output_device,
            rotation,
            h_flip,
            v_flip,
        );

        // Try up to 3 times: if v4l2src crashes, retry after a delay so scrcpy
        // has more time to start writing frames to /dev/video8.
        for attempt in 1..=3 {
            match start_relay_process(
                &self.sink_device,
                &self.output_device,
                rotation,
                h_flip,
                v_flip,
            ) {
                Ok(mut child) => {
                    log::info!("[pipeline] start_relay: spawned, waiting 1500ms for negotiation");
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    match child.try_wait() {
                        Ok(Some(status)) => {
                            log::error!("[pipeline] start_relay: attempt {} crashed with status {}, retrying", attempt, status);
                            drop(child);
                        }
                        Ok(None) => {
                            *relay = Some(child);
                            log::info!("[pipeline] start_relay: ok on attempt {}", attempt);
                            return;
                        }
                        Err(e) => {
                            log::warn!(
                                "[pipeline] start_relay: attempt {} try_wait error: {}",
                                attempt,
                                e
                            );
                            *relay = Some(child);
                            return;
                        }
                    }
                }
                Err(e) => {
                    log::error!(
                        "[pipeline] start_relay: attempt {} spawn failed: {}",
                        attempt,
                        e
                    );
                }
            }
            if attempt < 3 {
                log::info!("[pipeline] start_relay: waiting 2000ms before retry");
                std::thread::sleep(std::time::Duration::from_millis(2000));
            }
        }
        log::error!("[pipeline] start_relay: all 3 attempts exhausted");
    }

    /// Must be called from a background thread — restart_relay blocks ~3×3.5s for GStreamer negotiation.
    fn restart_relay(&self) {
        if self.output_device.is_empty() || self.output_device == self.sink_device {
            return;
        }

        // Kill the old relay process (take() so the Mutex is released before
        // we call start_relay, which also acquires relay_process).
        if let Some(mut child) = self.relay_process.lock().unwrap().take() {
            log::info!("[pipeline] restart_relay: killing old relay");
            let _ = child.kill();
            let _ = child.wait();
        }

        // Brief pause so the v4l2 device is released before the new relay opens it.
        std::thread::sleep(std::time::Duration::from_millis(200));

        self.start_relay();
    }

    fn start_scrcpy(&self) -> Result<(), String> {
        let mut child_guard = self.scrcpy_process.lock().unwrap();
        if child_guard.is_some() {
            log::info!("[pipeline] start_scrcpy: already running");
            return Ok(());
        }

        let args = scrcpy_command_args(&self.sink_device, self.camera_id.load(Ordering::SeqCst));
        log::info!(
            "[pipeline] start_scrcpy: spawning scrcpy with args={:?}",
            args
        );

        let mut child = Command::new("scrcpy")
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| {
                log::error!("[pipeline] start_scrcpy: spawn failed: {}", e);
                format!("failed to start scrcpy: {e}")
            })?;

        match child.try_wait() {
            Ok(Some(status)) => {
                log::error!(
                    "[pipeline] start_scrcpy: scrcpy exited immediately: {}",
                    status
                );
                return Err(format!("scrcpy exited immediately with status {status}"));
            }
            Ok(None) => {
                log::info!("[pipeline] start_scrcpy: scrcpy is running");
            }
            Err(e) => {
                log::warn!("[pipeline] start_scrcpy: try_wait error: {}", e);
            }
        }

        *child_guard = Some(child);
        Ok(())
    }

    fn stop_scrcpy(&self) {
        if let Some(mut child) = self.scrcpy_process.lock().unwrap().take() {
            log::info!("[pipeline] stop_scrcpy: killing scrcpy");
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    fn ensure_relay_running(&self) {
        if self.output_device.is_empty() || self.output_device == self.sink_device {
            return;
        }

        let mut relay = self.relay_process.lock().unwrap();
        let has_exited = match relay.as_mut() {
            Some(child) => matches!(child.try_wait(), Ok(Some(_))),
            None => true,
        };

        if has_exited {
            log::info!("[pipeline] ensure_relay_running: relay dead, restarting");
            *relay = None;
            drop(relay);
            self.start_relay();
        } else {
            log::info!("[pipeline] ensure_relay_running: relay alive");
        }
    }

    pub fn switch_camera(&self, new_camera_id: u32) {
        log::info!("[pipeline] switch_camera: {}", new_camera_id);

        self.ensure_relay_running();
        self.stop_scrcpy();

        std::thread::sleep(std::time::Duration::from_millis(300));

        self.camera_id.store(new_camera_id, Ordering::SeqCst);
        if let Err(e) = self.start_scrcpy() {
            log::error!("[pipeline] switch_camera: start_scrcpy FAILED: {}", e);
        }

        self.start_relay();
        log::info!("[pipeline] switch_camera done");
    }

    pub fn stop(&self) {
        log::info!("[pipeline] stop: stopping scrcpy and relay");
        self.stop_scrcpy();
        if let Some(mut relay) = self.relay_process.lock().unwrap().take() {
            let _ = relay.kill();
            let _ = relay.wait();
        }
        log::info!("[pipeline] stop: done");
    }

    /// Must be called from a background thread — restart_relay blocks.
    pub fn set_rotation(&self, rotation: Rotation) {
        let idx: u8 = match rotation {
            Rotation::Original => 0,
            Rotation::Rot90 => 1,
            Rotation::Rot180 => 2,
            Rotation::Rot270 => 3,
        };
        log::info!("[pipeline] set_rotation: {:?} (idx={})", rotation, idx);
        self.rotation_idx.store(idx, Ordering::SeqCst);
        self.restart_relay();
    }

    pub fn set_horizontal_flip(&self, on: bool) {
        log::info!("[pipeline] set_horizontal_flip: {}", on);
        self.h_flip.store(on, Ordering::SeqCst);
        self.restart_relay();
    }

    pub fn set_vertical_flip(&self, on: bool) {
        log::info!("[pipeline] set_vertical_flip: {}", on);
        self.v_flip.store(on, Ordering::SeqCst);
        self.restart_relay();
    }
}

impl Drop for VideoPipeline {
    fn drop(&mut self) {
        self.stop();
    }
}
