use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

use serde::Deserialize;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub device_type: String,
}

#[derive(Deserialize)]
struct RawDeviceEntry {
    name: String,
    #[serde(default)]
    r#type: String,
}

static DEVICE_DATA: OnceLock<HashMap<String, HashMap<String, DeviceInfo>>> = OnceLock::new();

fn data_file_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent()
        .unwrap_or(&PathBuf::from("."))
        .join("data")
        .join("wireless_24g_devices.json")
}

pub fn init_device_data() {
    let path = data_file_path();
    let data = match std::fs::read_to_string(&path) {
        Ok(content) => {
            match serde_json::from_str::<HashMap<String, HashMap<String, RawDeviceEntry>>>(&content) {
                Ok(raw) => {
                    let mut result = HashMap::new();
                    for (vid, pids) in raw {
                        let mut pids_map = HashMap::new();
                        for (pid, entry) in pids {
                            pids_map.insert(pid, DeviceInfo {
                                name: entry.name,
                                device_type: if entry.r#type.is_empty() { "other".to_string() } else { entry.r#type },
                            });
                        }
                        result.insert(vid, pids_map);
                    }
                    result
                }
                Err(_) => HashMap::new(),
            }
        }
        Err(_) => HashMap::new(),
    };
    DEVICE_DATA.set(data).ok();
}

pub fn is_wireless_24g(vid: &str, pid: &str) -> bool {
    DEVICE_DATA
        .get()
        .and_then(|data| data.get(vid))
        .map(|pids| pids.contains_key(pid))
        .unwrap_or(false)
}

pub fn get_device_name(vid: &str, pid: &str) -> Option<String> {
    DEVICE_DATA
        .get()
        .and_then(|data| data.get(vid))
        .and_then(|pids| pids.get(pid))
        .map(|info| info.name.clone())
}

pub fn get_device_type(vid: &str, pid: &str) -> String {
    DEVICE_DATA
        .get()
        .and_then(|data| data.get(vid))
        .and_then(|pids| pids.get(pid))
        .map(|info| info.device_type.clone())
        .unwrap_or_else(|| "other".to_string())
}

pub fn extract_vid_pid(pnp_id: &str) -> Option<(String, String)> {
    let upper = pnp_id.to_uppercase();
    let vid = match upper.find("VID_") {
        Some(pos) => {
            let start = pos + 4;
            if start + 4 <= upper.len() {
                upper[start..start + 4].to_string()
            } else {
                return None;
            }
        }
        None => return None,
    };
    let pid = match upper.find("PID_") {
        Some(pos) => {
            let start = pos + 4;
            if start + 4 <= upper.len() {
                upper[start..start + 4].to_string()
            } else {
                return None;
            }
        }
        None => return None,
    };
    Some((vid, pid))
}
