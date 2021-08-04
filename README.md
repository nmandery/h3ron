# h3ron

![ci](https://github.com/nmandery/h3ron/workflows/CI/badge.svg)

[Rust](https://rustlang.org) library for the [H3](https://h3geo.org) geospatial indexing system including python bindings.


## [h3ronpy](./h3ronpy) Python extension [![PyPI version](https://img.shields.io/pypi/v/h3ronpy)](https://pypi.python.org/pypi/h3ronpy/)

Python extension for the [H3](https://h3geo.org) geospatial indexing system exposing some functionalities of the `h3ron*` rust crates to the python language and integrating with the [numpy](https://numpy.org/), [pandas](https://pandas.pydata.org/),
[geopandas](https://geopandas.org/), [rasterio](https://rasterio.readthedocs.io/en/latest/) and [gdal](https://gdal.org/) libraries.


## Rust crates

This repository consists of multiple crates:

### [h3ron-h3-sys](./h3ron-h3-sys) [![Latest Version](https://img.shields.io/crates/v/h3ron-h3-sys.svg)](https://crates.io/crates/h3ron-h3-sys) [![Documentation](https://docs.rs/h3ron-h3-sys/badge.svg)](https://docs.rs/h3ron-h3-sys)

bindgen-generated bindings for the statically linked libh3 C library.

[Documentation](https://docs.rs/h3ron-h3-sys)


### [h3ron](./h3ron) [![Latest Version](https://img.shields.io/crates/v/h3ron.svg)](https://crates.io/crates/h3ron) [![Documentation](https://docs.rs/h3ron/badge.svg)](https://docs.rs/h3ron)

High-level rust API for H3.

[Documentation](https://docs.rs/h3ron)

### [h3ron-ndarray](h3ron-ndarray) [![Latest Version](https://img.shields.io/crates/v/h3ron-ndarray.svg)](https://crates.io/crates/h3ron-ndarray) [![Documentation](https://docs.rs/h3ron-ndarray/badge.svg)](https://docs.rs/h3ron-ndarray)

Integration with the [ndarray](https://github.com/rust-ndarray/ndarray) crate to generate H3 cells from raster data (using [gdal](https://github.com/georust/gdal), ...)

[Documentation](https://docs.rs/h3ron-ndarray)

For an overview of some features complementary to libh3 please see the [README](h3ronpy/README.md) of the python bindings.


## Why this name?

Well, coming up with a good name for a project while avoiding naming conflicts is hard. On the other hand are animal-based names always pretty easy to remember.

How to pronounce it? I got no idea - probably like the [heron bird family](https://en.wikipedia.org/wiki/Heron).

## License

[MIT](./LICENSE-MIT)
