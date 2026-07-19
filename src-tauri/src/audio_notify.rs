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

const WM_SYNC_CALLBACKS: u32 = 0x0400;

// ── 音量回调实现 ──────────────────────────────────────────

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

// ── 设备通知回调（IMMNotificationClient）──────────────────

#[implement(IMMNotificationClient)]
struct DeviceNotification {
    hwnd: HWND,
}

impl IMMNotificationClient_Impl for DeviceNotification_Impl {
    fn OnDeviceStateChanged(&self, _pwstrdeviceid: &PCWSTR, _dwnewstate: DEVICE_STATE) -> Result<()> {
        unsafe { let _ = PostMessageW(Some(self.hwnd), WM_SYNC_CALLBACKS, WPARAM(0), LPARAM(0)); }
        Ok(())
    }

    fn OnDeviceAdded(&self, _pwstrdeviceid: &PCWSTR) -> Result<()> {
        unsafe { let _ = PostMessageW(Some(self.hwnd), WM_SYNC_CALLBACKS, WPARAM(0), LPARAM(0)); }
        Ok(())
    }

    fn OnDeviceRemoved(&self, _pwstrdeviceid: &PCWSTR) -> Result<()> {
        unsafe { let _ = PostMessageW(Some(self.hwnd), WM_SYNC_CALLBACKS, WPARAM(0), LPARAM(0)); }
        Ok(())
    }

    fn OnDefaultDeviceChanged(
        &self,
        _edflow: EDataFlow,
        _erender: ERole,
        _pwstrdefaultdeviceid: &PCWSTR,
    ) -> Result<()> {
        Ok(())
    }

    fn OnPropertyValueChanged(&self, _pwstrdeviceid: &PCWSTR, _key: &PROPERTYKEY) -> Result<()> {
        Ok(())
    }
}

// ── 音频监控器 ───────────────────────────────────────────

struct AudioMonitor {
    enumerator: IMMDeviceEnumerator,
    callbacks: HashMap<String, (IAudioEndpointVolume, IAudioEndpointVolumeCallback)>,
    notification: IMMNotificationClient,
    app_handle: tauri::AppHandle,
}

impl Drop for AudioMonitor {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .enumerator
                .UnregisterEndpointNotificationCallback(&self.notification);
            for (_, (endpoint, callback)) in self.callbacks.drain() {
                let _ = endpoint.UnregisterControlChangeNotify(&callback);
            }
        }
    }
}

impl AudioMonitor {
    fn new(hwnd: HWND, app_handle: tauri::AppHandle) -> Result<Self> {
        unsafe {
            let enumerator: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

            let notification: IMMNotificationClient = DeviceNotification { hwnd }.into();
            enumerator.RegisterEndpointNotificationCallback(&notification)?;

            Ok(Self {
                enumerator,
                callbacks: HashMap::new(),
                notification,
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
            WS_EX_TOOLWINDOW,
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            None,
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

        let mut monitor = match AudioMonitor::new(hwnd, app_handle) {
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

        let monitor_ptr = Box::leak(Box::new(monitor));
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, monitor_ptr as *mut AudioMonitor as isize);

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
            WM_SYNC_CALLBACKS => {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA);
                if ptr != 0 {
                    let monitor = &mut *(ptr as *mut AudioMonitor);
                    monitor.sync_callbacks();
                }
                LRESULT(0)
            }
            WM_ENDSESSION => {
                crate::process::append_log(&format!(
                    "[audio_notify] WM_ENDSESSION received, wparam={}", wparam.0
                ));
                if wparam.0 != 0 {
                    let (enabled, devices) = crate::config::with_config(|c| {
                        (c.shutdown_volume_enabled, c.shutdown_volume_devices.clone())
                    });
                    crate::process::append_log(&format!(
                        "[audio_notify] shutdown config: enabled={}, devices={:?}",
                        enabled, devices
                    ));
                    if enabled && !devices.is_empty() {
                        crate::process::append_log("[audio_notify] shutdown: adjusting volume");
                        crate::audio::set_shutdown_volumes(&devices);
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
