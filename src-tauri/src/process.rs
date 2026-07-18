use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;
use std::process::Command;

/// 获取 exe 所在目录
pub fn exe_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."))
}

/// 创建 Windows 隐藏窗口命令
#[cfg(target_os = "windows")]
pub fn new_hidden_cmd(program: &str) -> Command {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x08000000;
    let mut cmd = Command::new(program);
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

#[cfg(not(target_os = "windows"))]
pub fn new_hidden_cmd(program: &str) -> Command {
    Command::new(program)
}

/// 获取日志文件路径
fn log_path() -> std::path::PathBuf {
    let retention = crate::config::with_config(|c| c.log_retention.clone());
    if retention == "once" {
        exe_dir().join(format!("debug_{}.log", std::process::id()))
    } else {
        exe_dir().join("debug.log")
    }
}

/// 追加日志到文件（标准级别）
pub fn append_log(msg: &str) {
    let (enabled, level) = crate::config::with_config(|c| (c.log_enabled, c.log_level.clone()));
    if !enabled || level != "standard" {
        return;
    }
    write_log(msg);
}

/// 追加日志到文件（详细级别）
pub fn append_log_detailed(msg: &str) {
    let (enabled, level) = crate::config::with_config(|c| (c.log_enabled, c.log_level.clone()));
    if !enabled || level != "detailed" {
        return;
    }
    write_log(msg);
}

fn write_log(msg: &str) {
    use std::io::Write;
    let timestamp = chrono_str();
    let line = format!("[{}]{}\n", timestamp, msg);
    let path = log_path();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open(&path)
    {
        let _ = file.write_all(line.as_bytes());
    }
}

/// 清理旧日志文件（根据保留时长设置）
pub fn clean_old_logs() {
    use std::time::{SystemTime, Duration};

    let (enabled, retention) = crate::config::with_config(|c| (c.log_enabled, c.log_retention.clone()));
    if !enabled {
        return;
    }

    let dir = exe_dir();
    let now = SystemTime::now();

    let max_age = match retention.as_str() {
        "once" => {
            // 一次模式：删除所有 debug*.log
            if let Ok(entries) = std::fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy();
                    if name_str.starts_with("debug") && name_str.ends_with(".log") {
                        let _ = std::fs::remove_file(entry.path());
                    }
                }
            }
            return;
        }
        "three_days" => Duration::from_secs(3 * 86400),
        "one_week" => Duration::from_secs(7 * 86400),
        "one_month" => Duration::from_secs(30 * 86400),
        _ => Duration::from_secs(86400), // one_day
    };

    // 非 once 模式：清理超过保留时长的 debug*.log 文件
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with("debug") && name_str.ends_with(".log") {
                if let Ok(meta) = entry.metadata() {
                    if let Ok(modified) = meta.modified() {
                        if let Ok(elapsed) = now.duration_since(modified) {
                            if elapsed > max_age {
                                let _ = std::fs::remove_file(entry.path());
                            }
                        }
                    }
                }
            }
        }
    }
}

fn chrono_str() -> String {
    use std::time::SystemTime;
    let Ok(dur) = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) else {
        return "??:??".into();
    };
    let secs = dur.as_secs();
    let h = (secs / 3600) % 24;
    let m = (secs / 60) % 60;
    let s = secs % 60;
    format!("{:02}:{:02}:{:02}", h, m, s)
}

/// 使用系统默认程序打开文件/URL
pub fn open_with_system(path: &str) -> Result<(), String> {
    let mut cmd = new_hidden_cmd("cmd");
    cmd.args(["/c", "start", "", path])
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// 将字符串转换为 Windows 宽字符串 (null-terminated UTF-16)
pub fn to_wide(s: &str) -> Vec<u16> {
    std::ffi::OsStr::new(s)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// 打开旧版声音控制面板 (mmsys.cpl)
pub fn open_sound_panel(panel: &str) {
    let arg = format!("shell32.dll,Control_RunDLL mmsys.cpl,,{}", panel);
    let wide_file = to_wide("rundll32.exe");
    let wide_arg = to_wide(&arg);
    let wide_verb = to_wide("open");
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            wide_verb.as_ptr(),
            wide_file.as_ptr(),
            wide_arg.as_ptr(),
            std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}

/// 打开现代 Windows 设置页面 (ms-settings:)
pub fn open_settings_page(page: &str) {
    let url = format!("ms-settings:{}", page);
    let wide_url = to_wide(&url);
    let wide_verb = to_wide("open");
    unsafe {
        windows_sys::Win32::UI::Shell::ShellExecuteW(
            std::ptr::null_mut(),
            wide_verb.as_ptr(),
            wide_url.as_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            windows_sys::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL,
        );
    }
}

/// 使用 PowerShell 执行脚本
pub fn run_powershell_script(script: &str, args: &[&str]) -> Result<String, String> {
    use std::process::Stdio;

    let mut cmd = new_hidden_cmd("powershell");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script]);
    cmd.args(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let arg_str = args.iter().map(|a| format!("\"{}\"", a)).collect::<Vec<_>>().join(" ");
    crate::process::append_log(&format!("[bt] SPAWN powershell -File {} {}", script, arg_str));

    let mut child = cmd.spawn().map_err(|e| {
        let msg = format!("[bt] SPAWN_FAILED: {}", e);
        crate::process::append_log(&msg);
        msg
    })?;

    let child_id = child.id();
    crate::process::append_log(&format!("[bt] CHILD_PID={}", child_id));

    // 超时 60 秒
    let timeout = std::time::Duration::from_secs(60);
    let start = std::time::Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = child.wait_with_output().map_err(|e| e.to_string())?;
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

                let log_msg = format!(
                    "[bt] SCRIPT_DONE exit={:?}\n  stdout: {}\n  stderr: {}",
                    status, stdout, stderr
                );
                crate::process::append_log(&log_msg);

                if !status.success() {
                    return Err(format!("Script failed (exit {}): {}", status.code().unwrap_or(-1), stderr));
                }

                return Ok(stdout);
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    let msg = format!("[bt] TIMEOUT after 60s, killed pid={}", child_id);
                    crate::process::append_log(&msg);
                    return Err(msg);
                }
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            Err(e) => {
                let msg = format!("[bt] TRY_WAIT_ERROR: {}", e);
                crate::process::append_log(&msg);
                return Err(e.to_string());
            }
        }
    }
}
