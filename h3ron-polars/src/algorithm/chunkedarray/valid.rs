use crate::{IndexChunked, IndexValue};
use polars_core::prelude::BooleanChunked;

pub trait H3IsValid {
    ///
    /// # Example
    ///
    /// ```
    /// use polars::prelude::UInt64Chunked;
    /// use polars_core::prelude::TakeRandom;
    /// use h3ron::{H3Cell, Index};
    /// use h3ron_polars::algorithm::chunkedarray::H3IsValid;
    /// use h3ron_polars::AsH3CellChunked;
    ///
    /// let cell = H3Cell::from_coordinate((4.5, 1.3).into(), 6).unwrap();
    /// let ca = UInt64Chunked::from_iter([
    ///         Some(cell.h3index()),
    ///         Some(55), // invalid
    ///         None,
    /// ]);
    ///
    /// let is_valid_ca = ca.h3cell().h3_is_valid();
    /// assert_eq!(is_valid_ca.len(), ca.len());
    ///
    /// assert_eq!(is_valid_ca.get(0), Some(true));
    /// assert_eq!(is_valid_ca.get(1), Some(false));
    /// assert_eq!(is_valid_ca.get(2), None);
    /// ```
    fn h3_is_valid(&self) -> BooleanChunked;

    /// Returns true when all contained h3indexes are valid.
    fn h3_all_valid(&self) -> bool;
}

impl<'a, IX: IndexValue> H3IsValid for IndexChunked<'a, IX> {
    fn h3_is_valid(&self) -> BooleanChunked {
        BooleanChunked::from_iter(
            self.iter_indexes_nonvalidated()
                .map(|maybe_index| maybe_index.map(|index| index.is_valid())),
        )
    }

    fn h3_all_valid(&self) -> bool {
        self.iter_indexes_validated()
            .all(|v| matches!(v, Some(Ok(_))))
    }
}
