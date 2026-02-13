//! CSS value list splitting utilities — 1:1 port of PostCSS 8.4.31 `list.js`.
//!
//! Splits CSS value lists by commas or spaces while respecting quoted strings
//! and parenthetical nesting.

/// Split a string by commas, respecting quotes and parens.
pub fn comma(string: &str) -> Vec<String> {
    split(string, &[','], true)
}

/// Split a string by whitespace characters, respecting quotes and parens.
pub fn space(string: &str) -> Vec<String> {
    split(string, &[' ', '\n', '\t'], false)
}

/// Core splitting function — port of `list.split()` from PostCSS 8.4.31.
///
/// - `separators`: characters to split on
/// - `last`: if true, always push the final segment (even if empty after trim)
pub fn split(string: &str, separators: &[char], last: bool) -> Vec<String> {
    let mut array: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut do_split = false;

    let mut func: usize = 0; // parenthesis nesting depth
    let mut in_quote = false;
    let mut prev_quote = '\0';
    let mut escape = false;

    for letter in string.chars() {
        if escape {
            escape = false;
        } else if letter == '\\' {
            escape = true;
        } else if in_quote {
            if letter == prev_quote {
                in_quote = false;
            }
        } else if letter == '"' || letter == '\'' {
            in_quote = true;
            prev_quote = letter;
        } else if letter == '(' {
            func += 1;
        } else if letter == ')' {
            if func > 0 {
                func -= 1;
            }
        } else if func == 0 && separators.contains(&letter) {
            do_split = true;
        }

        if do_split {
            let trimmed = current.trim().to_string();
            if !trimmed.is_empty() {
                array.push(trimmed);
            }
            current = String::new();
            do_split = false;
        } else {
            current.push(letter);
        }
    }

    if last || !current.is_empty() {
        let trimmed = current.trim().to_string();
        if last || !trimmed.is_empty() {
            array.push(trimmed);
        }
    }

    array
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_comma_simple() {
        assert_eq!(comma("a, b, c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_comma_with_parens() {
        assert_eq!(
            comma("rgb(0, 0, 0), red"),
            vec!["rgb(0, 0, 0)", "red"]
        );
    }

    #[test]
    fn test_comma_with_quotes() {
        assert_eq!(
            comma("'a, b', c"),
            vec!["'a, b'", "c"]
        );
    }

    #[test]
    fn test_space_simple() {
        assert_eq!(space("a b c"), vec!["a", "b", "c"]);
    }

    #[test]
    fn test_space_with_parens() {
        assert_eq!(
            space("calc(1px + 2px) 3px"),
            vec!["calc(1px + 2px)", "3px"]
        );
    }

    #[test]
    fn test_comma_trailing() {
        // With `last=true`, the final empty segment is included.
        assert_eq!(comma("a,"), vec!["a", ""]);
    }

    #[test]
    fn test_escape_handling() {
        assert_eq!(comma("a\\,b, c"), vec!["a\\,b", "c"]);
    }

    #[test]
    fn test_nested_parens() {
        assert_eq!(
            comma("calc(var(--x, 1) + 2), red"),
            vec!["calc(var(--x, 1) + 2)", "red"]
        );
    }

    #[test]
    fn test_empty_string() {
        let result: Vec<String> = comma("");
        assert_eq!(result, vec![""]);
    }

    #[test]
    fn test_space_multiple_whitespace() {
        assert_eq!(space("a   b"), vec!["a", "b"]);
    }
}
