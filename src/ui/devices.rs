use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub address: String,
    pub port: u16,
    #[serde(default)]
    pub connected: bool,
}

/// Shared, cloneable handle to the device list.
pub type DeviceStore = Rc<RefCell<Vec<Device>>>;

fn get_storage_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lenscast");
    fs::create_dir_all(&config_dir).ok();
    config_dir.join("devices.json")
}

/// Loads devices from disk.
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

/// Saves devices to disk.
pub fn save_devices(devices: &[Device]) {
    let path = get_storage_path();
    if let Ok(data) = serde_json::to_string_pretty(devices) {
        let _ = fs::write(path, data);
    }
}

pub fn new_device_store() -> DeviceStore {
    Rc::new(RefCell::new(load_devices()))
}

pub fn next_device_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Returns list of connected device addresses from adb.
pub fn get_connected_adb_devices() -> Vec<String> {
    let output = Command::new("adb")
        .args(["devices", "-l"])
        .output();

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

/// Updates connected status of all devices based on actual adb state.
/// Returns true if any device's status changed.
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