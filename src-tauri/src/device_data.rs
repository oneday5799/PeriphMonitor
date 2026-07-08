use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::OnceLock;

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub device_type: String,
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
    eprintln!("[device_data] Loading from: {:?}", path);
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(val) => {
                    let mut data: HashMap<String, HashMap<String, DeviceInfo>> = HashMap::new();
                    if let Some(obj) = val.as_object() {
                        for (vid, pids_val) in obj {
                            if let Some(pids_obj) = pids_val.as_object() {
                                let mut pids_map = HashMap::new();
                                for (pid, info_val) in pids_obj {
                                    let info = if let Some(info_obj) = info_val.as_object() {
                                        DeviceInfo {
                                            name: info_obj.get("name").and_then(|v| v.as_str()).unwrap_or("Unknown").to_string(),
                                            device_type: info_obj.get("type").and_then(|v| v.as_str()).unwrap_or("other").to_string(),
                                        }
                                    } else if let Some(name_str) = info_val.as_str() {
                                        // Backward compat: old format with just name string
                                        DeviceInfo {
                                            name: name_str.to_string(),
                                            device_type: "other".to_string(),
                                        }
                                    } else {
                                        DeviceInfo {
                                            name: "Unknown".to_string(),
                                            device_type: "other".to_string(),
                                        }
                                    };
                                    pids_map.insert(pid.clone(), info);
                                }
                                data.insert(vid.clone(), pids_map);
                            }
                        }
                    }
                    eprintln!("[device_data] Loaded {} vendors", data.len());
                    for (vid, pids) in &data {
                        eprintln!("[device_data]   VID {}: {} PIDs", vid, pids.len());
                    }
                    DEVICE_DATA.set(data).ok();
                }
                Err(e) => {
                    eprintln!("[device_data] JSON parse error: {}", e);
                    DEVICE_DATA.set(HashMap::new()).ok();
                }
            }
        }
        Err(e) => {
            eprintln!("[device_data] File read error: {}", e);
            DEVICE_DATA.set(HashMap::new()).ok();
        }
    }
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
