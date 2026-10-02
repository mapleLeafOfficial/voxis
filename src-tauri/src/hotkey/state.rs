// 热键状态机：hold（按住说话）/ lock（组合键 toggle）两种触发模式。
//
// 触发判定（lock 优先 + hold 延迟窗口）：
// - lock 全集达成 → 立即 toggle（lock ⊇ hold，必须先判，否则 lock 必被 hold 抢触发）
// - 仅 hold 全集达成 → 延迟 HOLD_DELAY 触发；窗口内补按出 lock 全集则取消 hold、改触发 lock
// - hold 会话已开始后再补按 Shift → 仍按 hold 语义（todo6 备注：简单优先）
// - lock 会话进行中再按 lock 全集 → 结束
use std::collections::HashSet;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use tauri::{AppHandle, Emitter, Manager};

use crate::session::{SessionManager, SessionState};

/// hold 触发延迟窗口：等可能的 lock 附加键（Ctrl+Win+Shift 三连按通常 <150ms）
const HOLD_DELAY: Duration = Duration::from_millis(150);
/// 状态机轮询 tick
const TICK: Duration = Duration::from_millis(50);

/// 触发模式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerKind {
    Hold,
    Lock,
}

/// 热键引擎状态
pub struct HotkeyEngine {
    hold_set: HashSet<String>,
    lock_set: HashSet<String>,
    polish_set: HashSet<String>,
    pressed: HashSet<String>,
    /// lock 边沿武装：只有从「不含 lock 全集」→「含全集」的下沿才 toggle，避免长按重复触发
    lock_armed: bool,
    /// polish 组合边沿武装（同 lock 逻辑，触发 manual 整理）
    polish_armed: bool,
    /// 热键当前持有的会话来源（None = 空闲）
    session_source: Option<TriggerKind>,
    /// hold 延迟触发倒计时（窗口内被 lock 抢走则取消）
    pending_hold: Option<tokio::time::Instant>,
}

impl HotkeyEngine {
    pub fn new(hold: Vec<String>, lock: Vec<String>, polish: Vec<String>) -> Self {
        Self {
            hold_set: hold.into_iter().collect(),
            lock_set: lock.into_iter().collect(),
            polish_set: polish.into_iter().collect(),
            pressed: HashSet::new(),
            lock_armed: false,
            polish_armed: false,
            session_source: None,
            pending_hold: None,
        }
    }

    /// 状态机主循环：阻塞收事件 + 定期检查 hold 延迟窗口
    pub fn run(
        mut self,
        app: AppHandle,
        mgr: std::sync::Arc<SessionManager>,
        rx: Receiver<super::RawKeyEvent>,
    ) {
        loop {
            match rx.recv_timeout(TICK) {
                Ok(ev) => {
                    // 暂停中（设置页录制组合键）：丢弃事件（含 up——录制结束保存配置会 restart 引擎重置状态）
                    if app
                        .try_state::<crate::state::AppState>()
                        .map(|s| s.hotkey_suspended.load(std::sync::atomic::Ordering::Relaxed))
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    self.on_event(&app, &mgr, ev);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
            // hold 延迟窗口到期：确认无 lock 附加键，触发 hold
            if let Some(deadline) = self.pending_hold {
                if tokio::time::Instant::now() >= deadline {
                    self.pending_hold = None;
                    if self.session_source.is_none()
                        && mgr.state() == SessionState::Idle
                        && self.hold_set.iter().all(|k| self.pressed.contains(k))
                    {
                        self.session_source = Some(TriggerKind::Hold);
                        self.start_session(&app, &mgr);
                    }
                }
            }
        }
    }

    /// 处理一个按键事件。返回调试描述（供 DevView 展示）。
    fn on_event(
        &mut self,
        app: &AppHandle,
        mgr: &std::sync::Arc<SessionManager>,
        ev: super::RawKeyEvent,
    ) {
        let desc = format!(
            "{} {}（按住: {}）",
            ev.name,
            if ev.pressed { "↓" } else { "↑" },
            self.held_sorted()
        );
        tracing::debug!("[hotkey] {desc}");

        if ev.pressed {
            self.pressed.insert(ev.name.clone());
        } else {
            self.pressed.remove(&ev.name);
        }

        let busy = mgr.state() != SessionState::Idle;
        let held = |set: &HashSet<String>| set.iter().all(|k| self.pressed.contains(k));

        if ev.pressed {
            // lock 优先：全集达成且处于未武装边沿（hold 会话中不判 lock——按 hold 语义）
            if !self.lock_set.is_empty()
                && held(&self.lock_set)
                && !self.lock_armed
                && self.session_source != Some(TriggerKind::Hold)
            {
                self.lock_armed = true;
                self.pending_hold = None; // lock 抢走 hold 窗口
                self.toggle_lock(app, mgr, busy);
                return;
            }
            // polish：manual 整理组合（常为 hold 超集，需在 hold 窗口到期前武装，取消窗口）
            if !self.polish_set.is_empty() && held(&self.polish_set) && !self.polish_armed {
                self.polish_armed = true;
                self.pending_hold = None;
                if !busy && self.session_source.is_none() {
                    tracing::info!("[hotkey] 触发 manual 整理");
                    let _ = app.emit(super::EV_DEBUG, "触发 manual 整理");
                    let app = app.clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        if let Err(e) = crate::polish::polish_clipboard_flow(&app) {
                            crate::commit::notify(&app, "Voxis", &format!("整理失败：{e}"));
                        }
                    });
                }
                return;
            }
            // hold：全集达成（且 lock 不满足）→ 进延迟窗口，等可能的 lock 附加键
            if !self.hold_set.is_empty()
                && held(&self.hold_set)
                && !held(&self.lock_set)
                && self.session_source.is_none()
                && !busy
                && self.pending_hold.is_none()
            {
                self.pending_hold =
                    Some(tokio::time::Instant::now() + tokio::time::Duration::from(HOLD_DELAY));
            }
        } else {
            // lock 解除武装：任一 lock 键离开按住集合
            if self.lock_armed && !held(&self.lock_set) {
                self.lock_armed = false;
            }
            // polish 解除武装：任一 polish 键离开按住集合
            if self.polish_armed && !held(&self.polish_set) {
                self.polish_armed = false;
            }
            // hold 结束：本组合起的会话，任一 hold 键松开即停
            if self.session_source == Some(TriggerKind::Hold)
                && self.hold_set.contains(&ev.name)
            {
                self.session_source = None;
                self.pending_hold = None;
                self.stop_session(app, mgr);
            }
        }
    }

    /// lock toggle：开始/结束
    fn toggle_lock(
        &mut self,
        app: &AppHandle,
        mgr: &std::sync::Arc<SessionManager>,
        busy: bool,
    ) {
        if self.session_source == Some(TriggerKind::Lock) {
            self.session_source = None;
            self.stop_session(app, mgr);
        } else if !busy && self.session_source.is_none() {
            self.session_source = Some(TriggerKind::Lock);
            self.start_session(app, mgr);
        }
    }

    fn start_session(&self, app: &AppHandle, mgr: &std::sync::Arc<SessionManager>) {
        tracing::info!("[hotkey] 触发开始会话（{:?}）", self.session_source);
        let _ = app.emit(super::EV_DEBUG, format!("触发开始（{:?}）", self.session_source));
        let app = app.clone();
        let mgr = mgr.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = SessionManager::start(&mgr, &app, None, false, None).await {
                tracing::error!("[hotkey] 启动会话失败: {e}");
            }
        });
    }

    fn stop_session(&self, app: &AppHandle, mgr: &std::sync::Arc<SessionManager>) {
        tracing::info!("[hotkey] 触发结束会话");
        let _ = app.emit(super::EV_DEBUG, "触发结束".to_string());
        let app = app.clone();
        let mgr = mgr.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(e) = SessionManager::stop(&mgr, &app).await {
                tracing::error!("[hotkey] 结束会话失败: {e}");
            }
        });
    }

    fn held_sorted(&self) -> String {
        let mut v: Vec<_> = self.pressed.iter().cloned().collect();
        v.sort();
        v.join("+")
    }
}
