// 热键运行权限自检：input 组（读 /dev/input）、uinput 组（上屏注入用）、ydotoold 存活。
// Windows 无权限概念：直接全就绪。
// 结果只报告不阻塞（ydotoold 在注入通道就绪前缺失属预期）。
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PermissionStatus {
    /// 可读至少一个 /dev/input/event*（input 组或等价权限）
    pub input_ok: bool,
    /// /dev/uinput 可写（uinput 组，todo7 文字注入需要）
    pub uinput_ok: bool,
    /// ydotoold socket 存在（todo7 前可为 false，仅报告）
    pub ydotoold_ok: bool,
    /// 人类可读的问题摘要（空 = 全部满足）
    pub problems: Vec<String>,
    /// 运行平台（"linux"/"windows"，前端按平台渲染面板）
    pub platform: String,
}

/// Windows：无权限概念，全部就绪
#[cfg(target_os = "windows")]
pub fn check() -> PermissionStatus {
    PermissionStatus {
        input_ok: true,
        uinput_ok: true,
        ydotoold_ok: true,
        problems: Vec::new(),
        platform: std::env::consts::OS.into(),
    }
}

/// 执行权限检查（Linux）
#[cfg(target_os = "linux")]
pub fn check() -> PermissionStatus {
    let input_ok = has_input_access();
    let uinput_ok = has_uinput_access();
    let ydotoold_ok = has_ydotoold();

    let mut problems = Vec::new();
    if !input_ok {
        problems.push("无 /dev/input 读权限：请运行 sudo usermod -aG input guxing 后重新登录".into());
    }
    if !uinput_ok {
        problems.push("无 /dev/uinput 写权限：请运行 sudo usermod -aG uinput guxing 后重新登录（todo7 文字上屏需要）".into());
    }
    if !ydotoold_ok {
        problems.push("ydotoold 未运行：todo7 文字上屏将不可用（当前可忽略）".into());
    }
    PermissionStatus { input_ok, uinput_ok, ydotoold_ok, problems, platform: std::env::consts::OS.into() }
}

#[cfg(target_os = "linux")]
fn has_input_access() -> bool {
    let Ok(entries) = std::fs::read_dir("/dev/input") else {
        return false;
    };
    // 能成功打开任一 event* 即满足
    entries.flatten().any(|e| {
        e.path()
            .file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("event"))
            .unwrap_or(false)
            && std::fs::File::open(e.path()).is_ok()
    })
}

#[cfg(target_os = "linux")]
fn has_uinput_access() -> bool {
    std::fs::OpenOptions::new()
        .write(true)
        .open("/dev/uinput")
        .is_ok()
}

#[cfg(target_os = "linux")]
fn has_ydotoold() -> bool {
    let uid = std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find(|l| l.starts_with("Uid:")).and_then(|l| {
                l.split_whitespace().nth(1).and_then(|v| v.parse::<u32>().ok())
            })
        })
        .unwrap_or(1000);
    std::path::Path::new(&format!("/run/user/{uid}/.ydotool_socket")).exists()
}
