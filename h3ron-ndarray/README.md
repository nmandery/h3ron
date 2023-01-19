# h3ron-ndarray

[![Latest Version](https://img.shields.io/crates/v/h3ron-ndarray.svg)](https://crates.io/crates/h3ron-ndarray) [![Documentation](https://docs.rs/h3ron-ndarray/badge.svg)](https://docs.rs/h3ron-ndarray)

Integration with the [ndarray](https://github.com/rust-ndarray/ndarray) crate to generate H3 data from raster data (using [gdal](https://github.com/georust/gdal), ...)

[Changelog](CHANGES.md)

This library is in parts parallelized using [rayon](https://github.com/rayon-rs/rayon). The number of threads can be controlled as
described in [the rayon FAQ](https://github.com/rayon-rs/rayon/blob/master/FAQ.md#how-many-threads-will-rayon-spawn)

## Maintenance status

In january 2023 the [h3o library](https://github.com/HydroniumLabs/h3o) - a port of H3 to rust - has been released. This brings many benefits including type safety, compilation to WASM and performance improvements
(example: [issue comparing raster to h3 conversion](https://github.com/nmandery/rasterh3/issues/1)).

A port of this library using [h3o](https://github.com/HydroniumLabs/h3o) instead of the H3 library exists here: [rasterh3](https://github.com/nmandery/rasterh3). A benchmark comparing these two implementations is available [here](https://github.com/nmandery/rasterh3/issues/1).
