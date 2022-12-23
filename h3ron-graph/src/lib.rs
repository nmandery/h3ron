//! # Features
//!
//! * **io_osm**: Enables parsing of OpenStreetMap files.
//! * **io_serde_util**: Convenience serialization helpers.

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
