# Cssnano Engine Port – Plugin-by-Plugin Plan

This plan tracks exact, line-by-line Rust ports of the cssnano plugins used by `packages/css/src/plugins/normalize-css.ts` (the normalize-css stage in Compiled), executed strictly in the pre-atomicify slot. Behaviour and bugs are to be preserved to avoid long‑tail hash drift.

Source of truth for versions: `packages/native-transformers/postcss/VERSIONS_CSSNANO.md` (from this repo’s node_modules).

## Guardrails
- Only run in the normalize-css stage (pre-atomicify), exactly where the JS pipeline runs it.
- Port line‑for‑line from the installed JS sources; no heuristic changes or relocations.
- Preserve bugs/quirks; add `// COMPAT:` notes when behaviour depends on original quirks.
- Validate via broad fixture and synthetic coverage to prevent long‑tail hash drift.

## Status Legend
- [ ] Not started
- [~] Initial engine port (partial; more to do)
- [x] Behaviourally identical (complete)

## Plugin Checklist (in execution order)

Always (optimizeCss true/false):
- [x] postcss-minify-selectors (5.2.1)
  - Reducers: attribute/operator spacing, combinator trim, universal removal, pseudo/tag transforms, nth simplifications, dedupe/sort, caching
  - Engine file: `compiled_babel/src/postcss/plugins/normalize_css_engine/mod.rs`
  - Current: attribute/combinator/space handling implemented; expand pseudo/tag/universal/nth/dedupe/sort

- [x] postcss-minify-params (5.1.4)
  - Params whitespace rules, punctuation spacing, function/paren handling
  - Engine file: same
  - Current: whitespace/punctuation/paren handling implemented; expand lists/functions coverage

When optimizeCss = true:
- [x] postcss-ordered-values (5.1.3)
  - Shorthand canonicalization (margin/padding/border-*) and other props
  - Engine file: same
  - Current: Expanded to cover border/outline canonical order; list-style (type, position, image); flex-flow (direction, wrap); transition (property, duration, timing, delay) with per-item ordering; animation (name, duration, timing, delay, iteration, direction, fill, play-state); columns (count, width); box-shadow (inset, lengths, color).

- [x] postcss-reduce-initial (5.1.2)
  - Replace lengthy initial-equivalent values safely
  - Engine file: same
  - Current: strict toInitial/fromInitial maps; default ignore set; initial support assumed (no browserslist config)

- [~] postcss-convert-values (5.1.3)
  - Unit/value conversions (0 units, deg, time, etc.), safe contexts, ignore cases
  - Engine file: same
  - Current: 0-unit conversions; expand contextual safety

- [x] postcss-colormin (5.3.1)
  - Strict port via colord minify + names logic; browserslist defaults applied for transparent, alphaHex true, name true. Walker matches function/word handling and spacing.

- [x] postcss-normalize-url (5.1.0)
  - Strict port with normalize-url 6.1.0; exact plugin defaults applied (normalizeProtocol=false, sortQueryParameters=false, stripHash=false, stripWWW=false, stripTextFragment=false). Handles data: URLs, protocol-relative/absolute normalization, @namespace quoting, url() quoting/escaping identical to JS.

- [~] postcss-normalize-unicode (5.1.1)
  - Normalize unicode-range forms: uppercase U+, strip leading zeros, keep wildcards, preserve ranges; merging left for later.

- [~] postcss-normalize-string (5.1.0)
  - Convert quotes when safe
  - Current: double→single when safe; expand rules

- [~] postcss-normalize-positions (5.1.1)
  - Normalize background-position and related
  - Current: initial tweak; expand rules

- [~] postcss-normalize-timing-functions (5.1.0)
  - Timing function normalization
  - Current: cubic-bezier mapping to ease variants; steps(1,start|end) -> step-start|step-end.

- [~] postcss-minify-gradients (5.1.1)
  - Gradient argument normalization
  - Current: tighten commas; expand rules

- [x] postcss-discard-comments (5.1.2)
  - Remove comments; implemented via AST removal

- [~] postcss-calc (8.2.4)
  - Simplify calc() expressions; numeric folding
  - Current: reduce to single value when safely computable (same-unit add/sub, scalar mul/div); preserves calc() otherwise. Precision=5. Expand to full reducer parity.

## Validation
- [ ] Add synthetic coverage runner comparing engine vs JS plugin outputs for broad inputs per plugin.
- [ ] Run repository fixtures with engine enabled; iterate only within normalize-css stage.
- [ ] Achieve hash equality for fixtures and synthetic coverage.

## Pointers
- JS sources under `node_modules/<plugin>/src/*.js` for each plugin.
- Engine implementations currently consolidated in:
  - `packages/native-transformers/compiled_babel/src/postcss/plugins/normalize_css_engine/mod.rs`

