#[derive(Debug, Clone, Default)]
pub struct SortOptions {
    pub sort_at_rules: Option<bool>,
    pub sort_shorthand: Option<bool>,
}

pub fn sort_atomic_style_sheet(_css: &str, _options: SortOptions) -> String {
    // TODO: port `packages/css/src/sort.ts`.
    _css.to_string()
}
