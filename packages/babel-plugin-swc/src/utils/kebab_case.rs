//! Kebab-case helper mirroring `packages/utils/src/kebab-case.ts`.

#[allow(dead_code)]
pub fn kebab_case(input: &str) -> String {
    let mut result = String::with_capacity(input.len());

    for ch in input.chars() {
        if ch.is_uppercase() || matches!(ch, '\u{00C0}'..='\u{00D6}' | '\u{00D8}'..='\u{00DE}') {
            result.push('-');
            for lower in ch.to_lowercase() {
                result.push(lower);
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::kebab_case;

    #[test]
    fn converts_camel_case() {
        assert_eq!(kebab_case("backgroundColor"), "background-color");
        assert_eq!(kebab_case("fontSize"), "font-size");
    }

    #[test]
    fn handles_unicode() {
        assert_eq!(kebab_case("DéjàVu"), "-déjà-vu");
    }
}
