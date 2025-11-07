use std::collections::HashSet;

use swc_core::css::ast::{ComponentValue, Declaration, DeclarationName, Stylesheet};

use super::super::transform::{Plugin, TransformContext};

#[derive(Debug, Default, Clone, Copy)]
pub struct DiscardDuplicates;

impl Plugin for DiscardDuplicates {
    fn name(&self) -> &'static str {
        "discard-duplicates"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        discard_top_level_duplicates(stylesheet);
    }
}

pub fn discard_duplicates() -> DiscardDuplicates {
    DiscardDuplicates
}

fn discard_top_level_duplicates(stylesheet: &mut Stylesheet) {
    for rule in &mut stylesheet.rules {
        if let swc_core::css::ast::Rule::ListOfComponentValues(list) = rule {
            remove_duplicate_declarations(&mut list.children);
        }
    }
}

fn remove_duplicate_declarations(values: &mut Vec<ComponentValue>) {
    if values.is_empty() {
        return;
    }

    let mut retain_mask = vec![false; values.len()];
    let mut seen = HashSet::new();

    for (index, component) in values.iter().enumerate().rev() {
        match component {
            ComponentValue::Declaration(declaration) => {
                if let Some(name) = declaration_name_key(&declaration) {
                    if seen.insert(name) {
                        retain_mask[index] = true;
                    }
                } else {
                    retain_mask[index] = true;
                }
            }
            _ => {
                retain_mask[index] = true;
            }
        }
    }

    let mut cursor = 0usize;
    values.retain_mut(|_| {
        let keep = retain_mask[cursor];
        cursor += 1;
        keep
    });
}

fn declaration_name_key(declaration: &Declaration) -> Option<String> {
    match &declaration.name {
        DeclarationName::Ident(ident) => Some(ident.value.to_string()),
        DeclarationName::DashedIdent(ident) => Some(ident.value.to_string()),
    }
}
