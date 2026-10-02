// 上屏引擎：剪贴板写入 → 键盘注入粘贴 → 结果三态（pasted / copied / none）。
// 编排入口 `commit()`：由 SessionManager 的 Committing 阶段调用（spawn_blocking，内部有 150ms 静默）。
pub mod clipboard;
pub mod inject;

use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

use crate::events::{CommittedPayload, COMMITTED};
use crate::state::AppState;

/// 提交结果三态（committed 事件 payload.result）
pub const RESULT_PASTED: &str = "pasted"; // 已上屏
pub const RESULT_COPIED: &str = "copied"; // 仅复制（注入失败/剪贴板模式）
pub const RESULT_NONE: &str = "none"; // 空文本，未提交

/// 统一桌面通知辅助
pub fn notify(app: &AppHandle, title: &str, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}

/// 提交编排（阻塞，调用方放 spawn_blocking）：
/// - 空文本 → 通知「未识别到内容」，none
/// - auto：copy → inject → pasted；注入失败 → 通知「已复制」，copied
/// - clipboard_only：copy → copied
/// 成功后发 committed 事件（气泡/DevView 消费）。
pub fn commit(app: &AppHandle, text: &str) -> &'static str {
    let empty = text.trim().is_empty();

    // 读提交模式（读锁异常退回 auto）
    let mode = app
        .try_state::<AppState>()
        .and_then(|s| s.config.read().ok().map(|c| c.commit.mode.trim().to_string()))
        .unwrap_or_else(|| "auto".into());

    if empty {
        tracing::info!("[commit] 空文本：仅通知");
        notify(app, "Voxis", "未识别到内容");
        let _ = app.emit(COMMITTED, CommittedPayload { text: text.to_string(), result: RESULT_NONE.into() });
        return RESULT_NONE;
    }

    // auto 整理（polish.mode==auto 时清洗，失败回退原文并通知）；整理后的 text 参与 copy/上屏
    let (text, _polished) = super::polish::apply_auto(app, text);

    if let Err(e) = clipboard::copy(app, &text) {
        tracing::error!("[commit] 剪贴板写入失败: {e}");
        notify(app, "Voxis", "复制失败：无法访问剪贴板");
        let _ = app.emit(COMMITTED, CommittedPayload { text: text.to_string(), result: RESULT_COPIED.into() });
        return RESULT_COPIED;
    }

    if mode == "clipboard_only" {
        tracing::info!("[commit] clipboard_only 模式：已复制");
        notify(app, "Voxis", "已复制，Ctrl+V 粘贴");
        let _ = app.emit(COMMITTED, CommittedPayload { text: text.to_string(), result: RESULT_COPIED.into() });
        return RESULT_COPIED;
    }

    match inject::inject_paste() {
        Ok(()) => {
            tracing::info!("[commit] 已上屏 {} 字", text.trim().chars().count());
            let _ = app.emit(COMMITTED, CommittedPayload { text: text.to_string(), result: RESULT_PASTED.into() });
            RESULT_PASTED
        }
        Err(e) => {
            tracing::warn!("[commit] 注入失败（回退复制）: {e}");
            notify(app, "Voxis", "已复制，Ctrl+V 粘贴");
            let _ = app.emit(COMMITTED, CommittedPayload { text: text.to_string(), result: RESULT_COPIED.into() });
            RESULT_COPIED
        }
    }
}
