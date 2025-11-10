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

## Phase 3 – Port `@compiled/babel-plugin` to SWC
- Implement an SWC transform that mirrors the Babel visitor shape, copying state handling (sheets cache, cssMap, pragma flags, includedFiles, resolver) verbatim.
- Recreate the `State` struct, cache, and transform behaviours, including file-pass caching and namespace hashing.
- Port every feature-specific visitor into Rust modules that keep the same folder paths and helper boundaries.
- Reproduce all helper utilities such as expression evaluation, runtime import management, JSX pragma stripping, code frame errors, CSS building, and class name compression.
- Integrate the Rust `postcss` pipeline to compute sheets/class names, ensuring selectors, conditionals, and map handling match current behaviour.
- Swap Babel’s resolver wiring for `oxc_resolver` while keeping the same option surface and exposing sync resolution identical to the JS contract.
- Preserve side effects such as `includedFiles`, `pathsToCleanup`, pragma-driven React imports, and hoisting semantics exactly as in the Babel implementation.
- **Progress:** Core transform scaffolding, state management, and major visitors (css prop, styled, classNames, xcss, cssMap) now run natively with parity-focused unit coverage.
- **Progress:** Expression evaluation, runtime/styled helpers, and CSS map utilities have been ported with cache integration and deterministic metadata emission.
- **Progress:** Finished the outstanding `build_css` template literal and arrow-function branches, exported the stringification helpers they rely on, added unit coverage mirroring the Babel fixtures, and wired the visitors to use the shared `build_css` entry point.
- **Progress:** Added production wrappers for the css prop, styled, classNames, xcss, and cssMap visitors so they call the shared `build_css` helper without bespoke builders, matching the Babel invocation surface.
- **Progress:** The native transform now recognises compiled imports, recording alias metadata and stripping handled specifiers to mirror the Babel entry visitor.
- **Progress:** Metadata now captures deduplicated style rules when extraction is enabled, mirroring the Babel + strip-runtime workflow for downstream bundlers.
- **Progress:** Added a compiled util cleanup pass that nulls out `css`/`keyframes` variable initialisers after their styles are extracted, mirroring Babel's deferred path replacement. Expanded the cleanup visitor so any remaining `css`/`keyframes` expressions (including call arguments and collection elements) are rewritten to `null`, matching the Babel replacement semantics, and wired the `paths_to_cleanup` queue so the native transform follows the same deferred replacement flow as the Babel plugin.
- **Progress:** Styled invocations now normalize destructured props within the visitor itself, keeping downstream builders aligned with the Babel plugin without altering unrelated expressions.
- **Progress:** Module scope population now records re-exported specifiers so cross-module bindings resolve through nested imports, and the binding resolver follows those re-exports when loading dependencies.
- **Progress:** Program exit now injects the generated-by banner comment and leading noop statement so emitted files mirror the Babel plugin footer semantics.
- **Progress:** Preserving leading file comments now mirrors Babel by capturing pre-existing headers before runtime imports/noops are inserted, ensuring license banners stay ahead of the generated banner.
- **Progress:** Multi-comment preservation now mirrors Babel order so stacked headers remain stable when runtime imports are inserted.
- **Progress:** The Node bridge now mirrors Babel's `onIncludedFiles` callback by stripping the function before calling the native transform and replaying it with the returned metadata.
- **Progress:** Cache behaviour now aligns with the Babel plugin, sharing module-resolution results across transforms when `cache: true` while preserving per-file isolation for `'file-pass'` runs.
- **Progress:** Added a JSX runtime guard that mirrors the Babel plugin by panicking when transformed `jsx` calls appear after Compiled imports, including the original diagnostic messaging.

## Phase 4 – Port `@compiled/babel-plugin-strip-runtime`
- Build a second SWC transform that duplicates the existing Babel visitor logic: collect atomic style rules, remove runtime components, rewrite JSX/call expressions, inject runtime imports, and support SSR metadata output and filesystem extraction options.
- Use the Rust `sort` port to order emitted CSS when writing to disk, mirroring the original defaults and option overrides.
- Maintain metadata structures and option parsing parity with the original implementation.

## Phase 5 – Integration & packaging
- Provide JS bindings that let existing build tools conditionally load the native transformer while keeping fallback paths for older environments.
- Ensure generated artifacts stay identical so downstream tooling continues to work without modification.
- Update build scripts to compile the Rust crates alongside existing packages and publish them under the same npm package names without altering file layout.

## Phase 6 – Verification & regression safety
- Build a compatibility harness that runs every existing fixture/test through both the Babel implementation and the new SWC transform, diffing ASTs, generated sheets, hashes, and metadata.
- Add dedicated tests for the Rust `postcss` pipeline comparing class name hashes and sheet text against the JS baseline.
- Automate integration checks in CI so future changes run both Rust and JS versions to guarantee ongoing equivalence.
- Established a fixtures update script that produces Babel baselines (`babel-out.js`, `babel-style-rules.json`) alongside the
  SWC outputs for each case, wiring it into the `@compiled/native-transformers` workspace for iterative parity checks.

## Phase 7 – Rollout
- Ship the Rust implementations behind an opt-in flag, gather real-world comparisons, then flip the default once parity is proven.
- Keep the JS sources temporarily for reference/tests until confidence is high, then archive them while preserving the directory structure so hashes referencing file paths remain stable.
