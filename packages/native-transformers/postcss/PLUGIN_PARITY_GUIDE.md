# PostCSS Plugin Parity Guide

This guide explains how to take a single PostCSS plugin from the JavaScript
reference in `packages/postcss-plugin-sources` and verify that the Rust
implementation under `packages/native-transformers/compiled_babel/src/postcss/plugins`
matches it **1:1** (structure, behaviour, and outputs).

## Prerequisites
- Install JS dependencies: `yarn install` (from repo root). If the spinner output
  blows past the terminal limit, you can instead bootstrap the Babel side with
  `npm install --legacy-peer-deps --registry=https://registry.npmjs.org --no-progress @babel/core`
  (plus any missing Babel helpers). This provides Babel, fixture helpers, and
  any plugin dependencies (e.g. `@atlaskit/tokens`).
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
4. **Isolate the plugin (now available)**
   - Set `POSTCSS_PLUGIN_UNDER_TEST=<plugin>` before running the fixtures to
     force both the Babel and Rust pipelines to wrap every non-matching plugin
     in a no-op while leaving the named plugin active. Optional stages
     (autoprefixer, increase-specificity, flatten-multiple-selectors) are
     automatically forced on when they are the plugin under test.
   - To exercise the entire pipeline in order, run
     `node packages/native-transformers/scripts/update-fixtures.js --each-plugin`.
     This re-invokes the runner once per gated plugin (plus an autoprefixer
     alias routed through `atomicify-rules`) and exits non-zero if any stage
     reports a mismatch.
   - The fixture runner writes plugin-scoped artifacts alongside the defaults:
     `babel-out.<plugin>.jsx`, `out.<plugin>.jsx`,
     `babel-style-rules.<plugin>.json`, and `swc-style-rules.<plugin>.json`.
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
