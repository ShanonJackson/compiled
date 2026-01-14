# PostCSS Engine Integration Plan

This document tracks the work to integrate a Rust PostCSS drop‑in into the native transformers and reach byte‑/hash‑parity with the original Babel + JS PostCSS pipeline.

## Goals
- Drop‑in replacement for the Babel plugin CSS pipeline, preserving hashing and ordering invariants.
- Run natively in Rust inside `packages/native-transformers` without WASM.
- Keep behaviour and known bugs identical where required for parity.

## Status Checklist

Core setup
- [x] Vendor PostCSS crate into `packages/native-transformers/postcss`
- [x] Flatten crate (`crates/postcss` -> `postcss/`)
- [x] Add workspace member in `packages/native-transformers/Cargo.toml`
- [x] LTO mitigation for local builds (`.cargo/config.toml`, profiles, scripts)
- [x] Feature‑gated engine in `compiled_babel` (`postcss_engine`)
- [x] PostCSS engine is the default pipeline when building with the `postcss_engine` feature; no env flag required.

Engine pipeline scaffolding
- [x] Build processor with ordered plugins mirroring JS pipeline
- [x] Discard Empty Rules plugin (declaration pruning + remove empty rules)
- [x] Normalize Whitespace pass (Raws clean)
- [x] Atomicify Rules (initial):
  - [x] Selector combination and `&` replacement
  - [x] Group/value hashing parity prototype
  - [x] Emit atomic sheets and collect class names
- [ ] Atomicify Rules (complete):
  - [x] At‑rule chain traversal and wrapping (include in group seed)
  - [x] Honour `classHashPrefix` (proto exists) and compression map
  - [x] Ensure class name ordering matches emitted sheets
 - [x] Wire engine normalize‑css stage (pre‑atomicify):
   - [x] minify‑selectors (initial engine port)
   - [x] minify‑params (initial engine port)
   - [x] ordered‑values/convert‑values/colormin/reduce‑initial (engine ports wired; expand reduce‑initial/convert‑values coverage)

Parity plugins (to port or emulate)
- [ ] `discard-duplicates`
- [ ] `parent-orphaned-pseudos`
- [ ] `postcss-nested` (bubble/unwrap config):
  - Bubble: `container`, `-moz-document`, `layer`, `else`, `when`, `starting-style`
  - Unwrap: `color-profile`, `counter-style`, `font-palette-values`, `page`, `property`
- [ ] `normalize-css` helpers (values/whitespace normalisation used pre‑hash)
- [ ] `expand-shorthands`
- [ ] `flatten-multiple-selectors` (+ follow‑up `discard-duplicates`)
- [ ] `increase-specificity` (optional by config)
- [ ] `sort-atomic-style-sheet` (respect `sortAtRules`/`sortShorthand`)
- [ ] `autoprefixer` (after hashing)
- [ ] `extract-stylesheets` (collect into sheets)

## Hashing invariants
- [x] Use shared murmur hash implementation from `utils_hash`
- [x] Group = hash(prefix + atChain + normalizedSelector + prop)[0..4]
- [x] Value = hash(value + `!important` if present)[0..4]
- [~] Confirm colour/value normalisation pre‑hash matches JS behaviour (`normalize-css`/`colormin`)

## Validation
- [ ] Wire engine behind feature flag in fixtures runner (env)
- [ ] Run `packages/native-transformers/scripts/update-fixtures.js`
- [ ] Achieve class name hash equality across fixtures
- [ ] Achieve sheet/text parity modulo allowed cosmetic diffs

## Pointers
- Engine staging and plugins: `packages/native-transformers/compiled_babel/src/postcss/postcss_pipeline.rs`
- Hashing reference (Rust): `packages/native-transformers/compiled_babel/src/utils/hash.rs`
- JS pipeline reference: `packages/css/src/transform.ts`
- Atomicify reference (JS): `packages/css/src/plugins/atomicify-rules.ts`

## How to build and try
- PowerShell: `cd packages/native-transformers; $env:RUSTFLAGS='-Clto=no'; cargo build -p compiled_babel --features postcss_engine`
- The transformer routes through the PostCSS engine by default (feature-enabled). No environment toggle is necessary.
- Fixture script: `node packages/native-transformers/scripts/update-fixtures.js`

## Notes
- At‑rule removal currently prunes empty rules; empty at‑rules will be addressed when a safe parent access strategy is added.
- The engine presently computes selectors within the atomicify pass rather than mutating AST via `postcss-nested`; parity will be ensured by mirroring its bubble/unwrap semantics in the combination logic.

## Guardrails (Parity Over Convenience)
- Do not introduce ad‑hoc normalization. Every normalization must map 1:1 to the original JS pipeline position and behaviour.
- Pre‑atomicify steps only include what the original used before hashing: `postcss-nested` (with exact bubble/unwrap), the `normalize-css` group (value/whitespace helpers used pre‑hash), minimal colour/value normalization if present, then `expand-shorthands`, then `atomicify-rules`.
- Post‑atomicify steps mirror the original: optional `flatten-multiple-selectors` + `discard-duplicates`, `increase-specificity`, `sort-atomic-style-sheet`, `autoprefixer`, `normalize-whitespace`, `extract-stylesheets`.
- When a behaviour is unclear, prefer porting the exact JS plugin logic (including quirks/bugs) rather than substituting.
- All changes validated by fixtures for both AST output parity (cosmetic diffs allowed) and style‑rules hash equality.
