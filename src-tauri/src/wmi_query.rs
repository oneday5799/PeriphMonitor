use std::collections::{HashMap, HashSet};
use serde::Deserialize;
use regex::Regex;
use wmi::WMIConnection;

use crate::device::{Device, DevType};
use crate::config;
use crate::classify::{classify_device, classify_bluetooth, is_wireless_24g_by_vid_pid, is_bt_service, is_generic_hid, is_system_device};
use crate::device_data;
use crate::bluetooth::find_paired_bluetooth_devices;

#[derive(Deserialize, Debug)]
#[serde(rename = "Win32_Battery")]
struct BatteryDevice {
    name: Option<String>,
    status: Option<String>,
    estimated_charge_remaining: Option<i32>,
}

fn try_insert(
    name: &str,
    display_name: Option<&str>,
    dt: DevType,
    status: &str,
    battery: Option<i32>,
    device_id: Option<String>,
    is_bluetooth: bool,
    is_wireless_24g: bool,
    dedup: bool,
    seen: &mut HashSet<String>,
    devices: &mut Vec<Device>,
) {
    let effective_name = display_name.unwrap_or(name);
    let cn = if display_name.is_some() {
        effective_name.to_string()
    } else {
        core_name(name)
    };
    let has_conn_type = is_bluetooth || is_wireless_24g;

    // If this device has no connection type, check if one with a type already exists
    if dedup && !has_conn_type {
        if devices.iter().any(|d| {
            let ecn = core_name(&d.name);
            (ecn == cn || d.name == cn) && (d.is_bluetooth || d.is_wireless_24g)
        }) {
            return; // Skip: a device with connection type already exists
        }
    }

    // If this device has a connection type, remove any plain (no-type) entry with same name
    if dedup && has_conn_type {
        if let Some(pos) = devices.iter().position(|d| {
            let ecn = core_name(&d.name);
            (ecn == cn || d.name == cn) && !d.is_bluetooth && !d.is_wireless_24g
        }) {
            devices.remove(pos);
        }
    }

    // Normal dedup: same name + same connection type
    let conn_tag = if is_bluetooth { "bt" } else if is_wireless_24g { "24g" } else { "usb" };
    let dedup_key = format!("{}:{}", cn, conn_tag);
    if dedup && !seen.insert(dedup_key) {
        if let Some(existing) = devices.iter_mut().find(|d| {
            let ecn = core_name(&d.name);
            let econn = if d.is_bluetooth { "bt" } else if d.is_wireless_24g { "24g" } else { "usb" };
            (ecn == cn || d.name == cn) && econn == conn_tag
        }) {
            if name.len() < existing.name.len() {
                existing.name = effective_name.to_string();
                existing.status = status.to_string();
                if existing.device_id.is_none() {
                    existing.device_id = device_id;
                }
                existing.is_bluetooth = existing.is_bluetooth || is_bluetooth;
                existing.is_wireless_24g = existing.is_wireless_24g || is_wireless_24g;
            }
        }
        return;
    }
    devices.push(Device {
        name: effective_name.to_string(),
        dt,
        status: status.to_string(),
        battery,
        device_id,
        is_bluetooth,
        is_wireless_24g,
    });
}

pub fn query_devices() -> Vec<Device> {
    let mut all = vec![];
    let mut seen = HashSet::new();

    let filter_enabled = config::with_config(|c| c.filter_enabled);
    let dedup_enabled = config::with_config(|c| c.dedup_devices);

    let com = unsafe { wmi::COMLibrary::assume_initialized() };

    let con = match WMIConnection::new(com) {
        Ok(c) => c,
        Err(_) => return all,
    };

    // Main PnPEntity query - whitelist only relevant device classes
    const PNPCLASS_WHITELIST: &[&str] = &["AudioEndpoint", "Bluetooth", "HIDClass", "Keyboard", "MEDIA", "Mouse", "Monitor"];
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

            // Client-side whitelist filter (WMI WHERE clause unreliable for PNPClass)
            if !PNPCLASS_WHITELIST.iter().any(|c| pnp.eq_ignore_ascii_case(c)) {
                continue;
            }

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
            let s = if connected { "已连接" } else { "已配对" };
            if n.is_empty() { continue; }

            // Skip Bluetooth service/profile entries (GAP, GATT, AVRCP, etc.)
            if pnp.eq_ignore_ascii_case("Bluetooth") && is_bt_service(&devid) {
                continue;
            }

            // Skip generic HID device names (multiple interfaces of same physical device)
            if pnp.eq_ignore_ascii_case("HIDClass") && is_generic_hid(&devid) {
                continue;
            }

            // Skip system devices (BT enumerators, adapters, virtual HID)
            if is_system_device(&devid) {
                continue;
            }

            let dt = classify_device(&n, &pnp, &u, &cap);
            let is_24g = is_wireless_24g_by_vid_pid(&u);
            let display_name = if is_24g {
                device_data::extract_vid_pid(&u)
                    .and_then(|(vid, pid)| device_data::get_device_name(&vid, &pid))
            } else {
                None
            };
            try_insert(&n, display_name.as_deref(), dt, s, None, None, false, is_24g, dedup_enabled, &mut seen, &mut all);
        }
    }

    // Paired Bluetooth devices via Windows Runtime API
    let mut bt_names: HashSet<String> = HashSet::new();
    if let Ok(btc_devices) = find_paired_bluetooth_devices() {
        for (name, connected, battery, device_id) in btc_devices {
            if name.is_empty() { continue; }
            let dt = match classify_bluetooth(&name) {
                Some(dt) => dt,
                None => continue,
            };
            let s = if connected { "已连接" } else { "已配对" };
            let cn = core_name(&name);
            bt_names.insert(cn.clone());
            if let Some(existing) = all.iter_mut().find(|d| core_name(&d.name) == cn && d.is_bluetooth) {
                existing.status = s.to_string();
                if battery.is_some() {
                    existing.battery = battery.map(|b| b as i32);
                }
                if existing.device_id.is_none() {
                    existing.device_id = Some(device_id);
                }
            } else {
                try_insert(&name, None, dt, s, battery.map(|b| b as i32), Some(device_id), true, false, dedup_enabled, &mut seen, &mut all);
            }
        }
    }

    // Battery
    if let Ok(r) = con.query::<BatteryDevice>() {
        for d in r {
            let (n, s) = (
                d.name.unwrap_or_default(),
                d.status.unwrap_or_default(),
            );
            if !n.is_empty() && (!dedup_enabled || seen.insert(format!("{}:usb", core_name(&n)))) {
                all.push(Device {
                    name: n,
                    dt: DevType::Battery,
                    status: s,
                    battery: d.estimated_charge_remaining,
                    device_id: None,
                    is_bluetooth: false,
                    is_wireless_24g: false,
                });
            }
        }
    }

    // Apply user-defined regex filter
    if filter_enabled {
        let filter_regex_str = config::with_config(|c| c.filter_regex.clone());
        if !filter_regex_str.is_empty() {
            if let Ok(re) = Regex::new(&format!("(?i)({})", filter_regex_str)) {
                all.retain(|d| !re.is_match(&d.name));
            }
        }
    }

    // Temporarily hide status for devices not detected by WinRT Bluetooth API
    for d in &mut all {
        if d.is_bluetooth && !bt_names.contains(&core_name(&d.name)) {
            d.status.clear();
        }
    }

    all
}

fn core_name(n: &str) -> String {
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
        " Hands-Free HF",
        " Hands-Free",
        " Handsfree",
        " A2DP SNK",
        " A2DP SRC",
        " Stereo",
        " LE",
        " Low Energy",
        " Audio",
        " HFP",
        " AG",
        " SNK",
        " SRC",
        " Avrcp 传输",
        " 音频网关服务",
    ] {
        if let Some(pos) = inner.strip_suffix(suffix) {
            return pos.to_string();
        }
    }
    inner
}
