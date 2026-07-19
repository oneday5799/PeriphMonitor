use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, RwLock};
use std::time::SystemTime;

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

static DEVICE_DATA: OnceLock<RwLock<HashMap<String, HashMap<String, DeviceInfo>>>> = OnceLock::new();
static LAST_MTIME: OnceLock<Mutex<Option<SystemTime>>> = OnceLock::new();

fn data_file_path() -> std::path::PathBuf {
    crate::process::exe_dir().join("data").join("wireless_24g_devices.json")
}

fn load_data_from_file() -> HashMap<String, HashMap<String, DeviceInfo>> {
    let path = data_file_path();
    match std::fs::read_to_string(&path) {
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
                    crate::process::append_log_detailed(&format!("[device_data] loaded {} VIDs", result.len()));
                    result
                }
                Err(e) => {
                    crate::process::append_log(&format!("[device_data] JSON parse error: {}", e));
                    HashMap::new()
                }
            }
        }
        Err(e) => {
            crate::process::append_log_detailed(&format!("[device_data] file not found: {}", e));
            HashMap::new()
        }
    }
}

pub fn init_device_data() {
    let data = load_data_from_file();
    DEVICE_DATA.set(RwLock::new(data)).ok();
}

pub fn reload_device_data() {
    let path = data_file_path();
    let current_mtime = std::fs::metadata(&path)
        .and_then(|m| m.modified())
        .ok();

    let last = LAST_MTIME.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = last.lock() {
        if *guard == current_mtime {
            return;
        }
        *guard = current_mtime;
    }

    if let Some(rw_lock) = DEVICE_DATA.get() {
        let new_data = load_data_from_file();
        if let Ok(mut data) = rw_lock.write() {
            *data = new_data;
        }
    }
}

pub fn is_wireless_24g(vid: &str, pid: &str) -> bool {
    let data = DEVICE_DATA.get().and_then(|rw_lock| rw_lock.read().ok());
    data.as_ref()
        .and_then(|d| d.get(vid))
        .map(|pids| pids.contains_key(pid))
        .unwrap_or(false)
}

pub fn get_device_name(vid: &str, pid: &str) -> Option<String> {
    let data = DEVICE_DATA.get().and_then(|rw_lock| rw_lock.read().ok());
    data.as_ref()
        .and_then(|d| d.get(vid))
        .and_then(|pids| pids.get(pid))
        .map(|info| info.name.clone())
}

pub fn get_device_type(vid: &str, pid: &str) -> String {
    let data = DEVICE_DATA.get().and_then(|rw_lock| rw_lock.read().ok());
    data.as_ref()
        .and_then(|d| d.get(vid))
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
