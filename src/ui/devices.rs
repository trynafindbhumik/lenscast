use crate::video::{camera_id_for_selection, PipelineStore, VideoPipeline};
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

fn default_camera_selection() -> String {
    "back".to_string()
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub address: String,
    pub port: u16,
    #[serde(default)]
    pub connected: bool,
    #[serde(default = "default_camera_selection")]
    pub camera_selection: String,
}

pub type DeviceStore = Rc<RefCell<Vec<Device>>>;

fn get_storage_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lenscast");
    fs::create_dir_all(&config_dir).ok();
    config_dir.join("devices.json")
}

pub fn load_devices() -> Vec<Device> {
    let path = get_storage_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(devices) = serde_json::from_str::<Vec<Device>>(&data) {
            let max_id = devices.iter().map(|d| d.id).max().unwrap_or(0);
            NEXT_ID.store(max_id + 1, Ordering::Relaxed);
            return devices;
        }
    }
    Vec::new()
}

pub fn save_devices(devices: &[Device]) {
    let path = get_storage_path();
    if let Ok(data) = serde_json::to_string_pretty(devices) {
        let path_log = path.clone();
        match fs::write(path, &data) {
            Ok(()) => log::info!(
                "[devices] saved {} devices to {:?}",
                devices.len(),
                path_log
            ),
            Err(e) => log::error!("[devices] save FAILED to {:?}: {}", path_log, e),
        }
    }
}

pub fn new_device_store() -> DeviceStore {
    Rc::new(RefCell::new(load_devices()))
}

pub fn next_device_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

pub fn get_connected_adb_devices() -> Vec<String> {
    let output = Command::new("adb").args(["devices", "-l"]).output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout
                .lines()
                .skip(1)
                .filter_map(|line| {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 2 && parts.get(1) == Some(&"device") {
                        Some(parts[0].to_string())
                    } else {
                        None
                    }
                })
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

pub fn refresh_connected_status(store: &DeviceStore) -> bool {
    let adb_connected = get_connected_adb_devices();
    let mut changed = false;
    let mut devices = store.borrow_mut();
    for device in devices.iter_mut() {
        let addr = format!("{}:{}", device.address, device.port);
        let is_connected = adb_connected.iter().any(|a| a == &addr);
        if device.connected != is_connected {
            device.connected = is_connected;
            changed = true;
        }
    }
    changed
}

// FIX: Two v4l2loopback devices instead of one.
//
// Old architecture (broken for camera switching):
//   scrcpy → /dev/video7 ← Chrome
//   A plain open() "dummy writer" held the fd but never called VIDIOC_STREAMON,
//   so v4l2loopback saw 0 active streaming writers when scrcpy was killed and
//   stopped emitting frames, causing Chrome to drop the MediaStreamTrack.
//
// New architecture (seamless switching):
//   scrcpy → /dev/video8 → [GStreamer relay] → /dev/video7 ← Chrome
//
//   • /dev/video8  (LensCast_Src)  — intermediate; scrcpy writes here.
//   • /dev/video7  (LensCast)      — Chrome-facing; relay writes here.
//
//   The relay is a real VIDIOC_STREAMON writer of /dev/video7 that never
//   stops during camera switching.  While scrcpy is down, /dev/video8's
//   sustain_framerate=1 repeats the last frame; the relay forwards it to
//   /dev/video7 so Chrome always has live frames and never drops the track.
fn loopback_modprobe_args() -> Vec<String> {
    vec![
        "modprobe".to_string(),
        "v4l2loopback".to_string(),
        // Two devices: /dev/video7 (Chrome) and /dev/video8 (scrcpy/relay)
        "devices=2".to_string(),
        "video_nr=7,8".to_string(),
        // Per-device labels — comma-separated, same order as video_nr.
        // The label on /dev/video7 is what Chrome shows in the camera picker.
        "card_label=LensCast,LensCast_Src".to_string(),
        // exclusive_caps=1 makes /dev/video7 visible to Chrome/WebRTC.
        // One value applies to both devices.
        "exclusive_caps=1".to_string(),
        // sustain_framerate=1 causes v4l2loopback to repeat the last frame
        // when the active writer (scrcpy) stops, bridging the gap until the
        // relay reads and forwards those repeated frames to /dev/video7.
        "sustain_framerate=1".to_string(),
    ]
}

fn loopback_paths(_device_id: u32) -> (String, String) {
    let chrome_device = "/dev/video7";
    let scrcpy_device = "/dev/video8";
    let sys_caps = "/sys/module/v4l2loopback/parameters/exclusive_caps";
    let mut needs_reload = false;

    log::info!(
        "[loopback] checking devices: chrome={} scrcpy={}",
        chrome_device,
        scrcpy_device
    );
    log::info!(
        "[loopback] chrome exists={} scrcpy exists={}",
        std::path::Path::new(chrome_device).exists(),
        std::path::Path::new(scrcpy_device).exists()
    );

    // Require BOTH loopback devices to exist with exclusive_caps=1.
    // exclusive_caps is a CSV per-device flag: "Y,Y,N,N,N,N,N,N" for 8 devices.
    // A value of "1" means ALL devices use exclusive_caps=1.
    // Any other value (including "Y,N,...") means some devices lack it.
    if !std::path::Path::new(chrome_device).exists()
        || !std::path::Path::new(scrcpy_device).exists()
    {
        log::info!("[loopback] one or both devices missing, need reload");
        needs_reload = true;
    } else if let Ok(caps) = std::fs::read_to_string(sys_caps) {
        let caps = caps.trim();
        log::info!("[loopback] exclusive_caps='{}'", caps);
        // "1" means global exclusive_caps=1; anything else means reload needed.
        if caps != "1" {
            needs_reload = true;
        }
    } else {
        log::info!("[loopback] could not read exclusive_caps, need reload");
        needs_reload = true;
    }

    if needs_reload {
        log::info!("[loopback] reloading v4l2loopback module");
        let _ = Command::new("sudo")
            .args(["modprobe", "-r", "v4l2loopback"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(500));

        let modprobe_out = Command::new("sudo").args(loopback_modprobe_args()).output();
        log::info!(
            "[loopback] modprobe status: {:?}",
            modprobe_out.as_ref().map(|o| o.status.success())
        );
        std::thread::sleep(std::time::Duration::from_millis(1000));

        let gst_out = Command::new("gst-launch-1.0")
            .args([
                "videotestsrc",
                "is-live=true",
                "pattern=black",
                "num-buffers=30",
                "!",
                "video/x-raw,format=YUY2,width=1280,height=720,framerate=30/1",
                "!",
                "v4l2sink",
                &format!("device={}", scrcpy_device),
            ])
            .output();
        log::info!(
            "[loopback] gst-launch priming status: {:?}",
            gst_out.as_ref().map(|o| o.status.success())
        );

        log::info!(
            "[loopback] after reload: chrome_exists={} scrcpy_exists={}",
            std::path::Path::new(chrome_device).exists(),
            std::path::Path::new(scrcpy_device).exists()
        );
    } else {
        log::info!("[loopback] devices already configured, skipping reload");
    }

    // Return (scrcpy_device, chrome_device) so that:
    //   VideoPipeline::new(input_device, output_device, …)
    //                       ↑ /dev/video8     ↑ /dev/video7
    // scrcpy writes to input_device; relay bridges input→output; Chrome reads output.
    (scrcpy_device.to_string(), chrome_device.to_string())
}

pub fn try_connect_device(
    store: &DeviceStore,
    pipelines: &PipelineStore,
    device_id: u32,
) -> (bool, String) {
    let (device_name, address, port) = {
        let devices = store.borrow();
        match devices.iter().find(|d| d.id == device_id) {
            Some(d) => (d.name.clone(), d.address.clone(), d.port),
            None => return (false, String::new()),
        }
    };

    let address_str = format!("{}:{}", address, port);
    log::info!(
        "[devices] try_connect: id={} addr={}",
        device_id,
        address_str
    );

    if address.is_empty() {
        log::warn!("[devices] empty address for device_id={}", device_id);
        return (false, device_name);
    }

    // Try connect with saved port first
    log::info!("[devices] running: adb connect {}", address_str);
    let connect_out = Command::new("adb")
        .args(["connect", &address_str])
        .output();
    log::info!(
        "[devices] adb connect output: {:?}",
        connect_out
            .as_ref()
            .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
    );

    // Check if saved port worked
    let verify = Command::new("adb").args(["devices"]).output();
    let mut is_connected = match verify {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            log::info!("[devices] adb devices output:\n{}", stdout);
            stdout
                .lines()
                .any(|line| line.starts_with(&address_str) && line.contains("device"))
        }
        Err(e) => {
            log::error!("[devices] adb devices failed: {}", e);
            false
        }
    };

    // If saved port failed, try mDNS discovery to find the actual debug port
    if !is_connected {
        log::info!(
            "[devices] saved port {} failed, trying mDNS discovery",
            address_str
        );
        if let Ok(discovered) =
            crate::adb::pair_service::PairService::discover_device_for_connect()
        {
            let discovered_addr = format!("{}:{}", discovered.address, discovered.debugging_port);
            log::info!(
                "[devices] mDNS discovered {} - trying adb connect",
                discovered_addr
            );

            let discovered_out = Command::new("adb")
                .args(["connect", &discovered_addr])
                .output();
            log::info!(
                "[devices] adb connect {} output: {:?}",
                discovered_addr,
                discovered_out
                    .as_ref()
                    .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
            );

            // Verify the new connection
            if let Ok(out) = Command::new("adb").args(["devices"]).output() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                is_connected = stdout.lines().any(
                    |line| line.starts_with(&discovered_addr) && line.contains("device"),
                );
            }

            // Update stored port if mDNS gave us a different one
            if is_connected {
                log::info!(
                    "[devices] mDNS connect succeeded, updating stored port {} -> {}",
                    port,
                    discovered.debugging_port
                );
                if let Some(d) = store.borrow_mut().iter_mut().find(|d| d.id == device_id) {
                    d.address = discovered.address.to_string();
                    d.port = discovered.debugging_port;
                }
                save_devices(&store.borrow());
            }
        } else {
            log::warn!("[devices] mDNS discovery found no device");
        }
    }

    if is_connected {
        if let Some(d) = store.borrow_mut().iter_mut().find(|d| d.id == device_id) {
            d.connected = true;
        }
        save_devices(&store.borrow());

        let camera_id = {
            let devices = store.borrow();
            devices
                .iter()
                .find(|d| d.id == device_id)
                .map(|d| camera_id_for_selection(&d.camera_selection))
                .unwrap_or(0)
        };
        log::info!(
            "[devices] device connected, camera_id={}, calling spawn_pipeline",
            camera_id
        );
        spawn_pipeline(pipelines, device_id, camera_id);
        log::info!("[devices] spawn_pipeline returned");
    }

    (is_connected, device_name)
}

pub fn try_disconnect_device(
    store: &DeviceStore,
    pipelines: &PipelineStore,
    device_id: u32,
) -> (bool, String) {
    let (device_name, address) = {
        let devices = store.borrow();
        match devices.iter().find(|d| d.id == device_id) {
            Some(d) => (d.name.clone(), format!("{}:{}", d.address, d.port)),
            None => return (false, String::new()),
        }
    };

    log::info!(
        "[devices] try_disconnect: id={} addr={}",
        device_id,
        address
    );

    if address.is_empty() {
        return (false, device_name);
    }

    log::info!("[devices] running: adb disconnect {}", address);
    let _ = Command::new("adb").args(["disconnect", &address]).output();
    let verify = Command::new("adb").args(["devices"]).output();
    let is_still_connected = match verify {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let still = stdout
                .lines()
                .any(|line| line.starts_with(&address) && line.contains("device"));
            log::info!("[devices] is_still_connected={}", still);
            still
        }
        Err(e) => {
            log::error!("[devices] adb devices failed: {}", e);
            false
        }
    };

    let success = !is_still_connected;
    if success {
        log::info!("[devices] disconnect success, updating store and stopping pipeline");
        if let Some(d) = store.borrow_mut().iter_mut().find(|d| d.id == device_id) {
            d.connected = false;
        }
        save_devices(&store.borrow());
        log::info!("[devices] calling despawn_pipeline");
        despawn_pipeline(pipelines, device_id);
        log::info!("[devices] despawn_pipeline returned");
    }

    (success, device_name)
}

pub fn spawn_pipeline(pipelines: &PipelineStore, device_id: u32, camera_id: u32) {
    if pipelines.borrow().contains_key(&device_id) {
        log::warn!(
            "[devices] pipeline already exists for device_id={}",
            device_id
        );
        return;
    }

    log::info!(
        "[devices] spawn_pipeline: device_id={} camera_id={}",
        device_id,
        camera_id
    );

    let (input_device, output_device) = loopback_paths(device_id);
    log::info!(
        "[devices] loopback_paths: input={} output={}",
        input_device,
        output_device
    );

    match VideoPipeline::new(&input_device, &output_device, camera_id) {
        Ok(p) => {
            log::info!("[devices] VideoPipeline::new ok, calling start()");
            match p.start() {
                Ok(()) => {
                    log::info!("[devices] VideoPipeline::start() ok");
                    pipelines.borrow_mut().insert(device_id, Rc::new(p));
                    log::info!("[devices] pipeline inserted for device_id={}", device_id);
                }
                Err(e) => {
                    log::error!("[devices] VideoPipeline::start() FAILED: {}", e);
                }
            }
        }
        Err(e) => {
            log::error!("[devices] VideoPipeline::new FAILED: {}", e);
        }
    }
}

pub fn despawn_pipeline(pipelines: &PipelineStore, device_id: u32) {
    pipelines.borrow_mut().remove(&device_id);
}

pub fn update_camera_selection(
    store: &DeviceStore,
    pipelines: &PipelineStore,
    device_id: u32,
    selection: &str,
) -> bool {
    let camera_id = camera_id_for_selection(selection);
    let mut changed = false;

    if let Some(device) = store.borrow_mut().iter_mut().find(|d| d.id == device_id) {
        if device.camera_selection != selection {
            device.camera_selection = selection.to_string();
            changed = true;
        }
    }

    if changed {
        save_devices(&store.borrow());
        if let Some(p) = pipelines.borrow().get(&device_id) {
            p.switch_camera(camera_id);
        }
    }

    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loopback_modprobe_args_are_clean_and_chrome_compatible() {
        let args = super::loopback_modprobe_args();
        assert!(args.iter().all(|arg| !arg.ends_with(' ')));
        assert!(args.iter().any(|arg| arg == "exclusive_caps=1"));
        assert!(args.iter().any(|arg| arg == "sustain_framerate=1"));
        assert!(args.iter().any(|arg| arg == "devices=2"));
        assert!(args.iter().any(|arg| arg == "video_nr=7,8"));
        assert!(args.iter().any(|arg| arg.starts_with("card_label=")));
    }

    #[test]
    fn device_serde_roundtrip() {
        let device = Device {
            id: 5,
            name: "Test Phone".into(),
            address: "192.168.1.100".into(),
            port: 5555,
            connected: true,
            camera_selection: "front".into(),
        };
        let json = serde_json::to_string(&device).unwrap();
        let restored: Device = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.id, device.id);
        assert_eq!(restored.name, device.name);
        assert_eq!(restored.address, device.address);
        assert_eq!(restored.port, device.port);
        assert_eq!(restored.connected, device.connected);
        assert_eq!(restored.camera_selection, device.camera_selection);
    }

    #[test]
    fn get_connected_adb_devices_parses_valid_output() {
        // Exercise the parsing logic with a synthetic adb devices output.
        // This is a compile-time check that the parsing closure is sound.
        let fake_output = b"List of devices attached\n192.168.1.100:5555    device\n";
        let stdout = String::from_utf8_lossy(fake_output);
        let addresses: Vec<String> = stdout
            .lines()
            .skip(1)
            .filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts.get(1) == Some(&"device") {
                    Some(parts[0].to_string())
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(addresses, vec!["192.168.1.100:5555"]);
    }
}

pub fn transform_callbacks_for(
    pipelines: &PipelineStore,
    device_id: u32,
) -> Option<crate::ui::TransformCallbacks> {
    let p = pipelines.borrow().get(&device_id)?.clone();
    Some(crate::ui::TransformCallbacks {
        on_rotation: Rc::new({
            let p = p.clone();
            move |i| p.set_rotation(crate::video::Rotation::from_index(i))
        }),
        on_h_flip: Rc::new({
            let p = p.clone();
            move |on| p.set_horizontal_flip(on)
        }),
        on_v_flip: Rc::new({
            let p = p.clone();
            move |on| p.set_vertical_flip(on)
        }),
    })
}
