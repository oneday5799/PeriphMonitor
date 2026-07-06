use tauri::Manager;

pub fn open_settings(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
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
        .background_color(tauri::utils::config::Color(243, 243, 243, 255))
        .visible(false)
        .build() {
            std::thread::sleep(std::time::Duration::from_millis(100));
            let _ = win.show();
            let _ = win.set_focus();
        }
    });
}

pub fn open_about(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("about") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        return;
    }
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

pub fn scale_factor(app: &tauri::AppHandle) -> f64 {
    app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.scale_factor())
        .unwrap_or(1.0)
}

#[cfg(target_os = "windows")]
pub fn set_rounded_corners(hwnd: isize) {
    unsafe {
        const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
        const DWMWCP_ROUND: u32 = 2;
        let preference = DWMWCP_ROUND;
        windows_sys::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            hwnd as *mut core::ffi::c_void,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        );
    }
}
