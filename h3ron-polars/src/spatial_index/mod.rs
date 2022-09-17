//! Spatial search indexes
//!
//! For some background on spatial search algorithms see [A dive into spatial search algorithms](https://blog.mapbox.com/a-dive-into-spatial-search-algorithms-ebd0c5e39d2a).
//!

#[cfg(feature = "kdbush")]
pub mod kdbush;

#[cfg(feature = "kdbush")]
pub use crate::spatial_index::kdbush::{BuildKDBushIndex, KDBushIndex};
