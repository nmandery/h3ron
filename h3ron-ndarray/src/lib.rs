//! # h3ron-ndarray
//!
//! Integration with the [ndarray](https://github.com/rust-ndarray/ndarray) crate to generate H3 data from raster data (using [gdal](https://github.com/georust/gdal), ...)
//!
//! This library is in parts parallelized using [rayon](https://github.com/rayon-rs/rayon). The number of threads can be controlled as
//! described in [the rayon FAQ](https://github.com/rayon-rs/rayon/blob/master/FAQ.md#how-many-threads-will-rayon-spawn)
//!
#[cfg(test)]
#[macro_use]
extern crate approx;
#[macro_use]
extern crate ndarray;

pub mod array;
pub mod transform;
pub mod error;
mod sphere;
pub mod resolution;

pub use crate::resolution::ResolutionSearchMode;
pub use crate::array::{
    H3Converter,
    AxisOrder
};
pub use crate::transform::Transform;
pub use crate::error::Error;