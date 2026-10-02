// Voxis 全局应用状态
use crate::audio::manager::CaptureManager;
use crate::config::{Config, SharedConfig};
use crate::session::SessionManager;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use std::sync::RwLock;

pub struct AppState {
    pub config: SharedConfig,
    /// 采集管理器（owner 线程持有 cpal::Stream，此处仅命令通道）
    pub capture: CaptureManager,
    /// 会话状态机（录音 + ASR 编排）
    pub session: Arc<SessionManager>,
    /// 热键引擎 evdev 监听句柄（持有即活着；drop 停读线程→rx 断开→状态机退出）。None=引擎未启动
    pub hotkey_monitor: std::sync::Mutex<Option<crate::hotkey::evdev::EvdevMonitor>>,
    /// 热键暂停（设置页录制组合键时置位：引擎丢弃所有事件，避免录制时触发会话）
    pub hotkey_suspended: AtomicBool,
}

impl AppState {
    pub fn new_with_rwlock(config: Config) -> Self {
        AppState {
            config: RwLock::new(config),
            capture: CaptureManager::new(),
            session: Arc::new(SessionManager::new()),
            hotkey_monitor: std::sync::Mutex::new(None),
            hotkey_suspended: AtomicBool::new(false),
        }
    }
}
