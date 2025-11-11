Build native bindings for @compiled/native-transformers

Prerequisites
- Rust toolchain (stable), Cargo available in PATH
- Node 20.15.1, Yarn 1.22.22
- Pin @swc/core to 1.11.18 at repo root (resolutions) and install: `yarn install`

Build (from packages/native-transformers)
- cargo build -p compiled_babel -p compiled_strip_runtime --release

Install bindings for Node loader
- Create platform subdirs under each package’s native folder and copy the built library as a .node file.
- Windows (x64, MSVC):
  - mkdir compiled_babel/native/win32-x64-msvc
  - mkdir compiled_strip_runtime/native/win32-x64-msvc
  - copy target/release/compiled_babel.dll compiled_babel/native/win32-x64-msvc/compiled_babel.node
  - copy target/release/compiled_strip_runtime.dll compiled_strip_runtime/native/win32-x64-msvc/compiled_strip_runtime.node
- macOS (arm64):
  - mkdir -p compiled_babel/native/darwin-arm64
  - mkdir -p compiled_strip_runtime/native/darwin-arm64
  - cp target/release/libcompiled_babel.dylib compiled_babel/native/darwin-arm64/compiled_babel.node
  - cp target/release/libcompiled_strip_runtime.dylib compiled_strip_runtime/native/darwin-arm64/compiled_strip_runtime.node
- Linux (x64, glibc):
  - mkdir -p compiled_babel/native/linux-x64-gnu
  - mkdir -p compiled_strip_runtime/native/linux-x64-gnu
  - cp target/release/libcompiled_babel.so compiled_babel/native/linux-x64-gnu/compiled_babel.node
  - cp target/release/libcompiled_strip_runtime.so compiled_strip_runtime/native/linux-x64-gnu/compiled_strip_runtime.node

Verify
- node scripts/update-fixtures.js
- actual.js should match babel-out.js and swc-style-rules.json should match babel-style-rules.json across fixtures.
