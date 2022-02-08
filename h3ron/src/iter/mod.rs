//! Iterator functionalities
//!
//! Most iterators in this module are implemented as generic functions. This allow to not just use them
//! with the collections in `std::collections`, but also to apply them to custom data structures.
//!
//! # Resolution handling
//!
//! * [`change_resolution`]
//! * [`change_resolution_tuple`]
//!
//! # Grid traversal
//!
//! * [`GridDiskBuilder`]
//! * [`neighbors_within_distance_window_or_default`]
//! * [`neighbors_within_distance_window`]
//! * [`neighbors_within_distance`]
//!
//! # Edges
//!
//! * [`H3DirectedEdgesBuilder`]
//! * [`continuous_cells_to_edges`]
//!
//! # Cell boundaries
//!
//! * [`CellBoundaryBuilder`]
//! * [`CellBoundaryIter`]
//!

pub use boundary::{CellBoundaryBuilder, CellBoundaryIter};
pub use edge::{continuous_cells_to_edges, CellsToEdgesIter, H3DirectedEdgesBuilder};
pub use grid_disk::GridDiskBuilder;
pub use neighbor::*;
pub use resolution::{change_resolution, change_resolution_tuple};

mod boundary;
mod edge;
mod grid_disk;
mod neighbor;
mod resolution;
