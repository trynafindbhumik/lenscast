// ── pipeline.rs ────────────────────────────────────────────────────────────
//
// Video pipeline using scrcpy + v4l2loopback + GStreamer relay.
//
// Architecture:
//   scrcpy  →  /dev/video8  →  [gst relay: v4l2src→v4l2sink]  →  /dev/video7  ←  Chrome
//
//   • /dev/video8 (LensCast_Src) — scrcpy writes raw YUV frames here.
//   • /dev/video7 (LensCast)      — Chrome reads from here via WebRTC.
//   • GStreamer relay reads from video8, converts, writes to video7 continuously.
//   • While scrcpy is killed between camera switches, v4l2loopback's
//     sustain_framerate=1 repeats the last frame on video8; the relay forwards
//     it to video7 so Chrome never sees a stalled track.
//   • Relay is started in start() (NOT new()) after scrcpy is writing.
//   • Relay is killed only in stop() (full device disconnect).

use super::types::Rotation;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
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

/// Spawns a persistent GStreamer relay:
///   v4l2src device=<input> ! videoconvert ! v4l2sink device=<output> sync=false
///
/// This process keeps /dev/video7 (output) continuously receiving frames.
/// When scrcpy (the writer of /dev/video8 / input) stops between camera
/// switches, v4l2loopback's sustain_framerate=1 repeats the last frame on
/// /dev/video8; the relay reads that frozen frame and writes it to /dev/video7,
/// so Chrome's MediaStreamTrack never ends.
fn start_relay_process(input_device: &str, output_device: &str) -> Result<Child, String> {
    let child = Command::new("gst-launch-1.0")
        .args([
            "-e",
            "v4l2src",
            &format!("device={}", input_device),
            "!",
            "videoconvert",
            "!",
            "v4l2sink",
            &format!("device={}", output_device),
            "sync=false",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("failed to start GStreamer relay: {e}"))?;
    Ok(child)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    /// /dev/video8 — intermediate device; scrcpy writes here, relay reads here.
    sink_device: String,
    /// /dev/video7 — Chrome-facing device; relay writes here, Chrome reads here.
    output_device: String,
    camera_id: AtomicU32,
    scrcpy_process: Mutex<Option<Child>>,
    /// Persistent GStreamer relay (sink_device → output_device).
    /// This is intentionally NEVER restarted during camera switching so that
    /// Chrome always has an active VIDIOC_STREAMON writer on output_device and
    /// never transitions its MediaStreamTrack to the 'ended' state.
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

        // NOTE: relay is NOT started here. It is started in start() AFTER scrcpy
        // is writing to sink_device. Starting relay before scrcpy → v4l2src finds
        // empty device → relay crashes → scrcpy connects to dead sink.
        let relay = None;

        Ok(Self {
            sink_device: scrcpy_device,
            output_device: chrome_device,
            camera_id: AtomicU32::new(camera_id),
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

        // Start scrcpy FIRST so /dev/video8 has a live writer before the relay
        // tries to read from it. scrcpy opens the v4l2sink and begins writing frames.
        log::info!("[pipeline] calling start_scrcpy()");
        self.start_scrcpy()?;

        // scrcpy connects via ADB and opens the v4l2sink. This takes ~1-2s.
        // Wait long enough for scrcpy to fully start streaming before the relay
        // tries to read from /dev/video8 (v4l2src needs an active STREAMON writer).
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

        log::info!(
            "[pipeline] start_relay: spawning gst-launch v4l2src={} v4l2sink={}",
            self.sink_device,
            self.output_device
        );

        // Try up to 3 times: if v4l2src crashes, retry after a delay so scrcpy
        // has more time to start writing frames to /dev/video8.
        for attempt in 1..=3 {
            match start_relay_process(&self.sink_device, &self.output_device) {
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

    pub fn set_rotation(&self, _rotation: Rotation) {}
    pub fn set_horizontal_flip(&self, _on: bool) {}
    pub fn set_vertical_flip(&self, _on: bool) {}
}

impl Drop for VideoPipeline {
    fn drop(&mut self) {
        self.stop();
    }
}
