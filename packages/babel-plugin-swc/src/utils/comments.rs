use swc_core::common::Span;

use crate::state::TransformState;

#[derive(Default)]
pub struct NodeComments {
    pub before: Vec<String>,
    pub current: Vec<String>,
}

pub fn get_node_comments(state: &TransformState, span: Span) -> NodeComments {
    let Some(source_map) = state.source_map() else {
        return NodeComments::default();
    };

    let start = source_map.lookup_char_pos(span.lo);
    let end = source_map.lookup_char_pos(span.hi);

    if start.line != end.line {
        return NodeComments::default();
    }

    let target_line = start.line;
    let before_line = target_line.checked_sub(1);
    let file = start.file;

    let mut comments = NodeComments::default();

    if let Some(prev_line) = before_line.and_then(|line| file.get_line((line - 1) as usize)) {
        if let Some(text) = extract_comment(prev_line.as_ref(), 0) {
            comments.before.push(text);
        }
    }

    if let Some(current_line) = file.get_line((target_line - 1) as usize) {
        if let Some(text) = extract_comment(current_line.as_ref(), end.col.0 as usize) {
            comments.current.push(text);
        }
    }

    comments
}

fn extract_comment(line: &str, start_col: usize) -> Option<String> {
    let slice = if start_col < line.len() {
        &line[start_col..]
    } else {
        ""
    };

    let comment_pos = slice.find("//")?;
    Some(slice[comment_pos + 2..].trim().to_string())
}
