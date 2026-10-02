// Voxis IPC 命令（todo1 基础 + todo2 录音 + todo4 会话）
use crate::audio::capture::CaptureCallbacks;
use crate::audio::devices;
use crate::config::{self, Config};
use crate::events;
use crate::state::AppState;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

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

// ---------- todo2：录音 ----------

#[tauri::command]
pub fn list_input_devices() -> Result<Vec<devices::InputDevice>, String> {
    devices::list_input_devices()
}

/// dev 临时命令：开始采集（todo4 由 start_session 替代）
#[tauri::command]
pub fn dev_capture_start(
    app: AppHandle,
    state: State<'_, AppState>,
    device: Option<String>,
) -> Result<(), String> {
    let max_duration = {
        let cfg = state.config.read().map_err(|e| format!("配置锁中毒: {e}"))?;
        Duration::from_secs(cfg.asr.max_duration as u64)
    };

    let emitter = app.clone();
    state.capture.start(
        device.clone(),
        Some(max_duration),
        CaptureCallbacks {
            on_volume: Box::new(move |level| {
                let _ = emitter.emit(events::VOLUME, level);
            }),
            on_pcm: Box::new(|chunk| {
                // todo3/todo4 接 ASR；当前仅统计
                tracing::trace!("PCM 块 {} 帧", chunk.len());
            }),
            on_max_duration: {
                let em = app.clone();
                Box::new(move || {
                    let _ = em.emit(events::MAX_DURATION, ());
                })
            },
            on_error: {
                let em = app.clone();
                Box::new(move |msg| {
                    let _ = em.emit(events::ERROR, events::ErrorPayload { source: "audio".into(), message: msg });
                })
            },
        },
    )?;
    tracing::info!("dev 采集已开始 device={device:?} max={max_duration:?}");
    Ok(())
}

/// dev 临时命令：停止采集
#[tauri::command]
pub fn dev_capture_stop(state: State<'_, AppState>) -> Result<(), String> {
    state.capture.stop()
}

/// 开始语音会话（todo6 由热键接管触发；`lock` 仅记录，语义在 todo6）
#[tauri::command]
pub async fn start_session(
    app: AppHandle,
    state: State<'_, AppState>,
    device: Option<String>,
    lock: Option<bool>,
) -> Result<(), String> {
    state
        .session
        .start(&app, device, lock.unwrap_or(false), None)
        .await
}

/// 结束语音会话（幂等）
#[tauri::command]
pub async fn stop_session(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    state.session.stop(&app).await
}

/// 热键运行权限自检（input/uinput/ydotoold），设置页与通知消费
#[tauri::command]
pub fn get_permission_status() -> crate::hotkey::permission::PermissionStatus {
    crate::hotkey::permission::check()
}
