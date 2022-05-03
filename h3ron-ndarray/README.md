# h3ron-ndarray

[![Latest Version](https://img.shields.io/crates/v/h3ron-ndarray.svg)](https://crates.io/crates/h3ron-ndarray) [![Documentation](https://docs.rs/h3ron-ndarray/badge.svg)](https://docs.rs/h3ron-ndarray)

Integration with the [ndarray](https://github.com/rust-ndarray/ndarray) crate to generate H3 data from raster data (using [gdal](https://github.com/georust/gdal), ...)

[Changelog](CHANGES.md)

This library is in parts parallelized using [rayon](https://github.com/rayon-rs/rayon). The number of threads can be controlled as
described in [the rayon FAQ](https://github.com/rayon-rs/rayon/blob/master/FAQ.md#how-many-threads-will-rayon-spawn)
