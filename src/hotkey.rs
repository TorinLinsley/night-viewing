// 快捷键的解析与文本化
pub const M_ALT: u32 = 0x0001;
pub const M_CTRL: u32 = 0x0002;
pub const M_SHIFT: u32 = 0x0004;
pub const M_WIN: u32 = 0x0008;

pub fn key_name(vk: u32) -> String {
    let s = match vk {
        0x26 => "↑".to_string(),
        0x28 => "↓".to_string(),
        0x25 => "←".to_string(),
        0x27 => "→".to_string(),
        0x21 => "PageUp".to_string(),
        0x22 => "PageDown".to_string(),
        0x24 => "Home".to_string(),
        0x23 => "End".to_string(),
        0x2D => "Insert".to_string(),
        0x2E => "Delete".to_string(),
        0x20 => "Space".to_string(),
        0x09 => "Tab".to_string(),
        0x0D => "Enter".to_string(),
        0x08 => "Backspace".to_string(),
        0x13 => "Pause".to_string(),
        0x2C => "PrintScreen".to_string(),
        v if (0x30..=0x39).contains(&v) => ((b'0' + (v - 0x30) as u8) as char).to_string(),
        v if (0x41..=0x5A).contains(&v) => ((b'A' + (v - 0x41) as u8) as char).to_string(),
        v if (0x70..=0x87).contains(&v) => format!("F{}", v - 0x6F),
        v if v == 0xBA => ";".to_string(),
        v if v == 0xBB => "=".to_string(),
        v if v == 0xBC => ",".to_string(),
        v if v == 0xBD => "-".to_string(),
        v if v == 0xBE => ".".to_string(),
        v if v == 0xBF => "/".to_string(),
        v if v == 0xC0 => "`".to_string(),
        v if v == 0xDB => "[".to_string(),
        v if v == 0xDC => "\\".to_string(),
        v if v == 0xDD => "]".to_string(),
        v if v == 0xDE => "'".to_string(),
        v => format!("0x{v:02X}"),
    };
    s
}

pub fn parse(spec: &str) -> Option<(u32, u32)> {
    let mut mods = 0u32;
    let mut vk = None;
    for part in spec.split('+') {
        let p = part.trim();
        if p.is_empty() {
            continue;
        }
        let low = p.to_ascii_lowercase();
        match low.as_str() {
            "ctrl" | "control" => mods |= M_CTRL,
            "alt" => mods |= M_ALT,
            "shift" => mods |= M_SHIFT,
            "win" | "super" | "meta" => mods |= M_WIN,
            _ => vk = vk_from(&low),
        }
    }
    let key = vk?;
    if mods == 0 {
        return None;
    }
    Some((mods, key))
}

fn vk_from(low: &str) -> Option<u32> {
    let v = match low {
        "up" | "↑" => 0x26,
        "down" | "↓" => 0x28,
        "left" | "←" => 0x25,
        "right" | "→" => 0x27,
        "pageup" | "pgup" => 0x21,
        "pagedown" | "pgdn" => 0x22,
        "home" => 0x24,
        "end" => 0x23,
        "insert" | "ins" => 0x2D,
        "delete" | "del" => 0x2E,
        "space" => 0x20,
        "tab" => 0x09,
        "enter" | "return" => 0x0D,
        "backspace" => 0x08,
        "pause" => 0x13,
        "printscreen" => 0x2C,
        ";" => 0xBA,
        "=" => 0xBB,
        "," => 0xBC,
        "-" => 0xBD,
        "." => 0xBE,
        "/" => 0xBF,
        "`" => 0xC0,
        "[" => 0xDB,
        "\\" => 0xDC,
        "]" => 0xDD,
        "'" => 0xDE,
        other => {
            let chars: Vec<char> = other.chars().collect();
            if chars.len() == 1 {
                let c = chars[0];
                if c.is_ascii_digit() {
                    return Some(0x30 + (c as u32 - '0' as u32));
                }
                if c.is_ascii_alphabetic() {
                    return Some(0x41 + (c.to_ascii_uppercase() as u32 - 'A' as u32));
                }
            }
            if let Some(num) = other.strip_prefix('f') {
                if let Ok(n) = num.parse::<u32>() {
                    if (1..=24).contains(&n) {
                        return Some(0x6F + n);
                    }
                }
            }
            return None;
        }
    };
    Some(v)
}

pub fn format(mods: u32, vk: u32) -> String {
    let mut out = String::new();
    if mods & M_CTRL != 0 {
        out.push_str("Ctrl+");
    }
    if mods & M_ALT != 0 {
        out.push_str("Alt+");
    }
    if mods & M_SHIFT != 0 {
        out.push_str("Shift+");
    }
    if mods & M_WIN != 0 {
        out.push_str("Win+");
    }
    out.push_str(&key_name(vk));
    out
}
