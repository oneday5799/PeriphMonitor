use std::sync::atomic::AtomicBool;
use std::sync::{Mutex, OnceLock};
use tauri::menu::MenuItem;

pub static TRAY_POS: OnceLock<Mutex<(f64, f64)>> = OnceLock::new();
pub static POPUP_POS: OnceLock<Mutex<(f64, f64)>> = OnceLock::new();
pub static ANIMATING: AtomicBool = AtomicBool::new(false);
pub static AUTO_START: AtomicBool = AtomicBool::new(false);
pub static AUTO_MENU_ITEM: OnceLock<Mutex<Option<MenuItem<tauri::Wry>>>> = OnceLock::new();
