#!/bin/sh
# Builds the browser playground into site/try/. Needs the wasm32-unknown-unknown target and wasm-bindgen-cli at
# the wasm-bindgen version in Cargo.lock.
set -e
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir site/try target/wasm32-unknown-unknown/release/mindme.wasm
