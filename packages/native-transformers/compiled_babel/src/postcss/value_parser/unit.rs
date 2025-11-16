#[derive(Clone, Debug, PartialEq)]
pub struct NumberUnit {
    pub number: String,
    pub unit: String,
}

pub fn unit(value: &str) -> Option<NumberUnit> {
    // Very small port of unit.js to support basic patterns
    if value.is_empty() {
        return None;
    }
    let mut pos = 0usize;
    let bytes = value.as_bytes();
    if bytes[pos] == b'+' || bytes[pos] == b'-' {
        pos += 1;
    }
    while pos < bytes.len() && bytes[pos].is_ascii_digit() {
        pos += 1;
    }
    if pos < bytes.len() && bytes[pos] == b'.' {
        pos += 1;
        while pos < bytes.len() && bytes[pos].is_ascii_digit() {
            pos += 1;
        }
    }
    // exponent not needed for our usage
    let (num, uni) = value.split_at(pos);
    if num.is_empty() {
        return None;
    }
    Some(NumberUnit {
        number: num.to_string(),
        unit: uni.to_string(),
    })
}
