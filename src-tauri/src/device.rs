use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum DevType {
    Audio,
    Battery,
    Bluetooth,
    Monitor,
    Other,
    Usb,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub name: String,
    pub dt: DevType,
    pub status: String,
    pub battery: Option<i32>,
}
