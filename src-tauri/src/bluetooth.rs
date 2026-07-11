use windows::Devices::Bluetooth::{BluetoothDevice, BluetoothLEDevice};
use windows::Devices::Enumeration::DeviceInformation;

use crate::device;
use crate::process;

/// 执行蓝牙连接/断开操作
pub fn bt_action(name: &str, action: &str) -> Result<String, String> {
    let device_id = device::get_device_id_by_name(name)
        .ok_or_else(|| format!("Device '{}' not found", name))?;

    let mac = device_id.rsplit('-').next().unwrap_or("").to_string();

    let script_path = find_bt_script()?;
    process::run_powershell_script(
        &script_path,
        &["-Mac", &mac, "-Action", action],
    )
}

fn find_bt_script() -> Result<String, String> {
    let candidates = [
        std::path::PathBuf::from("scripts/bt_action.ps1"),
        std::env::current_dir().unwrap_or_default().join("scripts/bt_action.ps1"),
        std::env::current_exe().ok()
            .and_then(|p| p.parent().map(|p| p.join("scripts/bt_action.ps1")))
            .unwrap_or_default(),
        std::env::current_exe().ok()
            .and_then(|p| p.parent().and_then(|p| p.parent()).map(|p| p.join("src-tauri/scripts/bt_action.ps1")))
            .unwrap_or_default(),
        std::env::current_exe().ok()
            .and_then(|p| p.parent().and_then(|p| p.parent().and_then(|p| p.parent())).map(|p| p.join("src-tauri/scripts/bt_action.ps1")))
            .unwrap_or_default(),
    ];
    candidates.iter()
        .find(|p| p.exists())
        .map(|p| p.to_string_lossy().to_string())
        .ok_or_else(|| "bt_action.ps1 not found".to_string())
}

pub fn find_paired_bluetooth_devices() -> Result<Vec<(String, bool, Option<u8>, String)>, Box<dyn std::error::Error>> {
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
                    let device_id_str = device_id.to_string();
                    result.push((name, connected, battery, device_id_str));
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
                    let device_id_str = device_id.to_string();
                    result.push((name, connected, battery, device_id_str));
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

/// Check connection status of a single Bluetooth device by name
pub fn check_device_connection(name: &str) -> Option<bool> {
    use windows::Devices::Bluetooth::BluetoothConnectionStatus;

    // Check classic Bluetooth devices
    if let Ok(btc_selector) = BluetoothDevice::GetDeviceSelectorFromPairingState(true) {
        if let Ok(btc_op) = DeviceInformation::FindAllAsyncAqsFilter(&btc_selector) {
            if let Ok(btc_devices_info) = btc_op.get() {
                for device_info in btc_devices_info.into_iter() {
                    if let Ok(device_id) = device_info.Id() {
                        if let Ok(future) = BluetoothDevice::FromIdAsync(&device_id) {
                            if let Ok(device) = future.get() {
                                if let Ok(device_name) = device.Name() {
                                    if device_name.to_string() == name {
                                        if let Ok(status) = device.ConnectionStatus() {
                                            return Some(status == BluetoothConnectionStatus::Connected);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Check BLE devices
    if let Ok(ble_selector) = BluetoothLEDevice::GetDeviceSelectorFromPairingState(true) {
        if let Ok(ble_op) = DeviceInformation::FindAllAsyncAqsFilter(&ble_selector) {
            if let Ok(ble_devices_info) = ble_op.get() {
                for device_info in ble_devices_info.into_iter() {
                    if let Ok(device_id) = device_info.Id() {
                        if let Ok(future) = BluetoothLEDevice::FromIdAsync(&device_id) {
                            if let Ok(device) = future.get() {
                                if let Ok(device_name) = device.Name() {
                                    if device_name.to_string() == name {
                                        if let Ok(status) = device.ConnectionStatus() {
                                            return Some(status == BluetoothConnectionStatus::Connected);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    None
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
