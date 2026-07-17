use serde::{Deserialize, Serialize};
use std::ptr;
use std::sync::{Arc, Mutex};
use windows::core::*;
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::Endpoints::*;
use windows::Win32::Media::Audio::*;
use windows::Win32::System::Com::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioDevice { pub id: String, pub name: String, pub volume: f32, pub is_muted: bool, pub is_default: bool }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeChangeEvent { pub device_id: String, pub volume: f32, pub is_muted: bool }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSession { pub id: String, pub name: String, pub icon: String, pub pid: u32, pub volume: f32, pub is_muted: bool, pub device_id: String, #[serde(default)] pub is_active: bool }

static VOLUME_STATE: once_cell::sync::Lazy<Arc<Mutex<Vec<VolumeState>>>> = once_cell::sync::Lazy::new(|| Arc::new(Mutex::new(Vec::new())));
struct VolumeState { device_id: String, volume: f32, is_muted: bool }

// ========== 底层 COM 辅助 ==========

unsafe fn get_default_render_device() -> Result<IMMDevice> {
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
    enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia)
}

unsafe fn get_session_manager(device: &IMMDevice) -> Result<IAudioSessionManager2> {
    device.Activate(CLSCTX_ALL, None)
}

// ========== 设备音量 ==========

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
                                                changes.push(VolumeChangeEvent { device_id: id_str.clone(), volume, is_muted });
                                                if let Some(s) = state.iter_mut().find(|s| s.device_id == id_str) { s.volume = volume; s.is_muted = is_muted; }
                                            }
                                        } else { state.push(VolumeState { device_id: id_str.clone(), volume, is_muted }); }
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
                    devices.push(AudioDevice { id: id_str, name, volume, is_muted, is_default: false });
                }
            }
        }
        if let Ok(mut state) = VOLUME_STATE.lock() {
            *state = devices.iter().map(|d| VolumeState { device_id: d.id.clone(), volume: d.volume, is_muted: d.is_muted }).collect();
        }
        Ok(devices)
    }
}

unsafe fn get_device_volume_state(device: &IMMDevice) -> Result<(f32, bool)> {
    let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
    let mute = endpoint.GetMute()?;
    Ok((endpoint.GetMasterVolumeLevelScalar()?, mute.as_bool()))
}

unsafe fn get_device_name(device: &IMMDevice) -> Result<String> {
    let store = device.OpenPropertyStore(STGM(0))?;
    let key = PROPERTYKEY { fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0), pid: 14 };
    let value = unsafe { store.GetValue(&key as *const _) }?;
    let name = format!("{}", value).trim().to_string();
    if name.is_empty() {
        let key_desc = PROPERTYKEY { fmtid: GUID::from_u128(0xa45c254e_df1c_4efd_8020_67d146a850e0), pid: 2 };
        let value_desc = unsafe { store.GetValue(&key_desc as *const _) }?;
        let name_desc = format!("{}", value_desc).trim().to_string();
        if !name_desc.is_empty() { return Ok(name_desc); }
        return Ok("Unknown Audio Device".to_string());
    }
    Ok(name)
}

pub fn set_device_volume(device_id: &str, volume: f32) -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDevice(&HSTRING::from(device_id))?;
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        endpoint.SetMasterVolumeLevelScalar(volume.max(0.0).min(1.0), ptr::null())?;
    }
    if let Ok(mut state) = VOLUME_STATE.lock() {
        if let Some(s) = state.iter_mut().find(|s| s.device_id == device_id) {
            s.volume = volume;
        }
    }
    Ok(())
}

pub fn toggle_device_mute(device_id: &str) -> Result<()> {
    let new_muted;
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let device = enumerator.GetDevice(&HSTRING::from(device_id))?;
        let endpoint: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None)?;
        let current = endpoint.GetMute()?;
        new_muted = !current.as_bool();
        endpoint.SetMute(new_muted, ptr::null())?;
    }
    if let Ok(mut state) = VOLUME_STATE.lock() {
        if let Some(s) = state.iter_mut().find(|s| s.device_id == device_id) {
            s.is_muted = new_muted;
        }
    }
    Ok(())
}

pub fn check_volume_changes() -> Vec<VolumeChangeEvent> { check_volume_changes_internal() }

// ========== 应用音量（参考 volume-controller 方式） ==========

pub fn enumerate_audio_sessions(_device_id: &str) -> Result<Vec<AudioSession>> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let mut all_sessions: Vec<AudioSession> = Vec::new();
        let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        let device_count = collection.GetCount()?;
        for di in 0..device_count {
            if let Ok(device) = collection.Item(di) {
                let dev_id = device.GetId().map(|id| id.to_string().unwrap_or_default()).unwrap_or_default();
                let dev_name = get_device_name(&device).unwrap_or_default();
                let session_manager: IAudioSessionManager2 = match device.Activate(CLSCTX_ALL, None) { Ok(m) => m, Err(_) => continue };
                let session_enumerator = match session_manager.GetSessionEnumerator() { Ok(e) => e, Err(_) => continue };
                let count = session_enumerator.GetCount().unwrap_or(0);
                for i in 0..count {
                    if let Ok(session_control) = session_enumerator.GetSession(i) {
                        let session_control2: IAudioSessionControl2 = match session_control.cast() { Ok(s) => s, Err(_) => continue };
                        let state = session_control2.GetState().unwrap_or(AudioSessionState(0));
                        if state.0 > 2 { continue; }
                        let pid = session_control2.GetProcessId().unwrap_or(0);
                        if pid == 0 { continue; }
                        let session_id = get_session_id(&session_control2).unwrap_or_default();
                        let (volume, is_muted) = if let Ok(sv) = session_control.cast::<ISimpleAudioVolume>() {
                            let vol = sv.GetMasterVolume().unwrap_or(0.0);
                            let muted = sv.GetMute().map(|b| b.as_bool()).unwrap_or(false);
                            (vol, muted)
                        } else {
                            (0.0, false)
                        };
                        let session_name = get_session_display_name(&session_control).unwrap_or_default();
                        let display_name = if !session_name.is_empty() && session_name != "Unknown App" { session_name } else { format!("App (PID: {})", pid) };
                        let icon = crate::app_icon::get_app_icon_by_pid(pid).unwrap_or_default();
                        let is_active = state.0 == 1;
                        let audio_session = AudioSession { id: session_id, name: display_name, icon, pid, volume, is_muted, device_id: dev_id.clone(), is_active };
                        if let Some(existing) = all_sessions.iter_mut().find(|s| s.pid == pid) {
                            if is_active && !existing.is_active {
                                *existing = audio_session;
                            }
                        } else {
                            all_sessions.push(audio_session);
                        }
                    }
                }
            }
        }
        Ok(all_sessions)
    }
}

pub fn set_session_volume(session_id: &str, volume: f32) -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        for di in 0..collection.GetCount().unwrap_or(0) {
            if let Ok(device) = collection.Item(di) {
                let sm: IAudioSessionManager2 = match device.Activate(CLSCTX_ALL, None) { Ok(m) => m, Err(_) => continue };
                let se = match sm.GetSessionEnumerator() { Ok(e) => e, Err(_) => continue };
                for i in 0..se.GetCount().unwrap_or(0) {
                    if let Ok(sc) = se.GetSession(i) {
                        let sc2: IAudioSessionControl2 = match sc.cast() { Ok(s) => s, Err(_) => continue };
                        if get_session_id(&sc2).unwrap_or_default() == session_id {
                            if let Ok(sv) = sc.cast::<ISimpleAudioVolume>() {
                                sv.SetMasterVolume(volume.max(0.0).min(1.0), ptr::null())?;
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

pub fn toggle_session_mute(session_id: &str) -> Result<()> {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
        let collection = enumerator.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)?;
        for di in 0..collection.GetCount().unwrap_or(0) {
            if let Ok(device) = collection.Item(di) {
                let sm: IAudioSessionManager2 = match device.Activate(CLSCTX_ALL, None) { Ok(m) => m, Err(_) => continue };
                let se = match sm.GetSessionEnumerator() { Ok(e) => e, Err(_) => continue };
                for i in 0..se.GetCount().unwrap_or(0) {
                    if let Ok(sc) = se.GetSession(i) {
                        let sc2: IAudioSessionControl2 = match sc.cast() { Ok(s) => s, Err(_) => continue };
                        if get_session_id(&sc2).unwrap_or_default() == session_id {
                            if let Ok(sv) = sc.cast::<ISimpleAudioVolume>() {
                                let current = sv.GetMute()?;
                                sv.SetMute(!current.as_bool(), ptr::null())?;
                                return Ok(());
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

unsafe fn get_session_display_name(session: &IAudioSessionControl) -> Result<String> {
    let display_name = session.GetDisplayName()?;
    if display_name.is_empty() { return Ok("Unknown App".to_string()); }
    Ok(display_name.to_string()?)
}

unsafe fn get_session_id(session: &IAudioSessionControl2) -> Result<String> {
    let id = session.GetSessionInstanceIdentifier()?;
    Ok(id.to_string()?)
}
