## Debugging Approach: css-nested-spread-active-after

When the `css-nested-spread-active-after` fixture exposed missing `:focus` / `:hover` style rules we needed the exact divergence point between Babel and the SWC port. The workflow that worked well:

1. **Instrument both implementations**
   - Added `COMPILED_CSS_TRACE` logging in the Babel plugin (`packages/css/src/plugins/atomicify-rules.ts`) and the Rust mirror (`compiled_babel/src/postcss/plugins/atomicify-rules.rs`) plus `transform-css-items` in both languages. With tracing enabled (`COMPILED_CSS_TRACE=1` and `COMPILED_CLI_TRACE=1`) every CSS chunk handed to PostCSS, as well as each generated selector/class, is printed.
2. **Run a single fixture with tracing**
   - `COMPILED_CSS_TRACE=1 COMPILED_CLI_TRACE=1 node packages/native-transformers/scripts/update-fixtures.js css-nested-spread-active-after > /tmp/css-trace.log 2>&1`
   - This captures Babel and SWC traces in the same log so differences are easy to diff.
3. **Locate the first divergence**
   - In this case Babel logged three separate `[css][atomicify] selector … rawSelector: '&:focus-visible:after'` entries (one per pseudo) while SWC logged a single `[atomicify.group] … sel='&:focus:after&:focus-visible:after&:hover:after'` showing we were hashing after merging selectors.
4. **Patch at the source**
   - Instead of compensating later, we changed `compiled_babel/src/postcss/postcss_pipeline.rs` so `atomicify_rules_plugin` now hashes each normalized selector independently (exactly how `packages/css/src/plugins/atomicify-rules.ts`’ `buildAtomicSelector()` does), then reran the traced fixture to confirm the log streams align.

Key takeaways: always compare the raw CSS handed into PostCSS, then inspect the atomicify stage because that’s where selectors get cloned/hashed. Logging in both languages with shared environment flags makes finding the first mismatch straightforward.
