#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnitValue {
    pub number: String,
    pub unit: String,
}

pub fn parse_unit(value: &str) -> Option<UnitValue> {
    if value.is_empty() {
        return None;
    }

    let bytes = value.as_bytes();
    let mut pos = 0usize;
    let len = bytes.len();

    if !starts_with_number(bytes) {
        return None;
    }

    if pos < len && (bytes[pos] == b'+' || bytes[pos] == b'-') {
        pos += 1;
    }

    while pos < len {
        let ch = bytes[pos];
        if !(b'0'..=b'9').contains(&ch) {
            break;
        }
        pos += 1;
    }

    if pos < len {
        let ch = bytes[pos];
        if ch == b'.' {
            let next = bytes.get(pos + 1);
            if matches!(next, Some(b'0'..=b'9')) {
                pos += 2;
                while pos < len {
                    let ch = bytes[pos];
                    if !(b'0'..=b'9').contains(&ch) {
                        break;
                    }
                    pos += 1;
                }
            }
        }
    }

    if pos < len {
        let ch = bytes[pos];
        if ch == b'e' || ch == b'E' {
            let next = bytes.get(pos + 1).copied();
            let next_next = bytes.get(pos + 2).copied();
            if matches!(next, Some(b'0'..=b'9'))
                || (matches!(next, Some(b'+' | b'-')) && matches!(next_next, Some(b'0'..=b'9')))
            {
                pos += if matches!(next, Some(b'+' | b'-')) {
                    3
                } else {
                    2
                };
                while pos < len {
                    let ch = bytes[pos];
                    if !(b'0'..=b'9').contains(&ch) {
                        break;
                    }
                    pos += 1;
                }
            }
        }
    }

    let number = value[..pos].to_string();
    let unit = value[pos..].to_string();
    Some(UnitValue { number, unit })
}

fn starts_with_number(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    let first = bytes[0];
    if first == b'+' || first == b'-' {
        if bytes.len() < 2 {
            return false;
        }
        let second = bytes[1];
        if (b'0'..=b'9').contains(&second) {
            return true;
        }
        if second == b'.' {
            return bytes.get(2).map_or(false, |c| (b'0'..=b'9').contains(c));
        }
        return false;
    }

    if first == b'.' {
        return bytes.get(1).map_or(false, |c| (b'0'..=b'9').contains(c));
    }

    (b'0'..=b'9').contains(&first)
}
