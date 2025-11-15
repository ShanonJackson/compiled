**Overview**
- Goal: Port Autoprefixer 10.4.14 to Rust 1:1 so our native SWC pipeline and the PostCSS-engine path produce identical CSS to the original Babel+PostCSS pipeline. No deviations — same data, same decisions, same bugs/quirks, same output shape.
- Why: Ensure perfect parity across style rules and hashing inputs. Autoprefixer affects emitted CSS; if it differs, our atomic style rules or downstream behavior can diverge. Correctness means byte-for-byte equality (modulo cosmetic serializer differences).

**Scope**
- Translate Autoprefixer’s code and data from `packages/postcss-plugin-sources/autoprefixer` (v10.4.14) 1:1.
- Use the same caniuse-lite dataset and browserslist resolution to select add/remove sets.
- Implement every behavior: properties, values, selectors, at-rules, and all hacks under `lib/hacks/*`.
- Respect defaults and allow `overrideBrowserslist` (no repo-specific flags beyond what JS supports).

**Key Requirements**
- Parity first: decisions and ordering must match JS. Preserve warning messages and edge-case handling (e.g., grid warnings, deprecated properties).
- Hash stability: Autoprefixer runs after hashing; it must not change class names but must affect emitted style rules identically to JS.
- PostCSS-engine path must produce the same final strings as Babel. When Autoprefixer mutates value ordering in JS, we may inject at emission-time in the engine path to preserve order.

**Data Translation**
- Port `data/prefixes.js` to Rust, reading vendored caniuse-lite JSON to build `{ [name]: { browsers, mistakes, feature, props? } }`.
- Keep a JSON snapshot (generated from the vendored JS) as a deterministic bootstrap while the Rust builder lands.
- Align prefix exceptions (`agents.prefix_exceptions`) and prefix strings with caniuse-lite agents.

**Targets (Browserslist)**
- Mirror `lib/browsers.js`:
  - Resolve `overrideBrowserslist`, `env`, and defaults using `oxc_browserslist`.
  - Implement `prefix()` with exceptions per version.
  - Implement helpers: `prefixes()`, `withPrefix()`.

**Engine**
- Implement `prefixes.rs` for:
  - `select()` to compute add/remove maps from the data and selected targets (including mistakes and notes).
  - `preprocess()` to build structures for properties, values, selectors, and at-rules (`@keyframes`, `@viewport`, `@supports`, `@resolution`).
- Implement `processor.rs` for:
  - `add()` and `remove()` traversals with all warning cases from JS.
  - Grid/flex special-casing and guardrails (e.g., subgrid, display contents, align/justify in grid contexts).

**Hacks**
- Port all of `lib/hacks/*` exactly and register them as in JS:
  - Flexbox-related (2009/2012 specs, display:flex/inline-flex, basis/flow/grow/shrink/direction/wrap).
  - Grid (areas, rows/columns, prefixes, warnings).
  - Gradients and image functions (linear/radial-gradient legacy, image-set, cross-fade, pixelated, intrinsic).
  - Transforms/transitions, backdrop-filter, appearance, user-select, text-decoration variants, placeholder/fullscreen/file-selector-button, logical props.
  - Selector and value mutations matching Autoprefixer.

**Pipeline Wiring**
- SWC path: run Autoprefixer after `sort-atomic-style-sheet` and before whitespace/extract exactly as JS.
- PostCSS-engine path: match the same stage; when JS mutates value order within a rule, inject at emission-time so the final string matches Babel (ordering preserved) while still keeping the Autoprefixer pass for parity.

**Testing And Verification**
- Use `packages/native-transformers/scripts/update-fixtures.js` to compare:
  - `out.jsx` vs `babel-out.jsx` (code-only differences tolerated per project rules, stylistic only).
  - `swc-style-rules.json` vs `babel-style-rules.json` (must match including vendor prefixes and order).
- Add fixtures for:
  - Flexbox/inline-flex, grid (including autoplace/no-autoplace), gradients, image-set/cross-fade.
  - Selectors, `@supports`, `@resolution`, `@keyframes` duplication.
  - Removal of obsolete prefixes and deprecation warnings.

**Logging And Diagnostics**
- Optional trace envs:
  - `COMPILED_CLI_TRACE=1` logs plugin entry/exit, decisions (targets, add/remove keys) and atomic emissions.
  - `COMPILED_CSS_TRACE=1` logs extracted sheets.
- DO NOT change behavior based on traces; they are read-only diagnostics.

**Milestones**
- Data builder ported (prefixes map) + snapshot fallback.
- Browserslist parity (prefix exceptions, defaults) validated.
- Core engine (select/preprocess/add/remove) complete.
- Hacks ported (phase by phase):
  - Flexbox + display flex variants
  - Grid + warnings
  - Gradients + image funcs
  - Transforms/transitions + appearance/user-select/backdrop-filter
  - Text/placeholder/fullscreen/logical props
- Engine emission alignment done (value-order-sensitive paths covered) — fixtures green.

**Constraints And Non-Goals**
- No repo-specific behavior toggles beyond what Autoprefixer supports.
- No “smart fixes” or bespoke patches — if a difference is found, change must be made where the JS does it or via a documented COMPAT note and emission-time injection solely to preserve output order.

**Deliverables**
- Rust Autoprefixer module integrated with both SWC and PostCSS-engine paths.
- Vendored caniuse-lite JSON and a Rust data builder for full reproducibility.
- Fixtures that validate style-rules parity across the covered surface area.

