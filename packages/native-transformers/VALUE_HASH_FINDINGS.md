# Value Hash Divergence Findings

## Background

Our hash function for the atomic class names splits the hash into two parts:
- `group` hash: computed off normalized selector + at-rule + property name.
- `value` hash: computed off the declaration value (including `!important`).

In Babel’s plugin (JS pipeline), the value hash is produced against the *raw declaration value string* before any cssnano-like minification steps run. That means values still contain the original whitespace that the author typed.

## SWC/PostCSS engine behavior today

In `compiled_babel/src/postcss/postcss_pipeline.rs` (the PostCSS-engine transform used by `fixtures_cli`), we currently:
1. Call `minify_color_value` and `minify_value_whitespace` on each declaration string.
2. Append `!important`.
3. Hash the string and emit CSS using that same (already minified) string.

Because the string is mutated before hashing, the `value` portion of the atomic class name diverges whenever author input has extra whitespace that Babel would have preserved for hashing. Example:
```
Input: marginLeft: 'var(--space-200, 4px)'
Babel value hash input: "var(--space-200, 4px)"
Our current input:       "var(--space-200,4px)"
```
Hashing those two strings yields different prefixes (`_bmks` vs `_bmks**e**`), causing fixture mismatches even though the final serialized CSS looks identical.

## Conclusion / next steps

To match Babel 1:1, the PostCSS-engine path needs to hash using the *pre-minified* value string (the original PostCSS AST raw) and only run whitespace/color minifiers on the copy that gets emitted. That likely means capturing the raw value before calling `minify_color_value` / `minify_value_whitespace` in `walk_and_emit` (and the other atomicify helpers) and threading both variants separately: one for `value_seed`/hashing, one for serialized CSS.

