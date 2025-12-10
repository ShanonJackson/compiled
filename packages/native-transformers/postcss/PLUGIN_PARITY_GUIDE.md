# PostCSS Plugin Parity Guide

This guide explains how to take a single PostCSS plugin from the JavaScript
reference in `packages/postcss-plugin-sources` and verify that the Rust
implementation under `packages/native-transformers/compiled_babel/src/postcss/plugins`
matches it **1:1** (structure, behaviour, and outputs).

## Prerequisites
- Install JS dependencies: `yarn install` (from repo root). This provides Babel,
  fixture helpers, and any plugin dependencies (e.g. `@atlaskit/tokens`).
- Build the Rust fixture runner: `cargo build --manifest-path packages/native-transformers/Cargo.toml -p fixtures_cli --release`.
- Ensure the Babel fixture harness can run: `node packages/native-transformers/scripts/update-fixtures.js`.

## Files to know per plugin
- **Rust translation**: `packages/native-transformers/compiled_babel/src/postcss/plugins/<plugin>.rs`
- **JS source**: `packages/postcss-plugin-sources/<js-package>/src/index.js`
- **Parity checklist**: `packages/native-transformers/postcss/PLUGIN_PARITY_PLAN.md`

## Verification loop (one plugin at a time)
1. **Locate the JS behaviour**
   - Open the JS source to understand traversal order, mutation rules, and any
     quirks (e.g. raw spacing checks, comment handling).
2. **Cross-map to Rust**
   - Confirm the Rust file mirrors the JS folder/file layout and function
     boundaries (helper names, traversal order, and control flow).
3. **Run fixtures**
   - Execute `node packages/native-transformers/scripts/update-fixtures.js` to
     generate Babel vs. Rust outputs for all fixtures. This reports any diffs in
     generated JS and style-rules JSON.
4. **Isolate the plugin**
   - When the single-plugin test switch described in
     `PLUGIN_PARITY_PLAN.md` is available, set
     `POSTCSS_PLUGIN_UNDER_TEST=<plugin>` before running the fixtures to limit
     the pipeline to the plugin under inspection.
   - Until that switch lands, keep the pipeline intact but focus on diffs that
     originate from the plugin's responsibilities (compare the JS source to the
     Rust implementation and add targeted logging if needed).
5. **Debug mismatches**
   - Add temporary `eprintln!` calls in the Rust plugin (mirroring console logs
     you might add in JS) to trace traversal order and captured values.
   - Compare against the JS source to ensure the same branches run for the same
     inputs; adjust the Rust code to mirror the JS logic exactly.
6. **Re-run fixtures**
   - Repeat `update-fixtures.js` until the diffs disappear. The plugin is only
     considered parity-complete when **all** fixtures match Babel outputs.
7. **Update the plan**
   - Mark the plugin as verified in `PLUGIN_PARITY_PLAN.md` once the fixtures
     pass without diffs. Leave notes if any intentional deviations were required
     (and why).

## Tips
- Prefer structural equivalence over behavioural approximation: if the JS code
  computes an intermediate array, do the same in Rust even if the compiler could
  optimise it away.
- Preserve raw/spacing-sensitive data whenever the JS plugin checks it; if the
  AST does not expose equivalent data, document the gap near the affected code
  and flag it in the plan.
- Keep changes tightly scoped to a single plugin to maintain a clear debugging
  surface.
