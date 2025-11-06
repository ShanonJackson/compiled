//! Helpers that mirror `src/css-map/process-selectors.ts` from the Babel plugin.
//!
//! The original implementation operates on Babel AST nodes. The SWC port works
//! with the evaluated CSS value representation instead, but retains the same
//! branching logic and error semantics so that cssMap behaves identically.

use std::collections::HashSet;

use crate::css::property::{CssObject, CssValue};

use super::{errors::ErrorMessages, EXTENDED_SELECTORS_KEY};

/// Merge the `selectors` block into the parent object while performing the same
/// validation as the Babel counterpart.
pub fn merge_extended_selectors_into_properties(
    variant_styles: &CssObject,
) -> Result<CssObject, ErrorMessages> {
    let mut selectors_block: Option<CssObject> = None;

    for (key, value) in variant_styles.iter() {
        if key == EXTENDED_SELECTORS_KEY {
            if selectors_block.is_some() {
                return Err(ErrorMessages::DuplicateSelectorsBlock);
            }

            selectors_block = match value.clone() {
                CssValue::Object(map) => Some(map),
                _ => return Err(ErrorMessages::SelectorsBlockValueType),
            };
        }
    }

    let mut combined: Vec<(String, CssValue)> = Vec::new();

    for (key, value) in variant_styles.iter() {
        if key != EXTENDED_SELECTORS_KEY {
            combined.push((key.clone(), value.clone()));
        }
    }

    if let Some(selectors) = selectors_block {
        for (key, value) in selectors.into_iter() {
            combined.push((key, value));
        }
    }

    let mut merged: CssObject = CssObject::new();
    let mut added_selectors: HashSet<String> = HashSet::new();

    for (key, value) in combined.into_iter() {
        if key == EXTENDED_SELECTORS_KEY {
            // The selectors key itself should never be emitted after the merge.
            continue;
        }

        if is_plain_selector(&key) {
            return Err(ErrorMessages::UseSelectorsWithAmpersand);
        }

        if is_at_rule(&key) {
            let CssValue::Object(block) = value.clone() else {
                return Err(ErrorMessages::AtRuleValueType);
            };

            for (rule_key, rule_value) in block.into_iter() {
                if rule_key.starts_with(':') {
                    return Err(ErrorMessages::UseSelectorsWithAmpersand);
                }

                let at_rule_name = format!("{key} {rule_key}");
                if !added_selectors.insert(at_rule_name.clone()) {
                    return Err(ErrorMessages::DuplicateAtRule);
                }

                merged.insert(at_rule_name, rule_value);
            }

            continue;
        }

        if matches!(value, CssValue::Object(_)) {
            if !added_selectors.insert(key.clone()) {
                return Err(ErrorMessages::DuplicateSelector);
            }
        }

        merged.insert(key, value);
    }

    Ok(merged)
}

pub fn is_plain_selector(selector: &str) -> bool {
    selector.starts_with(':')
}

pub fn is_at_rule(key: &str) -> bool {
    matches!(
        key,
        "@charset"
            | "@counter-style"
            | "@document"
            | "@font-face"
            | "@font-feature-values"
            | "@font-palette-values"
            | "@import"
            | "@keyframes"
            | "@layer"
            | "@media"
            | "@namespace"
            | "@page"
            | "@property"
            | "@scope"
            | "@scroll-timeline"
            | "@starting-style"
            | "@supports"
            | "@viewport"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_selectors_error() {
        let mut variant = CssObject::new();
        let mut base_selector = CssObject::new();
        base_selector.insert("color".to_string(), CssValue::String("red".into()));
        variant.insert(
            "&:hover".to_string(),
            CssValue::Object(base_selector.clone()),
        );

        let mut selectors = CssObject::new();
        selectors.insert("&:hover".to_string(), CssValue::Object(base_selector));
        variant.insert(
            EXTENDED_SELECTORS_KEY.to_string(),
            CssValue::Object(selectors),
        );

        let result = merge_extended_selectors_into_properties(&variant);
        assert!(matches!(result, Err(ErrorMessages::DuplicateSelector)));
    }
}
