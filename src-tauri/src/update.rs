use serde::Serialize;
use std::ffi::c_void;
use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::Networking::WinHttp::*;

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub has_update: bool,
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
}

#[derive(Debug, serde::Deserialize)]
struct GitHubRelease {
    tag_name: String,
    prerelease: bool,
    draft: bool,
    html_url: String,
}

fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// WinHTTP GET request, returns response body as String
fn winhttp_get(host: &str, path: &str) -> Result<String, String> {
    let user_agent = to_wide("PeriphMonitor");
    let host_wide = to_wide(host);
    let path_wide = to_wide(path);
    let verb = to_wide("GET");

    unsafe {
        let session = WinHttpOpen(
            user_agent.as_ptr(),
            WINHTTP_ACCESS_TYPE_DEFAULT_PROXY,
            std::ptr::null(),
            std::ptr::null(),
            0,
        );
        if session.is_null() {
            return Err("网络连接失败".to_string());
        }

        let connect = WinHttpConnect(session, host_wide.as_ptr(), 443, 0);
        if connect.is_null() {
            WinHttpCloseHandle(session);
            return Err("无法连接到服务器".to_string());
        }

        let request = WinHttpOpenRequest(
            connect,
            verb.as_ptr(),
            path_wide.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            std::ptr::null(),
            WINHTTP_FLAG_SECURE,
        );
        if request.is_null() {
            WinHttpCloseHandle(connect);
            WinHttpCloseHandle(session);
            return Err("请求创建失败".to_string());
        }

        WinHttpSetTimeouts(request, 5000, 10000, 10000, 10000);

        if WinHttpSendRequest(request, std::ptr::null(), 0, std::ptr::null_mut(), 0, 0, 0) == 0 {
            let err = GetLastError();
            WinHttpCloseHandle(request);
            WinHttpCloseHandle(connect);
            WinHttpCloseHandle(session);
            return if err == 12007 {
                Err("DNS 解析失败".to_string())
            } else if err == 12002 || err == 12030 {
                Err("网络连接超时".to_string())
            } else {
                Err(format!("网络错误 ({})", err))
            };
        }

        if WinHttpReceiveResponse(request, std::ptr::null_mut()) == 0 {
            let err = GetLastError();
            WinHttpCloseHandle(request);
            WinHttpCloseHandle(connect);
            WinHttpCloseHandle(session);
            return if err == 12002 || err == 12030 {
                Err("网络连接超时".to_string())
            } else {
                Err(format!("网络错误 ({})", err))
            };
        }

        // Check HTTP status code
        let mut status_code: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let mut index: u32 = 0;
        WinHttpQueryHeaders(
            request,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            std::ptr::null(),
            &mut status_code as *mut u32 as *mut c_void,
            &mut size,
            &mut index,
        );
        match status_code {
            200 => {}
            403 => {
                WinHttpCloseHandle(request);
                WinHttpCloseHandle(connect);
                WinHttpCloseHandle(session);
                return Err("GitHub API 请求过于频繁，请稍后再试".to_string());
            }
            code => {
                WinHttpCloseHandle(request);
                WinHttpCloseHandle(connect);
                WinHttpCloseHandle(session);
                return Err(format!("GitHub 服务器错误 ({})", code));
            }
        }

        let mut body = Vec::new();
        let mut buffer = [0u8; 4096];
        let mut bytes_read: u32;

        loop {
            bytes_read = 0;
            if WinHttpReadData(request, buffer.as_mut_ptr() as *mut c_void, buffer.len() as u32, &mut bytes_read) == 0
                || bytes_read == 0
            {
                break;
            }
            body.extend_from_slice(&buffer[..bytes_read as usize]);
        }

        WinHttpCloseHandle(request);
        WinHttpCloseHandle(connect);
        WinHttpCloseHandle(session);

        String::from_utf8(body).map_err(|_| "响应编码错误".to_string())
    }
}

/// 比较版本号：返回 latest > current
fn compare_versions(current: &str, latest: &str) -> bool {
    fn parse_parts(v: &str) -> Vec<u32> {
        v.trim_start_matches('v')
            .split('.')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    }
    parse_parts(current) < parse_parts(latest)
}

/// 检测 GitHub 是否有新版本
pub fn check_for_update(
    current_version: &str,
    include_prerelease: bool,
) -> Result<UpdateInfo, String> {
    let body = winhttp_get("api.github.com", "/repos/oneday5799/PeriphMonitor/releases")?;

    let releases: Vec<GitHubRelease> =
        serde_json::from_str(&body).map_err(|_| "响应数据解析失败".to_string())?;

    let latest = releases.iter().find(|r| {
        if r.draft {
            return false;
        }
        if r.prerelease && !include_prerelease {
            return false;
        }
        true
    });

    match latest {
        Some(release) => {
            let latest_ver = release.tag_name.trim_start_matches('v');
            let has_update = compare_versions(current_version, latest_ver);
            Ok(UpdateInfo {
                has_update,
                current_version: current_version.to_string(),
                latest_version: latest_ver.to_string(),
                release_url: release.html_url.clone(),
            })
        }
        None => Ok(UpdateInfo {
            has_update: false,
            current_version: current_version.to_string(),
            latest_version: current_version.to_string(),
            release_url: String::new(),
        }),
    }
}
