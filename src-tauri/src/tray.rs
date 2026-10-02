// 系统托盘常驻：两态图标（空闲/录音中）、左键点击=开始·停止会话、右键菜单（设置/开关/权限/退出）。
// 状态同步：SessionManager::emit_state → update_state。
use std::include_bytes;

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager};

use crate::session::SessionState;
use crate::state::AppState;

const ICON_IDLE: &[u8] = include_bytes!("../icons/tray/idle.png");
const ICON_RECORDING: &[u8] = include_bytes!("../icons/tray/recording.png");

/// 菜单项 ID
const M_SHOW: &str = "show";
const M_SETTINGS: &str = "settings";
const M_TOGGLE: &str = "toggle";
const M_PERM: &str = "perm";
const M_QUIT: &str = "quit";

pub struct Tray {
    pub icon: TrayIcon,
    pub toggle_item: MenuItem<tauri::Wry>,
}

/// 构建托盘（lib.rs setup 调用）
pub fn init(app: &AppHandle) -> Result<(), String> {
    let toggle_label = if current_state(app) == SessionState::Idle { "开始会话" } else { "停止会话" };
    let toggle = MenuItem::with_id(app, M_TOGGLE, toggle_label, true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let show = MenuItem::with_id(app, M_SHOW, "显示主窗口", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let settings = MenuItem::with_id(app, M_SETTINGS, "设置…", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let perm = MenuItem::with_id(app, M_PERM, "检查权限", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let quit = MenuItem::with_id(app, M_QUIT, "退出 Voxis", true, None::<&str>)
        .map_err(|e| e.to_string())?;
    let menu = Menu::with_items(app, &[&show, &settings, &toggle, &perm, &quit])
        .map_err(|e| e.to_string())?;

    let icon = tauri::image::Image::from_bytes(ICON_IDLE)
        .map_err(|e| format!("托盘图标解码失败: {e}"))?;
    let tray = TrayIconBuilder::with_id("voxis-tray")
        .icon(icon)
        .tooltip("Voxis 语音输入")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(handle_menu)
        .on_tray_icon_event(|tray, event| {
            // 左键点击：Idle → 开始；Recording/Committing → 停止
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                toggle_session(tray.app_handle());
            }
        })
        .build(app)
        .map_err(|e| format!("托盘构建失败: {e}"))?;

    app.manage::<Tray>(Tray { icon: tray, toggle_item: toggle });
    Ok(())
}

fn current_state(app: &AppHandle) -> SessionState {
    app.try_state::<AppState>()
        .map(|s| s.session.state())
        .unwrap_or(SessionState::Idle)
}

/// 会话状态变更 → 图标与菜单文案同步（session.rs emit_state 调用）
pub fn update_state(app: &AppHandle, state: SessionState) {
    let Some(tray) = app.try_state::<Tray>() else { return };
    let icon_bytes = if state == SessionState::Idle { ICON_IDLE } else { ICON_RECORDING };
    if let Ok(icon) = tauri::image::Image::from_bytes(icon_bytes) {
        let _ = tray.icon.set_icon(Some(icon));
    }
    let _ = tray.toggle_item.set_text(if state == SessionState::Idle { "开始会话" } else { "停止会话" });
}

/// 左键/菜单共用：切换会话（切换是 async 操作，spawn 到 tokio）
fn toggle_session(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let mgr = app.state::<AppState>().session.clone();
        if mgr.state() == SessionState::Idle {
            let _ = mgr.start(&app, None, true, None).await;
        } else {
            let _ = mgr.stop(&app).await;
        }
    });
}

fn handle_menu(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id().as_ref() {
        M_SHOW => show_main(app, None),
        M_SETTINGS => show_main(app, Some("/settings")),
        M_TOGGLE => toggle_session(app),
        M_PERM => {
            let perm = crate::hotkey::permission::check();
            let msg = if perm.input_ok && perm.uinput_ok && perm.ydotoold_ok {
                "权限全部就绪 ✓".to_string()
            } else {
                format!("权限缺失：{}", perm.problems.join("；"))
            };
            crate::commit::notify(app, "Voxis 权限检查", &msg);
        }
        M_QUIT => {
            tracing::info!("托盘退出：清理中");
            // 会话进行中先正常收尾（走 commit 管道），再退出
            if current_state(app) != SessionState::Idle {
                let app2 = app.clone();
                tauri::async_runtime::spawn(async move {
                    let mgr = app2.state::<AppState>().session.clone();
                    let _ = mgr.stop(&app2).await;
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await; // 给日志/commit 收尾
                    app2.exit(0);
                });
            } else {
                app.exit(0);
            }
        }
        _ => {}
    }
}

/// 显示主窗口；page 为 Some 时前端路由跳转（设置页）
fn show_main(app: &AppHandle, page: Option<&str>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.show();
        let _ = win.set_focus();
    }
    if let Some(p) = page {
        let _ = app.emit("ui://navigate", p);
    }
}
