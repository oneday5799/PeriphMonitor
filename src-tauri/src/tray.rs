use std::sync::atomic::Ordering;
use std::sync::Mutex;
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Listener,
};

use crate::config;
use crate::popup;
use crate::windows;
use crate::state::{TRAY_POS, AUTO_START, AUTO_MENU_ITEM};

pub fn init_auto_start() {
    AUTO_START.store(config::with_config(|c| c.auto_start), Ordering::Relaxed);
}

pub fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
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
                "show" => { crate::commands::toggle_popup(app.clone()); }
                "settings" => { windows::open_settings(app); }
                "about" => { windows::open_about(app); }
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
                "exit" => { std::process::exit(0); }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, event| {
            let app = tray.app_handle();
            if let tauri::tray::TrayIconEvent::Click {
                button, button_state, rect, ..
            } = event {
                if let Some(pos) = TRAY_POS.get() {
                    let sf = windows::scale_factor(app);
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
                    popup::toggle(app);
                }
            }
        })
        .build(app)?;

    let _ = TRAY_POS.get_or_init(|| {
        let handle = app.handle();
        let sf = windows::scale_factor(handle);
        let screen_w = handle.primary_monitor()
            .ok()
            .flatten()
            .map(|m| m.size().width as f64 / sf)
            .unwrap_or(1920.0);
        let screen_h = handle.primary_monitor()
            .ok()
            .flatten()
            .map(|m| m.size().height as f64 / sf)
            .unwrap_or(1080.0);
        Mutex::new((screen_w - 300.0, screen_h - 50.0))
    });

    let _app_handle = app.handle().clone();
    app.listen("config-changed", move |_| {
        let new_auto = config::with_config(|c| c.auto_start);
        AUTO_START.store(new_auto, Ordering::Relaxed);
        update_auto_text();
    });

    Ok(())
}

pub fn update_auto_text() {
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
