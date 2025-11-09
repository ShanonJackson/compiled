use once_cell::sync::Lazy;
use regex::Regex;

use crate::types::StyleRule;

static CLASS_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\.([A-Za-z_-][A-Za-z0-9_-]*)").expect("valid class name regex"));

static KEYFRAMES_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"@keyframes\s+([A-Za-z0-9_-]+)").expect("valid keyframes regex"));

fn split_rule(rule: &str) -> (String, String) {
    let trimmed = rule.trim();

    if trimmed.is_empty() {
        return (String::new(), String::new());
    }

    if let Some(start) = trimmed.find('{') {
        let mut depth = 0usize;
        let mut end = trimmed.len();

        for (idx, ch) in trimmed.char_indices().skip(start) {
            match ch {
                '{' => depth += 1,
                '}' => {
                    if depth == 0 {
                        continue;
                    }

                    depth -= 1;
                    if depth == 0 {
                        end = idx;
                        break;
                    }
                }
                _ => {}
            }
        }

        let selector = trimmed[..start].trim().to_string();
        let body = if start + 1 < end {
            trimmed[start + 1..end].trim().to_string()
        } else {
            String::new()
        };

        (selector, body)
    } else {
        (trimmed.to_string(), String::new())
    }
}

fn find_identifier(text: &str) -> Option<String> {
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return None;
    }

    if let Some(caps) = KEYFRAMES_REGEX.captures(trimmed) {
        return Some(caps[1].to_string());
    }

    CLASS_REGEX
        .captures(trimmed)
        .map(|caps| caps[1].to_string())
}

/// Parse a serialized atomic CSS rule into a structured representation so the
/// native transformer can emit metadata mirroring the Babel behaviour.
pub fn parse_style_rule(rule: &str) -> StyleRule {
    if rule.trim().is_empty() {
        return StyleRule::default();
    }

    let (selector, css_text) = split_rule(rule);

    let class_name = find_identifier(&selector)
        .or_else(|| find_identifier(&css_text))
        .unwrap_or_default();

    StyleRule {
        class_name,
        selector,
        css_text,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_style_rule;

    #[test]
    fn parses_simple_rule() {
        let rule = parse_style_rule("._syaz5scu{color:red}");
        assert_eq!(rule.class_name, "_syaz5scu");
        assert_eq!(rule.selector, "._syaz5scu");
        assert_eq!(rule.css_text, "color:red");
    }

    #[test]
    fn parses_rule_with_pseudo() {
        let rule = parse_style_rule("._1wyb1fwx:hover{color:red}");
        assert_eq!(rule.class_name, "_1wyb1fwx");
        assert_eq!(rule.selector, "._1wyb1fwx:hover");
        assert_eq!(rule.css_text, "color:red");
    }

    #[test]
    fn parses_media_rule() {
        let rule = parse_style_rule("@media print{._1wyb1fwx{color:red}}");
        assert_eq!(rule.class_name, "_1wyb1fwx");
        assert_eq!(rule.selector, "@media print");
        assert_eq!(rule.css_text, "._1wyb1fwx{color:red}");
    }

    #[test]
    fn parses_keyframes_rule() {
        let rule = parse_style_rule("@keyframes _123abc{0%{opacity:0}to{opacity:1}}");
        assert_eq!(rule.class_name, "_123abc");
        assert_eq!(rule.selector, "@keyframes _123abc");
        assert_eq!(rule.css_text, "0%{opacity:0}to{opacity:1}");
    }
}
