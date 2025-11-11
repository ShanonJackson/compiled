# Native transformer fixtures

Each fixture includes the following files for regression comparison between the Babel and SWC implementations:

- `in.jsx` – source input provided to both transformers.
- `babel-out.js` – output produced by `@compiled/babel-plugin` (+ strip-runtime where applicable).
- `out.js` – output produced by the native SWC transformers (canonical output for tests).
- `babel-style-rules.json` – JSON array of style rules emitted by the Babel extract flow.
- `swc-style-rules.json` – JSON array of style rules emitted by the native transformer when `extract` is enabled.

Use the `.template` directory as the canonical layout when adding new fixtures.

Run `yarn workspace @compiled/native-transformers fixtures:update` to regenerate
`babel-out.js`, `babel-style-rules.json`, `out.js`, and `swc-style-rules.json` for
every fixture. If the native transformers are unavailable, the script falls back
to Babel-derived metadata and keeps outputs formatted consistently.
