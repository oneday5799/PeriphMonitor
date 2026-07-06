#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod config;
mod device;
mod popup;
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
    tray::init_auto_start();

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
        .setup(|app| {
            tray::setup_tray(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            match event {
                tauri::WindowEvent::Focused(false) => {
                    if window.label() == "popup"
                        && !popup::ANIMATING.load(std::sync::atomic::Ordering::Relaxed)
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
