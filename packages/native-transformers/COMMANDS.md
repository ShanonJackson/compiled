node packages/native-transformers/scripts/update-fixtures.js
node packages/native-transformers/scripts/update-fixtures.js double-export

$env:RUSTFLAGS = '-Clto=no'
>> $env:CARGO_UNSTABLE_EDITION_2024 = '1'
>> $env:COMPILED_USE_POSTCSS = '1'
>> $env:CARGO_PROFILE_RELEASE_LTO = 'off'
>> cargo build --manifest-path packages/native-transformers/Cargo.toml -p fixtures_cli --release
