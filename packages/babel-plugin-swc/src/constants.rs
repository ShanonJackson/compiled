//! Constants mirrored from the original Babel implementation.
//!
//! Keeping these identifiers identical ensures the generated output remains
//! compatible with the existing runtime behaviour and any consumer tooling
//! that depends on the exact symbol names.

pub const DOM_PROPS_IDENTIFIER_NAME: &str = "__cmpldp";
pub const PROPS_IDENTIFIER_NAME: &str = "__cmplp";
pub const REF_IDENTIFIER_NAME: &str = "__cmplr";
pub const STYLE_IDENTIFIER_NAME: &str = "__cmpls";

pub const COMPILED_DIRECTIVE_DISABLE_LINE: &str = "@compiled-disable-line";
pub const COMPILED_DIRECTIVE_DISABLE_NEXT_LINE: &str = "@compiled-disable-next-line";
pub const COMPILED_DIRECTIVE_TRANSFORM_CSS_PROP: &str = "transform-css-prop";

pub const DEFAULT_CODE_EXTENSIONS: &[&str] = &[".js", ".jsx", ".ts", ".tsx"];

pub const COMPILED_IMPORT: &str = "@compiled/react";
pub const DEFAULT_IMPORT_SOURCES: &[&str] = &[COMPILED_IMPORT, "@atlaskit/css"];
