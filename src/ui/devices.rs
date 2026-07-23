use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{ AtomicU32, Ordering };
use crate::video::{ camera_id_for_selection, PipelineStore, VideoPipeline };

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
    eprintln!("[Save] Saving {} devices to {:?}", devices.len(), path);
    if let Ok(data) = serde_json::to_string_pretty(devices) {
        match fs::write(path, &data) {
            Ok(_) => eprintln!("[Save] Successfully saved"),
            Err(e) => eprintln!("[Save] Failed to save: {}", e),
        }
    }
}

pub fn new_device_store() -> DeviceStore {
    Rc::new(RefCell::new(load_devices()))
}

pub fn next_device_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

// FIXED: Removed ALL trailing spaces from "adb" and "devices"
pub fn get_connected_adb_devices() -> Vec<String> {
    let output = Command::new("adb").args(["devices", "-l"]).output();
    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.lines().skip(1).filter_map(|line| {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && parts.get(1) == Some(&"device") {
                    Some(parts[0].to_string())
                } else {
                    None
                }
            }).collect()
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

// FIXED: Removed ALL trailing spaces. 
// Previously, "modprobe " caused sudo to fail silently because it looked for 
// an executable literally named "modprobe " (with a space).
fn loopback_modprobe_args() -> Vec<String> {
    vec![
        "modprobe".to_string(),
        "v4l2loopback".to_string(),
        "devices=1".to_string(),
        "video_nr=7".to_string(),
        "card_label=LensCast".to_string(),
        "exclusive_caps=1".to_string(),
        "keep_format=1".to_string(),
        "sustain_framerate=1".to_string(),
    ]
}

fn loopback_paths(_device_id: u32) -> (String, String) {
    let device_path = "/dev/video7";
    let sys_caps = "/sys/module/v4l2loopback/parameters/exclusive_caps";
    let mut needs_reload = false;
    
    if !std::path::Path::new(sys_caps).exists() {
        needs_reload = true;
    } else if let Ok(caps) = std::fs::read_to_string(sys_caps) {
        if caps.trim() != "1" {
            needs_reload = true;
        }
    } else {
        needs_reload = true;
    }

    if needs_reload {
        eprintln!("[V4L2] v4l2loopback missing or misconfigured (exclusive_caps != 1). Reloading...");
        let _ = Command::new("sudo")
            .args(["modprobe", "-r", "v4l2loopback"])
            .output();
        std::thread::sleep(std::time::Duration::from_millis(500));

        let output = Command::new("sudo")
            .args(loopback_modprobe_args())
            .output();
        match output {
            Ok(out) => {
                if !out.status.success() {
                    eprintln!("[V4L2] modprobe failed with status: {}", out.status);
                    eprintln!("[V4L2] stderr: {}", String::from_utf8_lossy(&out.stderr));
                }
            }
            Err(e) => {
                eprintln!("[V4L2] Failed to execute sudo modprobe: {}", e);
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(1000));
        
        // Prime the device format and framerate using GStreamer.
        // This runs BEFORE scrcpy starts, so it won't cause VIDIOC_G_FMT conflicts.
        eprintln!("[V4L2] Setting format (YUY2 1280x720 @ 30fps) using GStreamer...");
        let fps_output = Command::new("gst-launch-1.0")
            .args([
                "videotestsrc", "is-live=true", "pattern=black", "num-buffers=30",
                "!", "video/x-raw,format=YUY2,width=1280,height=720,framerate=30/1",
                "!", "v4l2sink", &format!("device={}", device_path)
            ])
            .output();
        match fps_output {
            Ok(out) => {
                if !out.status.success() {
                    eprintln!("[V4L2] GStreamer failed to set format.");
                    eprintln!("[V4L2] stderr: {}", String::from_utf8_lossy(&out.stderr));
                } else {
                    eprintln!("[V4L2] Format and framerate set successfully via GStreamer.");
                }
            }
            Err(e) => {
                eprintln!("[V4L2] Failed to execute gst-launch-1.0: {}.", e);
            }
        }
    }

    (device_path.to_string(), device_path.to_string())
}

pub fn try_connect_device(
    store: &DeviceStore,
    pipelines: &PipelineStore,
    device_id: u32
) -> (bool, String) {
    let (device_name, address) = {
        let devices = store.borrow();
        match devices.iter().find(|d| d.id == device_id) {
            Some(d) => (d.name.clone(), format!("{}:{}", d.address, d.port)),
            None => return (false, String::new()),
        }
    };

    if address.is_empty() {
        return (false, device_name);
    }

    eprintln!("[Connect] trying to connect device {device_id} at {address}");
    let _ = Command::new("adb").args(["connect", &address]).output();
    std::thread::sleep(std::time::Duration::from_millis(500));

    let verify = Command::new("adb").args(["devices"]).output();
    let is_connected = match verify {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.lines().any(|line| line.starts_with(&address) && line.contains("device"))
        }
        Err(_) => false,
    };

    if is_connected {
        if let Some(d) = store.borrow_mut().iter_mut().find(|d| d.id == device_id) {
            d.connected = true;
        }
        save_devices(&store.borrow());
        
        let camera_id = {
            let devices = store.borrow();
            devices.iter().find(|d| d.id == device_id)
                .map(|d| camera_id_for_selection(&d.camera_selection))
                .unwrap_or(0)
        };
        spawn_pipeline(pipelines, device_id, camera_id);
    }

    (is_connected, device_name)
}

pub fn try_disconnect_device(
    store: &DeviceStore,
    pipelines: &PipelineStore,
    device_id: u32
) -> (bool, String) {
    let (device_name, address) = {
        let devices = store.borrow();
        match devices.iter().find(|d| d.id == device_id) {
            Some(d) => (d.name.clone(), format!("{}:{}", d.address, d.port)),
            None => return (false, String::new()),
        }
    };

    if address.is_empty() {
        return (false, device_name);
    }

    let _ = Command::new("adb").args(["disconnect", &address]).output();
    let verify = Command::new("adb").args(["devices"]).output();
    let is_still_connected = match verify {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.lines().any(|line| line.starts_with(&address) && line.contains("device"))
        }
        Err(_) => false,
    };

    let success = !is_still_connected;
    if success {
        if let Some(d) = store.borrow_mut().iter_mut().find(|d| d.id == device_id) {
            d.connected = false;
        }
        save_devices(&store.borrow());
        despawn_pipeline(pipelines, device_id);
    }

    (success, device_name)
}

pub fn spawn_pipeline(pipelines: &PipelineStore, device_id: u32, camera_id: u32) {
    if pipelines.borrow().contains_key(&device_id) {
        return;
    }

    let (input_device, output_device) = loopback_paths(device_id);
    match VideoPipeline::new(&input_device, &output_device, camera_id) {
        Ok(p) => {
            if let Err(e) = p.start() {
                eprintln!("[Video] pipeline start failed for device {device_id}: {e}");
                return;
            }
            pipelines.borrow_mut().insert(device_id, Rc::new(p));
        }
        Err(e) => eprintln!("[Video] pipeline init failed for device {device_id}: {e}"),
    }
}

pub fn despawn_pipeline(pipelines: &PipelineStore, device_id: u32) {
    pipelines.borrow_mut().remove(&device_id);
}

pub fn update_camera_selection(
    store: &DeviceStore,
    pipelines: &PipelineStore,
    device_id: u32,
    selection: &str
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
    #[test]
    fn loopback_modprobe_args_are_clean_and_chrome_compatible() {
        let args = super::loopback_modprobe_args();
        assert!(args.iter().all(|arg| !arg.ends_with(' ')));
        assert!(args.iter().any(|arg| arg == "exclusive_caps=1"));
        assert!(args.iter().any(|arg| arg == "keep_format=1"));
        assert!(args.iter().any(|arg| arg == "sustain_framerate=1"));
    }
}

pub fn transform_callbacks_for(
    pipelines: &PipelineStore,
    device_id: u32
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