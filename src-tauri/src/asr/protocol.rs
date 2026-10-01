// DashScope ASR 协议：JSON 文本帧控制 + 裸 PCM 二进制帧（无帧头）
// 参考：voice_input.py 已验证实现（run-task → task-started → binary → result-generated → finish-task → task-finished）
use serde_json::{json, Value};

/// 32 位十六进制任务 ID
pub fn new_task_id() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

/// 构造 run-task 请求
pub fn build_run_task(
    model: &str,
    sample_rate: u32,
    max_sentence_silence_ms: u32,
    semantic_punctuation: bool,
    language_hints: Option<&[String]>,
    task_id: &str,
) -> Value {
    let mut parameters = json!({
        "format": "pcm",
        "sample_rate": sample_rate,
        "max_sentence_silence": max_sentence_silence_ms,
        "semantic_punctuation_enabled": semantic_punctuation,
    });
    if let Some(langs) = language_hints {
        if !langs.is_empty() {
            parameters["language_hints"] = json!(langs);
        }
    }
    json!({
        "header": { "action": "run-task", "task_id": task_id, "streaming": "duplex" },
        "payload": {
            "task_group": "audio",
            "task": "asr",
            "function": "recognition",
            "model": model,
            "parameters": parameters,
            "input": {},
        }
    })
}

/// 构造 finish-task 请求
pub fn build_finish_task(task_id: &str) -> Value {
    json!({
        "header": { "action": "finish-task", "task_id": task_id, "streaming": "duplex" },
        "payload": { "input": {} },
    })
}

/// 服务端事件的解析结果
#[derive(Debug, Clone, PartialEq)]
pub enum ServerEvent {
    TaskStarted,
    /// 中间结果（未断句）
    Partial(String),
    /// 断句完成（sentence_end=true）
    Sentence(String),
    TaskFinished,
    Failed { code: String, message: String },
    /// 心跳等可忽略消息
    Ignored,
}

/// 解析服务端 JSON 文本帧
pub fn parse_server_message(text: &str) -> ServerEvent {
    let Ok(v) = serde_json::from_str::<Value>(text) else {
        return ServerEvent::Ignored;
    };
    let header = v.get("header").cloned().unwrap_or(Value::Null);
    let event = header.get("event").and_then(|e| e.as_str()).unwrap_or("");
    match event {
        "task-started" => ServerEvent::TaskStarted,
        "task-finished" => ServerEvent::TaskFinished,
        "task-failed" => ServerEvent::Failed {
            code: header
                .get("error_code")
                .and_then(|c| c.as_str())
                .unwrap_or("unknown")
                .to_string(),
            message: header
                .get("error_message")
                .and_then(|m| m.as_str())
                .unwrap_or("task-failed")
                .to_string(),
        },
        "result-generated" => {
            let sentence = v
                .pointer("/payload/output/sentence")
                .cloned()
                .unwrap_or(Value::Null);
            // 心跳消息（如空格）忽略
            if sentence.get("heartbeat").and_then(|h| h.as_bool()).unwrap_or(false) {
                return ServerEvent::Ignored;
            }
            let text = sentence.get("text").and_then(|t| t.as_str()).unwrap_or("");
            if sentence.get("sentence_end").and_then(|s| s.as_bool()).unwrap_or(false) {
                ServerEvent::Sentence(text.to_string())
            } else {
                ServerEvent::Partial(text.to_string())
            }
        }
        _ => ServerEvent::Ignored,
    }
}
