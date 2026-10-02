// 整理文字：qwen 系列对语音文本做口语清洗（剔语气词/口头禅、理顺标点，不改原意）。
// 走 DashScope OpenAI 兼容 chat/completions；blocking 客户端（调用方均在 spawn_blocking 内）。
use serde_json::{json, Value};

use crate::asr::key::resolve_api_key;
use crate::config::PolishConfig;
use crate::state::AppState;
use tauri::{AppHandle, Manager};

pub const DEFAULT_PROMPT: &str = "你是口语润色器。剔除语气词与口头禅（嗯、啊、然后、那个、就是说等），修正明显口误，理顺标点与分段；不得改写原意、不得增删事实、不得回答或续写内容。只输出整理后的文本。";

const ENDPOINT: &str = "https://dashscope.aliyuncs.com/compatible-mode/v1/chat/completions";
const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// 从 AppState 读整理配置 + 解析 Key
fn polish_cfg(app: &AppHandle) -> Result<(PolishConfig, String), String> {
    let (polish, api_key_cfg) = app
        .try_state::<AppState>()
        .and_then(|s| {
            s.config.read().ok().map(|c| (c.polish.clone(), c.api_key.clone()))
        })
        .ok_or("无法读取配置")?;
    let api_key = resolve_api_key(&api_key_cfg).ok_or("未配置 API Key")?;
    Ok((polish, api_key))
}

/// 口语清洗（阻塞，10s 超时）。成功返回整理后文本（可能为空，调用方自行回退）。
pub fn polish_text(polish: &PolishConfig, api_key: &str, text: &str) -> Result<String, String> {
    let prompt = polish
        .prompt_override
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_PROMPT);
    // max_tokens 按输入长度 ×1.5 估算（含标点余量），下限 64
    let max_tokens = ((text.chars().count() as i64 * 3) / 2 + 64).max(64);

    let body = json!({
        "model": polish.model,
        "temperature": 0,
        "max_tokens": max_tokens,
        "messages": [
            { "role": "system", "content": prompt },
            { "role": "user", "content": text },
        ],
    });

    let client = reqwest::blocking::Client::builder()
        .timeout(TIMEOUT)
        .build()
        .map_err(|e| format!("HTTP 客户端构建失败: {e}"))?;
    let resp = client
        .post(ENDPOINT)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .map_err(|e| format!("请求失败: {e}"))?;

    let status = resp.status();
    let v: Value = resp.json().map_err(|e| format!("响应解析失败: {e}"))?;
    if !status.is_success() {
        let msg = v["error"]["message"]
            .as_str()
            .unwrap_or("未知错误");
        return Err(format!("HTTP {status}: {msg}"));
    }
    let content = v["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("响应缺少 choices[0].message.content")?
        .trim()
        .to_string();
    Ok(content)
}

/// auto 模式入口（commit 编排内调用，spawn_blocking）：非空文本按 polish.mode=="auto" 清洗。
/// 返回 (最终文本, 是否整理过)。失败/空结果回退原文，并通知。
pub fn apply_auto(app: &AppHandle, text: &str) -> (String, bool) {
    let (polish, api_key) = match polish_cfg(app) {
        Ok(x) => x,
        Err(e) => {
            tracing::warn!("[polish] 配置不可用，跳过整理: {e}");
            return (text.to_string(), false);
        }
    };
    if polish.mode != "auto" || text.trim().is_empty() {
        return (text.to_string(), false);
    }
    let t0 = std::time::Instant::now();
    match polish_text(&polish, &api_key, text) {
        Ok(p) if !p.trim().is_empty() => {
            tracing::info!("[polish] auto 整理完成（{}ms）：{} 字 → {} 字",
                t0.elapsed().as_millis(), text.trim().chars().count(), p.chars().count());
            (p, true)
        }
        Ok(_) => {
            tracing::warn!("[polish] 整理结果为空，回退原文");
            super::commit::notify(app, "Voxis", "整理结果为空，已用原文");
            (text.to_string(), false)
        }
        Err(e) => {
            tracing::warn!("[polish] 整理失败（{}ms），回退原文: {e}", t0.elapsed().as_millis());
            super::commit::notify(app, "Voxis", "整理失败，已用原文");
            (text.to_string(), false)
        }
    }
}

/// manual 模式入口（热键/命令触发，spawn_blocking）：读剪贴板 → 清洗 → 写回 + 注入粘贴。
pub fn polish_clipboard_flow(app: &AppHandle) -> Result<String, String> {
    let (polish, api_key) = polish_cfg(app)?;
    if polish.mode == "off" {
        return Err("整理功能已关闭（polish.mode=off）".into());
    }
    let text = super::commit::clipboard::paste_text()?;
    if text.trim().is_empty() {
        return Err("剪贴板没有文本".into());
    }
    let t0 = std::time::Instant::now();
    let polished = polish_text(&polish, &api_key, &text).map_err(|e| {
        tracing::warn!("[polish] manual 整理失败: {e}");
        e
    })?;
    if polished.trim().is_empty() {
        return Err("整理结果为空".into());
    }
    super::commit::clipboard::copy(&polished)?;
    tracing::info!("[polish] manual 完成（{}ms）：{} 字 → {} 字",
        t0.elapsed().as_millis(), text.trim().chars().count(), polished.chars().count());
    // 尝试注入粘贴；局限：只影响光标处，不替换已粘贴进第三方应用的内容
    if let Err(e) = super::commit::inject::inject_paste() {
        tracing::info!("[polish] 注入粘贴跳过（已写剪贴板）: {e}");
        super::commit::notify(app, "Voxis", "已整理并复制，Ctrl+V 粘贴");
    } else {
        super::commit::notify(app, "Voxis", "已整理");
    }
    Ok(polished)
}
