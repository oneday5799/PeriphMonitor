use std::process::Command;

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

/// 追加日志到 debug.log 文件
pub fn append_log(msg: &str) {
    use std::io::Write;
    let timestamp = chrono_str();
    let line = format!("[{}]{}\n", timestamp, msg);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true).append(true)
        .open("debug.log")
    {
        let _ = file.write_all(line.as_bytes());
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

    // 超时 30 秒
    let timeout = std::time::Duration::from_secs(30);
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
                    let msg = format!("[bt] TIMEOUT after 30s, killed pid={}", child_id);
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
