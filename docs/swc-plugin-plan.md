# Compiled SWC Plugin Migration Plan

## Goals and non-goals
- **Goal:** replace the existing Babel-based CSS-in-JS transformation pipeline (`@compiled/babel-plugin` + `@compiled/babel-plugin-strip-runtime`) with a Rust SWC plugin that produces byte-identical (or cosmetically equivalent) JavaScript and style artifacts.
- **Goal:** merge configuration capabilities from both Babel plugins so that a single SWC plugin handles CSS compilation, runtime stripping, class name compression, extraction, and stylesheet emission.
- **Goal:** preserve deterministic hashing and the atomic CSS generation algorithm so the runtime output remains a drop-in replacement.
- **Goal:** statically evaluate imported values (including `import { value } from 'location'`) without the customizable resolver hook; use [`oxc_resolver`](https://docs.rs/oxc_resolver) to match webpack/enhanced-resolve semantics.
- **Goal:** focus on low-allocation Rust data structures and pre-allocation strategies to maximise throughput.
- **Non-goal:** support the Babel-specific `resolver` option or other JS runtime hooks that are not required in Rust.

## Summary of current Babel behaviour

### Plugin lifecycle and state
The Babel plugin initialises caches, tracks included files, and resolves configured import sources before running visitors.【F:packages/babel-plugin/src/babel-plugin.ts†L71-L119】 It stores transformation artefacts on the plugin state: compiled import bindings, collected sheets, cssMap entries, and a WeakMap of transformed paths.【F:packages/babel-plugin/src/types.ts†L124-L218】 Runtime imports and React fallbacks are appended at program exit, followed by cleanup of recorded paths.【F:packages/babel-plugin/src/babel-plugin.ts†L200-L239】

### API detection and visitors
`ImportDeclaration` visitors detect Compiled entry points (`styled`, `css`, `ClassNames`, `keyframes`, `cssMap`) by matching configured sources, deleting consumed specifiers, and enabling downstream transforms.【F:packages/babel-plugin/src/babel-plugin.ts†L241-L293】 The unified visitor handles CSS utilities (`css`, `keyframes`) and component factories (`styled`) by dispatching into specialised modules after normalising prop usage.【F:packages/babel-plugin/src/babel-plugin.ts†L295-L349】 JSX elements/attributes delegate to `ClassNames`, `css` prop, and `xcss` handlers when enabled.【F:packages/babel-plugin/src/babel-plugin.ts†L351-L367】

### Static evaluation and dependency resolution
The plugin performs deep static evaluation across identifiers, member expressions, functions, and call expressions to resolve CSS values or hoistable sheets.【F:packages/babel-plugin/src/utils/evaluate-expression.ts†L18-L122】 Evaluation relies on scope-aware binding resolution that traverses destructuring, cross-file imports, and cached module parses, optionally using a custom resolver or Node resolution rules.【F:packages/babel-plugin/src/utils/resolve-binding.ts†L17-L194】 Caching is provided by an LRU cache keyed with the shared murmur hash helper.【F:packages/babel-plugin/src/utils/cache.ts†L1-L128】

### CSS construction and hashing
Styled components and utilities funnel through `buildCss`/`transformCssItems`, which convert logical/object/conditional CSS fragments into atomic sheets and runtime class expressions.【F:packages/babel-plugin/src/utils/transform-css-items.ts†L1-L120】 CSS compilation delegates to `@compiled/css/transform`, preserving options such as optimisation, specificity, at-rule sorting, shorthand ordering, flattening, and class hash prefixes.【F:packages/css/src/transform.ts†L1-L88】 Atomic class names derive from murmur-hash group/value segments (`_{group}{value}`) with optional compression map lookups.【F:packages/css/src/plugins/atomicify-rules.ts†L1-L117】 CSS variable references are hashed into stable custom property names when arbitrary expressions are encountered.【F:packages/babel-plugin/src/utils/css-builders.ts†L650-L688】 Keyframe names are deterministically derived from hashing the declaration source to maintain referential stability.【F:packages/babel-plugin/src/utils/css-builders.ts†L461-L489】

### Runtime integration and metadata
The plugin injects runtime helpers (`ax`/`ac`/`ix`/`CC`/`CS`) depending on whether a className compression map is provided, and adds React imports when required.【F:packages/babel-plugin/src/utils/append-runtime-imports.ts†L24-L67】【F:packages/babel-plugin/src/babel-plugin.ts†L200-L207】 It preserves leading comments before inserting runtime artefacts.【F:packages/babel-plugin/src/babel-plugin.ts†L195-L239】

### Strip-runtime responsibilities
`@compiled/babel-plugin-strip-runtime` gathers emitted atomic style rules, optionally serialises them to metadata, injects `require()` statements for server-side collection, or writes CSS assets to disk with stable sorting.【F:packages/babel-plugin-strip-runtime/src/index.ts†L21-L102】 It removes runtime `CC`/`CS` specifiers and rewrites JSX or `React.createElement` invocations to inline children while stripping style declarations.【F:packages/babel-plugin-strip-runtime/src/index.ts†L106-L198】 Plugin options configure stylesheet paths, metadata extraction, destination directories, and deterministic sorting behaviour.【F:packages/babel-plugin-strip-runtime/src/types.ts†L13-L70】

### Configuration surface area
`@compiled/babel-plugin` exposes options for caching, React import control, nonce support, import sources, file inclusion callbacks, CSS optimisation, resolver overrides, extension lists, parser plugins, component name injection, class name compression, XCSS toggles, specificity tweaks, at-rule sorting, hash prefixing, and selector flattening.【F:packages/babel-plugin/src/types.ts†L12-L121】 `@compiled/babel-plugin-strip-runtime` adds stylesheet path injection, runtime exclusion, extraction directory configuration, and shorthand sorting flags.【F:packages/babel-plugin-strip-runtime/src/types.ts†L13-L40】

## Requirements for the SWC plugin
1. **Functional parity:** reproduce all transformation paths (styled, css prop, class names, keyframes, cssMap, xcss) and runtime stripping logic, while merging option handling into one configuration surface.
2. **Deterministic hashing:** reimplement the murmur-hash based algorithms for class groups, value hashes, keyframe names, and CSS custom property identifiers so generated class names remain identical.【F:packages/css/src/plugins/atomicify-rules.ts†L1-L47】【F:packages/babel-plugin/src/utils/css-builders.ts†L650-L688】
3. **Static evaluation:** provide SWC equivalents of `evaluateExpression` and `resolveBinding`, including recursive import analysis using `oxc_resolver` for file resolution, while respecting configured extensions and parser plugins.
4. **Style rule aggregation:** mirror strip-runtime behaviour by collecting generated style sheets, emitting metadata or filesystem assets when configured, and exposing the results alongside the transformed `Program` in Rust.
5. **Runtime injection:** insert runtime helpers, React imports, and pragma cleanup in SWC AST form, preserving comment handling semantics.
6. **Testing parity:** build fixtures that compare SWC output against Babel output (via `@babel/core` with both plugins) to guarantee behavioural equivalence.
7. **Performance discipline:** minimise allocations with `SmallVec`, `FxHashMap`, arena-backed string interning, and avoid repeated parsing/evaluation by memoising per-file computations.

## Proposed Rust architecture

### Crate layout
- `crates/compiled_swc_plugin/`: SWC plugin entry with `Cargo.toml`, exposing a `transform(program, config) -> (Program, StyleArtifacts)` API.
- `crates/compiled_css/`: Rust port of the CSS transformation pipeline mirroring `@compiled/css`, including atomicification, sorting, and hashing helpers.
- `crates/compiled_eval/`: shared static evaluation utilities (resolver integration, AST traversal, cache) to keep the plugin modular.
- Shared `hash` module implementing the murmurhash2 GC algorithm exactly once for reuse across crates.【F:packages/utils/src/hash.ts†L1-L39】

### Plugin state model
- Maintain a `TransformState` struct analogous to Babel state (compiled imports, pragma flags, sheets, cssMap cache, ignored member expressions, include files, transform cache, resolver handle).【F:packages/babel-plugin/src/types.ts†L124-L218】 Use `FxHashMap<String, SmallVec<[JsWord; 2]>>` for compiled imports to avoid heap churn.
- Track style rules as `Vec<String>` (or `SmallVec<[String; 4]>`) for local accumulation and optional metadata export.
- Provide `StyleArtifacts` struct containing `sheets`, `style_rules`, `metadata`, and optional file output plan for extraction.

### AST traversal strategy
- Leverage `swc_ecma_visit::Fold` or `VisitMut` to replicate Babel visitor structure:
  - Pre-pass sets up resolver/import sources and caches.
  - Visit `Module`/`Script` top-level to handle pragmas, comment filtering, React injection, and runtime imports.
  - Detect Compiled import sources by matching string literals, normalising relative paths via the configured root, and removing consumed specifiers before further traversal.
  - Implement dedicated handlers for `CallExpr`/`TaggedTpl`, `JSXElement`, and `JSXOpeningElement` mirroring the Babel dispatch graph.【F:packages/babel-plugin/src/babel-plugin.ts†L295-L367】
  - Replicate `normalizePropsUsage` semantics within SWC AST to maintain autop-run parity.

### Static evaluation & resolver
- Port the evaluation graph (identifiers, member expressions, functions, calls, unary/binary expressions) to operate on SWC nodes. Recreate scope analysis by consulting `swc_ecma_visit::Visit` with lexical environment tracking.
- Implement module loading using `oxc_resolver` to resolve specifiers relative to the current file and follow extension lists, defaulting to Babel’s `DEFAULT_CODE_EXTENSIONS` semantics.【F:packages/babel-plugin/src/utils/resolve-binding.ts†L11-L194】
- Parse foreign modules with `swc_ecma_parser`, applying configured syntax extensions to mimic Babel parser plugin support.
- Cache parsed modules and evaluated bindings via an LRU map keyed by hashed request/context combinations.【F:packages/babel-plugin/src/utils/cache.ts†L1-L128】

### CSS transformation port
- Translate the PostCSS pipeline to Rust by modelling equivalent passes:
  - Deduplicate and discard empty rules before nested expansion.
  - Normalise CSS (autoprefixing optional via `lightningcss`/`parcel_css` equivalent or by invoking existing JS via FFI if parity requires).
  - Expand shorthands, atomicify rules, flatten selectors, adjust specificity, and sort atomic sheets following the JS plugin order.【F:packages/css/src/transform.ts†L1-L88】
  - Implement atomic class name generation exactly as `atomicClassName`, including hash prefix and compression map support.【F:packages/css/src/plugins/atomicify-rules.ts†L38-L117】
  - Recreate CSS variable hashing and keyframe naming utilities for consistent outputs.【F:packages/babel-plugin/src/utils/css-builders.ts†L461-L489】【F:packages/babel-plugin/src/utils/css-builders.ts†L650-L688】
- Provide an API that returns sheets and class name lists for integration with the SWC plugin’s runtime expression builder.

### Runtime stripping integration
- After CSS compilation, aggregate style rules and honour the merged options (`styleSheetPath`, `compiledRequireExclude`, `extractStylesToDirectory`, `sortAtRules`, `sortShorthand`).【F:packages/babel-plugin-strip-runtime/src/index.ts†L21-L102】
- When `styleSheetPath` is configured, synthesise `require()` statements in the transformed AST; when extraction is configured, stage filesystem writes and insert relative CSS imports just like the Babel plugin.【F:packages/babel-plugin-strip-runtime/src/index.ts†L44-L102】
- Expose collected style rules via metadata for SSR usage when runtime injection is skipped.【F:packages/babel-plugin-strip-runtime/src/index.ts†L35-L41】
- Remove `CC`/`CS` imports and inline JSX/createElement children per the strip-runtime logic, ensuring SWC’s AST representation matches Babel’s behaviour.【F:packages/babel-plugin-strip-runtime/src/index.ts†L106-L198】

### Configuration merging
- Define a Rust `Config` struct that unions both option sets. Provide serde support for JSON/YAML loading if needed. Document option precedence (e.g. CSS sorting flags shared between compilation and extraction) to avoid ambiguity.
- Keep defaults identical to the Babel plugins (e.g. `optimizeCss` true, `processXcss` true, `flattenMultipleSelectors` true, `sortAtRules` true, `sortShorthand` true).【F:packages/babel-plugin/src/types.ts†L12-L121】【F:packages/babel-plugin-strip-runtime/src/types.ts†L13-L40】

### Testing strategy
1. **Fixture parity tests:** reuse existing Babel fixtures by running them through `@babel/core` with both Babel plugins and snapshotting the transformed code + style rules. Then run the SWC plugin on the same inputs (via `swc_ecma_transforms_testing`) and assert structural equality after normalising formatting.
2. **Hash determinism tests:** create focused Rust unit tests that feed CSS snippets through the Rust CSS transformer and verify class names match the Babel pipeline (via hard-coded expectations derived from fixtures).【F:packages/css/src/plugins/atomicify-rules.ts†L38-L47】
3. **Resolver tests:** craft scenarios with nested imports and destructuring to ensure `oxc_resolver`-powered evaluation finds static values identical to Babel’s behaviour.【F:packages/babel-plugin/src/utils/resolve-binding.ts†L17-L194】
4. **Runtime stripping tests:** validate that JSX/React outputs match Babel strip-runtime behaviour, including metadata emission and filesystem writes (using temp directories in Rust tests).【F:packages/babel-plugin-strip-runtime/src/index.ts†L21-L198】
5. **Performance benchmarks:** integrate Criterion benchmarks that compare transformation throughput across representative files, tracking allocations with `heaptrack` or `dhat` to guide optimisation.

### Performance considerations
- Use bump-allocated arenas (e.g. `bumpalo`) for transient AST nodes when constructing runtime expressions to minimise heap churn.
- Cache frequently used string literals (e.g. runtime helper identifiers) and leverage SWC’s `JsWord` interning.
- Batch style rule concatenation using `String::with_capacity` based on prior size estimates to avoid repeated reallocations.
- Share resolver instances across files when running in watch mode to avoid re-reading the filesystem unnecessarily, similar to the Babel cache initialisation when `cache: true` is provided.【F:packages/babel-plugin/src/babel-plugin.ts†L71-L118】

## Next steps
1. **Scaffold crates:** set up Cargo workspace entries, add dependencies (`swc_core`, `serde`, `oxc_resolver`, `murmur2` or custom implementation) and wire basic plugin entry that mirrors Babel’s no-op pass.
2. **Port hashing utilities:** implement the murmurhash helper in Rust and add regression tests comparing to the JS version using fixture data.【F:packages/utils/src/hash.ts†L1-L39】
3. **Implement CSS transformer:** port atomicify, sorting, and sheet extraction logic; ensure outputs match existing fixtures.
4. **Build evaluation/resolver layer:** translate `evaluateExpression`/`resolveBinding` features, integrate caching, and verify import traversal with Babel parity tests.
5. **Implement SWC visitors:** replicate styled/css/classNames/cssProp/xcss/cssMap handling, hooking into the Rust CSS transformer and evaluation layer.
6. **Integrate runtime stripping and extraction:** handle metadata, filesystem writes, and AST rewrites for runtime removal.
7. **Parity test harness:** add Rust integration tests that spawn Babel via Node to produce golden outputs and assert SWC equality.
8. **Performance tuning:** profile hot paths, apply allocation optimisations, and document throughput metrics.

