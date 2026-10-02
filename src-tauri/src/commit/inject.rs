// 键盘注入：模拟 Ctrl+V 粘贴到当前焦点/光标。
// Linux：Wayland 主路径 ydotool（uinput 内核注入，合成器路由到焦点应用）；X11 回退 xdotool。
// Windows：SendInput（todo3 实现，当前桩）。
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "linux")]
use std::process::Command;
use std::time::Duration;

/// 注入前静默等待：给合成器/应用同步剪贴板的窗口
const PASTE_QUIET: Duration = Duration::from_millis(150);

/// Windows：SendInput Ctrl+V（todo3 实现真身，当前桩）
#[cfg(target_os = "windows")]
pub fn inject_paste() -> Result<(), String> {
    std::thread::sleep(PASTE_QUIET);
    Err("windows 注入通道未实现（todo3 SendInput）".into())
}

/// 尝试注入粘贴（Linux）。Ok(()) = 已注入；Err(原因) = 不可用/失败（调用方回退 copied）。
#[cfg(target_os = "linux")]
pub fn inject_paste() -> Result<(), String> {
    // 注入前静默（剪贴板同步窗口）
    std::thread::sleep(PASTE_QUIET);

    // 1) ydotool：daemon socket 存在才尝试
    if let Some(sock) = ydotoold_socket() {
        // 键码：29=左Ctrl，47=V → Ctrl down, V down, V up, Ctrl up
        let out = Command::new("ydotool")
            .env("YDOTOOL_SOCKET", &sock)
            .args(["key", "29:1", "47:1", "47:0", "29:0"])
            .output()
            .map_err(|e| format!("ydotool 启动失败: {e}"))?;
        if out.status.success() {
            tracing::debug!("[commit] 注入：ydotool Ctrl+V");
            return Ok(());
        }
        return Err(format!(
            "ydotool 退出码非 0: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }

    // 2) X11 回退：xdotool（仅 DISPLAY 存在时）
    if std::env::var_os("DISPLAY").is_some() && which_exists("xdotool") {
        let out = Command::new("xdotool")
            .args(["key", "--clearmodifiers", "ctrl+v"])
            .output()
            .map_err(|e| format!("xdotool 启动失败: {e}"))?;
        if out.status.success() {
            tracing::debug!("[commit] 注入：xdotool Ctrl+V");
            return Ok(());
        }
        return Err("xdotool 退出码非 0".into());
    }

    Err("无可用注入通道（ydotoold 未运行且无 X11）".into())
}

/// 探测 ydotoold socket：YDOTOOL_SOCKET env → /run/user/<uid>/.ydotool_socket → /run/ydotoold/socket
#[cfg(target_os = "linux")]
fn ydotoold_socket() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("YDOTOOL_SOCKET") {
        let path = PathBuf::from(p);
        if path.exists() {
            return Some(path);
        }
    }
    let uid = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("Uid:"))
                .and_then(|l| l.split_whitespace().nth(1).and_then(|v| v.parse::<u32>().ok()))
        })
        .unwrap_or(1000);
    let candidates = [
        PathBuf::from(format!("/run/user/{uid}/.ydotool_socket")),
        PathBuf::from("/run/ydotoold/socket"),
    ];
    candidates.into_iter().find(|p| p.exists())
}

#[cfg(target_os = "linux")]
fn which_exists(prog: &str) -> bool {
    Command::new("which")
        .arg(prog)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
