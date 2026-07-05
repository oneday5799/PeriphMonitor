use std::collections::HashMap;
use serde::Deserialize;
use regex::Regex;
use wmi::WMIConnection;
use windows::Devices::Bluetooth::{BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;

use crate::device::{Device, DevType};
use crate::config;

#[derive(Deserialize, Debug)]
#[serde(rename = "Win32_Battery")]
struct BatteryDevice {
    name: Option<String>,
    status: Option<String>,
    estimated_charge_remaining: Option<i32>,
}

#[derive(Deserialize, Debug)]
#[serde(rename = "Win32_DesktopMonitor")]
struct MonitorDevice {
    name: Option<String>,
    status: Option<String>,
}

pub fn query_devices() -> Vec<Device> {
    let mut all = vec![];
    let mut seen = std::collections::HashSet::new();

    let log = |msg: &str| {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .create(true).append(true)
            .open("debug.log").unwrap();
        writeln!(f, "{}", msg).unwrap();
    };
    log("query_devices called");

    // Read filter state once at the start
    let filter_enabled = config::with_config(|c| c.filter_enabled);
    log(&format!("Filter enabled: {}", filter_enabled));

    // COM is already initialized in main() with COINIT_APARTMENTTHREADED.
    // Use assume_initialized() to avoid RPC_E_CHANGED_MODE from re-initializing.
    let com = unsafe { wmi::COMLibrary::assume_initialized() };
    log("COMLibrary assume_initialized OK");

    let con = match WMIConnection::new(com) {
        Ok(c) => {
            log("WMIConnection OK");
            c
        }
        Err(e) => {
            log(&format!("WMIConnection failed: {:?}", e));
            return all;
        }
    };
    log("Starting PnPEntity query...");

    // Main PnPEntity query
    if let Ok(rows) = con.raw_query::<HashMap<String, wmi::Variant>>(
        "SELECT Name, Status, PNPDeviceID, Caption, PNPClass, ConfigManagerErrorCode FROM Win32_PnPEntity",
    ) {
        for row in rows {
            let n = match row.get("Name") {
                Some(wmi::Variant::String(s)) => s.clone(),
                _ => continue,
            };
            let devid = match row.get("PNPDeviceID") {
                Some(wmi::Variant::String(s)) => s.clone(),
                _ => String::new(),
            };
            let cap = match row.get("Caption") {
                Some(wmi::Variant::String(s)) => s.clone(),
                _ => String::new(),
            };
            let pnp = match row.get("PNPClass") {
                Some(wmi::Variant::String(s)) => s.clone(),
                _ => String::new(),
            };
            let status_str = match row.get("Status") {
                Some(wmi::Variant::String(s)) => s.clone(),
                _ => String::new(),
            };
            let u = devid.to_uppercase();

            let err_val = row.get("ConfigManagerErrorCode").and_then(|v| match v {
                wmi::Variant::I2(v) => Some(*v as i64),
                wmi::Variant::I4(v) => Some(*v as i64),
                wmi::Variant::UI2(v) => Some(*v as i64),
                wmi::Variant::UI4(v) => Some(*v as i64),
                wmi::Variant::String(s) => s.parse::<i64>().ok(),
                wmi::Variant::Bool(v) => Some(if *v { 0 } else { 1 }),
                _ => None,
            });
            let connected = match err_val {
                Some(code) => code == 0,
                None => status_str == "OK",
            };
            let s = if connected {
                "已连接".to_string()
            } else {
                "已配对".to_string()
            };
            if n.is_empty() {
                continue;
            }
            let cn = core_name(&n);
            let dt: DevType;
            if pnp.eq_ignore_ascii_case("Audio") || pnp.eq_ignore_ascii_case("AudioEndpoint") {
                dt = DevType::Audio;
            } else if pnp.eq_ignore_ascii_case("Keyboard") || pnp.eq_ignore_ascii_case("Mouse") {
                dt = DevType::Usb;
            } else if pnp.eq_ignore_ascii_case("Bluetooth")
                || u.starts_with("BTHENUM\\")
                || u.starts_with("SWD\\")
            {
                let t = format!("{} {}", n, cap).to_lowercase();
                if is_audio(&t) {
                    dt = DevType::Audio;
                } else if is_usb(&n, &cap) {
                    dt = DevType::Usb;
                } else if is_bt_peripheral(&n) {
                    dt = DevType::Bluetooth;
                } else {
                    dt = DevType::Other;
                }
            } else if u.starts_with("USB\\") && is_usb(&n, &cap) {
                dt = DevType::Usb;
            } else {
                dt = DevType::Other;
            }
            if !seen.insert(cn.clone()) {
                if let Some(existing) = all.iter_mut().find(|d| core_name(&d.name) == cn) {
                    if n.len() < existing.name.len() {
                        existing.name = n;
                        existing.status = s;
                    }
                }
                continue;
            }
            all.push(Device {
                name: n,
                dt,
                status: s,
                battery: None,
            });
        }
    }

    // Paired Bluetooth devices via Windows Runtime API
    if let Ok(btc_devices) = find_paired_bluetooth_devices() {
        for (name, connected) in btc_devices {
            if name.is_empty() {
                continue;
            }
            let cn = core_name(&name);
            let t = name.to_lowercase();
            let dt: DevType;
            if is_audio(&t) {
                dt = DevType::Audio;
            } else if is_usb(&name, "") {
                dt = DevType::Usb;
            } else if is_bt_peripheral(&name) {
                dt = DevType::Bluetooth;
            } else {
                continue;
            }
            let s = if connected {
                "已连接".to_string()
            } else {
                "已配对".to_string()
            };
            if !seen.insert(cn.clone()) {
                if let Some(existing) = all.iter_mut().find(|d| core_name(&d.name) == cn) {
                    if name.len() < existing.name.len() {
                        existing.name = name;
                        existing.status = s;
                    }
                }
                continue;
            }
            all.push(Device {
                name,
                dt,
                status: s,
                battery: None,
            });
        }
    }

    // Battery
    if let Ok(r) = con.query::<BatteryDevice>() {
        for d in r {
            let (n, s) = (
                d.name.unwrap_or_default(),
                d.status.unwrap_or_default(),
            );
            if !n.is_empty() && seen.insert(core_name(&n)) {
                all.push(Device {
                    name: n,
                    dt: DevType::Battery,
                    status: s,
                    battery: d.estimated_charge_remaining,
                });
            }
        }
    }

    // Monitor
    if let Ok(r) = con.query::<MonitorDevice>() {
        for d in r {
            let (n, s) = (
                d.name.unwrap_or_default(),
                d.status.unwrap_or_default(),
            );
            if n.is_empty() || s != "OK" {
                continue;
            }
            if seen.insert(core_name(&n)) {
                all.push(Device {
                    name: n,
                    dt: DevType::Monitor,
                    status: s,
                    battery: None,
                });
            }
        }
    }

    // Remove Bluetooth devices that already appear as Audio or Input
    let audio_input_names: std::collections::HashSet<String> = all
        .iter()
        .filter(|d| d.dt == DevType::Audio || d.dt == DevType::Usb)
        .map(|d| core_name(&d.name))
        .collect();
    all.retain(|d| d.dt != DevType::Bluetooth || !audio_input_names.contains(&core_name(&d.name)));

    // Apply user-defined regex filter
    let filter_regex_str = config::with_config(|c| c.filter_regex.clone());
    log(&format!("Filter regex: {}", filter_regex_str));
    if filter_enabled && !filter_regex_str.is_empty() {
        if let Ok(re) = Regex::new(&format!("(?i)({})", filter_regex_str)) {
            let before = all.len();
            all.retain(|d| !re.is_match(&d.name));
            log(&format!("Regex filtered {} -> {} devices", before, all.len()));
        }
    }

    all
}

pub fn core_name(n: &str) -> String {
    let inner = if let Some(i) = n.find(" (") {
        if let Some(j) = n.rfind(')') {
            if j > i + 2 {
                n[i + 2..j].to_string()
            } else {
                n.to_string()
            }
        } else {
            n.to_string()
        }
    } else {
        n.to_string()
    };
    for suffix in &[
        " Hands-Free AG",
        " Hands-Free",
        " Handsfree",
        " Stereo",
        " LE",
        " Low Energy",
        " Audio",
        " HFP",
        " AG",
        " Avrcp 传输",
        " 音频网关服务",
    ] {
        if let Some(pos) = inner.strip_suffix(suffix) {
            return pos.to_string();
        }
    }
    inner
}

fn is_audio(n: &str) -> bool {
    let l = n.to_lowercase();
    [
        "headphone", "headset", "earphone", "earbuds", "speaker", "耳机", "音箱", "扬声器",
        "音响", "airpods", "hifi", "dac", "amp", "glasses", "眼镜",
    ]
    .iter()
    .any(|k| l.contains(k))
}

fn is_usb(n: &str, c: &str) -> bool {
    let t = format!("{} {}", n, c).to_lowercase();
    [
        "mouse", "keyboard", "controller", "gamepad", "鼠标", "键盘", "手柄", "xbox", "webcam",
        "logitech", "razer", "corsair", "keychron", "orochi", "deathadder", "viper",
        "gpro", "g pro", "basilisk", "naga", "blackwidow", "hunters", "kaira",
        "steelseries", "hyperx", "coolermaster", "roccat", "zte", "雷蛇", "罗技",
    ]
    .iter()
    .any(|k| t.contains(k))
}

fn is_bt_peripheral(n: &str) -> bool {
    let l = n.to_lowercase();
    [
        "orochi", "rk-100", "rk100", "redragon", "havit", "jbl", "jabra", "sony",
        "bose", "sennheiser", "beats", "samsung", "oppo", "vivo", "huawei", "xiaomi",
        "redmi", "realme", "oneplus", "apple", "airpods", "buds", "galaxy",
        "oculus", "quest", "ps5", "dualsense", "dualshock", "switch", "pro controller",
        "8bitdo", "gameSir", "flydigi", "盖世小鸡", "飞智",
    ]
    .iter()
    .any(|k| l.contains(k))
}

fn find_paired_bluetooth_devices() -> Result<Vec<(String, bool)>, Box<dyn std::error::Error>> {
    use windows::Devices::Bluetooth::BluetoothConnectionStatus;

    let mut result = Vec::new();

    // Find paired Classic Bluetooth devices
    let btc_selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true)?;
    let btc_op = DeviceInformation::FindAllAsyncAqsFilter(&btc_selector)?;
    let btc_devices_info = btc_op.get()?;
    for device_info in btc_devices_info.into_iter() {
        if let Ok(device_id) = device_info.Id() {
            if let Ok(future) = BluetoothDevice::FromIdAsync(&device_id) {
                if let Ok(device) = future.get() {
                    let name = device.Name()?.to_string();
                    let connected = device.ConnectionStatus()? == BluetoothConnectionStatus::Connected;
                    result.push((name, connected));
                }
            }
        }
    }

    // Find paired BLE devices
    let ble_selector = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)?;
    let ble_op = DeviceInformation::FindAllAsyncAqsFilter(&ble_selector)?;
    let ble_devices_info = ble_op.get()?;
    for device_info in ble_devices_info.into_iter() {
        if let Ok(device_id) = device_info.Id() {
            if let Ok(future) = BluetoothLEDevice::FromIdAsync(&device_id) {
                if let Ok(device) = future.get() {
                    let name = device.Name()?.to_string();
                    let connected = device.ConnectionStatus()? == BluetoothConnectionStatus::Connected;
                    result.push((name, connected));
                }
            }
        }
    }

    Ok(result)
}
