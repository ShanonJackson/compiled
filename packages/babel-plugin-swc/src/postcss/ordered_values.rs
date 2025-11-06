use once_cell::sync::Lazy;
use serde::Deserialize;
use std::collections::HashSet;

static BORDER_WIDTH_KEYWORDS: &[&str] = &["thin", "medium", "thick"];
static BORDER_STYLE_KEYWORDS: &[&str] = &[
    "none", "auto", "hidden", "dotted", "dashed", "solid", "double", "groove", "ridge", "inset",
    "outset",
];
static FLEX_DIRECTION_KEYWORDS: &[&str] = &["row", "row-reverse", "column", "column-reverse"];
static FLEX_WRAP_KEYWORDS: &[&str] = &["nowrap", "wrap", "wrap-reverse"];
static TRANSITION_TIMING_KEYWORDS: &[&str] = &[
    "ease",
    "linear",
    "ease-in",
    "ease-out",
    "ease-in-out",
    "step-start",
    "step-end",
];
static ANIMATION_TIMING_FUNCTIONS: &[&str] = &["steps", "cubic-bezier", "frames"];
static ANIMATION_TIMING_KEYWORDS: &[&str] = &[
    "ease",
    "ease-in",
    "ease-in-out",
    "ease-out",
    "linear",
    "step-end",
    "step-start",
];
static ANIMATION_DIRECTION_KEYWORDS: &[&str] =
    &["normal", "reverse", "alternate", "alternate-reverse"];
static ANIMATION_FILL_MODE_KEYWORDS: &[&str] = &["none", "forwards", "backwards", "both"];
static ANIMATION_PLAY_STATE_KEYWORDS: &[&str] = &["running", "paused"];
static VARIABLE_FUNCTIONS: &[&str] = &["var", "env", "constant"];
static MATH_FUNCTIONS: &[&str] = &["calc", "clamp", "max", "min"];

#[derive(Deserialize)]
struct ListStyleTypeData {
    #[serde(rename = "list-style-type")]
    values: Vec<String>,
}

static LIST_STYLE_TYPES: Lazy<HashSet<String>> = Lazy::new(|| {
    let data: ListStyleTypeData = serde_json::from_str(include_str!("data/list_style_types.json"))
        .expect("valid list-style-type data");
    data.values.into_iter().collect()
});

/// Normalises declaration values for shorthands that require ordered components.
pub fn normalise_ordered_value(property: &str, value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if should_abort(trimmed) {
        return None;
    }

    let lower_property = property.to_ascii_lowercase();
    let normalized_property = vendor_unprefixed(&lower_property);

    match normalized_property {
        "border"
        | "border-block"
        | "border-inline"
        | "border-block-end"
        | "border-block-start"
        | "border-inline-end"
        | "border-inline-start"
        | "border-top"
        | "border-right"
        | "border-bottom"
        | "border-left"
        | "outline" => normalize_border(trimmed),
        "box-shadow" => normalize_box_shadow(trimmed),
        "flex-flow" => normalize_flex_flow(trimmed),
        "transition" => normalize_transition(trimmed),
        "animation" => normalize_animation(trimmed),
        "list-style" => normalize_list_style(trimmed),
        "column-rule" => normalize_border(trimmed),
        "columns" => normalize_columns(trimmed),
        "grid-auto-flow" => normalize_grid_auto_flow(trimmed),
        "grid-column-gap" | "grid-row-gap" => normalize_grid_gap(trimmed),
        "grid-column" | "grid-row" | "grid-row-start" | "grid-row-end" | "grid-column-start"
        | "grid-column-end" => normalize_grid_line(trimmed),
        _ => None,
    }
}

fn vendor_unprefixed(property: &str) -> &str {
    if let Some(stripped) = property.strip_prefix('-') {
        let mut parts = stripped.splitn(2, '-');
        if let (Some(_vendor), Some(rest)) = (parts.next(), parts.next()) {
            return rest;
        }
    }
    property
}

fn should_abort(value: &str) -> bool {
    if value.contains("/*") || value.contains("___CSS_LOADER_IMPORT___") {
        return true;
    }

    let lower = value.to_ascii_lowercase();
    for name in VARIABLE_FUNCTIONS {
        if contains_function(&lower, name) {
            return true;
        }
    }

    false
}

fn contains_function(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let name_bytes = needle.as_bytes();
    let mut index = 0;

    while let Some(pos) = haystack[index..].find(needle) {
        let start = index + pos;
        if start > 0 && is_ident_char(bytes[start - 1]) {
            index = start + 1;
            continue;
        }

        let mut cursor = start + name_bytes.len();
        while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }

        if cursor < bytes.len() && bytes[cursor] == b'(' {
            return true;
        }

        index = start + 1;
    }

    false
}

fn is_ident_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum ComponentKind {
    Word,
    Function { name: String },
    String,
    Divider(char),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Component {
    raw: String,
    kind: ComponentKind,
}

fn tokenize(value: &str) -> Vec<Component> {
    let bytes = value.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        let ch = bytes[index];

        if ch.is_ascii_whitespace() {
            index += 1;
            continue;
        }

        if matches!(ch, b'/' | b',' | b':') {
            tokens.push(Component {
                raw: value[index..index + 1].to_string(),
                kind: ComponentKind::Divider(ch as char),
            });
            index += 1;
            continue;
        }

        if ch == b'\'' || ch == b'"' {
            let (next, raw) = read_string(value, index);
            tokens.push(Component {
                raw,
                kind: ComponentKind::String,
            });
            index = next;
            continue;
        }

        if is_ident_start(ch) {
            let mut end = index + 1;
            while end < bytes.len() && is_ident_char(bytes[end]) {
                end += 1;
            }

            let mut cursor = end;
            while cursor < bytes.len() && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }

            if cursor < bytes.len() && bytes[cursor] == b'(' {
                let (next, raw) = read_function(value, index);
                let name = value[index..end].to_ascii_lowercase();
                tokens.push(Component {
                    raw,
                    kind: ComponentKind::Function { name },
                });
                index = next;
                continue;
            }
        }

        let start = index;
        while index < bytes.len() {
            let current = bytes[index];
            if current.is_ascii_whitespace() || matches!(current, b',' | b'/' | b':') {
                break;
            }
            if current == b'(' {
                break;
            }
            index += 1;
        }

        if start == index {
            index += 1;
            continue;
        }

        let raw = value[start..index].to_string();
        tokens.push(Component {
            raw,
            kind: ComponentKind::Word,
        });
    }

    tokens
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'-' || byte == b'_'
}

fn read_string(value: &str, start: usize) -> (usize, String) {
    let bytes = value.as_bytes();
    let mut index = start + 1;
    let quote = bytes[start];
    let mut escaped = false;

    while index < bytes.len() {
        let ch = bytes[index];
        if ch == quote && !escaped {
            index += 1;
            break;
        }

        escaped = ch == b'\\' && !escaped;
        if ch != b'\\' {
            escaped = false;
        }

        index += 1;
    }

    (index, value[start..index].to_string())
}

fn read_function(value: &str, start: usize) -> (usize, String) {
    let bytes = value.as_bytes();
    let mut index = start;
    let mut depth = 0;

    while index < bytes.len() {
        let ch = bytes[index];
        index += 1;
        if ch == b'(' {
            depth += 1;
            break;
        }
    }

    while index < bytes.len() && depth > 0 {
        let ch = bytes[index];
        if ch == b'\'' || ch == b'"' {
            let (next, _) = read_string(value, index);
            index = next;
            continue;
        }
        if ch == b'(' {
            depth += 1;
        } else if ch == b')' {
            depth -= 1;
            if depth == 0 {
                index += 1;
                break;
            }
        }
        index += 1;
    }

    (index, value[start..index].to_string())
}

fn parse_unit(value: &str) -> Option<(String, String)> {
    let bytes = value.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let mut index = 0;
    let len = bytes.len();
    let mut code = bytes[index];

    if matches!(code, b'+' | b'-') {
        index += 1;
        if index >= len {
            return None;
        }
        code = bytes[index];
        if !code.is_ascii_digit() && code != b'.' {
            return None;
        }
    }

    if code == b'.' {
        if index + 1 >= len || !bytes[index + 1].is_ascii_digit() {
            return None;
        }
        index += 1;
    }

    while index < len && bytes[index].is_ascii_digit() {
        index += 1;
    }

    if index < len && bytes[index] == b'.' {
        index += 1;
        let mut has_digit = false;
        while index < len && bytes[index].is_ascii_digit() {
            index += 1;
            has_digit = true;
        }
        if !has_digit {
            return None;
        }
    }

    if index < len && (bytes[index] == b'e' || bytes[index] == b'E') {
        let mut exp_index = index + 1;
        if exp_index < len && matches!(bytes[exp_index], b'+' | b'-') {
            exp_index += 1;
        }
        if exp_index >= len || !bytes[exp_index].is_ascii_digit() {
            return Some((value[..index].to_string(), value[index..].to_string()));
        }
        index = exp_index + 1;
        while index < len && bytes[index].is_ascii_digit() {
            index += 1;
        }
    }

    if index == 0 {
        return None;
    }

    Some((value[..index].to_string(), value[index..].to_string()))
}

fn join_parts(parts: &[String]) -> String {
    parts
        .iter()
        .filter(|part| !part.is_empty())
        .map(|part| part.trim())
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

fn normalize_border(value: &str) -> Option<String> {
    let tokens = tokenize(value);
    if tokens.len() < 2 {
        return None;
    }

    let mut width = Vec::new();
    let mut style = Vec::new();
    let mut color = Vec::new();

    for token in tokens {
        match token.kind {
            ComponentKind::Function { name } => {
                if MATH_FUNCTIONS.contains(&name.as_str()) {
                    width.push(token.raw);
                } else {
                    color.push(token.raw);
                }
            }
            ComponentKind::Word => {
                let lower = token.raw.to_ascii_lowercase();
                if BORDER_STYLE_KEYWORDS.contains(&lower.as_str()) {
                    style.push(token.raw);
                } else if BORDER_WIDTH_KEYWORDS.contains(&lower.as_str()) {
                    width.push(token.raw);
                } else if let Some((_, unit)) = parse_unit(&token.raw) {
                    if !unit.is_empty() || token.raw.parse::<f64>().is_ok() {
                        width.push(token.raw);
                    } else {
                        color.push(token.raw);
                    }
                } else {
                    color.push(token.raw);
                }
            }
            ComponentKind::String => {
                color.push(token.raw);
            }
            ComponentKind::Divider(_) => {
                color.push(token.raw);
            }
        }
    }

    let mut parts = Vec::new();
    let width_part = join_parts(&width);
    if !width_part.is_empty() {
        parts.push(width_part);
    }
    let style_part = join_parts(&style);
    if !style_part.is_empty() {
        parts.push(style_part);
    }
    let color_part = join_parts(&color);
    if !color_part.is_empty() {
        parts.push(color_part);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn normalize_box_shadow(value: &str) -> Option<String> {
    let args = split_arguments(value);
    if args.is_empty() {
        return None;
    }

    let mut normalized = Vec::new();

    for arg in args {
        let tokens = tokenize(&arg);
        if tokens.is_empty() {
            normalized.push(arg.trim().to_string());
            continue;
        }

        let mut inset = Vec::new();
        let mut lengths = Vec::new();
        let mut color = Vec::new();
        let mut abort = false;

        for token in tokens {
            match token.kind {
                ComponentKind::Function { name } => {
                    if MATH_FUNCTIONS.contains(&name.as_str()) {
                        abort = true;
                        break;
                    }
                    color.push(token.raw);
                }
                ComponentKind::Word => {
                    let lower = token.raw.to_ascii_lowercase();
                    if lower == "inset" {
                        inset.push(token.raw);
                    } else if let Some((_, unit)) = parse_unit(&token.raw) {
                        if !unit.is_empty() || token.raw.parse::<f64>().is_ok() {
                            lengths.push(token.raw);
                        } else {
                            color.push(token.raw);
                        }
                    } else {
                        color.push(token.raw);
                    }
                }
                ComponentKind::String => color.push(token.raw),
                ComponentKind::Divider(_) => color.push(token.raw),
            }
        }

        if abort {
            return None;
        }

        let inset_part = join_parts(&inset);
        let lengths_part = join_parts(&lengths);
        let color_part = join_parts(&color);
        let mut parts = Vec::new();
        if !inset_part.is_empty() {
            parts.push(inset_part);
        }
        if !lengths_part.is_empty() {
            parts.push(lengths_part);
        }
        if !color_part.is_empty() {
            parts.push(color_part);
        }

        normalized.push(parts.join(" "));
    }

    Some(normalized.join(", "))
}

fn normalize_flex_flow(value: &str) -> Option<String> {
    let tokens = tokenize(value);
    if tokens.is_empty() {
        return None;
    }

    let mut direction = String::new();
    let mut wrap = String::new();

    for token in tokens {
        if let ComponentKind::Word = token.kind {
            let lower = token.raw.to_ascii_lowercase();
            if direction.is_empty() && FLEX_DIRECTION_KEYWORDS.contains(&lower.as_str()) {
                direction = token.raw;
            } else if wrap.is_empty() && FLEX_WRAP_KEYWORDS.contains(&lower.as_str()) {
                wrap = token.raw;
            }
        }
    }

    let mut parts = Vec::new();
    if !direction.is_empty() {
        parts.push(direction.trim().to_string());
    }
    if !wrap.is_empty() {
        parts.push(wrap.trim().to_string());
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn is_time_token(token: &Component) -> bool {
    if let Some((_, unit)) = parse_unit(&token.raw) {
        matches!(unit.as_str(), "s" | "ms")
    } else {
        false
    }
}

fn normalize_transition(value: &str) -> Option<String> {
    let args = split_arguments(value);
    if args.is_empty() {
        return None;
    }

    let mut normalized = Vec::new();

    for arg in args {
        let tokens = tokenize(&arg);
        if tokens.is_empty() {
            normalized.push(arg.trim().to_string());
            continue;
        }

        let mut property = Vec::new();
        let mut time1 = Vec::new();
        let mut time2 = Vec::new();
        let mut timing_function = Vec::new();

        for token in tokens {
            match &token.kind {
                ComponentKind::Function { name } => {
                    if ["steps", "cubic-bezier"].contains(&name.as_str()) {
                        timing_function.push(token.raw);
                    } else {
                        property.push(token.raw);
                    }
                }
                ComponentKind::Word => {
                    let lower = token.raw.to_ascii_lowercase();
                    if is_time_token(&token) {
                        if time1.is_empty() {
                            time1.push(token.raw);
                        } else {
                            time2.push(token.raw);
                        }
                    } else if TRANSITION_TIMING_KEYWORDS.contains(&lower.as_str()) {
                        timing_function.push(token.raw);
                    } else {
                        property.push(token.raw);
                    }
                }
                ComponentKind::String => property.push(token.raw),
                ComponentKind::Divider(_) => property.push(token.raw),
            }
        }

        let mut parts = Vec::new();
        let property_part = join_parts(&property);
        if !property_part.is_empty() {
            parts.push(property_part);
        }
        let time1_part = join_parts(&time1);
        if !time1_part.is_empty() {
            parts.push(time1_part);
        }
        let timing_part = join_parts(&timing_function);
        if !timing_part.is_empty() {
            parts.push(timing_part);
        }
        let time2_part = join_parts(&time2);
        if !time2_part.is_empty() {
            parts.push(time2_part);
        }

        normalized.push(parts.join(" "));
    }

    Some(normalized.join(", "))
}

fn is_iteration_count(value: &str) -> bool {
    if value.eq_ignore_ascii_case("infinite") {
        return true;
    }

    if let Some((_, unit)) = parse_unit(value) {
        unit.is_empty()
    } else {
        false
    }
}

fn normalize_animation(value: &str) -> Option<String> {
    let args = split_arguments(value);
    if args.is_empty() {
        return None;
    }

    let mut normalized = Vec::new();

    for arg in args {
        let tokens = tokenize(&arg);
        if tokens.is_empty() {
            normalized.push(arg.trim().to_string());
            continue;
        }

        let mut name = Vec::new();
        let mut duration = Vec::new();
        let mut timing_function = Vec::new();
        let mut delay = Vec::new();
        let mut iteration_count = Vec::new();
        let mut direction = Vec::new();
        let mut fill_mode = Vec::new();
        let mut play_state = Vec::new();

        for token in tokens {
            match &token.kind {
                ComponentKind::Function { name: func_name } => {
                    if ANIMATION_TIMING_FUNCTIONS.contains(&func_name.as_str()) {
                        if timing_function.is_empty() {
                            timing_function.push(token.raw);
                        } else {
                            name.push(token.raw);
                        }
                    } else {
                        name.push(token.raw);
                    }
                }
                ComponentKind::Word => {
                    let lower = token.raw.to_ascii_lowercase();
                    if is_time_token(&token) {
                        if duration.is_empty() {
                            duration.push(token.raw);
                        } else if delay.is_empty() {
                            delay.push(token.raw);
                        } else {
                            name.push(token.raw);
                        }
                    } else if ANIMATION_TIMING_KEYWORDS.contains(&lower.as_str()) {
                        if timing_function.is_empty() {
                            timing_function.push(token.raw);
                        } else {
                            name.push(token.raw);
                        }
                    } else if ANIMATION_DIRECTION_KEYWORDS.contains(&lower.as_str()) {
                        if direction.is_empty() {
                            direction.push(token.raw);
                        }
                    } else if ANIMATION_FILL_MODE_KEYWORDS.contains(&lower.as_str()) {
                        if fill_mode.is_empty() {
                            fill_mode.push(token.raw);
                        }
                    } else if ANIMATION_PLAY_STATE_KEYWORDS.contains(&lower.as_str()) {
                        if play_state.is_empty() {
                            play_state.push(token.raw);
                        }
                    } else if is_iteration_count(&token.raw) {
                        if iteration_count.is_empty() {
                            iteration_count.push(token.raw);
                        }
                    } else {
                        name.push(token.raw);
                    }
                }
                ComponentKind::String => name.push(token.raw),
                ComponentKind::Divider(_) => name.push(token.raw),
            }
        }

        let mut parts = Vec::new();
        let name_part = join_parts(&name);
        if !name_part.is_empty() {
            parts.push(name_part);
        }
        let duration_part = join_parts(&duration);
        if !duration_part.is_empty() {
            parts.push(duration_part);
        }
        let timing_part = join_parts(&timing_function);
        if !timing_part.is_empty() {
            parts.push(timing_part);
        }
        let delay_part = join_parts(&delay);
        if !delay_part.is_empty() {
            parts.push(delay_part);
        }
        let iteration_part = join_parts(&iteration_count);
        if !iteration_part.is_empty() {
            parts.push(iteration_part);
        }
        let direction_part = join_parts(&direction);
        if !direction_part.is_empty() {
            parts.push(direction_part);
        }
        let fill_part = join_parts(&fill_mode);
        if !fill_part.is_empty() {
            parts.push(fill_part);
        }
        let play_part = join_parts(&play_state);
        if !play_part.is_empty() {
            parts.push(play_part);
        }

        normalized.push(parts.join(" "));
    }

    Some(normalized.join(", "))
}

fn normalize_list_style(value: &str) -> Option<String> {
    let tokens = tokenize(value);
    if tokens.is_empty() {
        return None;
    }

    let mut types = Vec::new();
    let mut position = Vec::new();
    let mut image = Vec::new();

    for token in tokens {
        match token.kind {
            ComponentKind::Word => {
                let lower = token.raw.to_ascii_lowercase();
                if LIST_STYLE_TYPES.contains(&lower) {
                    types.push(token.raw);
                } else if matches!(lower.as_str(), "inside" | "outside") {
                    position.push(token.raw);
                } else if lower == "none" {
                    if types.iter().any(|value| value.eq_ignore_ascii_case("none")) {
                        image.push(token.raw);
                    } else {
                        types.push(token.raw);
                    }
                } else {
                    types.push(token.raw);
                }
            }
            ComponentKind::Function { .. } | ComponentKind::String => image.push(token.raw),
            ComponentKind::Divider(_) => image.push(token.raw),
        }
    }

    let mut parts = Vec::new();
    let type_part = join_parts(&types);
    if !type_part.is_empty() {
        parts.push(type_part);
    }
    let position_part = join_parts(&position);
    if !position_part.is_empty() {
        parts.push(position_part);
    }
    let image_part = join_parts(&image);
    if !image_part.is_empty() {
        parts.push(image_part);
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn normalize_columns(value: &str) -> Option<String> {
    let tokens = tokenize(value);
    if tokens.is_empty() {
        return None;
    }

    let mut widths = Vec::new();
    let mut other = Vec::new();

    for token in tokens {
        if let ComponentKind::Word = token.kind {
            if let Some((_, unit)) = parse_unit(&token.raw) {
                if !unit.is_empty() {
                    widths.push(token.raw.trim_start().to_string());
                    continue;
                }
            }
            other.push(token.raw.trim_start().to_string());
        } else {
            return None;
        }
    }

    if other.len() == 1 && widths.len() == 1 {
        let mut parts = Vec::new();
        if !widths[0].is_empty() {
            parts.push(widths[0].trim().to_string());
        }
        if !other[0].is_empty() {
            parts.push(other[0].trim().to_string());
        }
        if parts.is_empty() {
            None
        } else {
            Some(parts.join(" "))
        }
    } else {
        None
    }
}

fn normalize_grid_auto_flow(value: &str) -> Option<String> {
    let tokens = tokenize(value);
    if tokens.is_empty() {
        return None;
    }

    let mut front = String::new();
    let mut back = String::new();
    let mut should_normalize = false;

    for token in tokens {
        if let ComponentKind::Word = token.kind {
            let lower = token.raw.to_ascii_lowercase();
            if lower == "dense" {
                should_normalize = true;
                back = token.raw;
            } else if matches!(lower.as_str(), "row" | "column") {
                should_normalize = true;
                front = token.raw;
            } else {
                should_normalize = false;
                break;
            }
        } else {
            should_normalize = false;
            break;
        }
    }

    if should_normalize {
        let mut parts = Vec::new();
        if !front.trim().is_empty() {
            parts.push(front.trim().to_string());
        }
        if !back.trim().is_empty() {
            parts.push(back.trim().to_string());
        }
        Some(parts.join(" "))
    } else {
        None
    }
}

fn normalize_grid_gap(value: &str) -> Option<String> {
    let tokens = tokenize(value);
    if tokens.is_empty() {
        return None;
    }

    let mut front = String::new();
    let mut back = Vec::new();
    let mut should_normalize = false;

    for token in tokens {
        match token.kind {
            ComponentKind::Word => {
                if token.raw.eq_ignore_ascii_case("normal") {
                    should_normalize = true;
                    front = token.raw;
                } else {
                    back.push(token.raw);
                }
            }
            ComponentKind::Function { .. } | ComponentKind::String => back.push(token.raw),
            ComponentKind::Divider(_) => back.push(token.raw),
        }
    }

    if should_normalize {
        let mut parts = Vec::new();
        if !front.trim().is_empty() {
            parts.push(front.trim().to_string());
        }
        let back_part = join_parts(&back);
        if !back_part.is_empty() {
            parts.push(back_part);
        }
        Some(parts.join(" "))
    } else {
        None
    }
}

fn normalize_grid_line(value: &str) -> Option<String> {
    if value.contains('/') {
        let mapped: Vec<String> = value
            .split('/')
            .map(|segment| reorder_grid_line(segment))
            .collect();
        Some(mapped.join(" / "))
    } else {
        let normalized = reorder_grid_line(value);
        if normalized.trim() == value.trim() {
            None
        } else {
            Some(normalized)
        }
    }
}

fn reorder_grid_line(segment: &str) -> String {
    let mut front = Vec::new();
    let mut back = Vec::new();

    for token in segment.split_whitespace() {
        if token.eq_ignore_ascii_case("span") {
            front.push(token.trim());
        } else {
            back.push(token.trim());
        }
    }

    let mut parts = Vec::new();
    if !front.is_empty() {
        parts.push(front.join(" "));
    }
    if !back.is_empty() {
        parts.push(back.join(" "));
    }
    parts.join(" ")
}

fn split_arguments(value: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut depth = 0;
    let mut start = 0;
    let bytes = value.as_bytes();
    let mut index = 0;

    while index < bytes.len() {
        match bytes[index] {
            b'(' => {
                depth += 1;
                index += 1;
            }
            b')' => {
                if depth > 0 {
                    depth -= 1;
                }
                index += 1;
            }
            b'\'' | b'"' => {
                let (next, _) = read_string(value, index);
                index = next;
            }
            b',' if depth == 0 => {
                args.push(value[start..index].trim().to_string());
                index += 1;
                start = index;
            }
            _ => {
                index += 1;
            }
        }
    }

    let tail = value[start..].trim();
    if !tail.is_empty() {
        args.push(tail.to_string());
    }

    args
}

#[cfg(test)]
mod tests {
    use super::normalise_ordered_value;

    #[test]
    fn orders_border_values() {
        let one = normalise_ordered_value("border", "green solid 2px").unwrap();
        let two = normalise_ordered_value("border", "2px solid green").unwrap();
        assert_eq!(one, two);
    }

    #[test]
    fn normalises_box_shadow_components() {
        let value = normalise_ordered_value("box-shadow", "red inset 2px 1px").unwrap();
        assert_eq!(value, "inset 2px 1px red");
    }

    #[test]
    fn normalises_transition_arguments() {
        let value = normalise_ordered_value("transition", "opacity ease 200ms 50ms").unwrap();
        assert_eq!(value, "opacity 200ms ease 50ms");
    }

    #[test]
    fn normalises_animation_arguments() {
        let value =
            normalise_ordered_value("animation", "spin ease-in-out infinite 1s 200ms").unwrap();
        assert_eq!(value, "spin 1s ease-in-out 200ms infinite");
    }

    #[test]
    fn normalises_list_style() {
        let value = normalise_ordered_value("list-style", "outside none url(foo.png)").unwrap();
        assert_eq!(value, "none outside url(foo.png)");
    }
}
