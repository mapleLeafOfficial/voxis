// 输入设备枚举（PRD M2）
use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;

/// 路由首选设备： ALSA 的 `pipewire` PCM 由 WirePlumber 动态路由到当前默认源
/// （本机实测 `default` 别名不跟随默认源切换，虚拟麦克风场景会录到静音）
pub const PREFERRED_DEVICE: &str = "pipewire";

#[derive(Debug, Clone, Serialize)]
pub struct InputDevice {
    /// 设备名（同时作为配置里的 device id）
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

pub fn list_input_devices() -> Result<Vec<InputDevice>, String> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|d| d.name().ok());

    let mut out = Vec::new();
    let devices = host
        .input_devices()
        .map_err(|e| format!("枚举输入设备失败: {e}"))?;
    for d in devices {
        if let Ok(name) = d.name() {
            // PipeWire 下同名设备可能去重出现，按名字去重
            if out.iter().any(|x: &InputDevice| x.name == name) {
                continue;
            }
            out.push(InputDevice {
                id: name.clone(),
                is_default: default_name.as_deref() == Some(name.as_str()),
                name,
            });
        }
    }
    // 排序：pipewire（路由首选）→ ALSA default → 其余
    out.sort_by_key(|d| {
        if d.name == PREFERRED_DEVICE { 0 } else if d.is_default { 1 } else { 2 }
    });
    Ok(out)
}

/// 解析「未指定设备」时应使用的设备：优先 `pipewire` PCM，退回系统默认
pub fn resolve_default_device(host: &cpal::Host) -> Result<cpal::Device, String> {
    host.devices()
        .map_err(|e| format!("枚举设备失败: {e}"))?
        .find(|d| d.name().map(|n| n == PREFERRED_DEVICE).unwrap_or(false))
        .or_else(|| host.default_input_device())
        .ok_or_else(|| "没有可用的输入设备".to_string())
}
