use std::cell::{Cell, RefCell};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::fs::OpenOptions;
use super::types::Rotation;

pub fn camera_id_for_selection(selection: &str) -> u32 {
    match selection.trim().to_lowercase().as_str() {
        "front" => 1,
        _ => 0,
    }
}

// FIXED: Removed ALL trailing spaces to prevent scrcpy argument parsing errors
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrcpy_args_include_camera_source_and_v4l2_sink() {
        let args = scrcpy_command_args("/dev/video7", 0);
        assert!(args.contains(&"--video-source=camera".to_string()));
        assert!(args.contains(&"--v4l2-sink=/dev/video7".to_string()));
        assert!(args.contains(&"--camera-id=0".to_string()));
        assert!(args.iter().all(|arg| !arg.ends_with(' ')));
    }

    #[test]
    fn camera_selection_defaults_to_back_and_front() {
        assert_eq!(camera_id_for_selection("back"), 0);
        assert_eq!(camera_id_for_selection("front"), 1);
        assert_eq!(camera_id_for_selection("unknown"), 0);
    }
}

pub struct VideoPipeline {
    sink_device: String,
    camera_id: Cell<u32>,
    scrcpy_process: RefCell<Option<Child>>,
    _dummy_writer_flag: Arc<AtomicBool>,
}

impl VideoPipeline {
    pub fn new(input_device: &str, output_device: &str, camera_id: u32) -> Result<Self, String> {
        let sink_device = if output_device.is_empty() {
            input_device.to_string()
        } else {
            output_device.to_string()
        };

        // CRITICAL FIX: Start a persistent dummy writer thread.
        // This thread opens the V4L2 device and keeps the file descriptor open forever.
        // This ensures that the "writer count" in v4l2loopback never drops to 0,
        // which prevents the kernel from stopping the stream and Chrome from 
        // detecting a "device disconnected" event when scrcpy is restarted.
        // Unlike the GStreamer fallback, this dummy writer does NOT negotiate formats,
        // so it will not cause VIDIOC_G_FMT errors when scrcpy tries to start.
        let sink_device_clone = sink_device.clone();
        let flag = Arc::new(AtomicBool::new(true));
        let flag_clone = flag.clone();
        
        std::thread::spawn(move || {
            eprintln!("[V4L2] Starting persistent dummy writer for {}", sink_device_clone);
            loop {
                if !flag_clone.load(Ordering::Relaxed) {
                    break;
                }
                
                match OpenOptions::new().write(true).open(&sink_device_clone) {
                    Ok(_file) => {
                        eprintln!("[V4L2] Persistent dummy writer successfully opened {}", sink_device_clone);
                        // Keep the file descriptor open by sleeping in a loop.
                        while flag_clone.load(Ordering::Relaxed) {
                            std::thread::sleep(std::time::Duration::from_secs(3600));
                        }
                        break;
                    }
                    Err(e) => {
                        eprintln!("[V4L2] Dummy writer failed to open {}: {}. Retrying...", sink_device_clone, e);
                        std::thread::sleep(std::time::Duration::from_millis(500));
                    }
                }
            }
            eprintln!("[V4L2] Persistent dummy writer for {} exited.", sink_device_clone);
        });

        Ok(Self {
            sink_device: sink_device.clone(),
            camera_id: Cell::new(camera_id),
            scrcpy_process: RefCell::new(None),
            _dummy_writer_flag: flag,
        })
    }

    pub fn start(&self) -> Result<(), String> {
        eprintln!("[Video] starting direct scrcpy sink on {}", self.sink_device);
        self.start_scrcpy()
    }

    fn start_scrcpy(&self) -> Result<(), String> {
        let mut child_guard = self.scrcpy_process.borrow_mut();
        if child_guard.is_some() {
            return Ok(());
        }

        let args = scrcpy_command_args(&self.sink_device, self.camera_id.get());
        eprintln!("[Video] launching scrcpy with args: {:?}", args);

        let mut child = Command::new("scrcpy")
            .args(args)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("failed to start scrcpy: {e}"))?;

        match child.try_wait() {
            Ok(Some(status)) => return Err(format!("scrcpy exited immediately with status {status}")),
            Ok(None) => eprintln!("[Video] scrcpy started with pid {}", child.id()),
            Err(err) => eprintln!("[Video] could not check scrcpy process state: {err}"),
        }

        *child_guard = Some(child);
        Ok(())
    }

    fn stop_scrcpy(&self) {
        if let Some(mut child) = self.scrcpy_process.borrow_mut().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }

    pub fn switch_camera(&self, new_camera_id: u32) {
        eprintln!("[Video] switching camera to id {new_camera_id}");
        
        // 1. Stop scrcpy.
        // The persistent dummy writer is STILL holding the device open, so the 
        // kernel writer count never drops to 0. The last frame is seamlessly frozen.
        self.stop_scrcpy();
        
        // 2. Wait for scrcpy to fully release its file descriptor.
        std::thread::sleep(std::time::Duration::from_millis(300));
        
        self.camera_id.set(new_camera_id);
        
        // 3. Start scrcpy with the new camera. It seamlessly takes over the frozen stream.
        if let Err(e) = self.start_scrcpy() {
            eprintln!("[Video] failed to restart scrcpy for camera switch: {e}");
        }
    }

    pub fn stop(&self) {
        self.stop_scrcpy();
        // Signal the dummy writer thread to exit and close the file descriptor
        self._dummy_writer_flag.store(false, Ordering::Relaxed);
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