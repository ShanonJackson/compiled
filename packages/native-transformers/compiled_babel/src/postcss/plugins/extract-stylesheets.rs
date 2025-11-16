use once_cell::sync::Lazy;
use regex::Regex;
use swc_core::common::Spanned;
use swc_core::css::ast::{
    AtRule, AtRuleName, AtRulePrelude, ComponentValue, ListOfComponentValues, QualifiedRule,
    QualifiedRulePrelude, Rule, SimpleBlock, Stylesheet, Token,
};
use swc_core::css::codegen::{writer::basic::BasicCssWriter, CodeGenerator, CodegenConfig, Emit};

use super::super::transform::{Plugin, TransformContext};
use crate::postcss::utils::selector_stringifier;

#[derive(Debug, Default, Clone, Copy)]
pub struct ExtractStyleSheets;

impl Plugin for ExtractStyleSheets {
    fn name(&self) -> &'static str {
        "extract-stylesheets"
    }

    fn run(&self, stylesheet: &mut Stylesheet, ctx: &mut TransformContext<'_>) {
        if stylesheet.rules.is_empty() {
            return;
        }

        for rule in &stylesheet.rules {
            let Some(serialized) = serialize_rule(rule) else {
                continue;
            };

            ctx.push_sheet(serialized);
            if std::env::var("COMPILED_CSS_TRACE").is_ok() {
                if let Some(last) = ctx.sheets.last() {
                    eprintln!("[extract] sheet='{}'", last);
                }
            }
        }
    }
}

pub fn extract_stylesheets() -> ExtractStyleSheets {
    ExtractStyleSheets
}

static AT_RULE_SPACE_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"@(?:(media|supports|container|document|-moz-document))\(")
        .expect("valid at-rule spacing regex")
});

fn serialize_rule(rule: &Rule) -> Option<String> {
    let serialized = match rule {
        Rule::QualifiedRule(rule) => serialize_qualified_rule(rule),
        Rule::AtRule(rule) => serialize_at_rule(rule),
        Rule::ListOfComponentValues(list) => serialize_list_of_component_values(list),
    }?;

    let normalized = AT_RULE_SPACE_REGEX
        .replace_all(&serialized, |caps: &regex::Captures| format!("@{} (", &caps[1]))
        .into_owned();
    Some(normalized)
}

fn serialize_qualified_rule(rule: &QualifiedRule) -> Option<String> {
    match &rule.prelude {
        QualifiedRulePrelude::SelectorList(list) => {
            let selectors = selector_stringifier::serialize_selector_list(list);
            let block = serialize_simple_block(&rule.block)?;
            Some(format!("{selectors}{block}"))
        }
        QualifiedRulePrelude::RelativeSelectorList(list) => {
            let selectors = selector_stringifier::serialize_relative_selector_list(list);
            let block = serialize_simple_block(&rule.block)?;
            Some(format!("{selectors}{block}"))
        }
        QualifiedRulePrelude::ListOfComponentValues(list) => {
            if let Some(parsed) =
                selector_stringifier::parse_selector_list_from_component_values(list)
            {
                let selectors = selector_stringifier::serialize_selector_list(&parsed);
                let block = serialize_simple_block(&rule.block)?;
                Some(format!("{selectors}{block}"))
            } else {
                serialize_with_codegen(rule, true)
            }
        }
    }
}

fn serialize_at_rule(at_rule: &AtRule) -> Option<String> {
    let mut out = String::new();
    out.push('@');
    out.push_str(&at_rule_name(&at_rule.name));

    if let Some(prelude) = &at_rule.prelude {
        let serialized = serialize_at_rule_prelude(prelude)?;
        if !serialized.is_empty() {
            out.push(' ');
            out.push_str(&serialized);
        }
    }

    if let Some(block) = &at_rule.block {
        out.push_str(&serialize_simple_block(block)?);
    } else {
        out.push(';');
    }

    Some(out)
}

fn serialize_simple_block(block: &SimpleBlock) -> Option<String> {
    let (open, close) = match block.name.token {
        Token::LBrace => ('{', '}'),
        Token::LBracket => ('[', ']'),
        Token::LParen => ('(', ')'),
        _ => ('{', '}'),
    };

    let mut out = String::new();
    out.push(open);
    let contents = serialize_component_values(&block.value)?;
    out.push_str(&contents);
    out.push(close);
    Some(out)
}

fn serialize_component_values(values: &[ComponentValue]) -> Option<String> {
    let mut out = String::new();
    for value in values {
        let serialized = match value {
            ComponentValue::QualifiedRule(rule) => serialize_qualified_rule(rule)?,
            ComponentValue::AtRule(rule) => serialize_at_rule(rule)?,
            ComponentValue::SimpleBlock(block) => serialize_simple_block(block)?,
            ComponentValue::ListOfComponentValues(list) => {
                serialize_list_of_component_values(list)?
            }
            _ => serialize_with_codegen(value, true)?,
        };
        out.push_str(&serialized);
    }
    Some(out)
}

fn serialize_list_of_component_values(list: &ListOfComponentValues) -> Option<String> {
    serialize_component_values(&list.children)
}

fn serialize_with_codegen<T>(node: &T, minify: bool) -> Option<String>
where
    T: Spanned,
    for<'writer> CodeGenerator<BasicCssWriter<'writer, &'writer mut String>>: Emit<T>,
{
    let mut output = String::new();
    {
        let writer = BasicCssWriter::new(&mut output, None, Default::default());
        let mut generator = CodeGenerator::new(writer, CodegenConfig { minify });
        if generator.emit(node).is_err() {
            return None;
        }
    }
    Some(output)
}

fn serialize_at_rule_prelude(prelude: &AtRulePrelude) -> Option<String> {
    serialize_with_codegen(prelude, true)
}

fn at_rule_name(name: &AtRuleName) -> String {
    match name {
        AtRuleName::Ident(ident) => ident.value.to_string(),
        AtRuleName::DashedIdent(ident) => ident.value.to_string(),
    }
}
