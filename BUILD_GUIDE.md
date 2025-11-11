# Native Transformer Build Guide

This document describes how the JavaScript bridge modules interact with the
Rust native transformers that live in `packages/native-transformers`, and the
steps required to rebuild those bridges whenever the Rust crates change.

## 1. Architecture overview

The native Compiled transformers are implemented in Rust and exposed to Node.js
through lightweight JavaScript entry points:

- `packages/native-transformers/compiled_babel/index.js`
- `packages/native-transformers/compiled_strip_runtime/index.js`

Each entry point:

1. Calls `loadBinding` from `@node-rs/helper` with a module-specific name
   (`compiled_babel` or `compiled_strip_runtime`).
2. `loadBinding` resolves a `.node` shared library under a `native/<target>`
   directory next to the JS file (for example `native/linux-x64-gnu/compiled_babel.node`).
3. The loaded binding exposes the Rust `transform` function, which the JS module
   re-exports along with a `load()` helper that returns the cached binding.
4. Callers invoke `transform(program, config)` exactly like the original
   Babel plugins, and receive an SWC AST plus metadata populated by the Rust
   implementation.

Because the JavaScript files only act as thin loaders, keeping the `.node`
artifacts in sync with the compiled Rust code is essential.

## 2. Directory layout

```
packages/native-transformers/
  compiled_babel/
    index.js        # JS bridge that loads the compiled_babel.node artifact
    src/            # Rust implementation of the transformer
  compiled_strip_runtime/
    index.js        # JS bridge for the strip-runtime transformer
    src/            # Rust implementation
  target/           # Standard Cargo build output (created by `cargo build`)
```

During development you will also create platform-specific folders such as
`packages/native-transformers/compiled_babel/native/linux-x64-gnu/` that hold
the compiled `.node` binary copied from Cargo’s output.

## 3. Building the Rust crates

All builds should use the workspace manifest in
`packages/native-transformers/Cargo.toml`.

1. Ensure you have the required toolchain: Rust (stable), Node.js 20.15.1, Yarn
   1.22.22, and the platform build prerequisites for compiling Rust `cdylib`
   crates (e.g. a C toolchain on Linux/macOS, MSVC on Windows). Install
   `@node-rs/helper@^1.6.0` (used by the bridge to locate the `.node` files)
   and `@swc/core@^1.3.107` (used for parsing during fixture generation). Run
   `yarn install` after bumping the Rust crates to keep those dependencies
   fresh.
2. From the repository root run:

   ```sh
   cargo build --release -p compiled_babel
   cargo build --release -p compiled_strip_runtime
   ```

   The release profile produces optimized `.so` / `.dylib` / `.dll` artifacts in
   `packages/native-transformers/target/release/`.

## 4. Installing the Node bindings

`loadBinding` follows the same lookup rules that `@node-rs/helper` provides for
other napi-rs native modules:
`native/<platform>-<arch>-<libc>/<module>.node`. After building, copy the
compiled library for each transformer into that layout.

Example (Linux, x64, glibc):

```sh
# From packages/native-transformers
mkdir -p compiled_babel/native/linux-x64-gnu
cp target/release/libcompiled_babel.so \
  compiled_babel/native/linux-x64-gnu/compiled_babel.node

mkdir -p compiled_strip_runtime/native/linux-x64-gnu
cp target/release/libcompiled_strip_runtime.so \
  compiled_strip_runtime/native/linux-x64-gnu/compiled_strip_runtime.node
```

macOS and Windows follow the same pattern with their respective filenames and
folder names:

| Platform              | Cargo output              | Target folder                               |
| --------------------- | ------------------------- | ------------------------------------------- |
| macOS (Intel)         | `libcompiled_babel.dylib` | `native/darwin-x64/compiled_babel.node`     |
| macOS (Apple Silicon) | `libcompiled_babel.dylib` | `native/darwin-arm64/compiled_babel.node`   |
| Windows (MSVC)        | `compiled_babel.dll`      | `native/win32-x64-msvc/compiled_babel.node` |

Repeat the same copy step for `compiled_strip_runtime`. When working on multiple
platforms it is common to populate several `native/<target>` directories.

> **Tip:** the JS loader surfaces a helpful error message if it cannot find the
> binding. If you see “Failed to load native binding…” ensure the `.node` file
> exists at the expected path and was copied from a release build.

## 5. Regenerating bridges after Rust changes

Whenever the Rust implementation changes you should:

1. Rebuild the crates with `cargo build --release -p compiled_babel` and
   `cargo build --release -p compiled_strip_runtime`.
2. Copy the fresh artifacts into the `native/<target>/` directories as shown
   above (overwriting any previous `.node` files).
3. Run the regression tests to confirm behaviour:

   ```sh
   cargo test -p compiled_babel --manifest-path packages/native-transformers/Cargo.toml
   cargo test -p compiled_strip_runtime --manifest-path packages/native-transformers/Cargo.toml
   cargo test -p compiled_strip_runtime native_fixtures_match_baselines \
     --manifest-path packages/native-transformers/Cargo.toml
   ```

4. If the change affects emitted code or style rules, refresh the snapshot
   fixtures:

   ```sh
   yarn workspace @compiled/native-transformers fixtures:update
   ```

5. Commit both the updated Rust sources and any regenerated fixture snapshots or
   metadata produced by the native transformers.

## 6. Troubleshooting

- **Bindings not found:** Verify the `.node` files exist, match your platform
  triple, and were copied from a release build. Delete any stale artifacts under
  `native/` and repeat the copy step.
- **ABI mismatch after upgrading Node:** Rebuild the crates and reinstall the
  `.node` files. Different Node runtimes may require recompilation.
- **Fixture diffs after rebuild:** Use the fixture update command to regenerate
  `actual.js`, `out.js`, and the `*-style-rules.json` snapshots, then inspect the
  differences to ensure they are expected behavioural changes rather than
  regressions.

Following these steps keeps the JavaScript bridges and native binaries aligned
so consumers can rely on the Rust implementations without manual intervention.
