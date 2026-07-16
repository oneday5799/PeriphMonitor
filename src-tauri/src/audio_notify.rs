use std::sync::{Arc, Mutex};
use std::time::Duration;
use tauri::Emitter;
use windows::Win32::System::Com::*;

/// 音频通知回调结构体
struct AudioNotifyCallback {
    app_handle: tauri::AppHandle,
    last_volume: Arc<Mutex<Vec<(String, f32, bool)>>>,
}

impl AudioNotifyCallback {
    fn new(app_handle: tauri::AppHandle) -> Self {
        Self {
            app_handle,
            last_volume: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn check_and_emit(&self) {
        if let Ok(devices) = crate::audio::enumerate_output_devices() {
            let current: Vec<(String, f32, bool)> = devices
                .iter()
                .map(|d| (d.id.clone(), d.volume, d.is_muted))
                .collect();

            let mut last = self.last_volume.lock().unwrap();
            let mut changed = false;

            // 检查是否有变化
            for (id, volume, is_muted) in &current {
                if let Some((_, old_vol, old_mute)) = last.iter().find(|(l, _, _)| l == id) {
                    if (old_vol - volume).abs() > 0.001 || *old_mute != *is_muted {
                        changed = true;
                        break;
                    }
                } else {
                    changed = true;
                    break;
                }
            }

            // 检查是否有设备被移除
            if last.len() != current.len() {
                changed = true;
            }

            if changed {
                let changes: Vec<crate::audio::VolumeChangeEvent> = devices
                    .iter()
                    .map(|d| crate::audio::VolumeChangeEvent {
                        device_id: d.id.clone(),
                        volume: d.volume,
                        is_muted: d.is_muted,
                    })
                    .collect();
                if !changes.is_empty() {
                    let _ = self.app_handle.emit("volume-changed", &changes);
                }
                *last = current;
            }
        }
    }
}

/// 初始化音频通知回调
pub fn init_audio_notify(app_handle: tauri::AppHandle) {
    let callback = Arc::new(AudioNotifyCallback::new(app_handle));

    // 初始化时获取一次当前状态
    if let Ok(devices) = crate::audio::enumerate_output_devices() {
        let current: Vec<(String, f32, bool)> = devices
            .iter()
            .map(|d| (d.id.clone(), d.volume, d.is_muted))
            .collect();
        *callback.last_volume.lock().unwrap() = current;
    }

    // 启动监听线程（低频率检查）
    let callback_clone = callback.clone();
    std::thread::spawn(move || {
        unsafe { let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok(); }
        loop {
            std::thread::sleep(Duration::from_millis(500));
            callback_clone.check_and_emit();
        }
    });

    // 保存引用
    std::mem::forget(callback);
}
