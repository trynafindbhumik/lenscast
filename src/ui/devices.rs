use std::cell::RefCell;
use std::rc::Rc;
use std::sync::atomic::{AtomicU32, Ordering};

static NEXT_ID: AtomicU32 = AtomicU32::new(1);

//  Data types

#[derive(Clone, Debug)]
pub struct Device {
    pub id: u32,
    pub name: String,
}

/// Shared, cloneable handle to the device list.
pub type DeviceStore = Rc<RefCell<Vec<Device>>>;

pub fn new_device_store() -> DeviceStore {
    Rc::new(RefCell::new(Vec::new()))
}

pub fn next_device_id() -> u32 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}