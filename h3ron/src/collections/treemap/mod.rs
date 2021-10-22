use std::borrow::Borrow;
use std::iter::FromIterator;
use std::marker::PhantomData;

use roaring::RoaringTreemap;

use crate::collections::ContainsIndex;
use crate::Index;

#[cfg(feature = "use-serde")]
pub mod serde;

/// wrapper around [`roaring::RoaringTreemap`] to store h3 data.
///
/// The implementation of `roaring::RoaringTreemap` splits `u64` into two
/// `u32`. The first is used as the key for a `BTreeMap`, the second is used
/// in the bitmap value of that map. Due to the structure of h3 indexes, relevant parts
/// are only stored in the bitmap starting with approx h3 resolution 5. Below that it
/// makes little sense to use this `H3Treemap`.
#[derive(Clone)]
pub struct H3Treemap<T> {
    treemap: RoaringTreemap,
    phantom_data: PhantomData<T>,
}

impl<T, Q> FromIterator<Q> for H3Treemap<T>
where
    Q: Borrow<T>,
    T: Index,
{
    fn from_iter<I: IntoIterator<Item = Q>>(iter: I) -> Self {
        Self {
            treemap: RoaringTreemap::from_iter(iter.into_iter().map(|c| c.borrow().h3index())),
            phantom_data: Default::default(),
        }
    }
}

impl<T> Default for H3Treemap<T>
where
    T: Index,
{
    fn default() -> Self {
        Self {
            treemap: Default::default(),
            phantom_data: Default::default(),
        }
    }
}

impl<T> H3Treemap<T>
where
    T: Index,
{
    /// Pushes value in the treemap only if it is greater than the current maximum value.
    /// Returns whether the value was inserted.
    #[inline]
    pub fn push(&mut self, index: T) -> bool {
        self.treemap.push(index.h3index())
    }

    /// Adds a value to the set. Returns true if the value was not already present in the set.
    #[inline]
    pub fn insert(&mut self, index: T) -> bool {
        self.treemap.insert(index.h3index())
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.treemap.len() as usize
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.treemap.is_empty()
    }

    #[inline]
    pub fn contains(&self, index: &T) -> bool {
        self.treemap.contains(index.h3index())
    }

    #[inline]
    pub fn is_disjoint(&self, rhs: &Self) -> bool {
        self.treemap.is_disjoint(&rhs.treemap)
    }

    #[inline]
    pub fn is_subset(&self, rhs: &Self) -> bool {
        self.treemap.is_subset(&rhs.treemap)
    }

    #[inline]
    pub fn is_superset(&self, rhs: &Self) -> bool {
        self.treemap.is_superset(&rhs.treemap)
    }

    pub fn iter(&self) -> Iter<T> {
        Iter {
            inner_iter: self.treemap.iter(),
            phantom_data: Default::default(),
        }
    }
}

impl<I: Index> ContainsIndex<I> for H3Treemap<I> {
    fn contains_index(&self, index: &I) -> bool {
        self.contains(index)
    }
}

pub struct Iter<'a, T> {
    inner_iter: roaring::treemap::Iter<'a>,
    phantom_data: PhantomData<T>,
}

impl<'a, T> Iterator for Iter<'a, T>
where
    T: Index,
{
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner_iter.next().map(T::new)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner_iter.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use crate::H3Cell;

    use super::H3Treemap;

    #[test]
    fn iter() {
        let idx = H3Cell::try_from(0x89283080ddbffff_u64).unwrap();
        let mut treemap = H3Treemap::default();
        for cell in idx.k_ring(1).iter() {
            treemap.insert(cell);
        }
        assert_eq!(treemap.iter().count(), 7);
    }
}
