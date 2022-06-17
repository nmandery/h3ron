//! # Features
//!
//! * **io_gdal**: Writing of graphs to GDAL OGR datasets.
//! * **io_osm**: Enables parsing of OpenStreetMap files.

#![warn(
    clippy::all,
    clippy::correctness,
    clippy::suspicious,
    clippy::style,
    clippy::complexity,
    clippy::perf,
    nonstandard_style
)]

pub mod algorithm;
pub mod error;
pub mod graph;
pub mod io;

pub use crate::error::Error;
