use std::sync::Mutex;
use windows::Devices::Bluetooth::{BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;

use crate::device;
use crate::process;

/// 蓝牙操作全局锁，防止并发操作干扰适配器状态
static BT_LOCK: Mutex<()> = Mutex::new(());

/// 执行蓝牙连接/断开操作
pub fn bt_action(name: &str, action: &str) -> Result<String, String> {
    // 串行化蓝牙操作，防止并发竞争
    let _guard = BT_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let device_id = device::get_device_id_by_name(name)
        .ok_or_else(|| {
            let msg = format!("[bt] Device '{}' not found in device_id map", name);
            crate::process::append_log(&msg);
            format!("Device '{}' not found", name)
        })?;

    let mac = device_id.rsplit('-').next().unwrap_or("").to_string();
    let header = format!("[bt] {} device='{}' mac='{}' device_id='{}'", action.to_uppercase(), name, mac, device_id);
    crate::process::append_log(&header);

    let script_path = find_bt_script()?;
    crate::process::append_log_detailed(&format!("[bt] script: {}", script_path));

    // 使用设备 MAC 作为文件名后缀，避免并发时输出文件冲突
    let out_file = std::env::temp_dir().join(format!("bt_action_out_{}.txt", mac.replace(':', "")));
    let out_arg = out_file.to_string_lossy().to_string();

    let result = process::run_powershell_script(
        &script_path,
        &["-Mac", &mac, "-Action", action, "-OutFile", &out_arg],
    )?;

    let script_output = std::fs::read_to_string(&out_file)
        .unwrap_or_else(|e| {
            crate::process::append_log(&format!("[bt] READ_OUTFILE_FAILED: {}", e));
            String::new()
        });
    let _ = std::fs::remove_file(&out_file);

    let combined = if script_output.is_empty() {
        result
    } else {
        format!("{}\n{}", result, script_output.trim())
    };

    crate::process::append_log_detailed(&format!("[bt] result: {}", combined));

    // 检测设备未找到或蓝牙适配器异常
    if combined.contains("NOT_FOUND") {
        crate::process::append_log("[bt] 设备未找到 (NOT_FOUND)");
        return Err("设备未找到".to_string());
    }
    if combined.contains("NO_RADIO") {
        crate::process::append_log("[bt] 未检测到蓝牙适配器 (NO_RADIO)");
        return Err("未检测到蓝牙适配器".to_string());
    }

    Ok(combined)
}

fn find_bt_script() -> Result<String, String> {
    let exe_dir = crate::process::exe_dir();
    let candidates = [
        exe_dir.join("scripts/bt_action.ps1"),
        exe_dir.parent().map(|p| p.join("src-tauri/scripts/bt_action.ps1")).unwrap_or_default(),
        exe_dir.parent().and_then(|p| p.parent()).map(|p| p.join("src-tauri/scripts/bt_action.ps1")).unwrap_or_default(),
    ];
    candidates.iter()
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| {
            crate::process::append_log("[bt] bt_action.ps1 not found");
            "bt_action.ps1 not found".to_string()
        })
}

/// 蓝牙设备信息: (名称, 已连接, 设备ID)
type BtDeviceInfo = (String, bool, String);

/// 从 DeviceInformation 提取 Classic BT 设备信息
fn classic_device_from_info(device_info: &DeviceInformation) -> Option<BtDeviceInfo> {
    use windows::Devices::Bluetooth::BluetoothConnectionStatus;
    let device_id = device_info.Id().ok()?;
    let device = BluetoothDevice::FromIdAsync(&device_id).ok()?.join().ok()?;
    let name = device.Name().ok()?.to_string();
    let connected = device.ConnectionStatus().ok()? == BluetoothConnectionStatus::Connected;
    Some((name, connected, device_id.to_string()))
}

/// 从 DeviceInformation 提取 BLE 设备信息
fn ble_device_from_info(device_info: &DeviceInformation) -> Option<BtDeviceInfo> {
    use windows::Devices::Bluetooth::BluetoothConnectionStatus;
    let device_id = device_info.Id().ok()?;
    let device = BluetoothLEDevice::FromIdAsync(&device_id).ok()?.join().ok()?;
    let name = device.Name().ok()?.to_string();
    let connected = device.ConnectionStatus().ok()? == BluetoothConnectionStatus::Connected;
    Some((name, connected, device_id.to_string()))
}

pub fn find_paired_bluetooth_devices() -> Result<Vec<(String, bool, Option<u8>, String)>, Box<dyn std::error::Error>> {
    let mut result = Vec::new();

    // Classic Bluetooth devices
    let btc_selector = BluetoothDevice::GetDeviceSelectorFromPairingState(true)?;
    let btc_devices_info = DeviceInformation::FindAllAsyncAqsFilter(&btc_selector)?.join()?;
    for device_info in btc_devices_info.into_iter() {
        if let Some((name, connected, device_id)) = classic_device_from_info(&device_info) {
            let battery = read_btc_battery_from_device_id(&device_id);
            result.push((name, connected, battery, device_id));
        }
    }

    // BLE devices
    let ble_selector = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true)?;
    let ble_devices_info = DeviceInformation::FindAllAsyncAqsFilter(&ble_selector)?.join()?;
    for device_info in ble_devices_info.into_iter() {
        if let Some((name, connected, device_id)) = ble_device_from_info(&device_info) {
            let battery = read_ble_battery_from_id(&device_id);
            result.push((name, connected, battery, device_id));
        }
    }

    crate::process::append_log_detailed(&format!("[bt] find_paired_bluetooth_devices: found {} devices", result.len()));
    Ok(result)
}

fn read_ble_battery_from_id(device_id: &str) -> Option<u8> {
    let hstr = windows::core::HSTRING::from(device_id);
    let future = BluetoothLEDevice::FromIdAsync(&hstr).ok()?;
    let device = future.join().ok()?;
    read_ble_battery(&device)
}

fn read_ble_battery(ble_device: &BluetoothLEDevice) -> Option<u8> {
    use windows::Devices::Bluetooth::GenericAttributeProfile::{
        GattCharacteristicUuids, GattServiceUuids,
    };
    use windows::Storage::Streams::DataReader;

    let battery_service = GattServiceUuids::Battery().ok()?;
    let battery_level = GattCharacteristicUuids::BatteryLevel().ok()?;

    let services = ble_device.GetGattServicesForUuidAsync(battery_service).ok()?.join().ok()?;
    let service = services.Services().ok()?.into_iter().next()?;

    let chars = service.GetCharacteristicsForUuidAsync(battery_level).ok()?.join().ok()?;
    let char = chars.Characteristics().ok()?.into_iter().next()?;

    let buffer = char.ReadValueAsync().ok()?.join().ok()?.Value().ok()?;
    let reader = DataReader::FromBuffer(&buffer).ok()?;
    let level = reader.ReadByte().ok()?;
    Some(level)
}

/// Check connection status of a single Bluetooth device by name
pub fn check_device_connection(name: &str) -> Option<bool> {
    find_paired_bluetooth_devices().ok()?
        .into_iter()
        .find(|(n, _, _, _)| n == name)
        .map(|(_, connected, _, _)| connected)
}

fn read_btc_battery_from_device_id(device_id: &str) -> Option<u8> {
    let mac = device_id.rsplit('-').next()?;
    let mac_upper = mac.to_uppercase().replace(':', "");

    let class_guid = windows_sys::Win32::Devices::DeviceAndDriverInstallation::GUID_DEVCLASS_SYSTEM;
    let filter = windows_pnp::PnpFilter::Contains(&[
        "BTHENUM\\".to_string(),
        mac_upper.clone(),
    ]);
    let devices = windows_pnp::PnpEnumerator::enumerate_present_devices_and_filter_by_device_setup_class(
        class_guid, filter,
    ).ok()?;

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
