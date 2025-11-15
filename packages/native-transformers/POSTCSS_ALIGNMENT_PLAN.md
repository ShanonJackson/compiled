# POSTCSS Alignment Plan

Goal: Achieve a 1:1, bug-for-bug alignment with the Babel/PostCSS pipeline for selector processing and hashing — including whitespace behavior — with no behavioral deviations.

## Objectives

- Replicate `postcss-selector-parser` behavior in Rust so selector whitespace is preserved and controllable per node.
- Port `postcss-minify-selectors` 1:1 on top of the Rust PostCSS engine (not the SWC AST), including combinator spacing, attribute normalization, pseudos, dedupe/sort.
- Ensure selector strings used for hashing (group portion) are the minified selector strings produced by the engine, identical to Babel.
- Ensure emitted CSS selectors in style sheets match Babel stringification (no spaces around child combinators, etc.).

## Non‑Goals

- Changing class name hashing algorithm or value hashing behavior.
- Altering plugin ordering relative to Babel’s pipeline.

## Workstreams

1) Selector AST (engine)
- Add a whitespace‑preserving selector AST to the Rust PostCSS engine (parallel to postcss‑selector‑parser):
  - Nodes: `SelectorList`, `Selector`, `Compound`, `Combinator`, `Tag`, `Class`, `Id`, `Universal`, `Attribute`, `PseudoClass`, `PseudoElement`, `ForgivingSelectorList`.
  - Spacing/raws: per node `spaces.before/after`, `rawSpaceBefore/After`, plus attribute `raws.value`, operator spaces, and value spaces (to exactly mirror JS plugin expectations).
- API: `parse_selector(str) -> EngineSelectorAst`, `serialize_selector(ast, MinifyMode) -> String`.

2) Selector Parser
- Implement a tokenizer that preserves whitespace tokens and escapes; support:
  - Nesting `&`, combinators (`>`, `+`, `~`, descendant), attribute operators (`=`, `~=`, `|=`, `^=`, `$=`, `*=`), pseudos (incl. functional), pseudo‑elements, universal `*`.
  - Forgiving lists for `:is()`, `:where()`, etc.
  - Raw value/quote tracking for attribute values and pseudos (needed by unquoting and conversions).

3) Selector Serializer
- Implement a serializer honoring node spacing fields, with two modes:
  - Pretty: preserves spacing from AST.
  - Minified: enforces postcss‑minify‑selectors layout rules (no spaces around `> + ~`, single space for descendant combinator only when needed, attribute/pseudo spacing rules).

4) Port `postcss-minify-selectors`
- Re‑implement plugin logic 1:1 over the engine AST:
  - Combinator: trim spaces (result matches `value.length ? value : ' '` for descendant, else compact).
  - Attribute: join multi‑line `raws.value`, trim operator, trim `attribute`, set `spaces.*` and `raws.spaces.*` to empty where required, handle `insensitive` spacing.
  - Unquote when safe (mirror `canUnquote` rules).
  - Pseudos: `nth*` → `first/last*` when applicable; `even` → `2n`; `2n+1` → `odd`.
  - Pseudo elements: convert `::before`/`::after`/`::first-letter`/`::first-line` to `:before`/…
  - Universal: remove universal when followed by non‑combinator.
  - Dedupe nested selectors inside pseudos; sort lists for deterministic output.
  - Cache behavior and OnceExit processing semantics as per JS source.

5) Pipeline Integration
- Insert the engine `minify-selectors` in normalize phase before hashing, matching Babel order:
  - Reference: `packages/css/src/transform.ts:36, 72–79`.
- Use engine‑minified selector strings when building atomic class group hash:
  - Replace SWC prelude serialization with engine output in `build_atomic_selector()`.
  - File to update: `packages/native-transformers/compiled_babel/src/postcss/plugins/atomicify-rules.rs:206` (normalize + hash seed composition).

6) Emission Alignment
- Ensure final style sheet emission uses the engine’s minified selector strings to avoid SWC writer spacing differences:
  - Option A (preferred): Serialize selectors with engine and splice into output when serializing each rule in `extract-stylesheets`.
    - File: `packages/native-transformers/compiled_babel/src/postcss/plugins/extract-stylesheets.rs:36`.
  - Option B: Annotate/replace the SWC prelude with a pre‑serialized `SelectorList` string and instruct SWC to emit raw (only if SWC supports raw selector strings; otherwise stick to Option A).

7) Tests & Fixtures
- Unit tests for selector parser/serializer covering:
  - Combinators (`& > *` → `&>*`, descendant spacing).
  - Attributes (operator/value spacing, unquoting, `insensitive`).
  - Pseudos (`nth*` replacements, `even → 2n`, `2n+1 → odd`).
  - Universal removal, pseudo‑element normalization.
- Port JS plugin tests as close as possible using inputs/expected strings.
- Fixture validation: run `packages/native-transformers/scripts/update-fixtures.js`, confirm equality for:
  - `packages/native-transformers/tests/fixtures/css-child-combinator-spacing/babel-style-rules.json:1`
  - `packages/native-transformers/tests/fixtures/css-child-combinator-spacing/swc-style-rules.json:1`

## Integration Points (current code)

- Hashing selector source:
  - `packages/native-transformers/compiled_babel/src/postcss/plugins/atomicify-rules.rs:206`
- Minify‑selectors (current SWC‑based):
  - `packages/native-transformers/compiled_babel/src/postcss/plugins/minify-selectors.rs:176`
- Normalize CSS plugin assembly:
  - `packages/native-transformers/compiled_babel/src/postcss/plugins/normalize-css.rs:9`
- Extract style sheets (final emission):
  - `packages/native-transformers/compiled_babel/src/postcss/plugins/extract-stylesheets.rs:36`

## Risks & Mitigations

- Parser fidelity (escapes, forgiving lists):
  - Mitigation: mirror postcss‑selector‑parser behavior; add cross‑tests from JS sources.
- Performance: selector parsing is small; cache minified selector strings per rule.
- SWC interop: avoid relying on SWC codegen for selector formatting when exact spacing matters; prefer engine serializer for selectors.

## Milestones & Estimate

1) Selector AST + parser skeleton (whitespace‑preserving): 1.5–2.5 days
2) Serializer (pretty + minified): 0.5–1 day
3) Port `postcss-minify-selectors`: 1–2 days
4) Integration into pipeline + hashing: 0.5 day
5) Emission alignment in `extract-stylesheets`: 0.5 day
6) Tests + fixtures green: 1 day

Total: ~4.5–7.5 days

## Acceptance Criteria

- Selector strings used in hashing are byte‑identical to Babel after minification.
- Emitted style‑rules JSON matches Babel fixtures, including no spaces around child combinators and consistent attribute/pseudo formatting.
- No regressions in other selector‑related fixtures.

## Open Questions

- Do we want to fully port other selector‑affecting plugins that depend on postcss‑selector‑parser semantics now, or incrementally (start with minify‑selectors only)?
- Should final sheet serialization always use engine selector strings, or only where Babel asserts specific formatting?

