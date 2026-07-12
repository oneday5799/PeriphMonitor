#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![warn(unused_imports, dead_code)]

mod bluetooth;
mod classify;
mod commands;
mod config;
mod dedup;
mod device;
mod device_data;
mod popup;
mod process;
mod state;
mod tray;
mod windows;
mod wmi_query;

use tauri::Manager;

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
    device_data::init_device_data();
    tray::init_auto_start();

    // 启动时清空日志（基于 exe 所在目录）
    let log_path = process::exe_dir().join("debug.log");
    let _ = std::fs::remove_file(&log_path);

    let is_autostart = std::env::args().any(|a| a == "--autostart");

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if app.get_webview_window("popup").is_some() {
                popup::toggle(app);
            }
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
            commands::connect_bluetooth_device,
            commands::disconnect_bluetooth_device,
            commands::check_bt_connection,
            commands::open_bt_settings,
            commands::open_url,
            commands::open_24g_device_file,
            commands::toggle_device_tray,
            commands::get_tray_tooltip,
        ])
        .setup(move |app| {
            tray::setup_tray(app)?;
            if !is_autostart {
                popup::toggle(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::Focused(false) => {
                    if window.label() == "popup"
                        && !state::ANIMATING.load(std::sync::atomic::Ordering::Relaxed)
                        && window.is_visible().unwrap_or(false)
                    {
                        let app = window.app_handle();
                        popup::toggle(app);
                    }
                }
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    let label = window.label();
                    if label == "settings" || label == "about" {
                        api.prevent_close();
                        let _ = window.destroy();
                    } else if label == "popup" {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                }
                _ => {}
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
