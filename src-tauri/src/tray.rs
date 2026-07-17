use std::os::windows::ffi::OsStrExt;
use std::sync::atomic::Ordering;
use std::sync::{Mutex, OnceLock};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem, Submenu},
    tray::{TrayIcon, TrayIconBuilder},
    Emitter, Listener,
};

use crate::config;
use crate::popup;
use crate::windows;
use crate::state::{TRAY_POS, AUTO_START, AUTO_MENU_ITEM, get_devices_cache};
use crate::audio;

static TRAY_ICON: OnceLock<Mutex<Option<TrayIcon<tauri::Wry>>>> = OnceLock::new();
static AUDIO_DEVICES_SUBMENU: OnceLock<Mutex<Option<Submenu<tauri::Wry>>>> = OnceLock::new();

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

/// 根据缓存的设备信息构建 tooltip 文本
pub fn build_tooltip_text() -> String {
    let tray_devices = config::with_config(|c| c.tray_devices.clone());

    if tray_devices.is_empty() {
        return "外设监控".to_string();
    }

    let device_names = config::with_config(|c| c.device_names.clone());
    let cache = get_devices_cache();
    let devices = cache.lock().unwrap_or_else(|e| e.into_inner());

    let mut lines = Vec::new();
    for tray_name in &tray_devices {
        if let Some(dev) = devices.iter().find(|d| &d.name == tray_name) {
            let display_name = device_names.get(&dev.name).unwrap_or(&dev.name);
            let dot = if dev.status == "已连接" { "🟢" } else { "⚪" };
            match dev.battery {
                Some(battery) => lines.push(format!("{} {} - {}%", dot, display_name, battery)),
                None => lines.push(format!("{} {}", dot, display_name)),
            }
        }
    }

    if lines.is_empty() {
        "外设监控".to_string()
    } else {
        lines.join("\n")
    }
}

/// 更新托盘 tooltip
pub fn update_tooltip(_app: &tauri::AppHandle) {
    let tooltip = build_tooltip_text();

    if let Ok(guard) = TRAY_ICON.get_or_init(|| Mutex::new(None)).lock() {
        if let Some(ref tray) = *guard {
            let _ = tray.set_tooltip(Some(tooltip));
        }
    }
}

/// 后台刷新线程：定期查询设备并更新缓存，状态变化时自动更新 tooltip
pub fn start_device_watcher(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(10));

            // 没有托盘设备时不查询，节省资源
            let has_tray_devices = config::with_config(|c| !c.tray_devices.is_empty());
            if !has_tray_devices {
                continue;
            }

            let changed = refresh_devices_cache();
            if changed {
                let h = app_handle.clone();
                std::thread::spawn(move || update_tooltip(&h));
            }
        }
    });
}

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

    // 构建音频设备切换子菜单
    let audio_devices_menu = build_audio_devices_menu(app.handle())?;
    let _ = AUDIO_DEVICES_SUBMENU.get_or_init(|| Mutex::new(Some(audio_devices_menu.clone())));

    // 构建 Windows 声音设置子菜单
    let win_sound_menu = build_windows_sound_settings_menu(app.handle())?;

    let _ = AUTO_MENU_ITEM.get_or_init(|| Mutex::new(Some(auto_i.clone())));

    let menu = Menu::with_items(
        app,
        &[&show_i, &settings_i, &about_i, &audio_devices_menu, &win_sound_menu, &auto_i, &exit_i],
    )?;

    let _tray = TrayIconBuilder::with_id("main-tray")
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
                id if id.starts_with("audio_dev_") => {
                    let device_id = id[10..].to_owned();
                    if !device_id.is_empty() {
                        std::thread::spawn(move || {
                            let _ = audio::set_default_device(&device_id);
                            update_audio_devices_menu();
                        });
                    }
                }
                "win_sound_volume_mixer" => {
                    let wide_file = to_wide("sndvol.exe");
                    let wide_verb = to_wide("open");
                    unsafe {
                        let _ = windows_sys::Win32::UI::Shell::ShellExecuteW(
                            std::ptr::null_mut(),
                            wide_verb.as_ptr(),
                            wide_file.as_ptr(),
                            std::ptr::null(),
                            std::ptr::null(),
                            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
                        );
                    }
                }
                "win_sound_playback" => {
                    let _ = open_sound_panel("playback");
                }
                "win_sound_recording" => {
                    let _ = open_sound_panel("recording");
                }
                "win_sound_sounds" => {
                    let _ = open_sound_panel("sounds");
                }
                "win_sound_settings" => {
                    let _ = open_settings_page("sound");
                }
                "win_sound_app_volume" => {
                    let _ = open_settings_page("apps-volume");
                }
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

    if let Ok(mut guard) = TRAY_ICON.get_or_init(|| Mutex::new(None)).lock() {
        *guard = Some(_tray);
    }

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

    app.listen("config-changed", move |_| {
        let new_auto = config::with_config(|c| c.auto_start);
        AUTO_START.store(new_auto, Ordering::Relaxed);
        update_auto_text();
    });

    let handle = app.handle().clone();
    app.listen("tray-devices-changed", move |_| {
        let h = handle.clone();
        std::thread::spawn(move || update_tooltip(&h));
    });

    app.listen("audio-devices-changed", |_| {
        update_audio_devices_menu();
    });

    // 启动后台设备监控线程
    start_device_watcher(app.handle().clone());

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

/// 构建音频设备切换子菜单
fn build_audio_devices_menu(app: &tauri::AppHandle) -> Result<Submenu<tauri::Wry>, Box<dyn std::error::Error>> {
    let submenu = Submenu::with_id(app, "audio_devices", "切换音频设备", true)?;
    let devices = audio::enumerate_output_devices().unwrap_or_default();
    if devices.is_empty() {
        let empty = MenuItem::with_id(app, "audio_dev_empty", "无音频设备", false, None::<&str>)?;
        submenu.append(&empty)?;
    } else {
        for device in &devices {
            let check = if device.is_default { " ✓" } else { "" };
            let label = format!("{}{}", device.name, check);
            let item = MenuItem::with_id(app, format!("audio_dev_{}", device.id), label, true, None::<&str>)?;
            submenu.append(&item)?;
        }
    }
    Ok(submenu)
}

/// 更新音频设备切换子菜单（在设备列表变化时调用）
pub fn update_audio_devices_menu() {
    let Ok(tray_guard) = TRAY_ICON.get().unwrap().lock() else { return };
    let Some(ref tray) = *tray_guard else { return };
    let app = tray.app_handle().clone();

    let new_submenu = match build_audio_devices_menu(&app) {
        Ok(s) => s,
        Err(_) => return,
    };

    // 重建顶层菜单，替换音频子菜单
    let auto_text = if AUTO_START.load(Ordering::Relaxed) {
        "开机自启 ✓"
    } else {
        "开机自启"
    };
    let Ok(show_i) = MenuItem::with_id(&app, "show", "设备信息", true, None::<&str>) else { return };
    let Ok(settings_i) = MenuItem::with_id(&app, "settings", "设置", true, None::<&str>) else { return };
    let Ok(about_i) = MenuItem::with_id(&app, "about", "关于", true, None::<&str>) else { return };
    let Ok(auto_i) = MenuItem::with_id(&app, "auto_start", auto_text, true, None::<&str>) else { return };
    let Ok(exit_i) = MenuItem::with_id(&app, "exit", "退出", true, None::<&str>) else { return };
    let Ok(win_sound_menu) = build_windows_sound_settings_menu(&app) else { return };
    let _ = AUTO_MENU_ITEM.get_or_init(|| Mutex::new(Some(auto_i.clone())));

    if let Ok(menu) = Menu::with_items(
        &app,
        &[&show_i, &settings_i, &about_i, &new_submenu, &win_sound_menu, &auto_i, &exit_i],
    ) {
        let _ = tray.set_menu(Some(menu));
    }

    drop(tray_guard);
    if let Some(submenu_lock) = AUDIO_DEVICES_SUBMENU.get() {
        if let Ok(mut guard) = submenu_lock.lock() {
            *guard = Some(new_submenu);
        }
    }
}

/// 构建 Windows 声音设置子菜单
fn build_windows_sound_settings_menu(app: &tauri::AppHandle) -> Result<Submenu<tauri::Wry>, Box<dyn std::error::Error>> {
    let submenu = Submenu::with_id(app, "win_sound", "Windows 声音设置", true)?;
    let items = [
        ("win_sound_volume_mixer", "音量合成器 (Classic)"),
        ("win_sound_playback", "播放设备 (Classic)"),
        ("win_sound_recording", "录制设备 (Classic)"),
        ("win_sound_sounds", "声音 (Classic)"),
        ("win_sound_settings", "声音设置"),
        ("win_sound_app_volume", "音量合成器"),
    ];
    for (id, label) in items {
        let item = MenuItem::with_id(app, id, label, true, None::<&str>)?;
        submenu.append(&item)?;
    }
    Ok(submenu)
}

/// 打开旧版声音控制面板 (mmsys.cpl)
fn open_sound_panel(panel: &str) -> Result<(), Box<dyn std::error::Error>> {
    let arg = format!("shell32.dll,Control_RunDLL mmsys.cpl,,{}", panel);
    let wide_file = to_wide("rundll32.exe");
    let wide_arg = to_wide(&arg);
    let wide_verb = to_wide("open");
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            wide_verb.as_ptr(),
            wide_file.as_ptr(),
            wide_arg.as_ptr(),
            std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
    Ok(())
}

/// 打开现代 Windows 设置页面 (ms-settings:)
fn open_settings_page(page: &str) -> Result<(), Box<dyn std::error::Error>> {
    let url = format!("ms-settings:{}", page);
    let wide_url = to_wide(&url);
    let wide_verb = to_wide("open");
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            wide_verb.as_ptr(),
            wide_url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
    Ok(())
}

fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}
