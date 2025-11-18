# Post Implementation Notes

## `postcss-reduce-initial`
- The stored fixture `packages/native-transformers/tests/stored/fixtures/styled-box-sizing-content-box` diverges because the PostCSS engine port of `postcss-reduce-initial` always rewrites `box-sizing:content-box` to `box-sizing:initial`, while the Babel baseline keeps `content-box`.
- In the original JS cssnano pipeline (`packages/postcss-plugin-sources/postcss-reduce-initial/src/index.js`) the `initialSupport` flag is derived via `browserslist` + `caniuse-api`. With this repo’s default targets (which still include `op_mini all`) that check evaluates to **false**, so Babel never collapses `box-sizing:content-box`.
- The Rust engine version at `packages/native-transformers/compiled_babel/src/postcss/plugins/normalize_css_engine/reduce_initial/mod.rs` hard-codes `initial_support = true`, so it collapses every value listed in `TO_INITIAL`.
- To match Babel the default is now hard-coded to **false** (`packages/native-transformers/compiled_babel/src/postcss/plugins/normalize_css_engine/reduce_initial/mod.rs`). This keeps `box-sizing:content-box` intact until the browserslist/caniuse detection used by the Babel/plugin path is fully ported into the PostCSS engine.
