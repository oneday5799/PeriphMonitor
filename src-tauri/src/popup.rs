use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use tauri::Manager;

use crate::windows;

pub static TRAY_POS: OnceLock<Mutex<(f64, f64)>> = OnceLock::new();
pub static ANIMATING: AtomicBool = AtomicBool::new(false);
pub static POPUP_POS: OnceLock<Mutex<(f64, f64)>> = OnceLock::new();

const POPUP_W: f64 = 360.0;
const POPUP_H: f64 = 520.0;

/// cubic-bezier(0.62, 0, 0.32, 1) easing — same as win11React
fn cubic_bezier(t: f64) -> f64 {
    let p1x = 0.62;
    let p2x = 0.32;
    // solve x(t) = progress for t using Newton-Raphson
    let mut t_param = t;
    for _ in 0..8 {
        let x = 3.0 * p1x * t_param * (1.0 - t_param).powi(2)
            + 3.0 * p2x * t_param.powi(2) * (1.0 - t_param)
            + t_param.powi(3);
        let dx = 3.0 * p1x * (1.0 - t_param).powi(2)
            + 6.0 * (p2x - p1x) * t_param * (1.0 - t_param)
            + 3.0 * (1.0 - p2x) * t_param.powi(2);
        t_param -= (x - t) / dx;
        t_param = t_param.clamp(0.0, 1.0);
    }
    3.0 * t_param.powi(2) * (1.0 - t_param)
        + t_param.powi(3)
}

pub fn toggle(app: &tauri::AppHandle) {
    if ANIMATING.load(Ordering::Relaxed) {
        return;
    }

    let sf = windows::scale_factor(app);
    let screen_h = app.primary_monitor()
        .ok()
        .flatten()
        .map(|m| m.size().height as f64 / sf)
        .unwrap_or(1080.0);

    let (tray_x, tray_y) = TRAY_POS.get()
        .map(|m| *m.lock().unwrap())
        .unwrap_or((100.0, screen_h - 50.0));

    let target_x = tray_x - POPUP_W / 2.0;
    let target_y = tray_y - POPUP_H - 15.0;
    let start_y = screen_h + 10.0;

    if let Some(window) = app.get_webview_window("popup") {
        if window.is_visible().unwrap_or(false) {
            close(app, &window, target_x, target_y, start_y);
        } else {
            show(app, &window, target_x, start_y, target_y);
        }
    } else {
        create(app, target_x, target_y);
    }
}

fn close(
    _app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    target_x: f64,
    target_y: f64,
    start_y: f64,
) {
    ANIMATING.store(true, Ordering::Relaxed);
    let _ = window.set_always_on_top(false);
    let (cx, cy) = POPUP_POS.get()
        .map(|m| *m.lock().unwrap())
        .unwrap_or((target_x, target_y));
    let win = window.clone();
    std::thread::spawn(move || {
        animate_close(&win, cx, cy, start_y);
        ANIMATING.store(false, Ordering::Relaxed);
    });
}

fn show(
    _app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    target_x: f64,
    start_y: f64,
    target_y: f64,
) {
    ANIMATING.store(true, Ordering::Relaxed);
    let _ = window.set_always_on_top(false);
    let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition {
        x: target_x, y: start_y,
    }));
    let _ = window.show();
    let win = window.clone();
    std::thread::spawn(move || {
        animate_open(&win, target_x, start_y, target_y);
        ANIMATING.store(false, Ordering::Relaxed);
    });
}

fn create(app: &tauri::AppHandle, target_x: f64, target_y: f64) {
    if let Ok(win) = tauri::WebviewWindowBuilder::new(
        app, "popup", tauri::WebviewUrl::App("popup.html".into()),
    )
    .title("外设信息")
    .inner_size(POPUP_W, POPUP_H)
    .decorations(false)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .position(target_x, target_y)
    .build() {
        #[cfg(target_os = "windows")]
        if let Ok(hwnd) = win.hwnd() {
            windows::set_rounded_corners(hwnd.0 as isize);
        }
        let _ = win.show();
        let _ = win.set_focus();
        if let Some(pos) = POPUP_POS.get() {
            *pos.lock().unwrap() = (target_x, target_y);
        }
    }
}

fn animate_open(window: &tauri::WebviewWindow, x: f64, start_y: f64, end_y: f64) {
    let duration_ms = 250u64;
    let frames = 20;
    let step_ms = duration_ms / frames;
    for i in 0..=frames {
        let t = i as f64 / frames as f64;
        let eased = cubic_bezier(t);
        let y = start_y + (end_y - start_y) * eased;
        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        std::thread::sleep(std::time::Duration::from_millis(step_ms));
    }
    if let Some(pos) = POPUP_POS.get() {
        *pos.lock().unwrap() = (x, end_y);
    }
    let _ = window.set_always_on_top(true);
    let _ = window.set_focus();
}

fn animate_close(window: &tauri::WebviewWindow, x: f64, start_y: f64, end_y: f64) {
    let duration_ms = 200u64;
    let frames = 16;
    let step_ms = duration_ms / frames;
    for i in 0..=frames {
        let t = i as f64 / frames as f64;
        let eased = cubic_bezier(t);
        let y = start_y + (end_y - start_y) * eased;
        let _ = window.set_position(tauri::Position::Logical(tauri::LogicalPosition { x, y }));
        std::thread::sleep(std::time::Duration::from_millis(step_ms));
    }
    let _ = window.hide();
}
