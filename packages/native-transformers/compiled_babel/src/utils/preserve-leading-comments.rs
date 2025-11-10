use swc_core::common::Spanned;

use crate::types::TransformState;

/// Mirrors the behaviour of `@compiled/utils`'s `preserveLeadingComments` helper
/// by ensuring comments that originally preceded the first statement remain at the
/// top of the file after additional nodes are inserted ahead of the program body.
pub fn preserve_leading_comments<T>(items: &[T], state: &mut TransformState)
where
    T: Spanned,
{
    if state.file.comments.is_empty() {
        return;
    }

    let Some(first) = items.first() else {
        return;
    };

    let cutoff = first.span().lo();
    let mut leading = Vec::new();

    state.file.comments.retain(|comment| {
        if comment.span.hi <= cutoff {
            leading.push(comment.clone());
            false
        } else {
            true
        }
    });

    if leading.is_empty() {
        return;
    }

    leading.extend(state.file.comments.iter().cloned());
    state.file.comments = leading;
}
