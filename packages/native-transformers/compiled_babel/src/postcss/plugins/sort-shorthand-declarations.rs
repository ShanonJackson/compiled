use std::cmp::Ordering;

use swc_core::css::ast::{ComponentValue, Declaration, DeclarationName, Rule, Stylesheet};

use super::super::transform::{Plugin, TransformContext};

#[derive(Debug, Default, Clone, Copy)]
pub struct SortShorthandDeclarations;

impl Plugin for SortShorthandDeclarations {
    fn name(&self) -> &'static str {
        "sort-shorthand-declarations"
    }

    fn run(&self, stylesheet: &mut Stylesheet, _ctx: &mut TransformContext<'_>) {
        sort_stylesheet(stylesheet);
    }
}

pub fn sort_shorthand_declarations() -> SortShorthandDeclarations {
    SortShorthandDeclarations
}

fn sort_stylesheet(stylesheet: &mut Stylesheet) {
    sort_rules(&mut stylesheet.rules);
}

pub(crate) fn sort_rules(rules: &mut Vec<Rule>) {
    for rule in &mut *rules {
        match rule {
            Rule::QualifiedRule(rule) => sort_component_values(&mut rule.block.value),
            Rule::AtRule(at_rule) => {
                if let Some(block) = &mut at_rule.block {
                    sort_component_values(&mut block.value);
                }
            }
            Rule::ListOfComponentValues(list) => sort_component_values(&mut list.children),
        }
    }

    rules.sort_by(|a, b| {
        compare_declaration_buckets(first_declaration_in_rule(a), first_declaration_in_rule(b))
    });
}

fn sort_component_values(values: &mut Vec<ComponentValue>) {
    for value in &mut *values {
        match value {
            ComponentValue::QualifiedRule(rule) => sort_component_values(&mut rule.block.value),
            ComponentValue::AtRule(at_rule) => {
                if let Some(block) = &mut at_rule.block {
                    sort_component_values(&mut block.value);
                }
            }
            ComponentValue::SimpleBlock(block) => sort_component_values(&mut block.value),
            ComponentValue::ListOfComponentValues(list) => {
                sort_component_values(&mut list.children)
            }
            ComponentValue::KeyframeBlock(block) => sort_component_values(&mut block.block.value),
            _ => {}
        }
    }

    values.sort_by(|a, b| {
        compare_declaration_buckets(
            first_declaration_in_component(a),
            first_declaration_in_component(b),
        )
    });
}

fn compare_declaration_buckets(a: Option<&Declaration>, b: Option<&Declaration>) -> Ordering {
    match (a, b) {
        (Some(a_decl), Some(b_decl)) => {
            let a_bucket = shorthand_bucket_for_declaration(a_decl).unwrap_or(u32::MAX);
            let b_bucket = shorthand_bucket_for_declaration(b_decl).unwrap_or(u32::MAX);
            a_bucket.cmp(&b_bucket)
        }
        _ => Ordering::Equal,
    }
}

fn shorthand_bucket_for_declaration(declaration: &Declaration) -> Option<u32> {
    let name = match &declaration.name {
        DeclarationName::Ident(ident) => ident.value.as_ref(),
        DeclarationName::DashedIdent(ident) => ident.value.as_ref(),
    };

    shorthand_bucket(name)
}

fn first_declaration_in_rule(rule: &Rule) -> Option<&Declaration> {
    match rule {
        Rule::QualifiedRule(rule) => find_first_declaration(&rule.block.value),
        Rule::AtRule(at_rule) => at_rule
            .block
            .as_ref()
            .and_then(|block| find_first_declaration(&block.value)),
        Rule::ListOfComponentValues(list) => find_first_declaration(&list.children),
    }
}

fn first_declaration_in_component(component: &ComponentValue) -> Option<&Declaration> {
    match component {
        ComponentValue::Declaration(declaration) => Some(declaration),
        ComponentValue::QualifiedRule(rule) => find_first_declaration(&rule.block.value),
        ComponentValue::AtRule(at_rule) => at_rule
            .block
            .as_ref()
            .and_then(|block| find_first_declaration(&block.value)),
        ComponentValue::SimpleBlock(block) => find_first_declaration(&block.value),
        ComponentValue::ListOfComponentValues(list) => find_first_declaration(&list.children),
        ComponentValue::KeyframeBlock(block) => find_first_declaration(&block.block.value),
        _ => None,
    }
}

fn find_first_declaration(values: &[ComponentValue]) -> Option<&Declaration> {
    for value in values {
        if let Some(declaration) = first_declaration_in_component(value) {
            return Some(declaration);
        }
    }

    None
}

pub fn shorthand_bucket(property: &str) -> Option<u32> {
    let bucket = match property {
        "all" => 0,
        "animation"
        | "animation-range"
        | "background"
        | "border"
        | "border-image"
        | "border-radius"
        | "column-rule"
        | "columns"
        | "contain-intrinsic-size"
        | "container"
        | "flex"
        | "flex-flow"
        | "font"
        | "font-synthesis"
        | "gap"
        | "grid"
        | "grid-area"
        | "inset"
        | "list-style"
        | "margin"
        | "mask"
        | "mask-border"
        | "offset"
        | "outline"
        | "overflow"
        | "overscroll-behavior"
        | "padding"
        | "place-content"
        | "place-items"
        | "place-self"
        | "position-try"
        | "scroll-margin"
        | "scroll-padding"
        | "scroll-timeline"
        | "text-decoration"
        | "text-emphasis"
        | "text-wrap"
        | "transition"
        | "view-timeline" => 1,
        "border-color"
        | "border-style"
        | "border-width"
        | "font-variant"
        | "grid-column"
        | "grid-row"
        | "grid-template"
        | "inset-block"
        | "inset-inline"
        | "margin-block"
        | "margin-inline"
        | "padding-block"
        | "padding-inline"
        | "scroll-margin-block"
        | "scroll-margin-inline"
        | "scroll-padding-block"
        | "scroll-padding-inline" => 2,
        "border-block" | "border-inline" => 3,
        "border-top" | "border-right" | "border-bottom" | "border-left" => 4,
        "border-block-start" | "border-block-end" | "border-inline-start" | "border-inline-end" => {
            5
        }
        _ => return None,
    };

    Some(bucket)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postcss::transform::{TransformContext, TransformCssOptions};
    use swc_core::common::{input::StringInput, FileName, SourceMap};
    use swc_core::css::ast::Rule as CssRule;
    use swc_core::css::codegen::{
        writer::basic::BasicCssWriter, CodeGenerator, CodegenConfig, Emit,
    };
    use swc_core::css::parser::{parse_string_input, parser::ParserConfig};

    fn parse_stylesheet(css: &str) -> Stylesheet {
        let cm: std::sync::Arc<SourceMap> = Default::default();
        let fm = cm.new_source_file(FileName::Custom("test.css".into()).into(), css.into());
        let mut errors = vec![];
        parse_string_input::<Stylesheet>(
            StringInput::from(&*fm),
            None,
            ParserConfig::default(),
            &mut errors,
        )
        .expect("failed to parse stylesheet")
    }

    fn declaration_names_from_rule(rule: &CssRule) -> Vec<String> {
        match rule {
            CssRule::QualifiedRule(rule) => collect_declaration_names(&rule.block.value),
            CssRule::AtRule(at_rule) => at_rule
                .block
                .as_ref()
                .map(|block| collect_declaration_names(&block.value))
                .unwrap_or_default(),
            CssRule::ListOfComponentValues(list) => collect_declaration_names(&list.children),
        }
    }

    fn declaration_name_to_string(name: &DeclarationName) -> String {
        match name {
            DeclarationName::Ident(ident) => ident.value.to_string(),
            DeclarationName::DashedIdent(ident) => ident.value.to_string(),
        }
    }

    fn collect_declaration_names(values: &[ComponentValue]) -> Vec<String> {
        let mut names = Vec::new();

        for value in values {
            match value {
                ComponentValue::Declaration(declaration) => {
                    names.push(declaration_name_to_string(&declaration.name));
                }
                ComponentValue::QualifiedRule(rule) => {
                    names.extend(collect_declaration_names(&rule.block.value));
                }
                ComponentValue::AtRule(at_rule) => {
                    if let Some(block) = &at_rule.block {
                        names.extend(collect_declaration_names(&block.value));
                    }
                }
                ComponentValue::SimpleBlock(block) => {
                    names.extend(collect_declaration_names(&block.value));
                }
                ComponentValue::ListOfComponentValues(list) => {
                    names.extend(collect_declaration_names(&list.children));
                }
                ComponentValue::KeyframeBlock(block) => {
                    names.extend(collect_declaration_names(&block.block.value));
                }
                _ => {}
            }
        }

        names
    }

    fn serialize_stylesheet(stylesheet: &Stylesheet) -> String {
        let mut output = String::new();
        let writer = BasicCssWriter::new(&mut output, None, Default::default());
        let mut generator = CodeGenerator::new(writer, CodegenConfig { minify: false });
        generator
            .emit(stylesheet)
            .expect("failed to serialize stylesheet");
        output
    }

    #[test]
    fn places_shorthand_before_longhand() {
        let mut stylesheet = parse_stylesheet(".a { margin-top: 1px; margin: 0; }");
        let options = TransformCssOptions::default();
        let mut ctx = TransformContext::new(&options);
        SortShorthandDeclarations.run(&mut stylesheet, &mut ctx);

        let rule = stylesheet
            .rules
            .first()
            .expect("expected a rule after sorting");
        let names = declaration_names_from_rule(rule);
        assert_eq!(names, vec!["margin", "margin-top"]);
    }

    #[test]
    fn sorts_nested_blocks() {
        let mut stylesheet =
            parse_stylesheet("@media screen { .a { padding-left: 4px; padding: 0; } }");
        let options = TransformCssOptions::default();
        let mut ctx = TransformContext::new(&options);
        SortShorthandDeclarations.run(&mut stylesheet, &mut ctx);

        let names = stylesheet
            .rules
            .first()
            .and_then(|rule| match rule {
                CssRule::AtRule(at_rule) => {
                    at_rule
                        .block
                        .as_ref()
                        .and_then(|block| match block.value.first() {
                            Some(ComponentValue::QualifiedRule(rule)) => {
                                Some(collect_declaration_names(&rule.block.value))
                            }
                            _ => None,
                        })
                }
                _ => None,
            })
            .expect("expected declarations inside nested block");

        assert_eq!(names, vec!["padding", "padding-left"]);
    }

    #[test]
    fn preserves_nodes_without_declarations() {
        let mut stylesheet =
            parse_stylesheet(".a { /* comment */ color: red; } .b { display: block; }");
        let original = serialize_stylesheet(&stylesheet);
        let options = TransformCssOptions::default();
        let mut ctx = TransformContext::new(&options);
        SortShorthandDeclarations.run(&mut stylesheet, &mut ctx);
        let sorted = serialize_stylesheet(&stylesheet);

        assert_eq!(original, sorted);
    }
}
