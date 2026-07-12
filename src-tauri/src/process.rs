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
    let mut cmd = new_hidden_cmd("powershell");
    cmd.args(["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script]);
    cmd.args(args);
    let output = cmd.output().map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

    if !output.status.success() || !stderr.is_empty() {
        eprintln!(
            "[bt] PowerShell exited={:?} script={} args={:?}\n  stdout: {}\n  stderr: {}",
            output.status, script, args, stdout, stderr
        );
    }

    if !output.status.success() {
        return Err(format!("Script failed (exit {}): {}", output.status.code().unwrap_or(-1), stderr));
    }

    Ok(stdout)
}
