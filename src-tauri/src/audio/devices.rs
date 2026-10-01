// 输入设备枚举（PRD M2）
use cpal::traits::{DeviceTrait, HostTrait};
use serde::Serialize;

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
    // 默认设备排最前
    out.sort_by_key(|d| !d.is_default);
    Ok(out)
}
