use crate::utils_types::{CssItem, LogicalCssItem};

/// Merge consecutive unconditional CSS items while preserving the position of
/// any sheet entries. This mirrors the behaviour of the Babel helper and is
/// relied upon when normalising conditional CSS branches.
pub fn merge_subsequent_unconditional_css_items(items: Vec<CssItem>) -> Vec<CssItem> {
    let mut merged: Vec<CssItem> = Vec::new();
    let mut sheets: Vec<CssItem> = Vec::new();

    let mut index = 0usize;
    while index < items.len() {
        match &items[index] {
            CssItem::Sheet(_) => sheets.push(items[index].clone()),
            CssItem::Unconditional(_) => {
                let mut css = get_item_css(&items[index]);
                let mut last_index = index;

                let mut lookahead = index + 1;
                while lookahead < items.len() {
                    match &items[lookahead] {
                        CssItem::Unconditional(_) => {
                            css.push_str(&get_item_css(&items[lookahead]));
                            last_index = lookahead;
                        }
                        CssItem::Sheet(_) => sheets.push(items[lookahead].clone()),
                        _ => break,
                    }
                    lookahead += 1;
                }

                merged.push(CssItem::unconditional(css));
                index = last_index;
            }
            _ => merged.push(items[index].clone()),
        }

        index += 1;
    }

    sheets.into_iter().chain(merged.into_iter()).collect()
}

/// Helper that serialises a `CssItem` into the raw CSS string it represents.
/// This matches the behaviour of the Babel helper so downstream utilities can
/// reuse it during native transformations.
pub fn get_item_css(item: &CssItem) -> String {
    match item {
        CssItem::Conditional(conditional) => {
            let mut css = get_item_css(&conditional.consequent);
            css.push_str(&get_item_css(&conditional.alternate));
            css
        }
        CssItem::Unconditional(unconditional) => unconditional.css.clone(),
        CssItem::Logical(logical) => logical.css.clone(),
        CssItem::Sheet(sheet) => sheet.css.clone(),
        CssItem::Map(map) => map.css.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{get_item_css, merge_subsequent_unconditional_css_items};
    use crate::utils_types::{CssItem, LogicalCssItem, LogicalOperator, SheetCssItem};
    use swc_core::common::{SyntaxContext, DUMMY_SP};
    use swc_core::ecma::ast::{Expr, Ident};

    fn ident_expr(name: &str) -> Expr {
        Expr::Ident(Ident::new(name.into(), DUMMY_SP, SyntaxContext::empty()))
    }

    #[test]
    fn merges_adjacent_unconditional_items() {
        let items = vec![
            CssItem::unconditional("color: red;"),
            CssItem::unconditional("background: blue;"),
            CssItem::Logical(LogicalCssItem {
                css: "display: none;".into(),
                expression: ident_expr("flag"),
                operator: LogicalOperator::And,
            }),
            CssItem::unconditional("border: 0;"),
        ];

        let merged = merge_subsequent_unconditional_css_items(items);
        assert_eq!(merged.len(), 3);
        assert_eq!(get_item_css(&merged[0]), "color: red;background: blue;");
        assert!(matches!(merged[1], CssItem::Logical(_)));
        assert_eq!(get_item_css(&merged[2]), "border: 0;");
    }

    #[test]
    fn preserves_sheets_when_merging_unconditionals() {
        let items = vec![
            CssItem::Sheet(SheetCssItem {
                css: ".a{color:red;}".into(),
            }),
            CssItem::unconditional("margin: 0;"),
            CssItem::unconditional("padding: 0;"),
        ];

        let merged = merge_subsequent_unconditional_css_items(items);
        assert_eq!(merged.len(), 2);
        assert!(matches!(merged[0], CssItem::Sheet(_)));
        assert_eq!(get_item_css(&merged[1]), "margin: 0;padding: 0;");
    }

    #[test]
    fn get_item_css_serialises_variants() {
        let unconditional = CssItem::unconditional("color: red;");
        assert_eq!(get_item_css(&unconditional), "color: red;");

        let logical = CssItem::Logical(LogicalCssItem {
            css: "display: none;".into(),
            expression: ident_expr("flag"),
            operator: LogicalOperator::And,
        });
        assert_eq!(get_item_css(&logical), "display: none;");
    }
}
