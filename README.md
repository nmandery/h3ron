# h3ron

![ci](https://github.com/nmandery/h3ron/workflows/CI/badge.svg)

[Rust](https://rustlang.org) library for the [H3](https://h3geo.org) geospatial indexing system.

## Crates

This repository consists of multiple crates:

### [h3ron](./h3ron) [![Latest Version](https://img.shields.io/crates/v/h3ron.svg)](https://crates.io/crates/h3ron) [![Documentation](https://docs.rs/h3ron/badge.svg)](https://docs.rs/h3ron)

High-level rust API for H3.

[Documentation](https://docs.rs/h3ron) | [Changelog](h3ron/CHANGES.md)

### [h3ron-h3-sys](./h3ron-h3-sys) [![Latest Version](https://img.shields.io/crates/v/h3ron-h3-sys.svg)](https://crates.io/crates/h3ron-h3-sys) [![Documentation](https://docs.rs/h3ron-h3-sys/badge.svg)](https://docs.rs/h3ron-h3-sys)

bindgen-generated bindings for the statically linked libh3 C library.

[Documentation](https://docs.rs/h3ron-h3-sys) | [Changelog](h3ron-h3-sys/CHANGES.md)

### [h3ron-ndarray](h3ron-ndarray) [![Latest Version](https://img.shields.io/crates/v/h3ron-ndarray.svg)](https://crates.io/crates/h3ron-ndarray) [![Documentation](https://docs.rs/h3ron-ndarray/badge.svg)](https://docs.rs/h3ron-ndarray)

Integration with the [ndarray](https://github.com/rust-ndarray/ndarray) crate to generate H3 cells from raster data (using [gdal](https://github.com/georust/gdal), ...)

[Documentation](https://docs.rs/h3ron-ndarray) | [Changelog](h3ron-ndarray/CHANGES.md)

### [h3ron-graph](h3ron-graph) [![Latest Version](https://img.shields.io/crates/v/h3ron-graph.svg)](https://crates.io/crates/h3ron-graph) [![Documentation](https://docs.rs/h3ron-graph/badge.svg)](https://docs.rs/h3ron-graph)

Graph algorithms on edges of the H3 spatial indexing system.

[Documentation](https://docs.rs/h3ron-graph) | [Changelog](h3ron-graph/CHANGES.md)

## Python bindings

Python bindings for parts of the functionalities are available in the `h3ronpy` extension now located in an [own repository](https://github.com/nmandery/h3ronpy).
For an overview of some features complementary to libh3 please see the README of the python bindings.


## Why this name?

Well, coming up with a good name for a project while avoiding naming conflicts is hard. On the other hand are animal-based names always pretty easy to remember.

How to pronounce it? I got no idea - probably like the [heron bird family](https://en.wikipedia.org/wiki/Heron).

## License

[MIT](./LICENSE-MIT)

This repository contains some (modified) parts of the excellent [`pathfinding` crate](https://github.com/samueltardieu/pathfinding).

Some data in the `data` directory is derived from OpenStreetMap and as such is copyright by the OpenStreetMap contributors. For
the OSM license see [OSMs Copyright and License page](https://www.openstreetmap.org/copyright).
