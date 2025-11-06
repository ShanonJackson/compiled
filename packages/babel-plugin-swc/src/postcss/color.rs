use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::HashMap;

const EPSILON: f32 = 1e-6;

#[derive(Clone, Copy, Debug)]
struct Rgba {
    r: u8,
    g: u8,
    b: u8,
    a: f32,
}

static NAME_TO_HEX: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let raw: Value =
        serde_json::from_str(include_str!("data/color_names.json")).expect("valid JSON");
    let mut map = HashMap::new();

    let Value::Object(entries) = raw else {
        return map;
    };

    for (name, value) in entries {
        if let Value::String(hex) = value {
            let name = name.into_boxed_str();
            let hex = hex.into_boxed_str();
            map.insert(Box::leak(name), Box::leak(hex));
        }
    }

    map
});

static HEX_TO_NAME: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut map = HashMap::new();
    for (name, hex) in NAME_TO_HEX.iter() {
        map.insert(*hex, *name);
    }
    map
});

pub fn minify_color(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let color = parse_color(trimmed)?;
    let minified = choose_minified_string(color);

    if minified.len() < trimmed.len() {
        Some(minified)
    } else {
        let lowered = trimmed.to_lowercase();
        if lowered != trimmed {
            Some(lowered)
        } else if trimmed != value {
            Some(trimmed.to_string())
        } else {
            None
        }
    }
}

pub fn is_color(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return false;
    }

    if trimmed.eq_ignore_ascii_case("transparent") || trimmed.eq_ignore_ascii_case("currentcolor") {
        return true;
    }

    parse_color(trimmed).is_some()
}

fn choose_minified_string(color: Rgba) -> String {
    let mut candidates: Vec<String> = Vec::new();

    if let Some(hex) = color_to_hex(color) {
        candidates.push(hex);
    }

    candidates.push(color_to_rgb(color));
    candidates.push(color_to_hsl(color));

    if is_zero(color.a) && color.r == 0 && color.g == 0 && color.b == 0 {
        candidates.push("transparent".to_string());
    } else if is_one(color.a) {
        if let Some(name) = to_color_name(color) {
            candidates.push(name.to_string());
        }
    }

    candidates
        .into_iter()
        .min_by(|a, b| a.len().cmp(&b.len()))
        .unwrap_or_else(|| color_to_rgb(color))
}

fn parse_color(value: &str) -> Option<Rgba> {
    if value.starts_with('#') {
        return parse_hex(value);
    }

    let lower = value.to_ascii_lowercase();

    if lower.starts_with("rgb(") || lower.starts_with("rgba(") {
        return parse_rgb_function(&lower);
    }

    if lower.starts_with("hsl(") || lower.starts_with("hsla(") {
        return parse_hsl_function(&lower);
    }

    if lower == "transparent" {
        return Some(Rgba {
            r: 0,
            g: 0,
            b: 0,
            a: 0.0,
        });
    }

    NAME_TO_HEX
        .get(lower.as_str())
        .and_then(|hex| parse_hex(hex))
}

fn parse_hex(value: &str) -> Option<Rgba> {
    let hex = value.trim_start_matches('#');
    let (r, g, b, a) = match hex.len() {
        3 => {
            let r = parse_hex_pair(&hex[0..1])?;
            let g = parse_hex_pair(&hex[1..2])?;
            let b = parse_hex_pair(&hex[2..3])?;
            (r, g, b, 255)
        }
        4 => {
            let r = parse_hex_pair(&hex[0..1])?;
            let g = parse_hex_pair(&hex[1..2])?;
            let b = parse_hex_pair(&hex[2..3])?;
            let a = parse_hex_pair(&hex[3..4])?;
            (r, g, b, a)
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            (r, g, b, 255)
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            (r, g, b, a)
        }
        _ => return None,
    };

    Some(Rgba {
        r,
        g,
        b,
        a: (a as f32) / 255.0,
    })
}

fn parse_hex_pair(segment: &str) -> Option<u8> {
    let ch = segment.chars().next()?;
    let value = ch.to_digit(16)? as u8;
    Some(value * 17)
}

fn parse_rgb_function(input: &str) -> Option<Rgba> {
    let params = input
        .strip_prefix("rgb(")
        .or_else(|| input.strip_prefix("rgba("))?;
    let params = params.trim_end_matches(')');
    let parts: Vec<&str> = params.split(',').map(|part| part.trim()).collect();

    if parts.len() < 3 {
        return None;
    }

    let r = parse_rgb_component(parts[0])?;
    let g = parse_rgb_component(parts[1])?;
    let b = parse_rgb_component(parts[2])?;

    let a = if parts.len() >= 4 {
        parse_alpha_component(parts[3])?
    } else {
        1.0
    };

    Some(Rgba { r, g, b, a })
}

fn parse_rgb_component(component: &str) -> Option<u8> {
    if let Some(stripped) = component.strip_suffix('%') {
        let percentage = stripped.parse::<f32>().ok()?;
        let clamped = percentage.clamp(0.0, 100.0);
        let value = (clamped * 255.0 / 100.0).round();
        Some(value as u8)
    } else {
        let value = component.parse::<f32>().ok()?;
        let clamped = value.clamp(0.0, 255.0).round();
        Some(clamped as u8)
    }
}

fn parse_alpha_component(component: &str) -> Option<f32> {
    if let Some(stripped) = component.strip_suffix('%') {
        let percentage = stripped.parse::<f32>().ok()?;
        Some((percentage / 100.0).clamp(0.0, 1.0))
    } else {
        let value = component.parse::<f32>().ok()?;
        Some(value.clamp(0.0, 1.0))
    }
}

fn parse_hsl_function(input: &str) -> Option<Rgba> {
    let params = input
        .strip_prefix("hsl(")
        .or_else(|| input.strip_prefix("hsla("))?;
    let params = params.trim_end_matches(')');
    let parts: Vec<&str> = params.split(',').map(|part| part.trim()).collect();

    if parts.len() < 3 {
        return None;
    }

    let h = parse_hue(parts[0])?;
    let s = parse_percentage(parts[1])?;
    let l = parse_percentage(parts[2])?;

    let a = if parts.len() >= 4 {
        parse_alpha_component(parts[3])?
    } else {
        1.0
    };

    let (r, g, b) = hsl_to_rgb(h, s, l);
    Some(Rgba { r, g, b, a })
}

fn parse_hue(value: &str) -> Option<f32> {
    let value = value.trim();
    if let Some(stripped) = value.strip_suffix("deg") {
        stripped.parse::<f32>().ok()
    } else if let Some(stripped) = value.strip_suffix("rad") {
        stripped
            .parse::<f32>()
            .ok()
            .map(|radians| radians.to_degrees())
    } else if let Some(stripped) = value.strip_suffix("grad") {
        stripped.parse::<f32>().ok().map(|grad| grad * 0.9)
    } else if let Some(stripped) = value.strip_suffix("turn") {
        stripped.parse::<f32>().ok().map(|turn| turn * 360.0)
    } else {
        value.parse::<f32>().ok()
    }
}

fn parse_percentage(value: &str) -> Option<f32> {
    if let Some(stripped) = value.strip_suffix('%') {
        stripped.parse::<f32>().ok()
    } else {
        value.parse::<f32>().ok().map(|number| number * 100.0)
    }
}

fn hsl_to_rgb(h: f32, s_percent: f32, l_percent: f32) -> (u8, u8, u8) {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let s = (s_percent / 100.0).clamp(0.0, 1.0);
    let l = (l_percent / 100.0).clamp(0.0, 1.0);

    if s <= EPSILON {
        let value = (l * 255.0).round() as u8;
        return (value, value, value);
    }

    let q = if l < 0.5 {
        l * (1.0 + s)
    } else {
        l + s - l * s
    };
    let p = 2.0 * l - q;

    let r = hue_to_rgb(p, q, h / 360.0 + 1.0 / 3.0);
    let g = hue_to_rgb(p, q, h / 360.0);
    let b = hue_to_rgb(p, q, h / 360.0 - 1.0 / 3.0);

    (r, g, b)
}

fn hue_to_rgb(p: f32, q: f32, mut t: f32) -> u8 {
    if t < 0.0 {
        t += 1.0;
    }
    if t > 1.0 {
        t -= 1.0;
    }

    let value = if t < 1.0 / 6.0 {
        p + (q - p) * 6.0 * t
    } else if t < 1.0 / 2.0 {
        q
    } else if t < 2.0 / 3.0 {
        p + (q - p) * (2.0 / 3.0 - t) * 6.0
    } else {
        p
    };

    (value * 255.0).round() as u8
}

fn color_to_hex(color: Rgba) -> Option<String> {
    let alpha_byte = (color.a * 255.0).round() as u8;

    if color.a > 0.0 && color.a < 1.0 {
        let precise = (alpha_byte as f32) / 255.0;
        if (precise - color.a).abs() > 1e-3 {
            return None;
        }
    }

    let mut hex = format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b);

    if !is_one(color.a) {
        hex.push_str(&format!("{:02x}", alpha_byte));
    }

    let bytes = hex.as_bytes();
    if is_one(color.a) {
        if bytes[1] == bytes[2] && bytes[3] == bytes[4] && bytes[5] == bytes[6] {
            return Some(format!(
                "#{}{}{}",
                bytes[1] as char, bytes[3] as char, bytes[5] as char
            ));
        }
        Some(hex)
    } else {
        if bytes.len() == 9
            && bytes[1] == bytes[2]
            && bytes[3] == bytes[4]
            && bytes[5] == bytes[6]
            && bytes[7] == bytes[8]
        {
            return Some(format!(
                "#{}{}{}{}",
                bytes[1] as char, bytes[3] as char, bytes[5] as char, bytes[7] as char
            ));
        }
        Some(hex)
    }
}

fn color_to_rgb(color: Rgba) -> String {
    if is_one(color.a) {
        format!("rgb({},{},{})", color.r, color.g, color.b)
    } else {
        format!(
            "rgba({},{},{},{})",
            color.r,
            color.g,
            color.b,
            format_float(color.a)
        )
    }
}

fn color_to_hsl(color: Rgba) -> String {
    let r = color.r as f32 / 255.0;
    let g = color.g as f32 / 255.0;
    let b = color.b as f32 / 255.0;

    let max = r.max(g.max(b));
    let min = r.min(g.min(b));
    let mut h = 0.0;
    let l = (max + min) / 2.0;

    let mut s = 0.0;
    if (max - min).abs() > EPSILON {
        let d = max - min;
        s = if l > 0.5 {
            d / (2.0 - max - min)
        } else {
            d / (max + min)
        };

        h = if (max - r).abs() < EPSILON {
            (g - b) / d + if g < b { 6.0 } else { 0.0 }
        } else if (max - g).abs() < EPSILON {
            (b - r) / d + 2.0
        } else {
            (r - g) / d + 4.0
        };

        h /= 6.0;
    }

    let h_deg = (h * 360.0) % 360.0;
    let s_percent = s * 100.0;
    let l_percent = l * 100.0;

    if is_one(color.a) {
        format!(
            "hsl({},{:.0}%,{:.0}%)",
            format_float(h_deg),
            s_percent,
            l_percent
        )
    } else {
        format!(
            "hsla({},{:.0}%,{:.0}%,{})",
            format_float(h_deg),
            s_percent,
            l_percent,
            format_float(color.a)
        )
    }
}

fn to_color_name(color: Rgba) -> Option<&'static str> {
    if !is_one(color.a) {
        return None;
    }

    let hex = format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b);
    HEX_TO_NAME.get(hex.as_str()).copied()
}

fn is_zero(value: f32) -> bool {
    value.abs() <= EPSILON
}

fn is_one(value: f32) -> bool {
    (value - 1.0).abs() <= EPSILON
}

fn format_float(value: f32) -> String {
    if is_zero(value) {
        return "0".to_string();
    }

    if is_one(value) {
        return "1".to_string();
    }

    let mut s = format!("{:.6}", value);
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    if s.starts_with("0.") {
        s.remove(0);
    } else if s.starts_with("-0.") {
        s.remove(1);
    }
    s
}

#[cfg(test)]
mod tests {
    use super::minify_color;

    #[test]
    fn minifies_named_color() {
        assert_eq!(minify_color("rebeccapurple").as_deref(), Some("#639"));
    }

    #[test]
    fn lowercases_equivalent_values() {
        assert_eq!(minify_color("#FFAA00").as_deref(), Some("#fa0"));
    }

    #[test]
    fn leaves_unknown_values() {
        assert_eq!(minify_color("var(--color)").is_none(), true);
    }
}
