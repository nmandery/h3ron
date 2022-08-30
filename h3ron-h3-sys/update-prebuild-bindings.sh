#!/bin/bash

cargo clean
cargo build --release --features "bindgen"
cp `find  ../target/ -name 'bindings.rs'` src/prebuild_bindings.rs
