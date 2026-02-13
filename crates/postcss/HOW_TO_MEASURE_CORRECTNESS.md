# How to Measure Correctness

## Definition

**Correctness** means the Rust single-pass plugin (`FirstThreePlugins`) produces
**byte-identical CSS output** to JS PostCSS 8.4.31 running the three plugins
sequentially (`discardDuplicates` → `discardEmptyRules` → `parentOrphanedPseudos`),
for all inputs.

This is a strict definition — not "equivalent" or "semantically the same", but
**identical at the byte level**. This ensures no whitespace, comment, formatting,
or selector differences creep in.

## Quick Start

```bash
cd crates/postcss

# Run the automated correctness suite (39 test cases)
bash bench/verify.sh

# Run Rust unit tests (24 tests including combined interactions)
cargo test plugins::first_three
```

## What the Verification Tool Does

`bench/verify.sh` runs through a comprehensive test corpus:

1. For each test case, writes the CSS input to a temp file
2. Runs the **JS harness** (`node bench/verify_js.js input.css js_output.css`)
   — this uses PostCSS 8.4.31 with the exact 3 plugin implementations
   inlined from `packages/css/src/plugins/`
3. Runs the **Rust binary** (`verify_rust input.css rs_output.css`)
   — this uses the combined `FirstThreePlugins` single-pass plugin
4. Compares outputs with `diff` — any difference = FAIL
5. On failure, shows both outputs and the first diff for debugging

## Test Corpus Categories

The test corpus covers 39 cases across 5 categories:

### discardDuplicates (5 tests)
- No duplicates → unchanged
- Simple duplicate (same prop, different values → keep last)
- Multiple different props with duplicates
- Three occurrences of same prop → keep last only
- Duplicates with `!important`

### discardEmptyRules (8 tests)
- `value: undefined;` → removed
- `value: null;` → removed
- `value: ;` → removed
- `value:    ;` (whitespace-only) → removed
- Empty value inside a rule → decl removed, rule kept if siblings remain
- Rule removed when all children have empty values
- Multiple empty values in same rule → entire rule removed
- Non-empty siblings preserved

### parentOrphanedPseudos (11 tests)
- `:hover` → `&:hover`
- `::before` → `&::before` (pseudo-elements)
- `:first-child &` → `&:first-child &`
- `:hover, :active` → `&:hover, &:active` (multiple selector groups)
- `div, :active` → `div, &:active` (mixed groups)
- `&:hover` → unchanged (already has nesting selector)
- `div > :hover` → unchanged (pseudo not orphaned)
- `[data-look='h100']&` → unchanged
- Nested inside parent rule
- `:nth-child(2n+1)` (functional pseudo)
- `:not(.foo)` (functional pseudo with inner selector)

### Combined interactions (5 tests)
- Dedup → empty: `color: red; color: ;` → empty (dedup keeps `color: ;`, then empty removes it)
- Empty value removes orphaned-pseudo rule
- All three plugins exercised in one stylesheet
- Dedup + pseudo in same stylesheet
- Empty removes one rule, pseudo transforms another

### Edge cases (10 tests)
- Empty input, whitespace only, comment only
- Comments between duplicate declarations
- `@media` containing orphaned pseudos
- `@media` containing rules with empty values
- Deeply nested: `@media > div > :hover`
- `!important` declarations survive untouched
- Multi-line selector separator (`,\n`)
- Complex mixed stylesheet exercising all features

## How to Add a New Test Case

Add a call to `run_test` in `bench/verify.sh`:

```bash
run_test "description of what this tests" \
    'your css input here'
```

The test will automatically run through both JS and Rust and compare outputs.

## Architecture: Why Single-Pass Equals Three-Pass

The JS pipeline runs 3 separate plugins in order:
1. `discardDuplicates.Once(root)` — scans root children, deduplicates by prop
2. `discardEmptyRules.Declaration(node)` — removes empty-value decls and empty parent rules
3. `parentOrphanedPseudos.Once(root)` — walks rules, fixes orphaned pseudo selectors

The Rust `FirstThreePlugins.once()` runs two phases:

**Phase 1**: `discard_duplicates()` — identical to JS step 1. Must run first because
dedup can keep an empty-valued declaration (the last occurrence), which step 2 then removes.
Example: `color: red; color: ;` → dedup keeps `color: ;` → empty-rules removes it.

**Phase 2**: Single depth-first walk combining steps 2 and 3. This is safe because:
- Empty-value removal operates on **declaration values** (`node.value`)
- Orphaned-pseudo fixing operates on **rule selectors** (`rule.selector`)
- These are disjoint properties — no interaction possible

The collection pass in Phase 2 gathers all nodes first, then processes mutations,
avoiding borrow conflicts with the arena-based AST.

## Selector Transformation Equivalence

The JS plugin uses `postcss-selector-parser` with `walkPseudos()` + `insertBefore(nesting)`.
The Rust replacement is a character-level scanner that:

1. Skips selectors not starting with `:` (not orphaned)
2. Scans with state tracking: single/double quotes, `[...]` brackets, `\` escapes
3. At each `:` outside quotes/brackets: inserts `&` before it
4. Handles `::` (pseudo-elements) as a unit: `::before` → `&::before`, not `&:&:before`

This produces identical output because `postcss-selector-parser`'s `walkPseudos()`
visits every Pseudo AST node (including nested ones like `:hover` inside `:not(:hover)`),
and our scanner inserts `&` at every `:` boundary with the same semantics.

The `lossless: false` option in postcss-selector-parser normalizes whitespace, but since
individual selectors from `rule.selectors` are already trimmed by `list.comma()`, this
has no visible effect.

Selector re-joining preserves the original separator (`, ` or `,\n` etc.) by extracting
it from the original selector string with the same `/,[\s]*/` pattern that JS PostCSS's
`rule.selectors` setter uses.
