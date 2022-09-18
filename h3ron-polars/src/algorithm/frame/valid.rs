use crate::{AsH3IndexChunked, Error, IndexValue};
use polars::export::arrow::array::BooleanArray;
use polars::prelude::ArrowDataType;
use polars_core::prelude::{BooleanChunked, DataFrame};

pub trait FilterH3IsValid {
    fn filter_h3_is_valid<IX, S>(&self, index_column_name: S) -> Result<Self, Error>
    where
        Self: Sized,
        IX: IndexValue,
        S: AsRef<str>;
}

impl FilterH3IsValid for DataFrame {
    /// Remove all rows where the contained `Index` in column `index_column_name` is invalid.
    ///
    /// # Example
    ///
    /// ```
    /// use polars::prelude::{DataFrame, Series, NamedFrom, TakeRandom};
    /// use h3ron::{H3Cell, Index};
    /// use h3ron_polars::algorithm::frame::FilterH3IsValid;
    ///
    /// let h3index = H3Cell::from_coordinate((56.4, 23.2).into(), 5)
    ///     .unwrap()
    ///     .h3index();
    ///
    /// let df = DataFrame::new(vec![Series::new("x", vec![Some(56), Some(h3index), None])]).unwrap();
    /// let df = df.filter_h3_is_valid::<H3Cell, _>("x").unwrap();
    ///
    /// assert_eq!(df.shape().0, 1);
    /// assert_eq!(df.column("x").unwrap().u64().unwrap().get(0), Some(h3index));
    /// ```
    fn filter_h3_is_valid<IX, S>(&self, index_column_name: S) -> Result<Self, Error>
    where
        Self: Sized,
        IX: IndexValue,
        S: AsRef<str>,
    {
        let indexchunked = self
            .column(index_column_name.as_ref())?
            .u64()?
            .h3indexchunked::<IX>();
        let ba =
            BooleanArray::from_data(ArrowDataType::Boolean, indexchunked.validity_bitmap(), None);

        Ok(self.filter(&BooleanChunked::from(ba))?)
    }
}
