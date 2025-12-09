node packages/native-transformers/scripts/update-fixtures.js
node packages/native-transformers/scripts/update-fixtures.js double-export

# Verification Commands
- Build fixtures CLI (Rust): `env PATH="$HOME/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH" cargo build --manifest-path packages/native-transformers/Cargo.toml -p fixtures_cli --release`
- Run fixture update: `env PATH="$HOME/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH" node packages/native-transformers/scripts/update-fixtures.js`


```shell
$env:RUSTFLAGS = '-Clto=no'
$env:CARGO_UNSTABLE_EDITION_2024 = '1'
$env:CARGO_PROFILE_RELEASE_LTO = 'off'
$env:COMPILED_CSS_TRACE = '1'
$env:COMPILED_CLI_TRACE = '1'
cargo build --manifest-path packages/native-transformers/Cargo.toml -p fixtures_cli --release
```


env PATH="$HOME/.rustup/toolchains/1.89.0-aarch64-apple-darwin/bin:$HOME/.cargo/bin:$PATH" cargo build --manifest-path packages/native-transformers/Cargo.toml -p fixtures_cli --release
node scripts/update-fixtures.js
node jira/collect-style-rules-swc.js
cd /home/ubuntu/atlassian-frontend-monorepo && node platform/crates/compiled/crates/compiled_swc_plugin/jira/progress-report.js --root jira/tmp/style-rules
