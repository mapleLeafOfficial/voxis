// 会话事件总线：统一事件名与 payload（发到全局 webview）
use serde::Serialize;

pub const STATE: &str = "session://state";
pub const PARTIAL: &str = "session://partial";
pub const SENTENCE: &str = "session://sentence";
pub const VOLUME: &str = "session://volume";
pub const ERROR: &str = "session://error";
pub const COMMITTED: &str = "session://committed";
pub const MAX_DURATION: &str = "session://max_duration_reached";

#[derive(Debug, Clone, Serialize)]
pub struct ErrorPayload {
    /// "key" | "audio" | "asr"
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct CommittedPayload {
    pub text: String,
    /// 提交结果三态：pasted（已上屏）/ copied（仅复制）/ none（空文本）
    pub result: String,
}
