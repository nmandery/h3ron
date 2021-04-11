# h3ron

[rust](https://rustlang.org) library for the [H3](https://h3geo.org) geospatial indexing system.

For an overview of some features complementary to libh3 please see the [documentation notebook](h3ronpy/documentation.ipynb) of the python bindings.

This repository consists of multiple crates:

## [h3ron-h3-sys](./h3ron-h3-sys) [![Latest Version](https://img.shields.io/crates/v/h3ron-h3-sys.svg)](https://crates.io/crates/h3ron-h3-sys)

bindgen-generated bindings for the statically linked libh3 C library.

[Documentation](https://docs.rs/h3ron-h3-sys)


## [h3ron](./h3ron) [![Latest Version](https://img.shields.io/crates/v/h3ron.svg)](https://crates.io/crates/h3ron)

high level rust API including collections for selected parts of H3.

[Documentation](https://docs.rs/h3ron)

## [h3ron-ndarray](h3ron-ndarray) [![Latest Version](https://img.shields.io/crates/v/h3ron-ndarray.svg)](https://crates.io/crates/h3ron-ndarray)

Integration with the [ndarray](https://github.com/rust-ndarray/ndarray) crate to generate H3 data from raster data (using [gdal](https://github.com/georust/gdal), ...)

[Documentation](https://docs.rs/h3ron-ndarray)

## [h3ronpy](./h3ronpy) [![PyPI version](https://img.shields.io/pypi/v/h3ronpy)](https://pypi.python.org/pypi/h3ronpy/)

Python bindings for h3ron, build using [pyo3](https://github.com/PyO3/PyO3).

## Why this name?

Well, coming up with a good name for a project while avoiding naming conflicts is hard. On the other hand are animal-based names always pretty easy to remember.

How to pronounce it? I got no idea - probably like the [heron bird family](https://en.wikipedia.org/wiki/Heron).

## License

[MIT](./LICENSE-MIT)
