# Native transformer fixtures

Each fixture should include the following files to enable regression comparison between the Babel and SWC implementations:

- `in.jsx` – source file provided to both transformers.
- `out.js` – expected output emitted by the reference Babel transformers.
- `actual.js` – output produced by the native SWC transformer.
- `babel-out.js` – snapshot produced by piping `in.jsx` through `@compiled/babel-plugin` and `@compiled/babel-plugin-strip-runtime`.
- `babel-style-rules.json` – JSON array of style rules emitted by the Babel extract flow.
- `swc-style-rules.json` – JSON array of style rules emitted by the native transformer when `extract` is enabled.

Use the `.template` directory as the canonical layout when adding new fixtures.

Run `yarn workspace @compiled/native-transformers fixtures:update` to regenerate
`babel-out.js`, `babel-style-rules.json`, `out.js`, `actual.js`, and
`swc-style-rules.json` for every fixture. The command currently falls back to the
source file for `actual.js` when the native bindings are unavailable, but will
emit the real SWC output once the Rust transformers are fully implemented.
