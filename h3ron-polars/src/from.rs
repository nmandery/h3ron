use h3ron::{H3Cell, H3DirectedEdge, Index};
use polars::prelude::{IntoSeries, Series, UInt64Chunked};
use std::borrow::Borrow;

/// Convert an H3 index to an `Option<u64`> to store it in an `UInt64Chunked` array.
///
/// index values which are invalid H3 value for that type are set to invalid
/// in arrow validity mask.
pub trait ToUInt64Option {
    fn to_uint64_option(&self) -> Option<u64>;
}

macro_rules! impl_to_uint64_option {
    ($index_type:ty) => {
        impl ToUInt64Option for $index_type {
            fn to_uint64_option(&self) -> Option<u64> {
                if self.is_valid() {
                    Some(self.h3index())
                } else {
                    None
                }
            }
        }

        impl ToUInt64Option for Option<$index_type> {
            fn to_uint64_option(&self) -> Option<u64> {
                self.as_ref().map(|i| i.to_uint64_option()).flatten()
            }
        }
    };
}

impl_to_uint64_option!(H3Cell);
impl_to_uint64_option!(H3DirectedEdge);

pub trait FromIndexIterator {
    fn from_index_iter<I, IX>(iter: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<IX>,
        IX: ToUInt64Option;
}

impl FromIndexIterator for UInt64Chunked {
    fn from_index_iter<I, IX>(iter: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<IX>,
        IX: ToUInt64Option,
    {
        UInt64Chunked::from_iter(iter.into_iter().map(|v| v.borrow().to_uint64_option()))
    }
}

impl FromIndexIterator for Series {
    fn from_index_iter<I, IX>(iter: I) -> Self
    where
        I: IntoIterator,
        I::Item: Borrow<IX>,
        IX: ToUInt64Option,
    {
        UInt64Chunked::from_index_iter(iter).into_series()
    }
}

pub trait NamedFromIndexes {
    fn new_from_indexes<T, V>(name: &str, _: T) -> Self
    where
        T: AsRef<[V]>,
        V: ToUInt64Option;
}

impl NamedFromIndexes for UInt64Chunked {
    fn new_from_indexes<T, V>(name: &str, values: T) -> Self
    where
        T: AsRef<[V]>,
        V: ToUInt64Option,
    {
        let mut ca = UInt64Chunked::from_index_iter::<_, V>(values.as_ref());
        ca.rename(name);
        ca
    }
}

impl NamedFromIndexes for Series {
    fn new_from_indexes<T, V>(name: &str, values: T) -> Self
    where
        T: AsRef<[V]>,
        V: ToUInt64Option,
    {
        UInt64Chunked::new_from_indexes(name, values).into_series()
    }
}

#[cfg(test)]
mod tests {
    use crate::from::NamedFromIndexes;
    use h3ron::{H3Cell, Index};
    use polars::prelude::{TakeRandom, UInt64Chunked};

    #[test]
    fn test_invalid_index_are_arrow_invalid() {
        let cells = vec![
            H3Cell::from_coordinate((45.5, 45.3).into(), 5).unwrap(),
            H3Cell::new(55), // invalid
        ];

        let ca = UInt64Chunked::new_from_indexes("", cells);
        assert_eq!(ca.len(), 2);
        assert!(ca.get(0).is_some());
        assert!(ca.get(1).is_none());
    }
}
