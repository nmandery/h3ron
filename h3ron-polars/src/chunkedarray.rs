use crate::iter::{
    iter_indexes_nonvalidated, iter_indexes_validated, NonValidatedIndexIter, ValidatedIndexIter,
};
use h3ron::{H3Cell, H3DirectedEdge, Index};
use polars::export::arrow::bitmap::{Bitmap, MutableBitmap};
use polars::prelude::{TakeRandom, UInt64Chunked};
use std::marker::PhantomData;

pub trait IndexValue: Index + TryFrom<u64, Error = h3ron::Error> + Clone {}

impl IndexValue for H3Cell {}
impl IndexValue for H3DirectedEdge {}

#[derive(Clone)]
pub struct IndexChunked<'a, IX: IndexValue> {
    pub(crate) chunked_array: &'a UInt64Chunked,
    index_phantom: PhantomData<IX>,
}

impl<'a, IX: IndexValue> IndexChunked<'a, IX> {
    /// iterate over the `Index` values in this array.
    ///
    /// The contained `u64` values are validated and returned as Results
    pub fn iter_indexes_validated(&self) -> ValidatedIndexIter<IX> {
        iter_indexes_validated::<IX>(self.chunked_array)
    }

    /// iterate over the `Index` values in this array.
    ///
    /// The contained `u64` values are not validated, so there may be invalid `Index` values.
    pub fn iter_indexes_nonvalidated(&self) -> NonValidatedIndexIter<IX> {
        iter_indexes_nonvalidated::<IX>(self.chunked_array)
    }

    pub fn len(&self) -> usize {
        self.chunked_array.len()
    }

    pub fn is_empty(&self) -> bool {
        self.chunked_array.is_empty()
    }

    pub fn validity_bitmap(&self) -> Bitmap {
        let mut mask = MutableBitmap::with_capacity(self.len());
        for v in self.iter_indexes_nonvalidated() {
            mask.push(match v {
                None => false,
                Some(index) => index.is_valid(),
            })
        }
        mask.into()
    }
}

impl<'a, IX: IndexValue> TakeRandom for IndexChunked<'a, IX> {
    type Item = IX;

    /// get a nullable value by index. The returned `Index` is not validated.
    fn get(&self, index: usize) -> Option<Self::Item> {
        self.chunked_array.get(index).map(IX::new)
    }
}

pub trait AsH3IndexChunked {
    fn h3indexchunked<IX: IndexValue>(&self) -> IndexChunked<IX>;
}

impl AsH3IndexChunked for UInt64Chunked {
    fn h3indexchunked<IX: IndexValue>(&self) -> IndexChunked<IX> {
        IndexChunked {
            chunked_array: self,
            index_phantom: PhantomData::<IX>::default(),
        }
    }
}

pub trait AsH3CellChunked {
    fn h3cell(&self) -> IndexChunked<H3Cell>;
}

impl AsH3CellChunked for UInt64Chunked {
    fn h3cell(&self) -> IndexChunked<H3Cell> {
        self.h3indexchunked()
    }
}

pub trait AsH3DirectedEdgeChunked {
    fn h3directededge(&self) -> IndexChunked<H3DirectedEdge>;
}

impl AsH3DirectedEdgeChunked for UInt64Chunked {
    fn h3directededge(&self) -> IndexChunked<H3DirectedEdge> {
        self.h3indexchunked()
    }
}
