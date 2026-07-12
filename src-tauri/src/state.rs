use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, OnceLock};
use tauri::menu::MenuItem;

use crate::device::Device;

pub static TRAY_POS: OnceLock<Mutex<(f64, f64)>> = OnceLock::new();
pub static POPUP_POS: OnceLock<Mutex<(f64, f64)>> = OnceLock::new();
pub static ANIMATING: AtomicBool = AtomicBool::new(false);
pub static AUTO_START: AtomicBool = AtomicBool::new(false);
pub static AUTO_MENU_ITEM: OnceLock<Mutex<Option<MenuItem<tauri::Wry>>>> = OnceLock::new();

/// 设备缓存，用于托盘 tooltip 显示，避免重复 WMI 查询
pub static DEVICES_CACHE: OnceLock<Mutex<Vec<Device>>> = OnceLock::new();

pub fn get_devices_cache() -> &'static Mutex<Vec<Device>> {
    DEVICES_CACHE.get_or_init(|| Mutex::new(Vec::new()))
}

/// 刷新设备缓存，返回是否发生变化
pub fn refresh_devices_cache() -> bool {
    let new_devices = crate::wmi_query::query_devices();
    let cache = get_devices_cache();

    if let Ok(mut guard) = cache.lock() {
        if *guard != new_devices {
            *guard = new_devices;
            true
        } else {
            false
        }
    } else {
        false
    }
}
