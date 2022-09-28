use polars_core::prelude::{ChunkCompare, DataFrame, NamedFrom, Series, UInt8Chunked};

use crate::algorithm::chunkedarray::H3Resolution;
use crate::{AsH3IndexChunked, Error, IndexValue};

pub trait H3ResolutionOp {
    /// obtain the contained H3 resolutions
    fn h3_resolution<IX, S>(&self, index_column_name: S) -> Result<UInt8Chunked, Error>
    where
        IX: IndexValue,
        S: AsRef<str>;

    /// Split the dataframe into separate frames for each H3 resolution found in the contents.
    fn h3_split_by_resolution<IX, S>(&self, index_column_name: S) -> Result<Vec<(u8, Self)>, Error>
    where
        Self: Sized,
        IX: IndexValue,
        S: AsRef<str>;
}

const RSPLIT_R_COL_NAME: &str = "_rsplit_resolution";

impl H3ResolutionOp for DataFrame {
    fn h3_resolution<IX, S>(&self, index_column_name: S) -> Result<UInt8Chunked, Error>
    where
        IX: IndexValue,
        S: AsRef<str>,
    {
        let ic = self
            .column(index_column_name.as_ref())?
            .u64()?
            .h3indexchunked::<IX>();
        Ok(ic.h3_resolution())
    }

    fn h3_split_by_resolution<IX, S>(&self, index_column_name: S) -> Result<Vec<(u8, Self)>, Error>
    where
        Self: Sized,
        IX: IndexValue,
        S: AsRef<str>,
    {
        let resolutions = Series::new(
            RSPLIT_R_COL_NAME,
            self.h3_resolution::<IX, _>(index_column_name)?,
        );

        let distinct_resolutions: Vec<u8> = resolutions
            .drop_nulls()
            .unique()?
            .u8()?
            .into_iter()
            .flatten()
            .collect();

        match distinct_resolutions.len() {
            0 => Ok(vec![]),
            1 => Ok(vec![(distinct_resolutions[0], self.clone())]),
            _ => {
                let mut out_dfs = Vec::with_capacity(distinct_resolutions.len());
                for h3_resolution in distinct_resolutions {
                    let filtered = self.filter(&resolutions.equal(h3_resolution)?)?;
                    out_dfs.push((h3_resolution, filtered))
                }
                Ok(out_dfs)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use h3ron::{H3Cell, Index};
    use polars_core::frame::DataFrame;
    use polars_core::prelude::{NamedFrom, Series};

    use crate::algorithm::frame::H3ResolutionOp;

    #[test]
    fn split_frame_by_resolution() {
        let series = Series::new(
            "cell",
            vec![
                H3Cell::from_coordinate((45.6, -45.8).into(), 7)
                    .unwrap()
                    .h3index() as u64,
                H3Cell::from_coordinate((45.6, -10.2).into(), 8)
                    .unwrap()
                    .h3index() as u64,
                H3Cell::from_coordinate((45.6, 50.2).into(), 8)
                    .unwrap()
                    .h3index() as u64,
                H3Cell::from_coordinate((-23.1, -60.5).into(), 5)
                    .unwrap()
                    .h3index() as u64,
            ],
        );
        let value_series = Series::new("value", &(0u32..(series.len() as u32)).collect::<Vec<_>>());
        let df = DataFrame::new(vec![series, value_series]).unwrap();

        let parts = df.h3_split_by_resolution::<H3Cell, _>("cell").unwrap();
        assert_eq!(parts.len(), 3);
        for (h3_resolution, df) in parts {
            let expected = if h3_resolution == 8 { 2 } else { 1 };
            assert_eq!(df.shape(), (expected, 2));
        }
    }
}
