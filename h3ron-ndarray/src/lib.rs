//! # h3ron-ndarray
//!
//! Integration with the [ndarray](https://github.com/rust-ndarray/ndarray) crate to generate H3 data from raster data (using [gdal](https://github.com/georust/gdal), ...)
//!
//! This library is in parts parallelized using [rayon](https://github.com/rayon-rs/rayon). The number of threads can be controlled as
//! described in [the rayon FAQ](https://github.com/rayon-rs/rayon/blob/master/FAQ.md#how-many-threads-will-rayon-spawn)
//!

#![warn(
    clippy::all,
    clippy::correctness,
    clippy::suspicious,
    clippy::style,
    clippy::complexity,
    clippy::perf,
    clippy::nursery,
    nonstandard_style
)]

#[cfg(test)]
#[macro_use]
extern crate approx;
#[macro_use]
extern crate ndarray;

pub mod array;
pub mod error;
pub mod resolution;
mod sphere;
pub mod transform;

pub use crate::array::{AxisOrder, H3Converter};
pub use crate::error::Error;
pub use crate::resolution::ResolutionSearchMode;
pub use crate::transform::Transform;
