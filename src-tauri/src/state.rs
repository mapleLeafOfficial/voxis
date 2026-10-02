// Voxis 全局应用状态
use crate::audio::manager::CaptureManager;
use crate::config::{Config, SharedConfig};
use crate::session::SessionManager;
use std::sync::Arc;
use std::sync::RwLock;

pub struct AppState {
    pub config: SharedConfig,
    /// 采集管理器（owner 线程持有 cpal::Stream，此处仅命令通道）
    pub capture: CaptureManager,
    /// 会话状态机（录音 + ASR 编排）
    pub session: Arc<SessionManager>,
}

impl AppState {
    pub fn new_with_rwlock(config: Config) -> Self {
        AppState {
            config: RwLock::new(config),
            capture: CaptureManager::new(),
            session: Arc::new(SessionManager::new()),
        }
    }
}
