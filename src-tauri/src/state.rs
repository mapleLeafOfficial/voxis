// Voxis 全局应用状态
use crate::config::{Config, SharedConfig};
use std::sync::RwLock;

pub struct AppState {
    pub config: SharedConfig,
}

impl AppState {
    pub fn new_with_rwlock(config: Config) -> Self {
        AppState { config: RwLock::new(config) }
    }
}
