// Voxis — Linux 全局语音输入（复刻微信 PC 端体验）
pub mod asr;
pub mod audio;
mod commands;
mod config;
mod logging;
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
        ])
        .setup(|_app| {
            // dev 构建直接显示主窗口，方便调试；release 由托盘/命令唤起
            #[cfg(debug_assertions)]
            if let Some(win) = _app.get_webview_window("main") {
                let _ = win.show();
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("voxis 启动失败");
}
