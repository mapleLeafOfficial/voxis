// 剪贴板写入：GTK 主线程（走 mutter 原生 wl_data_device，GNOME/Wayland/X11 通用）
// → wl-clipboard-rs（wlroots 系合成器的 data-control）→ arboard（X11/XWayland）→ xclip/xsel（命令兜底）。
// GNOME 不支持 wlr-data-control，所以 GTK 通道必须放最前。
// 粘贴前写、commit 后不清剪贴板（保留文本供手动粘贴）。
use arboard::Clipboard;

/// 写入系统剪贴板。返回 Err(原因) 表示全部通道失败。
pub fn copy(app: &tauri::AppHandle, text: &str) -> Result<(), String> {
    // 1) GTK 主线程：voxis 本身是 GTK 进程，Clipboard 走合成器原生协议（mutter 的 wl_data_device）
    if copy_gtk(app, text) {
        tracing::debug!("[commit] 剪贴板写入：GTK");
        return Ok(());
    }
    tracing::warn!("[commit] GTK 剪贴板失败，回退 wl-clipboard-rs");

    // 2) Wayland data-control（wlroots 系）
    match copy_wayland(text) {
        Ok(()) => {
            tracing::debug!("[commit] 剪贴板写入：wl-clipboard-rs");
            return Ok(());
        }
        Err(e) => tracing::warn!("[commit] wl-clipboard-rs 失败: {e}，回退 arboard"),
    }

    // 3) arboard（X11 / 通用）
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

/// 读取系统剪贴板文本（manual 整理用）。GTK → wl-clipboard-rs → arboard。
pub fn paste_text(app: &tauri::AppHandle) -> Result<String, String> {
    if let Some(t) = paste_gtk(app) {
        return Ok(t);
    }
    tracing::warn!("[commit] GTK 剪贴板读失败，回退 wl-clipboard-rs");
    match paste_wayland() {
        Ok(t) => return Ok(t),
        Err(e) => tracing::warn!("[commit] wl-clipboard 读失败: {e}，回退 arboard"),
    }
    Clipboard::new()
        .and_then(|mut c| c.get_text())
        .map(|s| s.to_string())
        .map_err(|e| format!("arboard 读剪贴板失败: {e}"))
}

/// GTK 剪贴板（必须主线程；copy 从 spawn_blocking 调用，用 run_on_main_thread + channel 回传）。
#[cfg(target_os = "linux")]
fn copy_gtk(app: &tauri::AppHandle, text: &str) -> bool {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel::<bool>();
    let text = text.to_string();
    if app.run_on_main_thread(move || {
        let _ = tx.send(gtk_copy_impl(&text));
    })
    .is_err()
    {
        return false;
    }
    rx.recv_timeout(std::time::Duration::from_secs(1)).unwrap_or(false)
}

#[cfg(target_os = "linux")]
fn gtk_copy_impl(text: &str) -> bool {
    let Some(display) = gtk::gdk::Display::default() else {
        return false;
    };
    let Some(clip) = gtk::Clipboard::default(&display) else {
        return false;
    };
    clip.set_text(text);
    clip.store(); // 交给剪贴板管理器持久化，本进程不再需要持有
    true
}

/// GTK 读剪贴板（主线程，机制同 copy_gtk）
#[cfg(target_os = "linux")]
fn paste_gtk(app: &tauri::AppHandle) -> Option<String> {
    use std::sync::mpsc;
    let (tx, rx) = mpsc::channel::<Option<String>>();
    if app.run_on_main_thread(move || {
        let text = gtk::gdk::Display::default()
            .and_then(|d| gtk::Clipboard::default(&d))
            .and_then(|c| c.wait_for_text())
            .map(|s| s.to_string());
        let _ = tx.send(text);
    })
    .is_err()
    {
        return None;
    }
    rx.recv_timeout(std::time::Duration::from_secs(1)).ok().flatten()
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
