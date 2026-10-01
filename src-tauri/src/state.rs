// Voxis 全局应用状态
use crate::audio::manager::CaptureManager;
use crate::config::{Config, SharedConfig};
use std::sync::RwLock;

pub struct AppState {
    pub config: SharedConfig,
    /// 采集管理器（owner 线程持有 cpal::Stream，此处仅命令通道）
    pub capture: CaptureManager,
}

impl AppState {
    pub fn new_with_rwlock(config: Config) -> Self {
        AppState { config: RwLock::new(config), capture: CaptureManager::new() }
    }
}
