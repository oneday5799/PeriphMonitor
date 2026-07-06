use std::collections::{HashMap, HashSet};
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

fn classify_device(name: &str, pnp_class: &str, pnp_id: &str, caption: &str) -> DevType {
    let lower_combined = format!("{} {}", name, caption).to_lowercase();

    if pnp_class.eq_ignore_ascii_case("Audio") || pnp_class.eq_ignore_ascii_case("AudioEndpoint") {
        return DevType::Audio;
    }
    if pnp_class.eq_ignore_ascii_case("Keyboard") || pnp_class.eq_ignore_ascii_case("Mouse") {
        return DevType::Usb;
    }
    if pnp_class.eq_ignore_ascii_case("Bluetooth")
        || pnp_id.starts_with("BTHENUM\\")
        || pnp_id.starts_with("SWD\\")
    {
        if is_audio(&lower_combined) { return DevType::Audio; }
        if is_usb(name, caption) { return DevType::Usb; }
        if is_bt_peripheral(name) { return DevType::Bluetooth; }
        return DevType::Other;
    }
    if pnp_id.starts_with("USB\\") && is_usb(name, caption) {
        return DevType::Usb;
    }
    DevType::Other
}

fn classify_bluetooth(name: &str) -> Option<DevType> {
    let lower = name.to_lowercase();
    if is_audio(&lower) { return Some(DevType::Audio); }
    if is_usb(name, "") { return Some(DevType::Usb); }
    if is_bt_peripheral(name) { return Some(DevType::Bluetooth); }
    None
}

fn try_insert(
    name: &str,
    dt: DevType,
    status: &str,
    battery: Option<i32>,
    seen: &mut HashSet<String>,
    devices: &mut Vec<Device>,
) {
    let cn = core_name(name);
    if !seen.insert(cn.clone()) {
        if let Some(existing) = devices.iter_mut().find(|d| core_name(&d.name) == cn) {
            if name.len() < existing.name.len() {
                existing.name = name.to_string();
                existing.status = status.to_string();
            }
        }
        return;
    }
    devices.push(Device {
        name: name.to_string(),
        dt,
        status: status.to_string(),
        battery,
    });
}

pub fn query_devices() -> Vec<Device> {
    let mut all = vec![];
    let mut seen = HashSet::new();

    let filter_enabled = config::with_config(|c| c.filter_enabled);

    let com = unsafe { wmi::COMLibrary::assume_initialized() };

    let con = match WMIConnection::new(com) {
        Ok(c) => c,
        Err(_) => return all,
    };

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
            let s = if connected { "已连接" } else { "已配对" };
            if n.is_empty() { continue; }

            let dt = classify_device(&n, &pnp, &u, &cap);
            try_insert(&n, dt, s, None, &mut seen, &mut all);
        }
    }

    // Paired Bluetooth devices via Windows Runtime API
    // This pass provides accurate ConnectionStatus, so override WMI-inferred statuses
    let mut bt_names: HashSet<String> = HashSet::new();
    if let Ok(btc_devices) = find_paired_bluetooth_devices() {
        for (name, connected, battery) in btc_devices {
            if name.is_empty() { continue; }
            let dt = match classify_bluetooth(&name) {
                Some(dt) => dt,
                None => continue,
            };
            let s = if connected { "已连接" } else { "已配对" };
            let cn = core_name(&name);
            bt_names.insert(cn.clone());
            if let Some(existing) = all.iter_mut().find(|d| core_name(&d.name) == cn) {
                existing.status = s.to_string();
                if battery.is_some() {
                    existing.battery = battery.map(|b| b as i32);
                }
            } else {
                try_insert(&name, dt, s, battery.map(|b| b as i32), &mut seen, &mut all);
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
            if n.is_empty() || s != "OK" { continue; }
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

    // Remove Bluetooth devices that already appear as Audio or USB
    let audio_input_names: HashSet<String> = all
        .iter()
        .filter(|d| d.dt == DevType::Audio || d.dt == DevType::Usb)
        .map(|d| core_name(&d.name))
        .collect();
    all.retain(|d| d.dt != DevType::Bluetooth || !audio_input_names.contains(&core_name(&d.name)));

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
        if !bt_names.contains(&core_name(&d.name)) {
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

fn find_paired_bluetooth_devices() -> Result<Vec<(String, bool, Option<u8>)>, Box<dyn std::error::Error>> {
    use windows::Devices::Bluetooth::BluetoothConnectionStatus;

    let mut result = Vec::new();

    let btc_selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true)?;
    let btc_op = DeviceInformation::FindAllAsyncAqsFilter(&btc_selector)?;
    let btc_devices_info = btc_op.get()?;
    for device_info in btc_devices_info.into_iter() {
        if let Ok(device_id) = device_info.Id() {
            if let Ok(future) = BluetoothDevice::FromIdAsync(&device_id) {
                if let Ok(device) = future.get() {
                        let name = device.Name()?.to_string();
                        let connected = device.ConnectionStatus()? == BluetoothConnectionStatus::Connected;
                        let battery = read_btc_battery_from_device_id(&device_id.to_string());
                        result.push((name, connected, battery));
                    }
                }
            }
        }

    let ble_selector = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)?;
    let ble_op = DeviceInformation::FindAllAsyncAqsFilter(&ble_selector)?;
    let ble_devices_info = ble_op.get()?;
    for device_info in ble_devices_info.into_iter() {
        if let Ok(device_id) = device_info.Id() {
            if let Ok(future) = BluetoothLEDevice::FromIdAsync(&device_id) {
                if let Ok(device) = future.get() {
                        let name = device.Name()?.to_string();
                        let connected = device.ConnectionStatus()? == BluetoothConnectionStatus::Connected;
                        let battery = read_ble_battery(&device);
                        result.push((name, connected, battery));
                    }
                }
            }
        }

    Ok(result)
}

fn read_ble_battery(ble_device: &BluetoothLEDevice) -> Option<u8> {
    use windows::Devices::Bluetooth::GenericAttributeProfile::{
        GattCharacteristicUuids, GattServiceUuids,
    };
    use windows::Storage::Streams::DataReader;

    let _name = ble_device.Name().ok().map(|n| n.to_string()).unwrap_or_default();
    let battery_service = GattServiceUuids::Battery().ok()?;
    let battery_level = GattCharacteristicUuids::BatteryLevel().ok()?;

    let services = ble_device.GetGattServicesForUuidAsync(battery_service).ok()?.get().ok()?;
    let service = services.Services().ok()?.into_iter().next()?;

    let chars = service.GetCharacteristicsForUuidAsync(battery_level).ok()?.get().ok()?;
    let char = chars.Characteristics().ok()?.into_iter().next()?;

    let buffer = char.ReadValueAsync().ok()?.get().ok()?.Value().ok()?;
    let reader = DataReader::FromBuffer(&buffer).ok()?;
    let level = reader.ReadByte().ok()?;
    Some(level)
}

fn read_btc_battery_from_device_id(device_id: &str) -> Option<u8> {
    let mac = device_id.rsplit('-').next()?;
    let mac_upper = mac.to_uppercase().replace(':', "");

    // Use same approach as BlueGauge: filter by GUID_DEVCLASS_SYSTEM + BTHENUM instance ID
    let class_guid = windows_sys::Win32::Devices::DeviceAndDriverInstallation::GUID_DEVCLASS_SYSTEM;
    let filter = windows_pnp::PnpFilter::Contains(&[
        "BTHENUM\\".to_string(),
        mac_upper.clone(),
    ]);
    let devices = windows_pnp::PnpEnumerator::enumerate_present_devices_and_filter_by_device_setup_class(
        class_guid, filter,
    ).ok()?;

    // DEVPKEY_BLUETOOTH_BATTERY = {104EA319-6EE2-4701-BD47-8DDBF425BBE5}, pid=2
    let battery_key = windows_pnp::PnpDevicePropertyKey {
        fmtid: windows_pnp_uuid::Uuid::from_u128(0x104EA319_6EE2_4701_BD47_8DDBF425BBE5),
        pid: 2,
    };

    for device in devices {
        let instance_id = &device.device_instance_id;
        if !instance_id.contains("BTHENUM\\") || !instance_id.to_uppercase().contains(&mac_upper) {
            continue;
        }

        if let Some(props) = &device.device_instance_properties {
            if let Some(windows_pnp::PnpDevicePropertyValue::Byte(battery)) = props.get(&battery_key) {
                return Some(*battery);
            }
        }
    }
    None
}
