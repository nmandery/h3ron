//! # Hashing
//!
//! This crate uses `ahash` for its HashMap and HashSets. This hash hash shown in benchmarks to be
//! approx. 10% faster with H3 indexes than the standard SipHash used in rust. On the other hand it shows a higher
//! fluctuation in runtime during benchmarks. Interestingly the normally very fast
//! `rustc_hash` (uses `FxHash`) seems to be very slow with H3 cells and edges. Mostly noticed during
//! deserialization of graphs, but also during using the `pathfinding` crate which uses
//! `rustc_hash` internally. May be related to <https://github.com/rust-lang/rustc-hash/issues/14> and
//! the quadratic insertion cost issue described [here](https://accidentallyquadratic.tumblr.com/post/153545455987/rust-hash-iteration-reinsertion).
//!
//! `hashbrown` is used as it supports some APIs which are still unstable on `std::collections::HashMap`.
//!
pub mod compactedcellvec;
pub mod indexvec;

#[cfg(feature = "use-rayon")]
pub mod partitioned;

pub use ahash::RandomState;
use hashbrown;

pub use compactedcellvec::CompactedCellVec;

#[cfg(feature = "use-rayon")]
pub use partitioned::ThreadPartitionedMap;

use crate::{H3Cell, H3Edge};

pub type HashMap<K, V> = hashbrown::HashMap<K, V, RandomState>;
pub type HashSet<V> = hashbrown::HashSet<V, RandomState>;
pub type H3EdgeMap<V> = HashMap<H3Edge, V>;
pub type H3CellMap<V> = HashMap<H3Cell, V>;
pub type H3CellSet = HashSet<H3Cell>;
