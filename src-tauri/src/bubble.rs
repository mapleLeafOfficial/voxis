// 气泡浮窗管理：按 ui.bubble_pos 定位（四角 / 底部居中）、显示/隐藏、延迟淡出隐藏。
// Wayland：gtk-layer-shell（Overlay 层 + 锚定/margin，绕开 xdg-shell 不能自定位/置顶的限制）；
// X11 / 降级：传统 set_position + always_on_top。
// 窗口本身在 tauri.conf.json 预定义（520×88，透明、无装饰、不抢焦点）。
use std::time::Duration;
use tauri::{AppHandle, Manager, PhysicalPosition};

/// 四角模式与屏幕边缘的间距（逻辑像素）
const EDGE_MARGIN: i32 = 48;
/// 底部居中模式距屏幕底边的高度（逻辑像素，PRD：工作区上方 ~120px）
const BOTTOM_OFFSET: i32 = 120;
/// 气泡窗口逻辑尺寸兜底（拿不到 outer_size 时用）
const FALLBACK_W: i32 = 520;
const FALLBACK_H: i32 = 88;

/// 尝试将气泡窗口初始化为 layer-shell surface（仅 Wayland 有效）。
/// 返回 true 表示已接管定位（调用方跳过传统定位）；幂等（已初始化则直接返回 true）。
#[cfg(target_os = "linux")]
fn try_init_layer_shell(win: &tauri::WebviewWindow, pos_cfg: &str) -> bool {
    use gtk_layer_shell::LayerShell;
    let Ok(gtk_win) = win.gtk_window() else {
        return false;
    };
    if !gtk_layer_shell::is_supported() {
        return false;
    }
    if gtk_win.is_layer_window() {
        return true;
    }

    gtk_win.init_layer_shell();
    gtk_win.set_layer(gtk_layer_shell::Layer::Overlay);
    gtk_win.set_exclusive_zone(-1); // 不挤压其他 layer 表面

    // 锚定方式按位置配置：中心锚底（水平不锚 → 默认水平居中），四角锚两边
    use gtk_layer_shell::Edge;
    let anchor = |edges: &[Edge]| {
        for e in [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right] {
            gtk_win.set_anchor(e, edges.contains(&e));
        }
    };
    let set_margin = |top: i32, bottom: i32, left: i32, right: i32| {
        gtk_win.set_layer_shell_margin(Edge::Top, top);
        gtk_win.set_layer_shell_margin(Edge::Bottom, bottom);
        gtk_win.set_layer_shell_margin(Edge::Left, left);
        gtk_win.set_layer_shell_margin(Edge::Right, right);
    };
    match pos_cfg {
        "top_left" => {
            anchor(&[Edge::Top, Edge::Left]);
            set_margin(EDGE_MARGIN, 0, EDGE_MARGIN, 0);
        }
        "top_right" => {
            anchor(&[Edge::Top, Edge::Right]);
            set_margin(EDGE_MARGIN, 0, 0, EDGE_MARGIN);
        }
        "bottom_left" => {
            anchor(&[Edge::Bottom, Edge::Left]);
            set_margin(0, EDGE_MARGIN, EDGE_MARGIN, 0);
        }
        "bottom_right" => {
            anchor(&[Edge::Bottom, Edge::Right]);
            set_margin(0, EDGE_MARGIN, 0, EDGE_MARGIN);
        }
        // bottom_center（默认）：仅锚底 → 水平居中
        _ => {
            anchor(&[Edge::Bottom]);
            set_margin(0, BOTTOM_OFFSET, 0, 0);
        }
    }
    true
}

/// 显示气泡（会话开始时调用）：先按配置定位，再 show
pub fn show(app: &AppHandle) {
    let Some(win) = app.get_webview_window("bubble") else {
        return;
    };

    // 读取位置配置（读锁异常时退回默认）
    let pos_cfg = app
        .try_state::<crate::state::AppState>()
        .and_then(|s| {
            s.config
                .read()
                .ok()
                .map(|c| c.ui.bubble_pos.trim().to_string())
        })
        .unwrap_or_else(|| "bottom_center".into());

    // Wayland：layer-shell 接管定位；X11/降级：传统物理坐标定位
    #[cfg(target_os = "linux")]
    let layered = try_init_layer_shell(&win, &pos_cfg);
    #[cfg(not(target_os = "linux"))]
    let layered = false;

    if !layered {
        if let Ok(Some(monitor)) = win.current_monitor() {
            let ms = monitor.size();
            let mp = monitor.position();
            let (bw, bh) = win
                .outer_size()
                .map(|s| (s.width as i32, s.height as i32))
                .unwrap_or((FALLBACK_W, FALLBACK_H));

            let (x, y) = match pos_cfg.as_str() {
                "top_left" => (mp.x + EDGE_MARGIN, mp.y + EDGE_MARGIN),
                "top_right" => (mp.x + ms.width as i32 - bw - EDGE_MARGIN, mp.y + EDGE_MARGIN),
                "bottom_left" => (mp.x + EDGE_MARGIN, mp.y + ms.height as i32 - bh - EDGE_MARGIN),
                "bottom_right" => (
                    mp.x + ms.width as i32 - bw - EDGE_MARGIN,
                    mp.y + ms.height as i32 - bh - EDGE_MARGIN,
                ),
                _ => (
                    mp.x + (ms.width as i32 - bw) / 2,
                    mp.y + ms.height as i32 - bh - BOTTOM_OFFSET,
                ),
            };
            let _ = win.set_position(PhysicalPosition::new(x, y));
        }
    }

    let _ = win.show();
}

/// 隐藏气泡
pub fn hide(app: &AppHandle) {
    if let Some(win) = app.get_webview_window("bubble") {
        let _ = win.hide();
    }
}

/// 延迟隐藏：给前端留出结果徽标/错误摘要的展示与淡出动画时间
pub fn hide_delayed(app: AppHandle, ms: u64) {
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(ms)).await;
        hide(&app);
    });
}
