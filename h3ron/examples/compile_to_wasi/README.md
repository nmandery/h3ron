# Compile to WASI

This mini-tutorial show how to use `h3ron` when compiling to the wasm32-wasi target.

The requirements are:

* Rust with toolchain for wasm32-wasi, Cargo. This tutorial was written using rust 1.63.
* The [wasmtime](https://wasmtime.dev/) runtime.

Install the [WASI-SDK](https://github.com/WebAssembly/wasi-sdk) used to compile the C-sources of libh3:

```
make get-sdk
```

Compile and run:

```
make run
```

For any details please see the [Makefile](./Makefile). The linking stage needs a bit of extra work, which is 
explained there.
