use crate::algorithm::chunkedarray::H3IsValid;
use crate::{AsH3IndexChunked, Error, IndexChunked, IndexValue};
use polars_core::prelude::{DataFrame, Series};
#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;

/// Simple container to associate the name of the column containing h3 indexes
/// with a `DataFrame`.
///
/// This container is just a convenience wrapper to provide some often needed feature
/// of associating a column name with a dataframe. Most algorithms of this crate are traits
/// and can be directly applied to dataframes and series.
///
/// Some of the algorithms of this crate are available as methods on this struct.
#[derive(Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
pub struct H3DataFrame<IX: IndexValue> {
    df: DataFrame,
    h3index_column_name: String,
    index_phantom: PhantomData<IX>,
}

impl<IX: IndexValue> H3DataFrame<IX> {
    #[doc(hidden)]
    pub fn from_dataframe_nonvalidated<S>(df: DataFrame, h3index_column_name: S) -> Self
    where
        S: AsRef<str>,
    {
        Self {
            df,
            h3index_column_name: h3index_column_name.as_ref().to_owned(),
            index_phantom: PhantomData::<IX>,
        }
    }

    pub fn from_dataframe<S>(df: DataFrame, h3index_column_name: S) -> Result<Self, Error>
    where
        S: AsRef<str>,
    {
        let h3df = Self::from_dataframe_nonvalidated(df, h3index_column_name);
        h3df.validate()?;
        Ok(h3df)
    }

    pub fn dataframe(&self) -> &DataFrame {
        &self.df
    }

    pub fn dataframe_mut(&mut self) -> &mut DataFrame {
        &mut self.df
    }

    pub fn h3index_column_name(&self) -> &str {
        &self.h3index_column_name
    }

    pub fn h3index_series(&self) -> Result<&Series, Error> {
        Ok(self.df.column(&self.h3index_column_name)?)
    }

    pub fn h3indexchunked(&self) -> Result<IndexChunked<IX>, Error> {
        Ok(self.h3index_series()?.u64()?.h3indexchunked())
    }

    pub fn validate(&self) -> Result<(), Error> {
        if !self.h3indexchunked()?.h3_all_valid() {
            Err(Error::InvalidH3Indexes)
        } else {
            Ok(())
        }
    }

    pub fn into_dataframe(self) -> DataFrame {
        self.df
    }
}
