//! Iterator functionalites
//!
//! Most iterators in this module are implemented as generic functions. This allow to not just use them
//! with the collections in `std::collections`, but also to apply them to custom data structures.
//!
//! # Resolution handling
//!
//! * [`change_cell_resolution`]
//!
//! # Grid traversal
//!
//! * [`KRingBuilder`]
//! * [`neighbors_within_distance_window_or_default`]
//! * [`neighbors_within_distance_window`]
//! * [`neighbors_within_distance`]
//!
//! # Edges
//!
//! * [`H3EdgesBuilder`]
//!

mod edge;
mod kring;
mod neighbor;
mod resolution;

pub use edge::H3EdgesBuilder;
pub use kring::KRingBuilder;
pub use neighbor::*;
pub use resolution::change_cell_resolution;
