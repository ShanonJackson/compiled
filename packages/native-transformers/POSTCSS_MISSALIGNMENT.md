# PostCSS mismatches to align with Babel

Living notes of places where the Rust PostCSS pipeline still diverges from the JS/Babel reference. These should all be driven to zero by aligning each plugin implementation 1:1 (not by output patches).

## Known divergences

- **normalize-current-color**: Our Rust port was running on every declaration, splitting on spaces and re-stringifying even when no `currentColor` was present. This collapsed significant whitespace in non-color values (e.g. `grid-row: card-extra-fields /  end` -> `grid-row: card-extra-fields / end`), changing the value hash. The JS plugin is a no-op unless the value contains `currentColor`. Fix: short-circuit when the value lacks `currentColor`, so spacing survives into hashing.
- **Grid value spacing visibility**: Instrumentation shows `postcss-ordered-values` emits `card-extra-fields /  end` (matches JS). After `convert-values` (skipped for grid lines) and `colormin` (skipped), the double space remained. The collapse happened later due to `normalize-current-color` (see above). With the guard in place, grid line hashes should now match Babel.
- **Grid-template-areas serialization** (newly visible after the above fix): In fixtures like `cssmap-class-names-as-css`, our values are being split across lines (`"banner"` …) and hashed/serialized differently from Babel’s single-line string. Likely source: a plugin in the mid-pipeline re-parses string literals and reflows whitespace. Needs per-plugin parity check (ordered-values, normalize-string, reduce-initial, etc.) to mirror JS handling of multiline strings.
- **Keyframe whitespace in box-shadow** (e.g. `onboarding-target-keyframes`): Extra spaces remain before closing braces (`…10px rgba(...) )`), indicating a minifier/stringifier discrepancy in the normalize/minify chain for keyframe blocks.
- **Duplicate box-shadow class hash drift** (`spread-duplicates`): Different value hash implies a plugin upstream of hashing is normalizing box-shadow tokens differently (candidate: colormin/minify_gradients/minify_params whitespace).

## Next steps

- Build per-plugin parity harnesses against the vendored JS sources (`packages/postcss-plugin-sources`) so each Rust plugin can be fuzzed with small inputs and compared directly to the JS output.
- Prioritize plugins still touching grid spacing/string literals: normalize-string, reduce-initial, minify-params/whitespace, and any stringifiers that reflow multiline strings.
- Once parity is verified, remove temporary debug hooks and re-run fixtures; remaining mismatches will point to the next non-1:1 plugin to fix.
