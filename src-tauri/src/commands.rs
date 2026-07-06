use crate::config::{self, Config};
use crate::device::Device;
use crate::wmi_query::query_devices;
use tauri::Manager;

#[tauri::command(async)]
pub async fn get_devices() -> Vec<Device> {
    tokio::task::spawn_blocking(|| query_devices())
        .await
        .unwrap_or_default()
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
