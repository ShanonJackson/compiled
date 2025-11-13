#!/usr/bin/env bash
set -euo pipefail
export RUSTFLAGS='-Clto=no'
cargo check -p postcss
