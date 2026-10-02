// Voxis IPC 命令（todo1 基础 + todo2 录音 + todo4 会话 + todo8 设置）
use crate::audio::capture::CaptureCallbacks;
use crate::audio::devices;
use crate::config::{self, Config};
use crate::events;
use crate::state::AppState;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};

/// 主题变更事件（气泡等窗口即时跟随）
pub const EV_THEME: &str = "ui://theme";

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
pub fn set_config(app: AppHandle, state: State<'_, AppState>, config: Config) -> Result<(), String> {
    // 旧值快照（diff 用）
    let old = state
        .config
        .read()
        .map(|c| c.clone())
        .map_err(|e| format!("配置锁中毒: {e}"))?;

    // diff（写入前取好）
    let theme_new = config.ui.theme.clone();
    let polish_hotkey_new = config.polish.hotkey.clone();
    let hotkey_changed = old.hotkey.hold != config.hotkey.hold || old.hotkey.lock != config.hotkey.lock;
    let theme_changed = old.ui.theme != config.ui.theme;

    config::save(&config)?;
    {
        let mut guard = state
            .config
            .write()
            .map_err(|e| format!("配置锁中毒: {e}"))?;
        *guard = config;
    }
    tracing::info!("配置已更新并保存");

    // 热键变更（含 manual 整理组合）→ 重启引擎（立即生效，旧组合失效）
    if hotkey_changed || old.polish.hotkey != polish_hotkey_new {
        tracing::info!("热键配置变更，重启引擎");
        crate::hotkey::restart(&app);
    }
    // 主题变更 → 广播（气泡即时跟随）
    if theme_changed {
        let _ = app.emit(EV_THEME, theme_new);
    }
    // 音频设备/ASR 参数：下次会话自动生效（start_session 时读配置），无需动作
    Ok(())
}

/// 测试 API Key 连通性：用给定 key 走一次真实 WS 握手（等到 task-started 才算成功），随即立即收尾
#[tauri::command]
pub async fn test_api_key(state: State<'_, AppState>, key: String) -> Result<String, String> {
    let cfg = {
        let snapshot = state
            .config
            .read()
            .map(|c| c.clone())
            .map_err(|e| format!("配置锁中毒: {e}"))?;
        crate::session::build_asr_config(&snapshot, key)
    };
    // 复用 AsrSession::start：它等到 task-started 才返回，key 无效会在握手/建任务阶段报错
    let fut = async {
        let (session, mut rx) = crate::asr::client::AsrSession::start(cfg).await?;
        session.request_stop();
        // 排干事件流（最多 1s），确保服务端侧会话关闭
        let _ = tokio::time::timeout(Duration::from_millis(1000), async {
            while let Some(_ev) = rx.recv().await {}
        })
        .await;
        Ok::<(), String>(())
    };
    match tokio::time::timeout(Duration::from_secs(15), fut).await {
        Err(_) => Err("连接超时（15s）：检查网络或 ws_url".into()),
        Ok(Err(e)) => Err(e),
        Ok(Ok(())) => Ok("连接成功，Key 有效".into()),
    }
}

/// 可录入的规范键名全集（快捷键录制下拉/校验用）
#[tauri::command]
pub fn list_key_names() -> Vec<String> {
    crate::hotkey::keys::all_names()
}

/// 手动整理剪贴板（设置页/调试按钮）：返回整理后文本
#[tauri::command]
pub async fn polish_clipboard(app: AppHandle) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || crate::polish::polish_clipboard_flow(&app))
        .await
        .map_err(|e| format!("任务失败: {e}"))?
}

/// 开机自启状态
#[tauri::command]
pub fn get_autostart() -> bool {
    crate::autostart::is_enabled()
}

/// 开/关开机自启（写/删 ~/.config/autostart/voxis.desktop）
#[tauri::command]
pub fn set_autostart(enable: bool) -> Result<(), String> {
    crate::autostart::set_enabled(enable)
}

/// 暂停/恢复热键引擎（设置页录制组合键时用，避免录制过程触发会话）
#[tauri::command]
pub fn hotkey_suspend(state: State<'_, AppState>, suspended: bool) {
    state
        .hotkey_suspended
        .store(suspended, Ordering::Relaxed);
    tracing::info!("[hotkey] suspended = {suspended}");
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
