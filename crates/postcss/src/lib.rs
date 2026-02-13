pub mod ast;
pub mod container;
pub mod error;
pub mod input;
pub mod list;
pub mod parser;
pub mod plugin;
pub mod plugins;
pub mod processor;
pub mod raws;
pub mod stringifier;
pub mod tokenizer;

pub use ast::{CssNode, NodeId, Stylesheet};
pub use error::CssSyntaxError;
pub use input::Input;
pub use plugin::Plugin;
pub use processor::Processor;

/// Parse a CSS string into a Stylesheet AST.
pub fn parse(css: &str) -> Stylesheet {
    let input = Input::new(css, None);
    let mut stylesheet = Stylesheet::new();
    parser::parse(&input, &mut stylesheet);
    stylesheet
}

/// Stringify a Stylesheet AST back to a CSS string.
pub fn stringify(stylesheet: &Stylesheet) -> String {
    stringifier::stringify(stylesheet)
}

/// Parse CSS, run plugins, and return the resulting CSS string.
pub fn process(css: &str, plugins: Vec<Box<dyn Plugin>>) -> String {
    let mut proc = Processor::new(plugins);
    proc.process(css)
}
