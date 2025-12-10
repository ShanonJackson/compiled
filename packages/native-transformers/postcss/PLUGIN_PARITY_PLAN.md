# PostCSS Plugin 1:1 Parity Execution Plan

This plan defines how we will validate every PostCSS plugin in the Rust pipeline against the original JavaScript sources **one plugin at a time** using the fixture harness.

## Goals
- Ensure each Rust plugin is a byte-for-byte port of its JavaScript counterpart (same folder/filename structure, identical inputs/outputs, bugs included).
- Use the fixture runner (`packages/native-transformers/scripts/update-fixtures.js`) to diff the Rust and Babel/PostCSS pipelines per plugin, prioritizing correctness before performance.
- Advance strictly plugin-by-plugin (never two at once) so divergences are easy to locate and fix at their source.

## Harness we need
1. **Single-plugin execution switch**
   - Add a `POSTCSS_PLUGIN_UNDER_TEST=<name>` flag (or CLI arg) consumed by `fixtures_cli` so the Rust transform assembles a pipeline that only runs the selected plugin (others become identity pass-throughs that preserve AST/raws).
   - Mirror this in the Babel runner inside `update-fixtures.js` so the JS pipeline is also reduced to the same single plugin with identical options/environment (`AUTOPREFIXER`, `optimizeCss`, `classHashPrefix`, etc.).
2. **Unified fixture diff mode**
   - Extend `update-fixtures.js` to emit per-plugin artifacts (e.g. `out.<plugin>.js`, `babel-out.<plugin>.js`, `swc-style-rules.<plugin>.json`, `babel-style-rules.<plugin>.json`) when a plugin is under test, and fail if any diff exists.
   - Ensure the script exercises **all** fixtures so regressions are caught across every edge case.
3. **Debug surface**
   - Pipe the plugin name into logs and fixture snapshot headings to make it obvious which plugin produced a mismatch.
   - Keep timing metrics optional; correctness gates are the priority.

## Workflow per plugin
1. Set `POSTCSS_PLUGIN_UNDER_TEST=<plugin>` and run `node packages/native-transformers/scripts/update-fixtures.js` to produce JS+Rust outputs for every fixture.
2. If any diff appears, debug inside the Rust plugin implementation using the JS source in `packages/postcss-plugin-sources`, updating the Rust code until outputs match.
3. Once the plugin is clean across fixtures, clear the flag (returning to full pipeline) and move to the next plugin.

## Plugin inventory and source mapping (pipeline order)
| Order | Plugin | Rust location | JS source reference | Status |
| --- | --- | --- | --- | --- |
| 1 | `discard-duplicates` | `compiled_babel/src/postcss/plugins/discard-duplicates.rs` | `packages/postcss-plugin-sources/postcss-discard-duplicates/src/index.js` | ✅ Parity verified |
| 2 | `discard-empty-rules` | `compiled_babel/src/postcss/plugins/discard-empty-rules.rs` | `packages/postcss-plugin-sources/postcss-discard-empty/src/index.js` | ✅ Parity verified |
| 3 | `parent-orphaned-pseudos` | `compiled_babel/src/postcss/plugins/parent-orphaned-pseudos.rs` | `packages/css/src/plugins/parent-orphaned-pseudos.ts` | ✅ Parity verified |
| 4 | `postcss-nested` (bubble/unwrap config) | `compiled_babel/src/postcss/plugins/nested.rs` | `packages/postcss-plugin-sources/postcss-nested/src/index.js` | ✅ Parity verified |
| 5 | `normalize-css` (cssnano preset slice) | `compiled_babel/src/postcss/plugins/normalize_css/mod.rs` + submodules (`normalize_css_engine`, `colormin.rs`, `convert-values.rs`, `minify-params.rs`, `minify-selectors.rs`, `normalize-whitespace.rs`, `ordered-values.rs`, `reduce-initial/mod.rs`, etc.) | `packages/postcss-plugin-sources/cssnano-preset-default/src/index.js` plus specific plugins (e.g. `postcss-ordered-values/src/index.js`, `postcss-reduce-initial/src/index.js`, `postcss-convert-values/src/index.js`, `postcss-colormin/src/index.js`, `postcss-minify-params/src/index.js`, `postcss-minify-selectors/src/index.js`, `postcss-normalize-whitespace/src/index.js`) | ✅ Parity verified |
| 6 | `normalize-current-color` (custom addition) | `compiled_babel/src/postcss/plugins/normalize-current-color.rs` | `packages/css/src/plugins/normalize-current-color.ts` (JS reference stored in repo) | ✅ Parity verified |
| 7 | `expand-shorthands` | `compiled_babel/src/postcss/plugins/expand-shorthands/mod.rs` | `packages/css/src/plugins/expand-shorthands` | ✅ Parity verified |
| 8 | `atomicify-rules` | `compiled_babel/src/postcss/plugins/atomicify-rules.rs` | `packages/css/src/plugins/atomicify-rules.ts` | ✅ Parity verified |
| 9 | `flatten-multiple-selectors` (optional) | `compiled_babel/src/postcss/plugins/flatten-multiple-selectors.rs` | `packages/css/src/plugins/flatten-multiple-selectors.ts` | ✅ Parity verified |
| 10 | `discard-duplicates` (second run when flattening is enabled) | Same as #1 | Same as #1 | ✅ Parity verified |
| 11 | `increase-specificity` (optional) | `compiled_babel/src/postcss/plugins/increase-specificity.rs` | `packages/css/src/plugins/increase-specificity.ts` | ✅ Parity verified |
| 12 | `sort-atomic-style-sheet` | `compiled_babel/src/postcss/plugins/sort-atomic-style-sheet.rs` | `packages/css/src/plugins/sort-atomic-style-sheet.ts` | ✅ Parity verified |
| 13 | `autoprefixer` | `compiled_babel/src/postcss/plugins/vendor_autoprefixer` | `packages/postcss-plugin-sources/autoprefixer/lib/autoprefixer.js` | ✅ Parity verified |
| 14 | `normalize-whitespace` | `compiled_babel/src/postcss/plugins/normalize-whitespace.rs` | `packages/postcss-plugin-sources/postcss-normalize-whitespace/src/index.js` | ✅ Parity verified |
| 15 | `extract-stylesheets` | `compiled_babel/src/postcss/plugins/extract-stylesheets.rs` | `packages/css/src/plugins/extract-stylesheets.ts` | ✅ Parity verified |

Notes:
- `normalize-css` expands to the exact cssnano preset plugins filtered by `optimizeCss`; the harness flag must pass through the same `optimizeCss`/`AUTOPREFIXER` options that the fixtures use today.
- When a plugin is optional in JS (e.g. `flatten-multiple-selectors`, `increase-specificity`, `autoprefixer`), the single-plugin mode should force-enable only that plugin and stub the rest to no-ops so the comparison is meaningful.

## Immediate next steps
- Use the wired `POSTCSS_PLUGIN_UNDER_TEST` flag to isolate plugins; the runner now emits suffixed artifacts (`babel-out.<plugin>.jsx`, `out.<plugin>.jsx`, `babel-style-rules.<plugin>.json`, `swc-style-rules.<plugin>.json`).
- Drive each plugin through the full fixture suite with the flag set and note any mismatches here with links to their fixes.
- Keep the checklist aligned with fixture results, not just code inspection, now that single-plugin snapshots are available.
