#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod device;
mod wmi_query;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Listener, Manager,
};

static AUTO_START: AtomicBool = AtomicBool::new(false);
static TRAY_POS: OnceLock<Mutex<(f64, f64)>> = OnceLock::new();
static POPUP_ANIMATING: AtomicBool = AtomicBool::new(false);
static POPUP_POS: OnceLock<Mutex<(f64, f64)>> = OnceLock::new();
static AUTO_MENU_ITEM: OnceLock<Mutex<Option<MenuItem<tauri::Wry>>>> = OnceLock::new();

fn main() {
    // Init COM with apartment-threaded mode (same as Tauri) BEFORE Tauri starts.
    // This lets wmi use COMLibrary::assume_initialized() instead of re-initializing.
    unsafe {
        windows_sys::Win32::System::Com::CoInitializeEx(
            std::ptr::null(),
            0x2, // COINIT_APARTMENTTHREADED
        );
    }

    config::init_config();
    AUTO_START.store(config::with_config(|c| c.auto_start), Ordering::Relaxed);

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            let _ = app.get_webview_window("main").map(|w| w.show());
        }))
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .invoke_handler(tauri::generate_handler![
            commands::get_devices,
            commands::get_config,
            commands::update_config,
            commands::toggle_device_hidden,
            commands::open_settings,
            commands::toggle_popup,
            commands::exit_app,
            commands::close_window,
            commands::rename_device,
            commands::change_device_group,
            commands::toggle_group_hidden,
        ])
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::Focused(false) => {
                    if window.label() == "popup"
                        && !POPUP_ANIMATING.load(Ordering::Relaxed)
                        && window.is_visible().unwrap_or(false)
                    {
                        let app = window.app_handle();
                        toggle_popup(app);
                    }
                }
                _ => {}
            }
        })
        .setup(|app| {
            use tauri_plugin_autostart::ManagerExt;
            let autostart = app.autolaunch();
            let current = autostart.is_enabled().unwrap_or(false);
            let wanted = AUTO_START.load(Ordering::Relaxed);
            if wanted != current {
                let _ = if wanted { autostart.enable() } else { autostart.disable() };
            }

            let auto_text = if wanted { "开机自启 ✓" } else { "开机自启" };

            let show_i = MenuItem::with_id(app, "show", "设备信息", true, None::<&str>)?;
            let settings_i = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let about_i = MenuItem::with_id(app, "about", "关于", true, None::<&str>)?;
            let auto_i = MenuItem::with_id(app, "auto_start", auto_text, true, None::<&str>)?;
            let exit_i = MenuItem::with_id(app, "exit", "退出", true, None::<&str>)?;

            // Store auto_start menu item for later updates
            let _ = AUTO_MENU_ITEM.get_or_init(|| Mutex::new(Some(auto_i.clone())));

            let menu = Menu::with_items(
                app,
                &[&show_i, &settings_i, &about_i, &auto_i, &exit_i],
            )?;

            let _tray = TrayIconBuilder::new()
                .icon(Image::from_bytes(include_bytes!("../icons/tray-icon.png"))
                    .expect("Failed to load tray icon"))
                .tooltip("外设监控")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(move |app, event| {
                    match event.id.as_ref() {
                        "show" => { commands::toggle_popup(app.clone()); }
                        "settings" => { open_settings(app); }
                        "about" => { open_about(app); }
                        "auto_start" => {
                            let old = AUTO_START.load(Ordering::Relaxed);
                            let new_val = !old;
                            AUTO_START.store(new_val, Ordering::Relaxed);
                            config::with_config_mut(|c| c.auto_start = new_val);
                            use tauri_plugin_autostart::ManagerExt;
                            let autostart = app.autolaunch();
                            let _ = if new_val { autostart.enable() } else { autostart.disable() };
                            let text = if new_val { "开机自启 ✓" } else { "开机自启" };
                            let _ = auto_i.set_text(text);
                            let _ = app.emit("auto-start-changed", new_val);
                        }
                        "exit" => { app.exit(0); }
                        _ => {}
                    }
                })
                .on_tray_icon_event(|tray, event| {
                    let app = tray.app_handle();
                    if let tauri::tray::TrayIconEvent::Click {
                        button, button_state, rect, ..
                    } = event {
                        // Save tray position for ANY click (left or right)
                        if let Some(pos) = TRAY_POS.get() {
                            let sf = scale_factor(app);
                            let (px, py) = match rect.position {
                                tauri::Position::Physical(p) => (p.x as f64 / sf, p.y as f64 / sf),
                                tauri::Position::Logical(p) => (p.x, p.y),
                            };
                            *pos.lock().unwrap() = (px, py);
                        }

                        if button_state != tauri::tray::MouseButtonState::Up {
                            return;
                        }
                        if button == tauri::tray::MouseButton::Left {
                            toggle_popup(app);
                        }
                    }
                })
                .build(app)?;

            // Initialize tray position storage
            let _ = TRAY_POS.get_or_init(|| Mutex::new((100.0, 100.0)));

            // Listen for config changes to update tray menu
            let app_handle = app.handle().clone();
            app.listen("config-changed", move |_| {
                // Re-read config and update tray menu auto-start text
                let new_auto = config::with_config(|c| c.auto_start);
                AUTO_START.store(new_auto, Ordering::Relaxed);
                update_tray_auto_text();
            });

            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                let label = window.label();
                if label == "settings" || label == "about" {
                    api.prevent_close();
                    let _ = window.destroy();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}

fn toggle_popup(app: &tauri::AppHandle) {
    if POPUP_ANIMATING.load(Ordering::Relaxed) {
        return;
    }

    let sf = scale_factor(app);
    let screen_h = app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.size().height as f64 / sf)
        .unwrap_or(1080.0);

    let (tray_x, tray_y) = TRAY_POS.get()
        .map(|m| *m.lock().unwrap())
        .unwrap_or((100.0, screen_h - 50.0));

    let popup_w = 360.0;
    let popup_h = 520.0;
    let target_x = tray_x - popup_w / 2.0;
    let target_y = tray_y - popup_h - 15.0;
    let start_y = screen_h + 10.0;

    if let Some(window) = app.get_webview_window("popup") {
        if window.is_visible().unwrap_or(false) {
            // CLOSE
            POPUP_ANIMATING.store(true, Ordering::Relaxed);
            let _ = window.set_always_on_top(false);
            let (cx, cy) = POPUP_POS.get()
                .map(|m| *m.lock().unwrap())
                .unwrap_or((target_x, target_y));
            let win = window.clone();
            std::thread::spawn(move || {
                animate_close(&win, cx, cy, start_y);
                POPUP_ANIMATING.store(false, Ordering::Relaxed);
            });
        } else {
            // SHOW existing hidden window
            POPUP_ANIMATING.store(true, Ordering::Relaxed);
            let _ = window.set_always_on_top(false);
            let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition {
                x: target_x, y: start_y,
            }));
            let _ = window.show();
            let win = window.clone();
            std::thread::spawn(move || {
                animate_open(&win, target_x, start_y, target_y);
                POPUP_ANIMATING.store(false, Ordering::Relaxed);
            });
        }
    } else {
        // CREATE new window at correct position, no animation
        if let Ok(win) = tauri::WebviewWindowBuilder::new(
            app, "popup", tauri::WebviewUrl::App("popup.html".into()),
        )
        .title("外设信息")
        .inner_size(popup_w, popup_h)
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .position(target_x, target_y)
        .build() {
            // Set rounded corners before showing
            #[cfg(target_os = "windows")]
            if let Ok(hwnd) = win.hwnd() {
                set_rounded_corners(hwnd.0 as isize);
            }
            let _ = win.show();
            let _ = win.set_focus();
            // Save position for close animation
            if let Some(pos) = POPUP_POS.get() {
                *pos.lock().unwrap() = (target_x, target_y);
            }
        }
    }
}

fn animate_open(window: &tauri::WebviewWindow, x: f64, start_y: f64, end_y: f64) {
    let duration_ms = 200u64;
    let frames = 16;
    let step_ms = duration_ms / frames;
    for i in 0..=frames {
        let t = i as f64 / frames as f64;
        let eased = 1.0 - (1.0 - t).powi(3);
        let y = start_y + (end_y - start_y) * eased;
        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        std::thread::sleep(std::time::Duration::from_millis(step_ms));
    }
    // Save final position for close animation
    if let Some(pos) = POPUP_POS.get() {
        *pos.lock().unwrap() = (x, end_y);
    }
    let _ = window.set_always_on_top(true);
    let _ = window.set_focus();
}

fn animate_close(window: &tauri::WebviewWindow, x: f64, start_y: f64, end_y: f64) {
    let duration_ms = 150u64;
    let frames = 12;
    let step_ms = duration_ms / frames;
    for i in 0..=frames {
        let t = i as f64 / frames as f64;
        let eased = t * t * t; // ease-in
        let y = start_y + (end_y - start_y) * eased;
        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        std::thread::sleep(std::time::Duration::from_millis(step_ms));
    }
    let _ = window.hide();
}

fn open_settings(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(win) = tauri::WebviewWindowBuilder::new(
            &app,
            "settings",
            tauri::WebviewUrl::App("settings.html".into()),
        )
        .title("设置 - 外设监控")
        .inner_size(600.0, 500.0)
        .min_inner_size(400.0, 300.0)
        .visible(false)
        .build() {
            // Let the window-state plugin restore position, then show
            std::thread::sleep(std::time::Duration::from_millis(100));
            let _ = win.show();
            let _ = win.set_focus();
        }
    });
}

fn open_about(app: &tauri::AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(win) = tauri::WebviewWindowBuilder::new(
            &app,
            "about",
            tauri::WebviewUrl::App("about.html".into()),
        )
        .title("关于 外设监控")
        .inner_size(380.0, 360.0)
        .resizable(false)
        .visible(false)
        .center()
        .build() {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let _ = win.show();
            let _ = win.set_focus();
        }
    });
}

fn scale_factor(app: &tauri::AppHandle) -> f64 {
    app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0)
}

/// Set window corner preference to rounded (Windows 11 DWM)
fn set_rounded_corners(hwnd: isize) {
    unsafe {
        const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
        const DWMWCP_ROUND: u32 = 2;
        let preference = DWMWCP_ROUND;
        windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}

/// Update tray menu auto-start text to match current config
pub fn update_tray_auto_text() {
    if let Some(item) = AUTO_MENU_ITEM.get() {
        if let Ok(guard) = item.lock() {
            if let Some(ref mi) = *guard {
                let text = if AUTO_START.load(Ordering::Relaxed) {
                    "开机自启 ✓"
                } else {
                    "开机自启"
                };
                let _ = mi.set_text(text);
            }
        }
    }
}
