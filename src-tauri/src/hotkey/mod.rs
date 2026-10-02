// 全局热键引擎：evdev 直读实现系统级组合键（hold 按住说话 / lock toggle）。
// 本模块启动入口 `start()`：权限自检 → 枚举键盘 → 读线程汇聚 → 状态机消费 → 驱动会话。
#[cfg(target_os = "linux")]
pub mod evdev;
#[cfg(target_os = "windows")]
pub mod win;
pub mod keys;
pub mod monitor;
pub mod permission;
pub mod state;

pub use monitor::{MonitorHandle, RawKeyEvent};

use std::sync::mpsc;

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// 权限状态事件（设置页/通知消费，todo8）
pub const EV_PERMISSION: &str = "hotkey://permission";
/// 热键调试事件（DevView 展示按住集合与触发）
pub const EV_DEBUG: &str = "hotkey://debug";

/// 启动热键引擎（lib.rs setup 调用；失败只记日志/发事件，不阻塞应用启动）
pub fn start(app: &AppHandle) {
    if let Err(e) = spawn_engine(app) {
        tracing::error!("[hotkey] {e}");
    }
}

/// 重启引擎（设置页改了 hold/lock 后调用）：drop 旧监听（读线程退→rx 断开→状态机退）→ 重新起
pub fn restart(app: &AppHandle) {
    let state = app.state::<AppState>();
    if let Some(old) = state.hotkey_monitor.lock().expect("hotkey 锁").take() {
        drop(old); // 触发读线程退出
        tracing::info!("[hotkey] 旧引擎已停止，正在重启");
    }
    drop(state);
    if let Err(e) = spawn_engine(app) {
        tracing::error!("[hotkey] 重启失败：{e}");
    }
}

/// 引擎启动主体。成功后 EvdevMonitor 存入 AppState（持有即活着）。
fn spawn_engine(app: &AppHandle) -> Result<(), String> {
    // 1) 权限自检：无 input 读权限则无法工作，报告后退出
    let perm = permission::check();
    if !perm.input_ok {
        let msg = format!("权限不足，热键引擎未启动：{}", perm.problems.join("；"));
        let _ = app.emit(EV_PERMISSION, &perm);
        return Err(msg);
    }
    // uinput/ydotoold 缺失只报告（todo7 才需要）
    if !perm.uinput_ok || !perm.ydotoold_ok {
        tracing::warn!("[hotkey] 权限提醒：{}", perm.problems.join("；"));
        let _ = app.emit(EV_PERMISSION, &perm);
    }

    // 2) 读配置：hold/lock 键名 → 键集
    let cfg = app
        .try_state::<AppState>()
        .and_then(|s| s.config.read().ok().map(|c| c.hotkey.clone()))
        .ok_or_else(|| "无法读取配置，热键引擎未启动".to_string())?;
    let hold: Vec<String> = cfg.hold.iter().filter(|n| keys::valid_name(n)).cloned().collect();
    let lock: Vec<String> = cfg.lock.iter().filter(|n| keys::valid_name(n)).cloned().collect();
    // polish.hotkey = "Ctrl+Super+O"（manual 整理组合；mode=off 时也解析，配置切回 manual 无需重启）
    let polish_combo: Vec<String> = app
        .try_state::<AppState>()
        .and_then(|s| s.config.read().ok().map(|c| c.polish.hotkey.clone()))
        .unwrap_or_default()
        .split('+')
        .map(str::trim)
        .filter(|n| keys::valid_name(n))
        .map(str::to_string)
        .collect();
    if hold.is_empty() && lock.is_empty() {
        return Err("hold/lock 均未配置有效键名，热键引擎未启动".into());
    }

    // 3) 键盘事件源（Linux: evdev / Windows: 钩子），统一 RawKeyEvent 流
    #[cfg(target_os = "linux")]
    let (tx, rx) = mpsc::channel::<RawKeyEvent>();
    #[cfg(target_os = "windows")]
    let (tx, rx) = mpsc::channel::<RawKeyEvent>();
    let (monitor, n_devs) = {
        #[cfg(target_os = "linux")]
        {
            evdev::spawn_monitors(tx)
        }
        #[cfg(target_os = "windows")]
        {
            win::spawn_monitors(tx)
        }
    };
    if n_devs == 0 {
        return Err("未找到可读键盘设备，热键引擎未启动".into());
    }
    tracing::info!("[hotkey] 监听 {n_devs} 个键盘设备，hold={hold:?} lock={lock:?}");

    // 4) 状态机消费循环（阻塞线程；start/stop 的 async 部分由内部 spawn 到 tokio）
    let app2 = app.clone();
    let mgr = app.state::<AppState>().session.clone();
    tauri::async_runtime::spawn_blocking(move || {
        state::HotkeyEngine::new(hold, lock, polish_combo).run(app2, mgr, rx);
    });

    // 5) 句柄入 AppState：后续 restart 靠 drop 它停引擎
    *app.state::<AppState>().hotkey_monitor.lock().expect("hotkey 锁") = Some(monitor);
    Ok(())
}
