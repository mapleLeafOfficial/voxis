// Voxis 配置层 — 对应 PRD §3.1
// 路径：~/.config/voxis/config.json（0600 权限，原子写入）
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::RwLock;

// ---------- 各分组配置 ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct AsrConfig {
    pub model: String,
    pub ws_url: String,
    pub silence_ms: u32,
    pub semantic_punct: bool,
    pub language: String,
    pub max_duration: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct HotkeyConfig {
    pub hold: Vec<String>,
    pub lock: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct InputConfig {
    pub device: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct CommitConfig {
    pub mode: String, // "auto" | "clipboard_only"
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct PolishConfig {
    pub mode: String, // "off" | "auto" | "manual"
    pub model: String,
    pub hotkey: String,
    pub prompt_override: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct UiConfig {
    pub bubble_pos: String, // bottom_center | top_left | top_right | bottom_left | bottom_right
    pub theme: String,      // light | dark | system
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct GeneralConfig {
    pub autostart: bool,
    pub log_level: String,
}

// ---------- 顶层配置 ----------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub api_key: String,
    pub asr: AsrConfig,
    pub hotkey: HotkeyConfig,
    pub input: InputConfig,
    pub commit: CommitConfig,
    pub polish: PolishConfig,
    pub ui: UiConfig,
    pub general: GeneralConfig,
}

impl Config {
    /// 默认值（PRD §3.1）
    pub fn with_defaults() -> Self {
        Config {
            api_key: String::new(),
            asr: AsrConfig {
                // 实测 qwen3-asr-flash-streaming 对现有 Key 返回 ModelNotFound，默认用已验证可用的模型
                model: "qwen-audio-3.0-asr-flash-streaming".into(),
                ws_url: "wss://dashscope.aliyuncs.com/api-ws/v1/inference".into(),
                silence_ms: 1300,
                semantic_punct: true,
                language: String::new(),
                max_duration: 60,
            },
            hotkey: HotkeyConfig {
                // Windows 默认 Ctrl+Alt 系：Win 键按住/松开涉及系统行为（开始菜单），避开更干净
                #[cfg(target_os = "linux")]
                hold: vec!["Ctrl".into(), "Super".into()],
                #[cfg(target_os = "linux")]
                lock: vec!["Ctrl".into(), "Super".into(), "Shift".into()],
                #[cfg(target_os = "windows")]
                hold: vec!["Ctrl".into(), "Alt".into()],
                #[cfg(target_os = "windows")]
                lock: vec!["Ctrl".into(), "Alt".into(), "Shift".into()],
            },
            // ALSA `pipewire` PCM 跟随 WirePlumber 默认源（本机 `default` 别名不跟随，虚拟麦会静音）
            input: InputConfig { device: "pipewire".into() },
            commit: CommitConfig { mode: "auto".into() },
            polish: PolishConfig {
                mode: "off".into(),
                model: "qwen-flash".into(),
                #[cfg(target_os = "linux")]
                hotkey: "Ctrl+Super+O".into(),
                #[cfg(target_os = "windows")]
                hotkey: "Ctrl+Alt+O".into(),
                prompt_override: None,
            },
            ui: UiConfig { bubble_pos: "bottom_center".into(), theme: "system".into() },
            general: GeneralConfig { autostart: false, log_level: "info".into() },
        }
    }
}

// ---------- 路径 ----------

pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voxis")
        .join("config.json")
}

pub fn data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("voxis")
}

// ---------- 加载 / 保存 ----------

/// 读取配置；文件不存在或解析失败时写入默认配置并返回默认值。
pub fn load() -> Result<Config, String> {
    let path = config_path();
    if !path.exists() {
        let cfg = Config::with_defaults();
        save(&cfg)?;
        return Ok(cfg);
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("读取配置失败: {e}"))?;
    match serde_json::from_str::<Config>(&raw) {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            tracing::warn!("配置解析失败（{e}），使用默认配置");
            let cfg = Config::with_defaults();
            save(&cfg)?;
            Ok(cfg)
        }
    }
}

/// 原子保存：临时文件 → 0600 → rename。未知字段在解析时被忽略，保存即规范化。
pub fn save(cfg: &Config) -> Result<(), String> {
    let path = config_path();
    let dir = path
        .parent()
        .ok_or_else(|| "配置路径异常".to_string())?;
    fs::create_dir_all(dir).map_err(|e| format!("创建配置目录失败: {e}"))?;

    let json = serde_json::to_string_pretty(cfg).map_err(|e| format!("序列化配置失败: {e}"))?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("写入配置失败: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))
            .map_err(|e| format!("设置配置权限失败: {e}"))?;
    }
    fs::rename(&tmp, &path).map_err(|e| format!("替换配置文件失败: {e}"))?;
    Ok(())
}

/// 全局配置持有（AppState 使用）
pub type SharedConfig = RwLock<Config>;
