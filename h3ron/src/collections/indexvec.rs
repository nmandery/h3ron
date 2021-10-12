use crate::{Error, FromH3Index, Index};
use h3ron_h3_sys::H3Index;
use std::convert::TryFrom;
use std::marker::PhantomData;

/// A `H3Index`-Vec intended to interface between libh3 by providing
/// continuous memory to place h3indexes in.
///
/// Provides iterators to create `Index` implementing types from the primitive `H3Index`/`u64`
/// types. The iterators automatically skip all `0` values (as created some libh3 functions).
///
/// The struct does not provide a `len()` method as this would create the impression that accessing
/// this method is cheap. As a count of the contained elements requires checking each for `0`, that
/// functionality is provided by [`IndexVec::count()`]
#[derive(Debug)]
pub struct IndexVec<T: FromH3Index + Index> {
    inner_vec: Vec<H3Index>,
    phantom: PhantomData<T>,
}

const EMPTY_H3INDEX: H3Index = 0;

impl<T: FromH3Index + Index> Default for IndexVec<T> {
    fn default() -> Self {
        Self {
            inner_vec: vec![EMPTY_H3INDEX; 0],
            phantom: Default::default(),
        }
    }
}

impl<T: FromH3Index + Index> IndexVec<T> {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn with_length(length: usize) -> Self {
        Self {
            inner_vec: vec![EMPTY_H3INDEX; length],
            phantom: PhantomData::default(),
        }
    }

    /// capacity of the array, not necessarily the number of contained
    /// element
    pub fn capacity(&self) -> usize {
        self.inner_vec.capacity()
    }

    pub fn clear(&mut self) {
        unsafe {
            std::ptr::write_bytes(self.inner_vec.as_mut_ptr(), 0, self.inner_vec.len());
        }
    }

    pub fn as_slice(&self) -> &[H3Index] {
        &self.inner_vec
    }

    pub fn as_mut_slice(&mut self) -> &mut [H3Index] {
        &mut self.inner_vec
    }

    pub fn as_ptr(&self) -> *const H3Index {
        self.inner_vec.as_ptr()
    }

    pub fn as_mut_ptr(&mut self) -> *mut H3Index {
        self.inner_vec.as_mut_ptr()
    }

    pub fn iter(&self) -> UncheckedIter<'_, T> {
        self.iter_unchecked()
    }

    pub fn iter_checked(&self) -> CheckedIter<'_, T> {
        CheckedIter {
            inner_iter: self.inner_vec.iter(),
            phantom: Default::default(),
        }
    }

    pub fn iter_unchecked(&self) -> UncheckedIter<'_, T> {
        UncheckedIter {
            inner_iter: self.inner_vec.iter(),
            phantom: Default::default(),
        }
    }

    pub fn first(&self) -> Option<T> {
        self.iter_unchecked().next()
    }

    pub fn is_empty(&self) -> bool {
        self.first().is_none()
    }

    pub fn pop(&mut self) -> Option<T> {
        while let Some(h3index) = self.inner_vec.pop() {
            if h3index == EMPTY_H3INDEX {
                continue;
            }
            return Some(T::from_h3index(h3index));
        }
        None
    }

    pub fn shrink_to_fit(&mut self) {
        // remove all empty h3indexes before shrinking
        self.inner_vec.retain(|h3index| *h3index != EMPTY_H3INDEX);

        self.inner_vec.shrink_to_fit();
    }

    pub fn drain(&mut self) -> UncheckedDrain<'_, T> {
        UncheckedDrain {
            inner_drain: self.inner_vec.drain(..),
            phantom: Default::default(),
        }
    }

    pub fn append(&mut self, other: &mut IndexVec<T>) {
        self.inner_vec.append(&mut other.inner_vec)
    }

    pub fn dedup(&mut self) {
        self.inner_vec.dedup()
    }

    pub fn sort_unstable(&mut self) {
        self.inner_vec.sort_unstable()
    }

    pub fn count(&self) -> usize {
        self.iter_unchecked().count()
    }

    pub fn push(&mut self, item: T) {
        self.inner_vec.push(item.h3index())
    }
}

impl<'a, T: FromH3Index + Index> IntoIterator for &'a IndexVec<T> {
    type Item = T;
    type IntoIter = UncheckedIter<'a, T>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_unchecked()
    }
}

pub struct CheckedIter<'a, T: FromH3Index + Index> {
    inner_iter: std::slice::Iter<'a, H3Index>,
    phantom: PhantomData<T>,
}

impl<'a, T: FromH3Index + Index> Iterator for CheckedIter<'a, T> {
    type Item = Result<T, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        for h3index in &mut self.inner_iter {
            if *h3index == EMPTY_H3INDEX {
                continue;
            }
            let value = T::from_h3index(*h3index);
            return match value.validate() {
                Ok(_) => Some(Ok(value)),
                Err(e) => Some(Err(e)),
            };
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner_iter.size_hint()
    }
}

pub struct UncheckedIter<'a, T: FromH3Index + Index> {
    inner_iter: std::slice::Iter<'a, H3Index>,
    phantom: PhantomData<T>,
}

impl<'a, T: FromH3Index + Index> Iterator for UncheckedIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        for h3index in &mut self.inner_iter {
            if *h3index == EMPTY_H3INDEX {
                continue;
            }
            return Some(T::from_h3index(*h3index));
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner_iter.size_hint()
    }
}

pub struct UncheckedDrain<'a, T: FromH3Index + Index> {
    inner_drain: std::vec::Drain<'a, H3Index>,
    phantom: PhantomData<T>,
}

impl<'a, T: FromH3Index + Index> Iterator for UncheckedDrain<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        for h3index in &mut self.inner_drain {
            if h3index == EMPTY_H3INDEX {
                continue;
            }
            return Some(Self::Item::from_h3index(h3index));
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner_drain.size_hint()
    }
}

impl<T: FromH3Index + Index> From<IndexVec<T>> for Vec<T> {
    fn from(mut index_vec: IndexVec<T>) -> Self {
        index_vec.drain().collect()
    }
}

impl<T: FromH3Index + Index> TryFrom<Vec<H3Index>> for IndexVec<T> {
    type Error = Error;

    fn try_from(h3index_vec: Vec<H3Index>) -> Result<Self, Self::Error> {
        for h3index in h3index_vec.iter() {
            let value = T::from_h3index(*h3index);
            value.validate()?
        }
        Ok(Self {
            inner_vec: h3index_vec,
            phantom: Default::default(),
        })
    }
}
