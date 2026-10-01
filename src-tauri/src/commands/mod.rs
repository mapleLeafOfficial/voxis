// Voxis IPC 命令（todo1 基础命令集）
use crate::config::{self, Config};
use crate::state::AppState;
use tauri::{AppHandle, Manager, State};

#[tauri::command]
pub fn ping() -> String {
    "pong".into()
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<Config, String> {
    state
        .config
        .read()
        .map(|c| c.clone())
        .map_err(|e| format!("配置锁中毒: {e}"))
}

#[tauri::command]
pub fn set_config(state: State<'_, AppState>, config: Config) -> Result<(), String> {
    config::save(&config)?;
    let mut guard = state
        .config
        .write()
        .map_err(|e| format!("配置锁中毒: {e}"))?;
    *guard = config;
    tracing::info!("配置已更新并保存");
    Ok(())
}

/// 显示主窗口（dev 阶段入口；todo10 托盘接管）
#[tauri::command]
pub fn show_main_window(app: AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
}
