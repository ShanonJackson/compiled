Hash Input Differences Audit

This note documents why the hashes emitted by the Rust rewrite differ from the original JS pipeline. The hashing function itself is equivalent; the inputs going into the hash differ at a few points. Keeping the inputs byte‑for‑byte identical will restore 1:1 hashes.

Summary

- Selector string used in the group hash differs.
  - JS source: `packages/css/src/plugins/atomicify-rules.ts:41,44`
    - Group seed: `${prefix}${opts.atRule}${selectors}${node.prop}`
    - `normalizeSelector()` only trims and prepends `& ` when missing; otherwise returns the trimmed selector unchanged.
  - Rust source: `packages/native-transformers/compiled_babel/src/postcss/plugins/atomicify-rules.rs:261,363`
    - Re‑serializes selectors via SWC codegen (minify: false), which can change spacing compared to PostCSS’ string.
    - Additional normalization collapses `& :pseudo` to `&:pseudo` and, when both leading and trailing nesting are detected, appends a trailing ` &`. JS never appends this.

- At‑rule params string used in the group hash differs.
  - JS source: `packages/css/src/plugins/atomicify-rules.ts:41`, with params normalized earlier by `postcss-minify-params` (`packages/css/src/plugins/normalize-css.ts`).
  - Rust source: `packages/native-transformers/compiled_babel/src/postcss/plugins/atomicify-rules.rs:402` and `sort-atomic-style-sheet.rs:164`
    - Re‑serializes the at‑rule prelude via SWC codegen (minify: true). If this minification differs from cssnano’s, identical queries can produce different text, changing both grouping (merge/sort) and the group hash seed.

- Declaration value string used in the value hash differs.
  - JS source: `packages/css/src/plugins/atomicify-rules.ts:46`
    - Hashes `node.value` after cssnano normalization (e.g., convert‑values, colormin) so the textual value is canonicalized and compact.
  - Rust source: `packages/native-transformers/compiled_babel/src/postcss/plugins/atomicify-rules.rs:389`
    - Re‑serializes values via SWC codegen (minify: false). This can introduce spacing or formatting differences from cssnano’s output, altering the value hash input.

Pipeline Ordering (context)

- JS: `packages/css/src/transform.ts:30`
  - Runs `normalizeCSS()` (includes `postcss-minify-selectors` and `postcss-minify-params` always, plus additional value/colour normalization in prod) before `atomicifyRules()`. Atomic hashing uses the post‑normalized strings.
- Rust: `packages/native-transformers/compiled_babel/src/postcss/transform.rs`
  - Also runs a normalization stage before `atomicify_rules()`, but current normalizers rely on SWC re‑serialization, not PostCSS’ exact string forms, leading to textual deltas.

Impact on Hashes

- Group hash mismatches stem from selector and at‑rule param text differences.
- Value hash mismatches stem from declaration value text differences.
- The underlying hash algorithm is a faithful port (see `packages/native-transformers/compiled_babel/src/utils/hash.rs`).

At‑Rules “not grouped” observation

- In JS, at‑rule grouping/merging relies on cssnano‑normalized prelude text; identical queries collapse together and sort deterministically.
  - JS references: `packages/css/src/transform.ts` (normalize then atomicify, then `sortAtomicStyleSheet`).
- In Rust, at‑rule grouping goes through `merge-duplicate-at-rules` and `sort-atomic-style-sheet`:
  - `packages/native-transformers/compiled_babel/src/postcss/plugins/merge-duplicate-at-rules.rs`
  - `packages/native-transformers/compiled_babel/src/postcss/plugins/sort-atomic-style-sheet.rs`
  - Both depend on string keys built from re‑serialized at‑rule preludes (`minify: true`). If SWC’s serialization differs from cssnano’s canonical text, otherwise‑identical queries won’t match keys and thus won’t merge/group. This also means the at‑rule text fed into the group hash differs.

What to align (no code changes made here)

- Selector string fed into the group hash should be the exact post‑normalized selector text JS produces. Avoid additional normalization inside `atomicify` beyond what JS `normalizeSelector()` does.
- At‑rule params used for both merging/sorting and hashing should match cssnano’s `postcss-minify-params` output byte‑for‑byte.
- Declaration value string fed into the value hash should match the cssnano‑normalized text (convert‑values, colormin, etc.). Avoid using a fresh SWC pretty print for the hash input.

Concrete references

- JS hashing and normalization
  - `packages/css/src/plugins/atomicify-rules.ts:41,44,46`
  - `packages/css/src/transform.ts:30`
- Rust hashing inputs and serialization points
  - `packages/native-transformers/compiled_babel/src/postcss/plugins/atomicify-rules.rs:261,363,389,402`
  - `packages/native-transformers/compiled_babel/src/postcss/plugins/merge-duplicate-at-rules.rs`
  - `packages/native-transformers/compiled_babel/src/postcss/plugins/sort-atomic-style-sheet.rs`

Takeaway

The hash function is correct; the byte strings being hashed are not yet identical to the originals due to differences in how selectors, at‑rule params, and values are normalized/serialized. Align those inputs with the JS pipeline’s post‑cssnano text and both the hashes and at‑rule grouping will match 1:1.

