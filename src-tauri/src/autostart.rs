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

/// Windows：HKCU\...\Run 注册表自启
#[cfg(target_os = "windows")]
mod platform {
    use windows::core::w;
    use windows::Win32::Foundation::{ERROR_NO_MORE_ITEMS, ERROR_SUCCESS, HANDLE};
    use windows::Win32::System::Registry::{
        RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
        HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SAM_FLAGS, REG_SZ,
        REG_VALUE_TYPE,
    };

    const RUN_KEY: windows::core::PCWSTR =
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
    const VALUE: windows::core::PCWSTR = w!("voxis");

    fn open(access: REG_SAM_FLAGS) -> Result<HKEY, String> {
        let mut hk = HKEY::default();
        let r = unsafe { RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, Some(0), access, &mut hk) };
        if r != ERROR_SUCCESS {
            return Err(format!("打开注册表 Run 键失败: {r:?}"));
        }
        Ok(hk)
    }

    pub fn is_enabled() -> bool {
        let Ok(hk) = open(KEY_QUERY_VALUE) else { return false };
        let mut ty = REG_VALUE_TYPE::default();
        let mut len = 0u32;
        let r = unsafe { RegQueryValueExW(hk, VALUE, None, Some(&mut ty), None, Some(&mut len)) };
        unsafe { let _ = RegCloseKey(hk); }
        r == ERROR_SUCCESS
    }

    pub fn set_enabled(enable: bool) -> Result<(), String> {
        if !enable {
            let hk = open(KEY_SET_VALUE)?;
            let r = unsafe { RegDeleteValueW(hk, VALUE) };
            unsafe { let _ = RegCloseKey(hk); }
            if r == ERROR_SUCCESS || r == ERROR_NO_MORE_ITEMS {
                tracing::info!("[autostart] 已禁用开机自启（注册表）");
                return Ok(());
            }
            return Err(format!("删除自启值失败: {r:?}"));
        }
        let exe = std::env::current_exe()
            .map(|p| format!("{} --minimized", p.display()))
            .unwrap_or_else(|_| "voxis --minimized".into());
        let wide: Vec<u16> = exe.encode_utf16().chain(std::iter::once(0)).collect();
        let hk = open(KEY_SET_VALUE)?;
        let r = unsafe {
            RegSetValueExW(
                hk,
                VALUE,
                Some(0),
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    wide.as_ptr().cast(),
                    wide.len() * 2,
                )),
            )
        };
        unsafe { let _ = RegCloseKey(hk); }
        if r == ERROR_SUCCESS {
            tracing::info!("[autostart] 已启用开机自启（注册表）：{exe}");
            Ok(())
        } else {
            Err(format!("写入自启值失败: {r:?}"))
        }
    }

    // HANDLE 类型占位避免 unused import 警告（windows crate 重导出链）
    const _: HANDLE = HANDLE(std::ptr::null_mut());
}

#[cfg(target_os = "windows")]
pub use platform::{is_enabled, set_enabled};

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
