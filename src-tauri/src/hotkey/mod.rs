// 全局热键引擎：evdev 直读实现系统级组合键（hold 按住说话 / lock toggle）。
// 本模块启动入口 `start()`：权限自检 → 枚举键盘 → 读线程汇聚 → 状态机消费 → 驱动会话。
pub mod evdev;
pub mod keys;
pub mod permission;
pub mod state;

use std::sync::mpsc;

use tauri::{AppHandle, Emitter, Manager};

use crate::state::AppState;

/// 权限状态事件（设置页/通知消费，todo8）
pub const EV_PERMISSION: &str = "hotkey://permission";
/// 热键调试事件（DevView 展示按住集合与触发）
pub const EV_DEBUG: &str = "hotkey://debug";

/// 启动热键引擎（lib.rs setup 调用；失败只记日志/发事件，不阻塞应用启动）
pub fn start(app: &AppHandle) {
    // 1) 权限自检：无 input 读权限则无法工作，报告后退出
    let perm = permission::check();
    if !perm.input_ok {
        tracing::error!("[hotkey] 权限不足，热键引擎未启动：{}", perm.problems.join("；"));
        let _ = app.emit(EV_PERMISSION, &perm);
        return;
    }
    // uinput/ydotoold 缺失只报告（todo7 才需要）
    if !perm.uinput_ok || !perm.ydotoold_ok {
        tracing::warn!("[hotkey] 权限提醒：{}", perm.problems.join("；"));
        let _ = app.emit(EV_PERMISSION, &perm);
    }

    // 2) 读配置：hold/lock 键名 → 键集
    let cfg = app
        .try_state::<AppState>()
        .and_then(|s| s.config.read().ok().map(|c| c.hotkey.clone()));
    let Some(cfg) = cfg else {
        tracing::error!("[hotkey] 无法读取配置，热键引擎未启动");
        return;
    };
    let hold: Vec<String> = cfg.hold.iter().filter_map(|n| keys::key_from_name(n).map(|_| n.clone())).collect();
    let lock: Vec<String> = cfg.lock.iter().filter_map(|n| keys::key_from_name(n).map(|_| n.clone())).collect();
    if hold.is_empty() && lock.is_empty() {
        tracing::warn!("[hotkey] hold/lock 均未配置有效键名，热键引擎未启动");
        return;
    }

    // 3) 枚举键盘设备并启动读线程
    let (tx, rx) = mpsc::channel::<evdev::RawKeyEvent>();
    let (monitor, n_devs) = evdev::spawn_monitors(tx);
    if n_devs == 0 {
        tracing::error!("[hotkey] 未找到可读键盘设备（/dev/input/event*），热键引擎未启动");
        return;
    }
    tracing::info!("[hotkey] 监听 {n_devs} 个键盘设备，hold={hold:?} lock={lock:?}");
    std::mem::forget(monitor); // 引擎生命周期 = 应用生命周期，不 drop

    // 4) 状态机消费循环（阻塞线程；start/stop 的 async 部分由内部 spawn 到 tokio）
    let app = app.clone();
    let mgr = app.state::<AppState>().session.clone();
    tauri::async_runtime::spawn_blocking(move || {
        state::HotkeyEngine::new(hold, lock).run(app, mgr, rx);
    });
}
