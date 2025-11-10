use swc_core::css::ast::{Rule, Stylesheet};
use swc_core::css::codegen::{writer::basic::BasicCssWriter, CodeGenerator, CodegenConfig, Emit};

use super::super::transform::{Plugin, TransformContext};

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
        }
    }
}

pub fn extract_stylesheets() -> ExtractStyleSheets {
    ExtractStyleSheets
}

fn serialize_rule(rule: &Rule) -> Option<String> {
    let mut output = String::new();
    {
        let writer = BasicCssWriter::new(&mut output, None, Default::default());
        let mut generator = CodeGenerator::new(writer, CodegenConfig { minify: true });
        if generator.emit(rule).is_err() {
            return None;
        }
    }

    Some(output)
}
