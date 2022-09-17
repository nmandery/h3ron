use crate::error::Error;
use h3ron::Index;
use polars::prelude::{PolarsIterator, UInt64Chunked};
use std::marker::PhantomData;

pub struct ValidatedIndexIter<'a, IX> {
    phantom_data: PhantomData<IX>,
    inner_iter: Box<dyn PolarsIterator<Item = Option<u64>> + 'a>,
}

impl<'a, IX> Iterator for ValidatedIndexIter<'a, IX>
where
    IX: Index + TryFrom<u64, Error = h3ron::Error>,
{
    type Item = Option<Result<IX, Error>>;

    fn next(&mut self) -> Option<Self::Item> {
        #[allow(clippy::manual_flatten)]
        match &mut self.inner_iter.next() {
            None => None,
            Some(index_opt) => match index_opt {
                Some(h3index) => Some(Some(IX::try_from(*h3index).map_err(Error::from))),
                None => Some(None),
            },
        }
    }
}

/// iterate over the `Index` values in the given array.
///
/// The contained `u64` values are validated and returned as Results
pub fn iter_indexes_validated<IX>(ca: &UInt64Chunked) -> ValidatedIndexIter<IX>
where
    IX: Index + TryFrom<u64, Error = h3ron::Error>,
{
    ValidatedIndexIter {
        phantom_data: PhantomData::<IX>::default(),
        inner_iter: ca.into_iter(),
    }
}

pub struct NonValidatedIndexIter<'a, IX> {
    phantom_data: PhantomData<IX>,
    inner_iter: Box<dyn PolarsIterator<Item = Option<u64>> + 'a>,
}

impl<'a, IX> Iterator for NonValidatedIndexIter<'a, IX>
where
    IX: Index,
{
    type Item = Option<IX>;

    fn next(&mut self) -> Option<Self::Item> {
        #[allow(clippy::manual_flatten)]
        match &mut self.inner_iter.next() {
            None => None,
            Some(index_opt) => match index_opt {
                Some(h3index) => Some(Some(IX::new(*h3index))),
                None => Some(None),
            },
        }
    }
}

/// iterate over the `Index` values in this array.
///
/// The contained `u64` values are not validated, so there may be invalid `Index` values.
pub fn iter_indexes_nonvalidated<IX>(ca: &UInt64Chunked) -> NonValidatedIndexIter<IX>
where
    IX: Index,
{
    NonValidatedIndexIter {
        phantom_data: PhantomData::<IX>::default(),
        inner_iter: ca.into_iter(),
    }
}
