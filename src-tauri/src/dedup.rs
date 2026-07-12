use crate::device::{Device, DevType};
use std::collections::HashSet;

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

pub fn try_insert(
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
    if dedup && !has_conn_type && devices.iter().any(|d| {
        let ecn = core_name(&d.name);
        (ecn == cn || d.name == cn) && (d.is_bluetooth || d.is_wireless_24g)
    }) {
        return; // Skip: a device with connection type already exists
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
