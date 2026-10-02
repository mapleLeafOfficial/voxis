// 开机自启：写/删 ~/.config/autostart/voxis.desktop（XDG 自启标准，GNOME 原生支持）。
use std::path::PathBuf;

fn desktop_path() -> Option<PathBuf> {
    let config_home = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(config_home.join("autostart").join("voxis.desktop"))
}

const DESKTOP_CONTENT: &str = r#"[Desktop Entry]
Type=Application
Name=Voxis
Comment=全局语音输入
Exec=voxis --minimized
Icon=voxis
Terminal=false
X-GNOME-Autostart-enabled=true
Categories=Utility;
"#;

/// 当前是否已启用自启
pub fn is_enabled() -> bool {
    desktop_path().map(|p| p.exists()).unwrap_or(false)
}

/// 启用/禁用自启。Exec 路径在 todo10 打包后统一校正（当前占位 PATH 查找）。
pub fn set_enabled(enable: bool) -> Result<(), String> {
    let path = desktop_path().ok_or("无法定位 autostart 目录（缺 HOME/XDG_CONFIG_HOME）")?;
    if !enable {
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| format!("删除自启项失败: {e}"))?;
            tracing::info!("[autostart] 已禁用开机自启");
        }
        return Ok(());
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建 autostart 目录失败: {e}"))?;
    }
    std::fs::write(&path, DESKTOP_CONTENT).map_err(|e| format!("写入自启项失败: {e}"))?;
    tracing::info!("[autostart] 已启用开机自启（{:?}）", path);
    Ok(())
}
