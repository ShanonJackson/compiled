use once_cell::sync::Lazy;
use std::collections::HashMap;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum ErrorMessages {
    NoTaggedTemplate,
    NumberOfArgument,
    ArgumentType,
    AtRuleValueType,
    SelectorsBlockValueType,
    DefineMap,
    NoSpreadElement,
    NoObjectMethod,
    StaticVariantObject,
    DuplicateAtRule,
    DuplicateSelector,
    DuplicateSelectorsBlock,
    StaticPropertyKey,
    SelectorBlockWrongPlace,
    UseSelectorsWithAmpersand,
    UseVariantOfCssMap,
}

static ERROR_TEXT: Lazy<HashMap<ErrorMessages, &'static str>> = Lazy::new(|| {
    use ErrorMessages::*;
    HashMap::from([
        (NoTaggedTemplate, "cssMap function cannot be used as a tagged template expression."),
        (NumberOfArgument, "cssMap function can only receive one argument."),
        (ArgumentType, "cssMap function can only receive an object."),
        (AtRuleValueType, "Value of at-rule block must be an object."),
        (
            SelectorsBlockValueType,
            "Value of `selectors` key must be an object.",
        ),
        (
            DefineMap,
            "CSS Map must be declared at the top-most scope of the module.",
        ),
        (NoSpreadElement, "Spread element is not supported in CSS Map."),
        (NoObjectMethod, "Object method is not supported in CSS Map."),
        (
            StaticVariantObject,
            "The variant object must be statically defined.",
        ),
        (
            DuplicateAtRule,
            "Cannot declare an at-rule more than once in CSS Map.",
        ),
        (DuplicateSelector, "Cannot declare a selector more than once in CSS Map."),
        (
            DuplicateSelectorsBlock,
            "Duplicate `selectors` key found in cssMap; expected either zero `selectors` keys or one.",
        ),
        (StaticPropertyKey, "Property key may only be a static string."),
        (
            SelectorBlockWrongPlace,
            "`selector` key was defined in the wrong place.",
        ),
        (
            UseSelectorsWithAmpersand,
            "This selector is applied to the parent element, and so you need to specify the ampersand symbol (&) directly before it. For example, `:hover` should be written as `&:hover`.",
        ),
        (
            UseVariantOfCssMap,
            "You must use the variant of a CSS Map object (eg. `styles.root`), not the root object itself, eg. `styles`.",
        ),
    ])
});

const HELP_LINK: &str =
    "Check out our documentation for cssMap examples: https://compiledcssinjs.com/docs/api-cssmap";

pub fn create_error_message(code: ErrorMessages) -> String {
    let text = ERROR_TEXT
        .get(&code)
        .copied()
        .unwrap_or("Unknown cssMap error");

    format!("{}\n\n{}", text, HELP_LINK)
}
