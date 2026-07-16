use serde::{Deserialize, Serialize};
use std::ptr;
use std::sync::{Arc, Mutex};
use std::thread;
use tauri::Emitter;
use tokio::sync::mpsc;
use windows::core::*;
use windows::Win32::Foundation::BOOL;
use windows::Win32::Media::Audio::Endpoints::*;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;
use windows::Win32::UI::Shell::PropertiesSystem::*;

/// Audio device information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub volume: f32,
    pub is_muted: bool,
    pub is_default: bool,
}

/// Volume change event for Tauri
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeChangeEvent {
    pub device_id: String,
    pub volume: f32,
    pub is_muted: bool,
}

/// Audio session (application playing sound)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSession {
    pub id: String,
    pub name: String,
    pub pid: u32,
    pub volume: f32,
    pub is_muted: bool,
    pub device_id: String,
}

/// Global state for volume monitoring
static VOLUME_STATE: once_cell::sync::Lazy<Arc<Mutex<Vec<VolumeState>>>> =
    once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(Vec::new())));

struct VolumeState {
    device_id: String,
    volume: f32,
    is_muted: bool,
}

/// Start background thread to monitor volume changes
/// Uses channel + tokio task to properly emit events
pub fn start_volume_watcher(app_handle: tauri::AppHandle) {
    // Initialize volume state first
    let _ = enumerate_output_devices();

    // Create channel for communication
    let (tx, mut rx) = mpsc::channel::<Vec<VolumeChangeEvent>>(32);

    // Spawn tokio task to receive and emit events
    tauri::async_runtime::spawn(async move {
        while let Some(changes) = rx.recv().await {
            if !changes.is_empty() {
                let _ = app_handle.emit("volume-changed", &changes);
            }
        }
    });

    // Spawn background thread to monitor volume
    thread::spawn(move || {
        // Initialize COM for this thread
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok();
        }

        loop {
            thread::sleep(std::time::Duration::from_millis(200));

            let changes = check_volume_changes_internal();

            if !changes.is_empty() {
                // Send to tokio task via channel
                if tx.blocking_send(changes).is_err() {
                    break; // Channel closed, exit thread
                }
            }
        }
    });
}

/// Check for volume changes (internal version without COM init)
fn check_volume_changes_internal() -> Vec<VolumeChangeEvent> {
    let mut changes = Vec::new();

    unsafe {
        if let Ok(enumerator) = CoCreateInstance::<_, IMMDeviceEnumerator>(&MMDeviceEnumerator, None, CLSCTX_ALL) {
            if let Ok(collection) = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) {
                if let Ok(count) = collection.GetCount() {
                    if let Ok(mut state) = VOLUME_STATE.lock() {
                        for i in 0..count {
                            if let Ok(device) = collection.Item(i) {
                                if let Ok(id) = device.GetId() {
                                    let id_str = id.to_string().unwrap_or_default();
                                    if let Ok((volume, is_muted)) = get_device_volume_state(&device) {
                                        let existing = state.iter().find(|s| s.device_id == id_str);
                                        if let Some(old) = existing {
                                            if (old.volume - volume).abs() > 0.001 || old.is_muted != is_muted {
                                                changes.push(VolumeChangeEvent {
                                                    device_id: id_str.clone(),
                                                    volume,
                                                    is_muted,
                                                });
                                                if let Some(s) = state.iter_mut().find(|s| s.device_id == id_str) {
                                                    s.volume = volume;
                                                    s.is_muted = is_muted;
                                                }
                                            }
                                        } else {
                                            state.push(VolumeState {
                                                device_id: id_str.clone(),
                                                volume,
                                                is_muted,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    changes
}

/// Enumerate audio output devices
pub fn enumerate_output_devices() -> Result<Vec<AudioDevice>> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        let count = collection.GetCount()?;

        let mut devices = Vec::new();

        for i in 0..count {
            if let Ok(device) = collection.Item(i) {
                if let Ok(id) = device.GetId() {
                    let id_str = id.to_string()?;
                    let name = get_device_name(&device).unwrap_or_else(|_| "Unknown Device".to_string());
                    let (volume, is_muted) = get_device_volume_state(&device).unwrap_or((0.0, false));

                    devices.push(AudioDevice {
                        id: id_str.clone(),
                        name,
                        volume,
                        is_muted,
                        is_default: false,
                    });
                }
            }
        }

        // Update volume state for monitoring
        if let Ok(mut state) = VOLUME_STATE.lock() {
            *state = devices.iter().map(|d| VolumeState {
                device_id: d.id.clone(),
                volume: d.volume,
                is_muted: d.is_muted,
            }).collect();
        }

        Ok(devices)
    }
}

/// Get device volume and mute state
unsafe fn get_device_volume_state(device: &IMMDevice) -> Result<(f32, bool)> {
    let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
    let volume = endpoint.GetMasterVolumeLevelScalar()?;
    let mute = endpoint.GetMute()?;
    Ok((volume, mute.as_bool()))
}

/// Get device friendly name using IPropertyStore
unsafe fn get_device_name(device: &IMMDevice) -> Result<String> {
    let store = device.OpenPropertyStore(STGM(0))?;
    let key = PROPERTYKEY {
        fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
        pid: 14,
    };
    let value = store.GetValue(&key)?;
    let name = format!("{}", value);
    let name = name.trim().to_string();

    if name.is_empty() {
        let key_desc = PROPERTYKEY {
            fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0),
            pid: 2,
        };
        let value_desc = store.GetValue(&key_desc)?;
        let name_desc = format!("{}", value_desc);
        let name_desc = name_desc.trim().to_string();
        if !name_desc.is_empty() {
            return Ok(name_desc);
        }
        return Ok("Unknown Audio Device".to_string());
    }

    Ok(name)
}

/// Set device volume (0.0 to 1.0)
pub fn set_device_volume(device_id: &str, volume: f32) -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device_id_wide = HSTRING::from(device_id);
        let device = enumerator.GetDevice(&device_id_wide)?;
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        let clamped_volume = volume.max(0.0).min(1.0);
        endpoint.SetMasterVolumeLevelScalar(clamped_volume, ptr::null())?;
    }
    Ok(())
}

/// Toggle device mute state
pub fn toggle_device_mute(device_id: &str) -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device_id_wide = HSTRING::from(device_id);
        let device = enumerator.GetDevice(&device_id_wide)?;
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        let current = endpoint.GetMute()?;
        let new_mute = BOOL::from(!current.as_bool());
        endpoint.SetMute(new_mute, ptr::null())?;
    }
    Ok(())
}

/// Check for volume changes (for manual polling if needed)
pub fn check_volume_changes() -> Vec<VolumeChangeEvent> {
    check_volume_changes_internal()
}

/// Enumerate audio sessions for a device
pub fn enumerate_audio_sessions(device_id: &str) -> Result<Vec<AudioSession>> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device_id_wide = HSTRING::from(device_id);
        let device = enumerator.GetDevice(&device_id_wide)?;

        let session_manager: IAudioSessionManager2 = device.Activate(CLSCTX_ALL, None)?;
        let session_enumerator = session_manager.GetSessionEnumerator()?;
        let count = session_enumerator.GetCount()?;

        let mut sessions = Vec::new();

        for i in 0..count {
            if let Ok(session_control) = session_enumerator.GetSession(i) {
                let session_control2: IAudioSessionControl2 = session_control.cast()?;
                let state = session_control2.GetState()?;
                if state.0 != 1 {
                    continue;
                }

                let pid = session_control2.GetProcessId()?;
                let display_name = get_session_display_name(&session_control).unwrap_or_else(|_| "Unknown".to_string());
                let session_id = get_session_id(&session_control2).unwrap_or_default();

                let simple_volume: ISimpleAudioVolume = session_control.cast()?;
                let volume = simple_volume.GetMasterVolume()?;
                let is_muted = simple_volume.GetMute()? == BOOL::from(true);

                sessions.push(AudioSession {
                    id: session_id,
                    name: display_name,
                    pid,
                    volume,
                    is_muted,
                    device_id: device_id.to_string(),
                });
            }
        }

        Ok(sessions)
    }
}

/// Get session display name
unsafe fn get_session_display_name(session: &IAudioSessionControl) -> Result<String> {
    let display_name = session.GetDisplayName()?;
    if display_name.is_empty() {
        return Ok("Unknown App".to_string());
    }
    Ok(display_name.to_string()?)
}

/// Get session instance identifier
unsafe fn get_session_id(session: &IAudioSessionControl2) -> Result<String> {
    let id = session.GetSessionInstanceIdentifier()?;
    Ok(id.to_string()?)
}

/// Set session volume (simplified)
#[allow(dead_code)]
pub fn set_session_volume(_session_id: &str, _volume: f32) -> Result<()> {
    Ok(())
}

/// Toggle session mute (simplified)
#[allow(dead_code)]
pub fn toggle_session_mute(_session_id: &str) -> Result<()> {
    Ok(())
}
