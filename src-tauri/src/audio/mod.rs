// Voxis 录音引擎（PRD M2）— cpal 采集 → 重采样 16kHz/mono/i16 → PCM 回调 + RMS 音量事件
pub mod capture;
pub mod devices;
pub mod wav;
pub mod manager;

/// 事件名常量（todo4 会整合进 events.rs；当前 dev 命令直接用字面量）
    #[allow(dead_code)]
pub mod events {
    pub const VOLUME: &str = "session://volume";
    pub const ERROR: &str = "session://error";
    pub const MAX_DURATION: &str = "session://max_duration_reached";
}
