## Vendor Prefixing Alignment Plan

### Why this exists
Our SWC/PostCSS pipeline still diverges from Babel because vendor prefixes are bolted on **after** atomic rules are emitted, while the original JS implementation injects prefixes *inside* the `atomicify-rules` plugin (immediately before the sheets are pushed). As a result:

- Any property/value/selector hacks inserted by the standalone `vendor_autoprefixer` plugin never make it into the final sheets.
- String-level post-processing (`apply_vendor_prefixes_to_sheets`) was required to patch obvious gaps, but it inevitably misses cases.
- Every new difference must be rediscovered fixture‑by‑fixture, which doesn’t scale for large codebases.

To be 1:1 we have to mirror the JS structure, quirks, and data flow exactly.

### Requirements
1. **Single source of prefix data**: use the same `prefixes.json`/`agents.json` loader (`PrefixDB`) for both the SWC path and the PostCSS engine. The selection logic (`select_add_remove`, `build_value_prefix_map`, selector hacks, etc.) must live in `vendor_autoprefixer` so every consumer shares identical behaviour.
2. **Inject prefixes where Babel does**: atomic emission (our `atomicify_rules_plugin` and `extract_stylesheets_plugin`) must receive the autoprefixer data up‑front and emit prefixed declarations/selectors before pushing to `AtomicCollector`, just like Babel’s `atomicify-rules`.
3. **Selector hacks inline**: placeholder, placeholder-shown, etc. have to be cloned inside the emission step so the resulting sheets match Babel rule-for-rule (ignoring order).
4. **No bespoke post-processing**: once the emission step mirrors Babel, the old `apply_vendor_prefixes_to_sheets`-style patches and the standalone `vendor_autoprefixer` plugin should be unnecessary on the PostCSS engine path.
5. **Respect existing structure**: new Rust modules need to remain in the same logical places as their Babel counterparts (e.g. `vendor_autoprefixer/mod.rs`, `postcss/plugins/atomicify-rules.rs`, `postcss/plugins/extract-stylesheets.rs`) so future diffing stays easy.

### Implementation outline
1. **Autoprefixer config object**  
   - Extend `vendor_autoprefixer::AutoprefixerData` to expose property/value/selector helpers (`prefixed_decls`, `placeholder_selector_variants`, etc.).  
   - Load it once in `build_processor`, guarded by `AUTOPREFIXER` env var, and share via `Arc`.

2. **Atomic emission changes**  
   - Thread the optional `Arc<AutoprefixerData>` into `atomicify_rules_plugin` and `extract_stylesheets_plugin`.  
   - When emitting each atomic declaration:
     - Build the full set of declaration clones (prefixed properties, intrinsic keyword rewrites, `display:flex` hacks) using `AutoprefixerData`.  
     - Emit selector variants (placeholder, etc.) before hashing so every clone becomes its own sheet, mirroring Babel’s `atomicify-rules`.
   - Remove the ad-hoc `-moz-fit-content` injection and any other manual prefix hacks: they should now come from the data-driven helpers.

3. **Plugin clean-up**  
   - Once the emission step outputs prefixed declarations, drop the standalone `vendor_autoprefixer_plugin` (or keep it disabled for PostCSS engine builds) to avoid double work.  
   - Ensure the SWC pipeline (which already uses the Rust `vendor_autoprefixer` module) continues to share the same helper functions.

4. **Fixture validation**  
   - Re-run the failing fixtures (`css-vendor-prefix`, `cssmap-*`, `lozenge-*`, `styled-placeholder-token`) and compare the sheets ignoring order.
   - Remove any leftover post-processing or temporary tracing once parity is confirmed.

### Success criteria
- Every autoprefixer-enabled fixture emits the same rule set as Babel (order aside).  
- No string-level or sheet-level patching remains.  
- The prefixing logic is centrally defined in `vendor_autoprefixer`, and both pipelines consume it at the same logical stage as the JS implementation.
