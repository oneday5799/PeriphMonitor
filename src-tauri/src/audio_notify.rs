use std::collections::HashMap;
use tauri::Emitter;
use windows::core::*;
use windows::Win32::Foundation::*;
use windows::Win32::Media::Audio::Endpoints::*;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_core::implement;

use crate::audio::VolumeChangeEvent;

const WM_TIMER_CHECK: u32 = 1;
const TIMER_INTERVAL_MS: u32 = 10_000;

// ── COM 回调实现 ──────────────────────────────────────────

#[implement(IAudioEndpointVolumeCallback)]
struct VolumeCallback {
    app_handle: tauri::AppHandle,
    device_id: String,
}

impl IAudioEndpointVolumeCallback_Impl for VolumeCallback_Impl {
    fn OnNotify(&self, pnotify: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> Result<()> {
        unsafe {
            if let Some(data) = pnotify.as_ref() {
                let _ = self.app_handle.emit(
                    "volume-changed",
                    vec![VolumeChangeEvent {
                        device_id: self.device_id.clone(),
                        volume: data.fMasterVolume,
                        is_muted: data.bMuted.as_bool(),
                    }],
                );
            }
        }
        Ok(())
    }
}

// ── 音频监控器 ───────────────────────────────────────────

struct AudioMonitor {
    enumerator: IMMDeviceEnumerator,
    callbacks: HashMap<String, (IAudioEndpointVolume, IAudioEndpointVolumeCallback)>,
    app_handle: tauri::AppHandle,
}

impl Drop for AudioMonitor {
    fn drop(&mut self) {
        for (_, (endpoint, callback)) in self.callbacks.drain() {
            unsafe {
                let _ = endpoint.UnregisterControlChangeNotify(&callback);
            }
        }
    }
}

impl AudioMonitor {
    fn new(app_handle: tauri::AppHandle) -> Result<Self> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
            Ok(Self {
                enumerator,
                callbacks: HashMap::new(),
                app_handle,
            })
        }
    }

    fn sync_callbacks(&mut self) {
        unsafe {
            let collection = match self
                .enumerator
                .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            {
                Ok(c) => c,
                Err(_) => return,
            };

            let count = collection.GetCount().unwrap_or(0);
            let mut current_ids = Vec::with_capacity(count as usize);

            for i in 0..count {
                if let Ok(device) = collection.Item(i) {
                    if let Ok(id) = device.GetId() {
                        let id_str = id.to_string().unwrap_or_default();
                        current_ids.push(id_str.clone());

                        if !self.callbacks.contains_key(&id_str) {
                            self.register_device(&device, &id_str);
                        }
                    }
                }
            }

            let to_remove: Vec<String> = self
                .callbacks
                .keys()
                .filter(|id| !current_ids.contains(id))
                .cloned()
                .collect();
            for id in to_remove {
                if let Some((endpoint, callback)) = self.callbacks.remove(&id) {
                    let _ = endpoint.UnregisterControlChangeNotify(&callback);
                }
            }

            let _ = self.app_handle.emit("audio-devices-changed", ());
        }
    }

    unsafe fn register_device(&mut self, device: &IMMDevice, id: &str) {
        let endpoint: IAudioEndpointVolume = match device.Activate(CLSCTX_ALL, None) {
            Ok(e) => e,
            Err(_) => return,
        };

        let callback: IAudioEndpointVolumeCallback = VolumeCallback {
            app_handle: self.app_handle.clone(),
            device_id: id.to_string(),
        }
        .into();

        if endpoint.RegisterControlChangeNotify(&callback).is_ok() {
            self.callbacks
                .insert(id.to_string(), (endpoint, callback));
        }
    }
}

// ── STA 线程 ─────────────────────────────────────────────

pub fn init_audio_notify(app_handle: tauri::AppHandle) {
    std::thread::spawn(move || unsafe {
        let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if hr.is_err() {
            crate::process::append_log("[audio_notify] CoInitializeEx failed");
            return;
        }

        let class_name: Vec<u16> = "AudioNotifyMsgWindow\0".encode_utf16().collect();
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(audio_msg_wnd_proc),
            hInstance: HINSTANCE(std::ptr::null_mut()),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..std::mem::zeroed()
        };
        RegisterClassExW(&wc);

        let hwnd = match CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            Some(HINSTANCE(std::ptr::null_mut())),
            None,
        ) {
            Ok(h) => h,
            Err(_) => {
                crate::process::append_log("[audio_notify] CreateWindowExW failed");
                return;
            }
        };

        let mut monitor = match AudioMonitor::new(app_handle) {
            Ok(m) => m,
            Err(e) => {
                crate::process::append_log(&format!(
                    "[audio_notify] AudioMonitor::new failed: {}",
                    e
                ));
                return;
            }
        };
        monitor.sync_callbacks();

        let monitor_ptr = &mut monitor as *mut AudioMonitor;
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, monitor_ptr as isize);

        let _ = SetTimer(Some(hwnd), 1, TIMER_INTERVAL_MS, None);

        crate::process::append_log("[audio_notify] STA thread started");

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, Some(hwnd), 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        crate::process::append_log("[audio_notify] STA thread stopped");
    });
}

extern "system" fn audio_msg_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe {
        match msg {
            WM_TIMER => {
                if wparam.0 as u32 == WM_TIMER_CHECK {
                    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                    if ptr != 0 {
                        let monitor = &mut *(ptr as *mut AudioMonitor);
                        monitor.sync_callbacks();
                    }
                }
                LRESULT(0)
            }
            WM_DESTROY => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if ptr != 0 {
                    drop(Box::from_raw(ptr as *mut AudioMonitor));
                    SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                }
                PostQuitMessage(0);
                LRESULT(0)
            }
            _ => DefWindowProcW(hwnd, msg, wparam, lparam),
        }
    }
}
