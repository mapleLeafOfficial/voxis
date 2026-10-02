// evdev 键码 ↔ 规范名（配置与 UI 录制共用）。修饰键左右键归并为同一名。
use evdev::KeyCode;

/// 键 → 规范名（Ctrl/Super/Shift/Alt/Space/…/A..Z/0..9）；不认识的键返回 None（热键状态机忽略）
pub fn canonical_name(key: KeyCode) -> Option<&'static str> {
    use evdev::KeyCode as K;
    let k = key.0;
    Some(match k {
        x if x == K::KEY_LEFTCTRL.0 || x == K::KEY_RIGHTCTRL.0 => "Ctrl",
        x if x == K::KEY_LEFTMETA.0 || x == K::KEY_RIGHTMETA.0 => "Super",
        x if x == K::KEY_LEFTSHIFT.0 || x == K::KEY_RIGHTSHIFT.0 => "Shift",
        x if x == K::KEY_LEFTALT.0 || x == K::KEY_RIGHTALT.0 => "Alt",
        x if x == K::KEY_SPACE.0 => "Space",
        x if x == K::KEY_ENTER.0 || x == K::KEY_KPENTER.0 => "Enter",
        x if x == K::KEY_TAB.0 => "Tab",
        x if x == K::KEY_ESC.0 => "Esc",
        // 字母 A..Z
        x if (K::KEY_A.0..=K::KEY_Z.0).contains(&x) => {
            ALPHA[(x - K::KEY_A.0) as usize]
        }
        // 数字 0..9
        x if (K::KEY_1.0..=K::KEY_0.0).contains(&x) => {
            let d = if x == K::KEY_0.0 { 0 } else { x - K::KEY_1.0 + 1 };
            DIGITS[d as usize]
        }
        _ => return None,
    })
}

/// 规范名 → 代表键（配置解析用；Ctrl 取左键为代表，判定时左右等价已由 canonical_name 归并）
pub fn key_from_name(name: &str) -> Option<KeyCode> {
    Some(match name {
        "Ctrl" => KeyCode::KEY_LEFTCTRL,
        "Super" => KeyCode::KEY_LEFTMETA,
        "Shift" => KeyCode::KEY_LEFTSHIFT,
        "Alt" => KeyCode::KEY_LEFTALT,
        "Space" => KeyCode::KEY_SPACE,
        "Enter" => KeyCode::KEY_ENTER,
        "Tab" => KeyCode::KEY_TAB,
        "Esc" => KeyCode::KEY_ESC,
        s if s.len() == 1 => {
            use evdev::KeyCode as K;
            let c = s.chars().next()?;
            match c {
                'a'..='z' | 'A'..='Z' => {
                    K::new(K::KEY_A.0 + (c.to_ascii_uppercase() as u16 - b'A' as u16))
                }
                '1'..='9' => K::new(K::KEY_1.0 + (c as u16 - b'1' as u16)),
                '0' => K::KEY_0,
                _ => return None,
            }
        }
        _ => return None,
    })
}

const ALPHA: [&str; 26] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];
const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];

/// 设置页可录制的规范键名全集（修饰键 + 功能键 + 字母 + 数字）
pub fn all_names() -> Vec<String> {
    let mut v: Vec<String> = [
        "Ctrl", "Super", "Shift", "Alt", "Space", "Enter", "Tab", "Esc",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    v.extend(ALPHA.iter().map(|s| s.to_string()));
    v.extend(DIGITS.iter().map(|s| s.to_string()));
    v
}
