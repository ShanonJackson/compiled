use crate::utils_css::kebab_case;
use crate::utils_types::{
    ConditionalCssItem, CssItem, CssMapItem, CssOutput, LogicalCssItem, SheetCssItem,
    UnconditionalCssItem,
};

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

fn wrap_with_selector(selector: &str, css: String) -> String {
    format!("{selector} {{ {css} }}")
}

fn to_css_rule_internal(selector: &str, item: &CssItem) -> CssItem {
    match item {
        CssItem::Conditional(conditional) => CssItem::Conditional(ConditionalCssItem {
            test: conditional.test.clone(),
            consequent: Box::new(to_css_rule_internal(selector, &conditional.consequent)),
            alternate: Box::new(to_css_rule_internal(selector, &conditional.alternate)),
        }),
        CssItem::Unconditional(unconditional) => CssItem::Unconditional(UnconditionalCssItem {
            css: wrap_with_selector(selector, unconditional.css.clone()),
        }),
        CssItem::Logical(logical) => CssItem::Logical(LogicalCssItem {
            expression: logical.expression.clone(),
            operator: logical.operator,
            css: wrap_with_selector(selector, logical.css.clone()),
        }),
        CssItem::Sheet(sheet) => CssItem::Sheet(SheetCssItem {
            css: wrap_with_selector(selector, sheet.css.clone()),
        }),
        CssItem::Map(map) => CssItem::Map(CssMapItem {
            name: map.name.clone(),
            expression: map.expression.clone(),
            css: wrap_with_selector(selector, map.css.clone()),
        }),
    }
}

/// Map the CSS output to rule blocks for the provided selector. Mirrors the
/// behaviour of the Babel helper which recursively wraps each item while
/// preserving conditional branches.
pub fn to_css_rule(selector: &str, result: &CssOutput) -> CssOutput {
    let css = result
        .css
        .iter()
        .map(|item| to_css_rule_internal(selector, item))
        .collect();

    CssOutput {
        css,
        variables: result.variables.clone(),
    }
}

fn declaration_css(key: &str, css: String) -> String {
    format!("{}: {};", kebab_case(key), css)
}

fn to_css_declaration_internal(key: &str, item: &CssItem) -> CssItem {
    match item {
        CssItem::Sheet(sheet) => CssItem::Sheet(sheet.clone()),
        CssItem::Conditional(conditional) => CssItem::Conditional(ConditionalCssItem {
            test: conditional.test.clone(),
            consequent: Box::new(to_css_declaration_internal(key, &conditional.consequent)),
            alternate: Box::new(to_css_declaration_internal(key, &conditional.alternate)),
        }),
        CssItem::Unconditional(unconditional) => CssItem::Unconditional(UnconditionalCssItem {
            css: declaration_css(key, unconditional.css.clone()),
        }),
        CssItem::Logical(logical) => CssItem::Logical(LogicalCssItem {
            expression: logical.expression.clone(),
            operator: logical.operator,
            css: declaration_css(key, logical.css.clone()),
        }),
        CssItem::Map(map) => CssItem::Map(CssMapItem {
            name: map.name.clone(),
            expression: map.expression.clone(),
            css: declaration_css(key, map.css.clone()),
        }),
    }
}

/// Convert the CSS output into property declarations for the provided key,
/// mirroring the Babel helper that kebab-cases the property name and reuses the
/// existing item shape.
pub fn to_css_declaration(key: &str, result: &CssOutput) -> CssOutput {
    let css = result
        .css
        .iter()
        .map(|item| to_css_declaration_internal(key, item))
        .collect();

    CssOutput {
        css,
        variables: result.variables.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        get_item_css, merge_subsequent_unconditional_css_items, to_css_declaration, to_css_rule,
    };
    use crate::utils_types::{
        ConditionalCssItem, CssItem, CssOutput, LogicalCssItem, LogicalOperator, SheetCssItem,
    };
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

    #[test]
    fn to_css_rule_wraps_items_in_selector() {
        let item = CssItem::unconditional("color: red;");
        let output = CssOutput {
            css: vec![item],
            variables: Vec::new(),
        };

        let result = to_css_rule(".foo", &output);
        assert_eq!(result.css.len(), 1);

        match &result.css[0] {
            CssItem::Unconditional(unconditional) => {
                assert_eq!(unconditional.css, ".foo { color: red; }");
            }
            _ => panic!("expected unconditional item"),
        }
    }

    #[test]
    fn to_css_rule_recursively_wraps_conditionals() {
        let conditional = CssItem::Conditional(ConditionalCssItem {
            test: Expr::Ident(Ident::new("flag".into(), DUMMY_SP, SyntaxContext::empty())),
            consequent: Box::new(CssItem::unconditional("color: red;")),
            alternate: Box::new(CssItem::unconditional("color: blue;")),
        });

        let output = CssOutput {
            css: vec![conditional],
            variables: Vec::new(),
        };

        let result = to_css_rule(".foo", &output);
        match &result.css[0] {
            CssItem::Conditional(mapped) => {
                if let CssItem::Unconditional(unconditional) = mapped.consequent.as_ref() {
                    assert_eq!(unconditional.css, ".foo { color: red; }");
                } else {
                    panic!("expected unconditional consequent");
                }

                if let CssItem::Unconditional(unconditional) = mapped.alternate.as_ref() {
                    assert_eq!(unconditional.css, ".foo { color: blue; }");
                } else {
                    panic!("expected unconditional alternate");
                }
            }
            _ => panic!("expected conditional item"),
        }
    }

    #[test]
    fn to_css_declaration_maps_values() {
        let item = CssItem::Logical(LogicalCssItem {
            css: "var(--token)".into(),
            expression: ident_expr("flag"),
            operator: LogicalOperator::And,
        });

        let output = CssOutput {
            css: vec![item],
            variables: Vec::new(),
        };

        let result = to_css_declaration("fontWeight", &output);
        match &result.css[0] {
            CssItem::Logical(logical) => {
                assert_eq!(logical.css, "font-weight: var(--token);");
            }
            _ => panic!("expected logical item"),
        }
    }

    #[test]
    fn to_css_declaration_preserves_sheets() {
        let sheet_css = ".a { color: red; }".to_string();
        let sheet = CssItem::Sheet(SheetCssItem {
            css: sheet_css.clone(),
        });

        let output = CssOutput {
            css: vec![sheet],
            variables: Vec::new(),
        };

        let result = to_css_declaration("color", &output);
        assert_eq!(result.css.len(), 1);

        match &result.css[0] {
            CssItem::Sheet(mapped) => assert_eq!(mapped.css, sheet_css),
            _ => panic!("expected sheet item"),
        }
    }
}
