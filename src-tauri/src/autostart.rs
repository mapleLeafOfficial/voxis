// 开机自启。
// Linux：写/删 ~/.config/autostart/voxis.desktop（XDG 自启标准，GNOME 原生支持）。
// Windows：注册表 HKCU\...\Run（todo4 实现，当前桩）。
#[cfg(target_os = "linux")]
use std::path::PathBuf;

#[cfg(target_os = "linux")]
fn desktop_path() -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(config_home.join("autostart").join("voxis.desktop"))
}

/// Windows：注册表 Run 键（todo4 实现）
#[cfg(target_os = "windows")]
pub fn is_enabled() -> bool {
    tracing::debug!("[autostart] windows 桩：todo4 实现注册表查询");
    false
}

/// Windows：注册表 Run 键写入（todo4 实现）
#[cfg(target_os = "windows")]
pub fn set_enabled(enable: bool) -> Result<(), String> {
    let _ = enable;
    tracing::debug!("[autostart] windows 桩：todo4 实现注册表写入");
    Err("windows 自启未实现（todo4 注册表 Run 键）".into())
}

/// 当前是否已启用自启（Linux）
#[cfg(target_os = "linux")]
pub fn is_enabled() -> bool {
    desktop_path().map(|p| p.exists()).unwrap_or(false)
}

/// 启用/禁用自启（Linux）。Exec 用当前二进制实际路径 + --minimized（仅托盘）。
#[cfg(target_os = "linux")]
pub fn set_enabled(enable: bool) -> Result<(), String> {
    let path = desktop_path().ok_or("无法定位 autostart 目录（缺 HOME/XDG_CONFIG_HOME）")?;
    if !enable {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("删除自启项失败: {e}"))?;
            tracing::info!("[autostart] 已禁用开机自启");
        }
        return Ok(());
    }
    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "/usr/bin/voxis".into());
    let content = format!(
        "[Desktop Entry]\nType=Application\nName=Voxis\nComment=全局语音输入\nExec={} --minimized\nIcon=voxis\nTerminal=false\nX-GNOME-Autostart-enabled=true\nCategories=Utility;\n",
        shell_quote(&exe)
    );
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建 autostart 目录失败: {e}"))?;
    }
    std::fs::write(&path, content).map_err(|e| format!("写入自启项失败: {e}"))?;
    tracing::info!("[autostart] 已启用开机自启（{:?}）", path);
    Ok(())
}

/// desktop Exec 的简单引号包裹（路径含空格时）
#[cfg(target_os = "linux")]
fn shell_quote(s: &str) -> String {
    if s.contains(' ') {
        format!("\"{}\"", s)
    } else {
        s.to_string()
    }
}
