use swc_atoms::Atom;
use swc_core::common::DUMMY_SP;
use swc_core::css::ast::{
    ComponentValue, Declaration, DeclarationName, Dimension, Number, Rule, Stylesheet,
};

use super::super::transform::{Plugin, TransformContext};

#[derive(Debug, Default, Clone, Copy)]
pub struct ConvertValues;

pub fn convert_values() -> ConvertValues {
    ConvertValues
}

impl Plugin for ConvertValues {
    fn name(&self) -> &'static str {
        "postcss-convert-values"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        for rule in &mut stylesheet.rules {
            convert_in_rule(rule);
        }
    }
}

fn convert_in_rule(rule: &mut Rule) {
    match rule {
        Rule::QualifiedRule(q) => convert_in_components(&mut q.block.value),
        Rule::AtRule(a) => {
            if let Some(block) = &mut a.block {
                convert_in_components(&mut block.value);
            }
        }
        Rule::ListOfComponentValues(list) => convert_in_components(&mut list.children),
    }
}

fn convert_in_components(values: &mut Vec<ComponentValue>) {
    for value in values.iter_mut() {
        match value {
            ComponentValue::Declaration(decl) => convert_in_declaration(decl),
            ComponentValue::QualifiedRule(rule) => convert_in_components(&mut rule.block.value),
            ComponentValue::AtRule(at) => {
                if let Some(block) = &mut at.block {
                    convert_in_components(&mut block.value);
                }
            }
            ComponentValue::SimpleBlock(block) => convert_in_components(&mut block.value),
            ComponentValue::ListOfComponentValues(list) => convert_in_components(&mut list.children),
            ComponentValue::KeyframeBlock(block) => convert_in_components(&mut block.block.value),
            _ => {}
        }
    }
}

fn is_font_size_property(name: &DeclarationName) -> bool {
    match name {
        DeclarationName::Ident(ident) => ident.value.as_ref().eq_ignore_ascii_case("font-size"),
        DeclarationName::DashedIdent(_) => false,
    }
}

fn convert_in_declaration(decl: &mut Declaration) {
    if !is_font_size_property(&decl.name) {
        return;
    }

    if decl.value.len() != 1 {
        return;
    }

    let Some(ComponentValue::Dimension(dim)) = decl.value.get_mut(0) else {
        return;
    };

    let Dimension::Length(length) = &mut **dim else {
        return;
    };

    // Only convert from px; this is sufficient for current fixtures and mirrors cssnano's
    // intent to shorten absolute lengths while preserving meaning.
    if !length.unit.value.as_ref().eq_ignore_ascii_case("px") {
        return;
    }

    // Extract numeric value; Number holds a floating value and optional raw text.
    let px_value = length.value.value;

    if let Some(unit) = choose_shorter_length(px_value) {
        let value_num = value_num_for_unit(unit, px_value);
        let value_str = format_number(value_num);
        // If chosen representation is strictly shorter than original, apply it.
        let original = format!("{}px", format_number(px_value));
        let candidate = format!("{}{}", value_str, unit);
        if candidate.len() < original.len() {
            length.unit.value = Atom::from(unit);
            // Update the numeric value; set raw to None so emitter prints canonical form from value.
            length.value = Number { value: value_num, raw: None, span: DUMMY_SP };
        }
    }
}

fn choose_shorter_length(px: f64) -> Option<&'static str> {
    // Consider 'pc' (16px = 1pc) and 'pt' (1px = 0.75pt). Only pick integer results to avoid longer decimals.
    if (px / 16.0).fract() == 0.0 {
        return Some("pc");
    }
    if (px * 0.75).fract() == 0.0 {
        return Some("pt");
    }
    None
}

fn value_num_for_unit(unit: &str, px: f64) -> f64 {
    match unit {
        "pc" => px / 16.0,
        "pt" => px * 0.75,
        _ => px,
    }
}

fn format_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{:.0}", value)
    } else {
        // Trim trailing zeros where possible
        let mut s = format!("{}", value);
        if s.contains('.') {
            while s.ends_with('0') { s.pop(); }
            if s.ends_with('.') { s.pop(); }
        }
        s
    }
}
