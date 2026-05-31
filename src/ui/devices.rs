use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

//  Data types

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Device {
    pub id: u32,
    pub name: String,
    pub address: String,
    pub port: u16,
}

/// Shared, cloneable handle to the device list.
pub type DeviceStore = Rc<RefCell<Vec<Device>>>;

/// Returns the path to the device storage file.
fn get_storage_path() -> PathBuf {
    let config_dir = dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("lenscast");
    fs::create_dir_all(&config_dir).ok();
    config_dir.join("devices.json")
}

/// Load devices from disk.
pub fn load_devices() -> Vec<Device> {
    let path = get_storage_path();
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(devices) = serde_json::from_str::<Vec<Device>>(&data) {
            // Update NEXT_ID to avoid collisions
            let max_id = devices.iter().map(|d| d.id).max().unwrap_or(0);
            NEXT_ID.store(max_id + 1, Ordering::Relaxed);
            return devices;
        }
    }
    Vec::new()
}

/// Save devices to disk.
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