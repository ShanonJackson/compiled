# Class Name And Style-Rules Ordering — Alignment Plan

This document captures what we observed about Babel’s ordering, how our current native pipeline behaves, and the minimal, non‑deviating change to align our output 1:1 with Babel.

## What Babel Does (Observed)

- Pipeline stages inside `@compiled/css` (`transformCss`):
  - expand-shorthands → atomicify-rules → flatten-multiple-selectors → discard-duplicates → (increase-specificity) → sort-atomic-style-sheet → normalize-whitespace → extract-style-sheets
  - The atomicify plugin invokes a callback (classNames accumulator). This reports encounter order of atomic rules as they’re built.
  - The final “source of truth” for ordering is the list of sheets emitted by `extract-style-sheets` after sorting has happened. In our fixture logs:
    - Atomic encounter order for the main block: `display → font-size → gap → padding-top → padding-right → padding-bottom → padding-left → border → background-color → color`
    - Final extracted sheets order: `gap → padding-top → padding-right → padding-bottom → padding-left → border → display → font-size → background-color → color`
  - In declaration-only inputs (no top-level rules), `sort-atomic-style-sheet` operates over collected declarations (catch‑all). Even though there are no top-level `Rule` nodes, shorthand ordering is still applied at this stage and is reflected in the extraction order.
  - Downstream (Babel plugin `build-styled-component.ts`) uses the `classNames` coming back from `transformCss`. In practice these reflect the extracted sheet order (post sort), not the raw atomic encounter order.

## Our Current Native Behaviour

- We previously implemented a builder‑side sort (in Rust) to try to match Babel’s order. This is a deviation from where Babel derives ordering and was removed.
- Our native PostCSS engine produces `sheets` in the correct (final) order. However, the `class_names` we return from the transform currently reflect the raw encounter order (pre sort/extraction) and therefore can differ from the final sheet sequence.
- This mismatch shows up as a code‑only difference in JSX `className` ordering (e.g. `border` appearing before `padding-*`), while `style-rules` (sheets) themselves are already ordered like Babel.

## Minimal Change To Align (No Deviations)

Target: make our transform return `class_names` in the same order as the final extracted `sheets`, just like Babel’s `transformCss` outcome — without touching sheet formatting or engine stages (so hashes remain identical).

Implementation:

1) In `packages/native-transformers/compiled_babel/src/postcss/transform.rs` (the PostCSS-backed transform):
   - After building `TransformCssResult { sheets, class_names }` (or right before returning it), reorder `class_names` to the order implied by `sheets`.
   - Use a small helper to parse the first class selector from each sheet (e.g. scan for the first `.` then read until `{` or whitespace/comma).
   - Build a mapping `class → first index in sheets` and sort `class_names` by this index; preserve encounter order as a tiebreaker; append any classes not found in sheets at the end (stable).
   - Do not change sheet contents or serialization — this keeps hash parity intact.

2) Do not sort in the builder (`build-styled-component.rs` / `transform-css-items.rs`). These should only consume the `class_names` returned from the transform, possibly applying compression, mirroring Babel’s plugin behaviour.

Why this is correct:

- The logs show Babel’s final `classNames` effectively follow extraction order, not raw atomic encounter order.
- Reordering `class_names` by `sheets` at the transform boundary reproduces this exactly and is done at the same phase as Babel returns it.
- No formatting/whitespace in `sheets` is changed; thus `style-rules` and their hashes remain identical.

## Longer-Term Exactness (Future Work)

For a stricter 1:1 structure (if/when we want to mirror internals, not only outputs):

- Make the engine mutate AST during atomicify (insert atomic rules) rather than emitting sheet strings at that stage.
- Run `sort-atomic-style-sheet` over the AST (including shorthand declaration sorting and pseudo ordering for top-level rules and under at-rules).
- Extract sheets after sorting by walking the AST; return both `sheets` and `class_names` (the latter rebuilt from the final tree order when needed).

This is functionally equivalent to the minimal change above but places behaviour in the identical stages as Babel.

## Validation Plan

1) Add tracing (temporary) in Babel to confirm:
   - Atomic encounters: `[css] atomic encounter: _…`
   - Sorting checkpoints in `sort-atomic-style-sheet`: collected/after-shorthand/after-pseudo/final
   - Extraction order: `[css] extract sheet: ._….{…}`
   - JSX classNames in `build-styled-component`: `[babel-plugin] jsx classNames: […]`

2) Implement the `class_names` reorder in our `transform.rs` and re-run fixtures:
   - Expect JSX `className` strings to match Babel (e.g. `gap → padding-* → border → …`).
   - Expect `style-rules` content and hashes to remain identical (no formatting changes).

3) Remove all builder-side sorting; it is a deviation. The builder should only compress and pass through the order returned by the transform.

## Notes

- Declaration‑only CSS (no selectors) is common for styled blocks; in this case, top-level `Rule` sorting isn’t visible, but shorthand sorting still occurs and determines extraction order. The observed logs in fixtures corroborate this.
- If a class appears in `class_names` but not in any final `sheets` (rare, e.g. dynamic paths), preserve its original index after all sheet‑mapped classes.

