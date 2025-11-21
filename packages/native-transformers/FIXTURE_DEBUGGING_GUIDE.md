# SWC Fixture Debugging Guide

This is the quickest loop to capture a Jira breakage into a fixture, debug it, and guard it against regressions. The goal: go from “file X fails” → minimal fixture → fix → fixture green.

## 1) Reproduce and isolate
- Start from the failing Jira file (e.g. `jira/src/packages/navigation-apps/atlassian-navigation/src/ui/notifications/main.tsx`).
- Rip out a minimal snippet that still fails. Prefer the smallest expression/hook/style that triggers the panic/error.
- If tokens are involved, keep the tokenized form produced by the Babel pre-pass (you can log it in the Jira collector or run the tokens plugin on the snippet).
- Decide fixture name: short, descriptive, kebab-case (e.g. `notifications-call-expression`).

## 2) Create the fixture skeleton
- Under `packages/native-transformers/tests/fixtures/<name>/` add:
  - `in.jsx` or `in.js` containing the minimal repro. Prefer stripping TypeScript syntax to keep the fixture lean unless the bug is TS-specific; only use `.tsx` when JSX is required.
  - `config.json` only if you need non-default options (see `tests/fixtures/README.md` for shape).
- Keep inputs tiny; prune imports and props; inline constants as needed to isolate the behavior.

## 3) Generate baselines
- From `packages/native-transformers/` run:
  - `node scripts/update-fixtures.js --only <name>` to produce:
    - `babel-out.js`, `babel-style-rules.json`
    - `out.js`, `swc-style-rules.json`
  - If generation fails, capture the error output in a temporary note; often the SWC panic stack points to the code path to investigate.

## 4) Compare and diagnose
- Open the four outputs side-by-side; focus on the diff between `babel-style-rules.json` and `swc-style-rules.json`. Ignore style rule ordering differences (we normalize/sort). Codegen differences between `babel-out.js` and `out.js` are less important than style-rule parity—use them only to understand why style rules diverged.
- Common failure categories:
  - Parser/AST handling (e.g. `TsConstAssertion has no name`).
  - Unsupported expressions in style hashing / keyframes serialization.
  - Missing resolver/importSources setup leading to “no Compiled APIs in scope”.
  - CSS parse errors from the PostCSS engine.
- Narrow the fixture further if possible; smaller inputs make debugging faster.

## 5) Fix in code
- Make changes in the relevant SWC plugin code under `packages/native-transformers/<...>` (same folder as the buggy logic). Aim for 1:1 parity with the Babel implementation in the counterpart file/folder, translating the original JS behavior into Rust. If exact parity isn’t possible (e.g. Babel-only API), add a `// COMPAT:` comment explaining the deviation and rationale.
- Add targeted comments if the logic is subtle or differs intentionally from Babel’s behavior.
- Avoid sweeping refactors while hunting a single regression; keep the change tight for reviewability.
- If you change Rust code, rebuild the CLI before rerunning fixtures so the collector sees your changes:
  - From `platform/crates/scompiled`: `cargo build -p fixtures_cli --bin style_rules_cli --release`

## 6) Re-run fixture and verify
- Re-run: `node scripts/update-fixtures.js --only <name>`.
- Confirm the fixture now passes (green output, updated `swc-style-rules.json` matches `babel-style-rules.json` and `out.js` aligns with `babel-out.js`).
- If order-only differences remain, consider normalizing order inside the transformer rather than accepting churn.

## 7) Guard and land
- Leave the new fixture files checked in.
- If you added logging or temporary debug scaffolding, remove it before finalizing.
- Summarize the root cause + fix in the PR description so future regressions are easy to triage.
- Run the full fixture suite before landing to confirm no regressions: `node scripts/update-fixtures.js` (CI will also run it, but catching locally is faster).

## Quick commands
- Update single fixture: `node scripts/update-fixtures.js --only <name>`
- Update all fixtures (slow): `node scripts/update-fixtures.js`
- Fixture layout reference: `tests/fixtures/README.md`

## Tips for speed
- Keep one terminal for running `update-fixtures.js --only <name>` on repeat, another for editing.
- When a panic references a specific util (e.g. `object-property-to-string.rs`), search that file for the panic string to jump to the code path.
- If the issue involves imports/resolvers, craft the fixture with relative imports that mirror the Jira structure to reproduce the same resolver path.
