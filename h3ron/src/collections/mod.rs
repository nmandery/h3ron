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
use std::hash::Hash;

pub use ahash::RandomState;
use hashbrown;

pub use compactedcellvec::CompactedCellVec;
#[cfg(feature = "use-rayon")]
pub use partitioned::ThreadPartitionedMap;
#[cfg(feature = "use-roaring")]
pub use treemap::H3Treemap;

use crate::{H3Cell, H3Edge, Index};

pub mod compactedcellvec;
pub mod indexvec;

#[cfg(feature = "io")]
pub mod compressed;
#[cfg(feature = "use-rayon")]
pub mod partitioned;
#[cfg(feature = "use-roaring")]
pub mod treemap;

/// generic trait to check if a index is contained in a collection
pub trait ContainsIndex<I: Index> {
    /// check if the given [`Index`] is contained
    fn contains_index(&self, index: &I) -> bool;
}

pub type HashMap<K, V> = hashbrown::HashMap<K, V, RandomState>;
pub type HashSet<V> = hashbrown::HashSet<V, RandomState>;
pub type H3EdgeMap<V> = HashMap<H3Edge, V>;
pub type H3CellMap<V> = HashMap<H3Cell, V>;
pub type H3CellSet = HashSet<H3Cell>;

impl<I: Index + Eq + Hash> ContainsIndex<I> for HashSet<I> {
    fn contains_index(&self, index: &I) -> bool {
        self.contains(index)
    }
}

impl<I: Index + Eq + Hash, V> ContainsIndex<I> for HashMap<I, V> {
    fn contains_index(&self, index: &I) -> bool {
        self.contains_key(index)
    }
}
