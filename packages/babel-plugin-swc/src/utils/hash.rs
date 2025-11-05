//! Murmur2 hashing routine ported from the JavaScript implementation in
//! `packages/utils/src/hash.ts`.
//!
//! The Babel plugin relies on this helper when building atomic class names.
//! To guarantee cache key parity between the Rust and TypeScript pipelines we
//! keep the implementation byte-for-byte compatible with the original version.

/// Hashes a string using the murmur2 algorithm.
///
/// The function mirrors the JavaScript implementation which operates on
/// unsigned 32bit integers and returns the lower-case base36 representation of
/// the final hash. All bitwise operations are carefully cast to ensure the
/// wrap-around semantics match those of the JavaScript runtime.
#[allow(dead_code)]
pub fn murmur2_hash(value: &str, seed: u32) -> String {
    let mut length = value.len();
    let mut h = seed ^ length as u32;
    let mut index = 0usize;

    let bytes = value.as_bytes();

    while length >= 4 {
        let mut k = (bytes[index] as u32)
            | ((bytes[index + 1] as u32) << 8)
            | ((bytes[index + 2] as u32) << 16)
            | ((bytes[index + 3] as u32) << 24);

        k = k.wrapping_mul(0x5bd1e995);
        k ^= k >> 24;
        k = k.wrapping_mul(0x5bd1e995);

        h = h.wrapping_mul(0x5bd1e995) ^ k;

        length -= 4;
        index += 4;
    }

    match length {
        3 => {
            h ^= (bytes[index + 2] as u32) << 16;
            h ^= (bytes[index + 1] as u32) << 8;
            h ^= bytes[index] as u32;
            h = h.wrapping_mul(0x5bd1e995);
        }
        2 => {
            h ^= (bytes[index + 1] as u32) << 8;
            h ^= bytes[index] as u32;
            h = h.wrapping_mul(0x5bd1e995);
        }
        1 => {
            h ^= bytes[index] as u32;
            h = h.wrapping_mul(0x5bd1e995);
        }
        _ => {}
    }

    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1e995);
    h ^= h >> 15;

    to_base36(h)
}

fn to_base36(mut value: u32) -> String {
    if value == 0 {
        return "0".to_owned();
    }

    let mut buffer = Vec::new();

    while value > 0 {
        let digit = value % 36;
        let ch = match digit {
            0..=9 => (b'0' + digit as u8) as char,
            _ => (b'a' + (digit - 10) as u8) as char,
        };
        buffer.push(ch);
        value /= 36;
    }

    buffer.iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::murmur2_hash;

    #[test]
    fn matches_js_reference() {
        assert_eq!(murmur2_hash("", 0), "0");
        assert_eq!(murmur2_hash("compiled", 0), "3mvezc");
        assert_eq!(murmur2_hash("compiled", 1), "yzbs45");
        assert_eq!(murmur2_hash("css", 0), "12w0n9j");
        assert_eq!(murmur2_hash("css", 123), "9o2v1a");
    }
}
