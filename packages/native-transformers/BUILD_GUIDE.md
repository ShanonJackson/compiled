Native Transformers Build & Fixtures Guide

Prerequisites
- Rust (stable) and a C toolchain (MSVC on Windows)
- Node 20.15.1, Yarn 1.22.22
- Install JS deps at repo root for Babel baselines: `yarn install`

Build Native Crates
- From `packages/native-transformers`:
  - `cargo build --release -p compiled_babel -p compiled_strip_runtime`

Build Fixtures CLI (preferred for SWC outputs)
- Still in `packages/native-transformers`:
  - `cargo build --release -p fixtures_cli`
- The script auto-builds this binary if missing.

Regenerate Fixtures
- Command: `node packages/native-transformers/scripts/update-fixtures.js`
- What happens:
  - Babel baselines are generated with `@compiled/babel-plugin` + `@compiled/babel-plugin-strip-runtime`.
  - SWC baselines are generated via the native CLI: `target/release/fixtures_cli[.exe]`.
  - Files per fixture directory are updated: `babel-out.js`, `out.js`, `actual.js`, `babel-style-rules.json`, `swc-style-rules.json`.

Notes
- JS bridges in `compiled_babel/index.js` and `compiled_strip_runtime/index.js` are not required for fixtures; they remain for optional SWC loader parity and will attempt `@swc/core/node` when used directly.
- If your environment fails linking due to LTO, you can set `RUSTFLAGS="-Clto=no"` for builds.

