// 最小 WAV 读取（16kHz/mono/PCM16）— 冒烟与测试用
pub fn load_16k_mono(path: &str) -> Result<Vec<i16>, String> {
    let b = std::fs::read(path).map_err(|e| format!("读 wav 失败: {e}"))?;
    if b.len() < 44 || &b[0..4] != b"RIFF" {
        return Err("不是 RIFF WAV".into());
    }
    let mut i = 12usize;
    let (mut rate, mut ch, mut bits, mut data): (u32, u16, u16, &[u8]) =
        (0, 0, 0, &[]);
    while i + 8 <= b.len() {
        let id = &b[i..i + 4];
        let size = u32::from_le_bytes(b[i + 4..i + 8].try_into().unwrap()) as usize;
        if id == b"fmt " {
            ch = u16::from_le_bytes(b[i + 10..i + 12].try_into().unwrap());
            rate = u32::from_le_bytes(b[i + 12..i + 16].try_into().unwrap());
            bits = u16::from_le_bytes(b[i + 22..i + 24].try_into().unwrap());
        } else if id == b"data" {
            let end = (i + 8 + size).min(b.len());
            data = &b[i + 8..end];
        }
        i += 8 + size + (size & 1);
    }
    if (rate, ch, bits) != (16000, 1, 16) {
        return Err(format!("需 16kHz/mono/PCM16，实际 {rate}Hz/{ch}ch/{bits}bit"));
    }
    Ok(data
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]))
        .collect())
}
