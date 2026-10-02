// 剪贴板写入：wl-clipboard-rs（Wayland 原生）→ arboard（跨平台回退）→ xclip/xsel（X11 命令兜底）。
// 粘贴前写、commit 后不清剪贴板（保留文本供手动粘贴）。
use arboard::Clipboard;

/// 写入系统剪贴板。返回 Err(原因) 表示全部通道失败。
pub fn copy(text: &str) -> Result<(), String> {
    // 1) Wayland 原生：fork 模式派生后台进程持有剪贴板（直到被覆盖），不阻塞
    #[cfg(target_os = "linux")]
    match copy_wayland(text) {
        Ok(()) => {
            tracing::debug!("[commit] 剪贴板写入：wl-clipboard-rs");
            return Ok(());
        }
        Err(e) => tracing::warn!("[commit] wl-clipboard-rs 失败: {e}，回退 arboard"),
    }

    // 2) arboard（X11 / 通用）
    match Clipboard::new().and_then(|mut c| c.set_text(text.to_string())) {
        Ok(()) => {
            tracing::debug!("[commit] 剪贴板写入：arboard");
            Ok(())
        }
        Err(e) => {
            tracing::warn!("[commit] arboard 失败: {e}，回退 xclip/xsel");
            copy_command(text)
        }
    }
}

/// 读取系统剪贴板文本（manual 整理用）。wl-clipboard-rs → arboard 回退。
pub fn paste_text() -> Result<String, String> {
    #[cfg(target_os = "linux")]
    match paste_wayland() {
        Ok(t) => return Ok(t),
        Err(e) => tracing::warn!("[commit] wl-clipboard 读失败: {e}，回退 arboard"),
    }
    Clipboard::new()
        .and_then(|mut c| c.get_text())
        .map(|s| s.to_string())
        .map_err(|e| format!("arboard 读剪贴板失败: {e}"))
}

#[cfg(target_os = "linux")]
fn paste_wayland() -> Result<String, String> {
    use std::io::Read;
    use wl_clipboard_rs::paste::{get_contents, ClipboardType, MimeType, Seat};
    let (mut reader, _mime) = get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Any)
        .map_err(|e| format!("wayland 读剪贴板失败: {e}"))?;
    let mut buf = String::new();
    reader.read_to_string(&mut buf).map_err(|e| format!("读取剪贴板数据失败: {e}"))?;
    Ok(buf)
}

#[cfg(target_os = "linux")]
fn copy_wayland(text: &str) -> Result<(), String> {
    use wl_clipboard_rs::copy::{ClipboardType, MimeType, Options, Source};
    // 默认后台服务 + Unlimited 请求：派生服务线程持有剪贴板，直到被其他应用覆盖
    let mut opts = Options::new();
    opts.clipboard(ClipboardType::Regular);
    opts.copy(Source::Bytes(text.as_bytes().into()), MimeType::Autodetect)
        .map_err(|e| e.to_string())
}

/// 命令行兜底：xclip → xsel
fn copy_command(text: &str) -> Result<(), String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    for (prog, args) in [("xclip", vec!["-selection", "clipboard"]), ("xsel", vec!["--clipboard", "--input"])] {
        if which_exists(prog) {
            let child = Command::new(prog)
                .args(&args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn();
            match child {
                Ok(mut c) => {
                    if c.stdin.as_mut().expect("stdin").write_all(text.as_bytes()).is_ok()
                        && c.wait().map(|s| s.success()).unwrap_or(false)
                    {
                        tracing::debug!("[commit] 剪贴板写入：{prog}");
                        return Ok(());
                    }
                }
                Err(e) => tracing::warn!("[commit] {prog} 启动失败: {e}"),
            }
        }
    }
    Err("所有剪贴板通道均失败".into())
}

fn which_exists(prog: &str) -> bool {
    std::process::Command::new("which")
        .arg(prog)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
