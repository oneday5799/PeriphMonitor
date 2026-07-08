use crate::config::{self, Config};
use crate::device::Device;
use crate::wmi_query::query_devices;
use crate::bluetooth::check_device_connection;
use tauri::Manager;

// Store discovered device IDs for connect/disconnect operations
use std::sync::OnceLock;
use std::collections::HashMap;
static DEVICE_IDS: OnceLock<std::sync::Mutex<HashMap<String, String>>> = OnceLock::new();

fn get_device_ids() -> &'static std::sync::Mutex<HashMap<String, String>> {
    DEVICE_IDS.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

#[tauri::command(async)]
pub async fn get_devices() -> Vec<Device> {
    let devices = tokio::task::spawn_blocking(|| query_devices())
        .await
        .unwrap_or_default();

    // Store device IDs for connect/disconnect operations
    if let Ok(mut ids) = get_device_ids().lock() {
        ids.clear();
        for dev in &devices {
            if let Some(ref device_id) = dev.device_id {
                ids.insert(dev.name.clone(), device_id.clone());
            }
        }
    }

    devices
}

#[tauri::command]
pub fn toggle_popup(app: tauri::AppHandle) {
    crate::popup::toggle(&app);
}

#[tauri::command]
pub fn open_settings(app: tauri::AppHandle) {
    crate::windows::open_settings(&app);
}

#[tauri::command]
pub fn exit_app(app: tauri::AppHandle) {
    app.exit(0);
}

#[tauri::command]
pub fn get_config() -> Config {
    config::with_config(|c| c.clone())
}

#[tauri::command]
pub fn update_config(app: tauri::AppHandle, new_config: Config) {
    config::with_config_mut(|c| {
        *c = new_config;
    });
    let _ = app.emit("config-changed", ());
}

use tauri::Emitter;

#[tauri::command]
pub fn toggle_device_hidden(app: tauri::AppHandle, name: String) {
    config::with_config_mut(|c| {
        if let Some(pos) = c.hidden_devices.iter().position(|h| h == &name) {
            c.hidden_devices.remove(pos);
        } else {
            c.hidden_devices.push(name);
        }
    });
    let _ = app.emit("config-changed", ());
}

#[tauri::command]
pub fn open_bt_settings() -> Result<String, String> {
    std::process::Command::new("cmd")
        .args(["/c", "start", "ms-settings:bluetooth"])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok("opened".to_string())
}

#[tauri::command]
pub fn close_window(app: tauri::AppHandle, name: String) {
    if let Some(window) = app.get_webview_window(&name) {
        let _ = window.close();
    }
}

#[tauri::command]
pub fn rename_device(original: String, new_name: String) {
    config::with_config_mut(|c| {
        if new_name.is_empty() {
            c.device_names.remove(&original);
        } else {
            c.device_names.insert(original, new_name);
        }
    });
}

#[tauri::command]
pub fn change_device_group(app: tauri::AppHandle, name: String, group: String) {
    config::with_config_mut(|c| {
        if group.is_empty() {
            c.device_groups.remove(&name);
        } else {
            c.device_groups.insert(name, group);
        }
    });
    let _ = app.emit("config-changed", ());
}

#[tauri::command]
pub fn toggle_group_hidden(app: tauri::AppHandle, group: String) {
    config::with_config_mut(|c| {
        if let Some(pos) = c.hidden_groups.iter().position(|g| g == &group) {
            c.hidden_groups.remove(pos);
        } else {
            c.hidden_groups.push(group);
        }
    });
    let _ = app.emit("config-changed", ());
}

#[tauri::command(async)]
pub async fn disconnect_bluetooth_device(name: String) -> Result<String, String> {
    bt_action(&name, "disconnect").await
}

#[tauri::command(async)]
pub async fn connect_bluetooth_device(name: String) -> Result<String, String> {
    bt_action(&name, "connect").await
}

#[tauri::command(async)]
pub async fn check_bt_connection(name: String) -> Result<Option<bool>, String> {
    let result = tokio::task::spawn_blocking(move || check_device_connection(&name))
        .await
        .map_err(|e| e.to_string())?;
    Ok(result)
}

async fn bt_action(name: &str, action: &str) -> Result<String, String> {
    eprintln!("[BT] {}: {}", action, name);
    let device_id = {
        let ids = get_device_ids().lock().map_err(|e| e.to_string())?;
        ids.get(name).cloned().ok_or_else(|| format!("Device '{}' not found", name))?
    };

    let mac = device_id.rsplit('-').next().unwrap_or("").to_string();
    let mac_clone = mac.clone();
    let action = action.to_string();

    let candidates = [
        std::path::PathBuf::from("scripts/bt_action.ps1"),
        std::env::current_dir().unwrap_or_default().join("scripts/bt_action.ps1"),
        std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.join("scripts/bt_action.ps1"))).unwrap_or_default(),
        std::env::current_exe().ok().and_then(|p| p.parent().and_then(|p| p.parent()).map(|p| p.join("src-tauri/scripts/bt_action.ps1"))).unwrap_or_default(),
    ];

    let script_path = candidates.iter().find(|p| p.exists()).cloned()
        .ok_or("bt_action.ps1 not found")?;
    let path_str = script_path.to_string_lossy().to_string();
    eprintln!("[BT] Script: {}", path_str);

    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", &path_str, "-Mac", &mac_clone, "-Action", &action])
            .output()
    }).await.map_err(|e| e.to_string())?
      .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    eprintln!("[BT] stdout: {}", stdout);
    if !stderr.is_empty() { eprintln!("[BT] stderr: {}", stderr); }

    Ok(stdout.trim().to_string())
}


