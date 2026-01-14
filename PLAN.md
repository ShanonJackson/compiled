# Plan for Native Rust/SWC Transformers

## Phase 0 – Deep audit & traceability [Completed]
- Captured a full parity audit in `packages/native-transformers/PHASE0_AUDIT.md`, covering the plugin APIs, visitor side effects, hashing invariants, and module-to-module mapping we must reproduce in Rust.
- Recorded definitive option and state shapes (plus their implicit defaults) to anchor the Rust `State` structs and configuration handling.

## Phase 1 – Rust workspace & folder parity [Completed]
- Created a Cargo workspace containing two crates: `compiled_babel` and `compiled_strip_runtime`. Mirrored the JS folder layout by mapping each top-level TS directory to a Rust module tree and kept filenames/paths identical so hashes tied to relative file paths remain stable.
- Exposed thin JS shims that load the Rust transformers through `@swc/core`’s native API, keeping existing entry points untouched for consumers expecting the current package surface.

## Phase 2 – `postcss.rs` parity layer
- Recreate `@compiled/css`’s pipeline in Rust (`postcss.rs`) so `transformCss` and `sort` deliver byte-for-byte identical outputs and class-name hashes. Implement a Rust version of every plugin currently chained inside the existing JS implementation, preserving invocation order, default option semantics, and callback wiring for class collection.
- Port each PostCSS plugin into dedicated Rust modules, following the original algorithm line-by-line to guarantee identical AST mutations and bugs.
- Re-implement the hashing utilities and any helpers used by these plugins so class name computation and compression remain unchanged.
- Build `postcss.rs` entry points mirroring `transformCss`/`sort` signatures, returning sheet vectors and class name vectors exactly as the JS version does.
- **Progress:** Established a dedicated `postcss` module with Rust structs mirroring the JS entry points, a plugin trait, and stub modules for every PostCSS helper so line-by-line translations can land without disturbing the overall folder structure. Ported behaviourally accurate translations for `discard-empty-rules`, `discard-duplicates`, `normalize-current-color`, `parent-orphaned-pseudos`, `flatten-multiple-selectors`, `increase-specificity`, `merge-duplicate-at-rules`, `atomicify-rules`, `expand-shorthands`, `sort-shorthand-declarations`, `sort-atomic-style-sheet`, `extract-style-sheets`, **and the `postcss-nested` plugin** (including the default bubble/unwrap configuration used by the Babel transform), and wired a SWC-backed plugin pipeline that mirrors the Babel ordering so serialized sheets come from the transformed AST while placeholders continue to expose the remaining hooks. Added whitespace-preserving value parsing helpers plus targeted unit coverage so shorthand expansion outputs stay byte-identical with the Babel/PostCSS baseline for hashing. Implemented the media query parsing utilities (`parse-media-query`, range parsers, and at-rule sorters) and pseudo-selector ordering so at-rules, shorthand sorting, and LVFHA ordering match the Babel/PostCSS behaviour. The nested translation now reproduces bubbling, unwrap, and `@at-root` hoisting semantics so nested rules, at-rules, and declaration hoisting follow the original PostCSS plugin exactly. Ported `postcss-normalize-whitespace` to native Rust so declaration values, function spacing, and IE hack handling serialize identically to the Babel/PostCSS baseline, and inserted it into the SWC pipeline ahead of stylesheet extraction. **Reimplemented `postcss-discard-comments` so important license banners (and all comments in non-optimised builds) are captured before SWC drops them, then re-emitted with original spacing during serialisation.** Completed a faithful Rust port of `postcss-minify-params`, covering browserslist-driven `all` handling, aspect-ratio reduction, deterministic argument sorting, and regression tests for media and supports queries.
- Added a native `postcss-ordered-values` translation with canonical spacing helpers, cache parity, and regression tests covering border, transition, animation, columns, list-style, and box-shadow scenarios to mirror the Babel/PostCSS output. Implemented a faithful Rust port of `postcss-reduce-initial`, including the browserslist + caniuse feature detection and the original from/to initial data tables so declarations collapse to `initial` (or expand from it) identically to cssnano.

## Phase 3 – Port `@compiled/babel-plugin` to SWC [Completed]
- Mirrored the Babel plugin end-to-end with native SWC visitors, covering css prop/styled/ClassNames/xcss/cssMap transforms, cleanup queues, JSX/runtime guards, script handling, and shared helpers for runtime imports, evaluation, and CSS building.
- Integrated the Rust PostCSS pipeline and hashing utilities so extracted sheets, atomic class names, and style-rule metadata remain byte-for-byte identical to the Babel implementation across fixtures and regression tests.
- Recreated state/resolver infrastructure (including module caching, `includedFiles`, comment/noop injection, `onIncludedFiles` bridge, and diagnostic handlers) with parity-focused unit and integration tests to validate dependency resolution, caching semantics, keyframe capture, and metadata emission.

## Phase 4 – Port `@compiled/babel-plugin-strip-runtime` [Completed]
- Build a second SWC transform that duplicates the existing Babel visitor logic: collect atomic style rules, remove runtime components, rewrite JSX/call expressions, inject runtime imports, and support SSR metadata output and filesystem extraction options.
- Use the Rust `sort` port to order emitted CSS when writing to disk, mirroring the original defaults and option overrides.
- Maintain metadata structures and option parsing parity with the original implementation.
- Delivered a native strip-runtime visitor with helper parity for `createElement`, JSX/automatic runtime call sites, identifier cleanup, and CC/CS import removal, backed by exhaustive unit coverage mirroring the Babel plugin.
- Integrated the Rust sorter, stylesheet extraction, SSR metadata/`compiledRequireExclude` flow, and filename guard behaviour so disk output, metadata, and error handling remain byte-for-byte with Babel.
- Added regression tests spanning stylesheet `require` injection, filesystem extraction (modules and scripts), automatic runtime reduction, metadata ordering, and comment preservation to validate parity before integration work.

## Phase 5 – Verification & regression safety
- Build a compatibility harness that runs every existing fixture/test through both the Babel implementation and the new SWC transform, diffing ASTs, generated sheets, hashes, and metadata.
- Add dedicated tests for the Rust `postcss` pipeline comparing class name hashes and sheet text against the JS baseline.
- Automate integration checks in CI so future changes run both Rust and JS versions to guarantee ongoing equivalence.
- Established a fixtures update script that produces Babel baselines (`babel-out.js`, `babel-style-rules.json`) alongside the SWC outputs for each case, wiring it into the `@compiled/native-transformers` workspace for iterative parity checks.
- Added Rust integration tests that execute the native compiled + strip-runtime pipeline against the fixture suite, asserting SWC style rules match Babel baselines and that the stored `actual.js` snapshots stay in sync with the transformer output.

## Phase 6 – Rollout
- Ship the Rust implementations behind an opt-in flag, gather real-world comparisons, then flip the default once parity is proven.
- Keep the JS sources temporarily for reference/tests until confidence is high, then archive them while preserving the directory structure so hashes referencing file paths remain stable.
