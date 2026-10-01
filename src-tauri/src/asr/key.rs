// API Key 解析：env QWEN_API_KEY > DASHSCOPE_API_KEY > 配置文件
pub fn resolve_api_key(config_key: &str) -> Option<String> {
    for var in ["QWEN_API_KEY", "DASHSCOPE_API_KEY"] {
        if let Ok(v) = std::env::var(var) {
            let v = v.trim();
            if !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    let k = config_key.trim();
    if k.is_empty() { None } else { Some(k.to_string()) }
}
