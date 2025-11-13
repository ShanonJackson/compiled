# Cssnano v6.0.0 Parity Plan

This plan tracks a faithful, bug‑compatible Rust port of the cssnano preset used in the original JS pipeline (cssnano v6.0.0), executed strictly in the pre‑atomicify normalization slot.

## Goals
- Replicate cssnano‑derived normalization verbatim (order, defaults, and bugs), matching the original Babel + JS pipeline.
- Keep normalization strictly in the normalize‑css stage before atomicify, never elsewhere.
- Achieve class‑hash equality and style‑rules byte parity across fixtures.

## Scope & Sources
- Entry: packages/css/src/plugins/normalize-css.ts
- Pipeline placement: packages/css/src/transform.ts:62
- Cssnano version: 6.0.0 (preset‑default)
- Need exact plugin versions from lockfile (or vendored sources) to freeze behaviour.

## Plugin Set (and Order)
Always on (regardless of optimizeCss):
- postcss-minify-selectors
- postcss-minify-params

When optimizeCss = true:
- postcss-ordered-values
- postcss-reduce-initial
- postcss-convert-values
- postcss-colormin
- postcss-normalize-url
- postcss-normalize-unicode
- postcss-normalize-string
- postcss-normalize-positions
- postcss-normalize-timing-functions
- postcss-minify-gradients
- postcss-discard-comments
- postcss-calc

Custom additions (optimizeCss = true):
- normalize-current-color

## Guardrails
- No ad‑hoc normalization; only the above plugins in the exact normalize‑css slot.
- Preserve plugin defaults/options and bugs from the referenced versions.
- Any ambiguity resolved by referring to the exact JS sources/versions.

## Rust Module Layout
Under the PostCSS engine in compiled_babel:
- packages/native-transformers/compiled_babel/src/postcss/plugins/normalize_css/
  - minify_selectors.rs
  - minify_params.rs
  - ordered_values.rs
  - reduce_initial.rs
  - convert_values.rs
  - colormin.rs
  - normalize_url.rs
  - normalize_unicode.rs
  - normalize_string.rs
  - normalize_positions.rs
  - normalize_timing_functions.rs
  - minify_gradients.rs
  - discard_comments.rs
  - calc.rs
  - normalize_current_color.rs
  - mod.rs

## Execution Plan
1) Pin Versions
- [ ] Collect plugin versions used with cssnano‑preset‑default@6.0.0 from lockfile or vendored JS.

2) Replace Temporary Stubs
- [x] Port postcss-minify-selectors (initial engine port; expand edge cases next).
- [x] Port postcss-minify-params (initial engine port; expand edge cases next).

3) Hash‑Affecting Normalizers
- [x] Wire postcss-ordered-values (complete: border/outline, list-style, flex-flow, transition, animation, columns, box-shadow).
- [x] Wire postcss-convert-values (initial: 0px/‑0 to 0; expand next).
- [x] Port postcss-colormin (strict colord/minify parity; defaults via browserslist with transparent handling).
- [~] Port postcss-reduce-initial (safe subset; expand coverage).

4) Remaining Normalizers
- [x] Port normalize-url (strict, normalize-url@6.1.0 defaults as in plugin).
- [~] Port normalize-unicode/string/positions/timing-functions (initial coverage present; expand).
- [~] Port postcss-minify-gradients (initial coverage present; expand rules/edge cases).
- [x] Port postcss-discard-comments (AST removal).
- [~] Port postcss-calc (reducer subset; expand to full parity).
- [x] Port normalize-current-color (custom) when optimizeCss = true.

5) Wire & Validate
- [x] Invoke these plugins only in normalize‑css stage (pre‑atomicify) in build_processor.
- [ ] Run fixtures with engine: node packages/native-transformers/scripts/update-fixtures.js
- [ ] Match class hashes and style‑rules bytes; iterate only within this stage.

## Milestones
- M1: Selectors/params parity (hash stability preserved).
- M2: Ordered/convert/color parity (hash equality target).
- M3: Full normalize‑css parity (fixtures green).

## Notes
- optimizeCss defaults to true (packages/babel-plugin/src/types.ts:50). If off, only minify‑selectors/params should run.
- Keep plugin behaviour identical, including edge‑case bugs; add COMPAT notes where applicable.


## Progress Update
- Ordered Values: complete (border/outline, list-style, flex-flow, transition, animation, columns, box-shadow). Spacing assembled via explicit Space nodes and getValue logic.
- Convert Values: unitless zero for length types; refine contexts next.
- Colormin: strict port via colord; rgb/rgba%/hsl/hsla opaque to hex; transparent mapping; hex shortening; named-color substitution when shorter; spacing after function→word substitutions matches JS.
- Normalize URL: strict port with normalize-url@6.1.0 defaults as used by plugin; @namespace/url() quoting preserved identically.
- Normalize String: improved double→single conversion by removing unnecessary escapes.
- Normalize Positions: background-position 0px→0 for first two components; center center→center; keep extras.
- Normalize Timing Functions: cubic-bezier to standard names; steps(1,start|end) → step-start/end.
- Minify Gradients: tighten commas/slashes; trim inner spaces; remove default linear-gradient direction; expand remaining edge cases next.
- Discard Comments: implemented with AST removal.
- Calc: reducer subset (+/- same units; scalar mul/div); preserve calc() otherwise; expand to full parity.
- Normalize Current Color: canonicalize to currentColor.

