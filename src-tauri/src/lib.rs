// Voxis — Linux 全局语音输入（复刻微信 PC 端体验）
pub mod asr;
pub mod audio;
mod bubble;
mod commands;
mod config;
pub mod events;
mod logging;
pub mod session;
mod state;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 配置先行：日志级别依赖它；失败则回退默认并继续启动
    let config = match config::load() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("加载配置失败: {e}，使用默认配置");
            config::Config::with_defaults()
        }
    };
    logging::init(&config.general.log_level);
    tracing::info!("voxis 启动，配置已加载");

    tauri::Builder::default()
        // 单实例必须最先注册：二次启动 → 唤起已有实例主窗口
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            tracing::info!("检测到二次启动，唤起主窗口");
            if let Some(win) = app.get_webview_window("main") {
                let _ = win.show();
                let _ = win.set_focus();
            }
        }))
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::new_with_rwlock(config))
        .invoke_handler(tauri::generate_handler![
            commands::ping,
            commands::get_config,
            commands::set_config,
            commands::show_main_window,
            commands::list_input_devices,
            commands::dev_capture_start,
            commands::dev_capture_stop,
            commands::start_session,
            commands::stop_session,
        ])
        .setup(|_app| {
            // dev 构建直接显示主窗口，方便调试；release 由托盘/命令唤起
            #[cfg(debug_assertions)]
            if let Some(win) = _app.get_webview_window("main") {
                let _ = win.show();
            }

            // 冒烟钩子：--smoke-session 无头跑一轮会话（启动→6s→停止→退出）
            if std::env::args().any(|a| a == "--smoke-session") {
                let handle = _app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use tauri::Manager;
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                    // VOXIS_SMOKE_WAV：指定 16k/mono/PCM16 wav 则绕过音频子系统直接泵 PCM（确定性验证）
                    let wav = std::env::var("VOXIS_SMOKE_WAV").ok().filter(|s| !s.is_empty());
                    let override_pcm = match wav {
                        Some(p) => match crate::audio::wav::load_16k_mono(&p) {
                            Ok(v) => Some(v),
                            Err(e) => {
                                tracing::error!("[smoke] wav 加载失败: {e}");
                                std::process::exit(1);
                            }
                        },
                        None => None,
                    };
                    tracing::info!("[smoke] start_session");
                    let r = handle
                        .state::<AppState>()
                        .session
                        .start(&handle, None, false, override_pcm)
                        .await;
                    tracing::info!("[smoke] start 结果: {r:?}");
                    if r.is_ok() {
                        tokio::time::sleep(std::time::Duration::from_millis(6000)).await;
                        let r = handle.state::<AppState>().session.stop(&handle).await;
                        tracing::info!("[smoke] stop 结果: {r:?}");
                    }
                    tracing::info!("[smoke] 完成，退出");
                    std::process::exit(0);
                });
            }

            // 冒烟钩子：--smoke-loop 同进程连续 3 轮 start/stop（泄漏检查）
            if std::env::args().any(|a| a == "--smoke-loop") {
                let handle = _app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    use tauri::Manager;
                    tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                    let wav = std::env::var("VOXIS_SMOKE_WAV").ok().filter(|s| !s.is_empty());
                    let override_pcm = match wav {
                        Some(p) => match crate::audio::wav::load_16k_mono(&p) {
                            Ok(v) => Some(v),
                            Err(e) => {
                                tracing::error!("[smoke-loop] wav 加载失败: {e}");
                                std::process::exit(1);
                            }
                        },
                        None => None,
                    };
                    let before = std::fs::read_to_string("/proc/self/status")
                        .ok()
                        .and_then(|s| {
                            s.lines().find(|l| l.starts_with("VmRSS:")).map(|l| l.to_string())
                        });
                    tracing::info!("[smoke-loop] 起始内存 {:?}", before);
                    for round in 1..=3 {
                        tracing::info!("[smoke-loop] === 第 {round} 轮 ===");
                        let r = handle
                            .state::<AppState>()
                            .session
                            .start(&handle, None, false, override_pcm.clone())
                            .await;
                        tracing::info!("[smoke-loop] start: {r:?}");
                        if r.is_err() {
                            std::process::exit(1);
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(6000)).await;
                        let r = handle.state::<AppState>().session.stop(&handle).await;
                        tracing::info!("[smoke-loop] stop: {r:?}");
                        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    }
                    let after = std::fs::read_to_string("/proc/self/status")
                        .ok()
                        .and_then(|s| {
                            s.lines().find(|l| l.starts_with("VmRSS:")).map(|l| l.to_string())
                        });
                    tracing::info!("[smoke-loop] 结束内存 {:?}（起始 {:?}）", after, before);
                    tracing::info!("[smoke-loop] 完成，退出");
                    std::process::exit(0);
                });
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("voxis 启动失败");
}
