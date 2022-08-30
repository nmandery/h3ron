
# https://doc.rust-lang.org/nightly/nightly-rustc/rustc_target/spec/wasm32_wasi/index.html
export WASI_SDK_PATH:=$(CURDIR)/wasi-sdk
export CC_wasm32-wasi := $(WASI_SDK_PATH)/bin/clang
export CARGO_TARGET_WASM32_WASI_LINKER := $(WASI_SDK_PATH)/bin/clang

# 128 bit floats are not supported by rust. See https://github.com/rust-lang/compiler-builtins .
# This manifests in errors like:
#
# ```
# wasmtime ../../../target/wasm32-wasi/debug/compile_to_wasi.wasm
# Error: failed to run main module `../../../target/wasm32-wasi/debug/compile_to_wasi.wasm`
#
# Caused by:
#     0: failed to instantiate "../../../target/wasm32-wasi/debug/compile_to_wasi.wasm"
#     1: unknown import: `env::__extenddftf2` has not been defined
# ```
#
# To resolve this we link to the runtime library included in the SDK:
export RUSTFLAGS:=-C target-feature=-crt-static -C link-arg=-L$(WASI_SDK_PATH)/lib/clang/14.0.4/lib/wasi -C link-arg=-lclang_rt.builtins-wasm32



get-sdk:
	wget https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-16/wasi-sdk-16.0-linux.tar.gz
	tar xf wasi-sdk-16.0-linux.tar.gz
	rm wasi-sdk-16.0-linux.tar.gz
	mv wasi-sdk-16.0 $(WASI_SDK_PATH)

run:
	#cargo wasi run
	cargo build --target wasm32-wasi --release
	wasmtime ../../../target/wasm32-wasi/release/compile_to_wasi.wasm

clean:
	rm -rf $(WASI_SDK_PATH)
	cargo clean
