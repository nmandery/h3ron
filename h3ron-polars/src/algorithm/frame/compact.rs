use crate::algorithm::chunkedarray::H3CompactCells;
use crate::{AsH3CellChunked, Error};
use h3ron::collections::H3CellSet;
use h3ron::iter::change_resolution;
use h3ron::{H3Cell, Index};
use polars::export::rayon::iter::ParallelIterator;
use polars::prelude::{
    col, ChunkUnique, DataFrame, DataType, IntoLazy, IntoSeries, NamedFrom, Series,
};
use polars_core::POOL;
use std::borrow::Borrow;
use std::cmp::Ordering;

pub trait H3CompactDataframe {
    /// Compact the cells in the column named `cell_column_name`.
    ///
    /// This is done by first grouping the dataframe using all other columns and then
    /// compacting the list of cells of each group.
    fn h3_compact_dataframe<S>(
        self,
        cell_column_name: S,
        return_exploded: bool,
    ) -> Result<Self, Error>
    where
        Self: Sized,
        S: AsRef<str>;
}

impl H3CompactDataframe for DataFrame {
    fn h3_compact_dataframe<S>(
        self,
        cell_column_name: S,
        return_exploded: bool,
    ) -> Result<Self, Error>
    where
        S: AsRef<str>,
    {
        let group_by_columns = self
            .fields()
            .iter()
            .filter_map(|field| {
                if field.name() != cell_column_name.as_ref() {
                    Some(col(field.name()))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if group_by_columns.is_empty() {
            let cellchunked = self.column(cell_column_name.as_ref())?.u64()?.h3cell();
            let compacted_series =
                Series::new(cell_column_name.as_ref(), cellchunked.h3_compact_cells()?);

            if return_exploded {
                Ok(DataFrame::new(vec![compacted_series])?)
            } else {
                Ok(DataFrame::new(vec![Series::new(
                    cell_column_name.as_ref(),
                    vec![compacted_series],
                )])?)
            }
        } else {
            let grouped = self
                .lazy()
                .groupby(&group_by_columns)
                .agg(&[col(cell_column_name.as_ref()).list()])
                .collect()?;

            let listchunked_cells = grouped.column(cell_column_name.as_ref())?.list()?;
            let compacted_series_vec = POOL.install(|| {
                // Ordering is preserved. see https://github.com/rayon-rs/rayon/issues/551
                listchunked_cells
                    .par_iter()
                    .map(compact_maybe_series)
                    .collect::<Result<Vec<_>, _>>()
            })?;

            let mut grouped = grouped.drop(cell_column_name.as_ref())?;
            grouped.with_column(Series::new(cell_column_name.as_ref(), compacted_series_vec))?;

            if return_exploded {
                Ok(grouped.explode([cell_column_name.as_ref()])?)
            } else {
                Ok(grouped)
            }
        }
    }
}

fn compact_maybe_series(maybe_series: Option<Series>) -> Result<Series, Error> {
    let compacted_series = if let Some(series) = maybe_series {
        series.u64()?.h3cell().h3_compact_cells()?.into_series()
    } else {
        Series::new_empty("", &DataType::UInt64)
    };
    Ok(compacted_series)
}

pub trait H3UncompactDataframe {
    /// Uncompact the cells in the column named `cell_column_name`.
    ///
    /// Implements the reverse of [H3CompactDataframe].
    fn h3_uncompact_dataframe<S>(
        self,
        cell_column_name: S,
        target_resolution: u8,
    ) -> Result<Self, Error>
    where
        Self: Sized,
        S: AsRef<str>;

    /// Uncompact the cells in the column named `cell_column_name` while only returning the cells from
    /// the given `subset`.
    ///
    /// Implements the reverse of [H3CompactDataframe].
    fn h3_uncompact_dataframe_subset<S>(
        self,
        cell_column_name: S,
        target_resolution: u8,
        subset: &H3CellSet,
    ) -> Result<Self, Error>
    where
        Self: Sized,
        S: AsRef<str>;

    /// Uncompact the cells in the column named `cell_column_name` while only returning the cells from
    /// the given `subset`.
    ///
    /// Implements the reverse of [H3CompactDataframe].
    fn h3_uncompact_dataframe_subset_iter<S, I>(
        self,
        cell_column_name: S,
        target_resolution: u8,
        subset: I,
    ) -> Result<Self, Error>
    where
        Self: Sized,
        S: AsRef<str>,
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
    {
        let subset =
            change_resolution(subset, target_resolution).collect::<Result<H3CellSet, _>>()?;
        self.h3_uncompact_dataframe_subset(cell_column_name, target_resolution, &subset)
    }
}

impl H3UncompactDataframe for DataFrame {
    fn h3_uncompact_dataframe<S>(
        self,
        cell_column_name: S,
        target_resolution: u8,
    ) -> Result<Self, Error>
    where
        Self: Sized,
        S: AsRef<str>,
    {
        uncompact_df(self, cell_column_name, target_resolution, |_| true)
    }

    fn h3_uncompact_dataframe_subset<S>(
        self,
        cell_column_name: S,
        target_resolution: u8,
        subset: &H3CellSet,
    ) -> Result<Self, Error>
    where
        Self: Sized,
        S: AsRef<str>,
    {
        uncompact_df(self, cell_column_name, target_resolution, |cell| {
            subset.contains(cell)
        })
    }
}

const UNCOMPACT_JOIN_COL_NAME: &str = "_uncompact_join_idx";

fn uncompact_df<Filter, S>(
    df: DataFrame,
    cell_column_name: S,
    target_resolution: u8,
    filter: Filter,
) -> Result<DataFrame, Error>
where
    Filter: Fn(&H3Cell) -> bool,
    S: AsRef<str>,
{
    let unique_cell_ca = df.column(cell_column_name.as_ref())?.u64()?.unique()?;
    let cellchunked = unique_cell_ca.h3cell();

    let mut original_indexes = Vec::with_capacity(cellchunked.len());
    let mut uncompacted_indexes = Vec::with_capacity(cellchunked.len());

    // invalid cells are ignored
    for cell in cellchunked.iter_indexes_validated().flatten().flatten() {
        match cell.resolution().cmp(&target_resolution) {
            Ordering::Less => {
                for cell_child in cell.get_children(target_resolution)?.iter().filter(&filter) {
                    original_indexes.push(cell.h3index() as u64);
                    uncompacted_indexes.push(cell_child.h3index() as u64);
                }
            }
            Ordering::Equal => {
                if filter(&cell) {
                    original_indexes.push(cell.h3index() as u64);
                    uncompacted_indexes.push(cell.h3index() as u64);
                }
            }
            Ordering::Greater => {
                // ignore higher resolution data above the requested target_resolution
            }
        }
    }

    if original_indexes == uncompacted_indexes {
        // nothing to do
        return Ok(df);
    }

    let df = df
        .lazy()
        .inner_join(
            DataFrame::new(vec![
                Series::new(cell_column_name.as_ref(), original_indexes),
                Series::new(UNCOMPACT_JOIN_COL_NAME, uncompacted_indexes),
            ])?
            .lazy(),
            col(cell_column_name.as_ref()),
            col(cell_column_name.as_ref()),
        )
        .drop_columns(&[cell_column_name.as_ref()])
        .rename(&[UNCOMPACT_JOIN_COL_NAME], &[cell_column_name.as_ref()])
        .collect()?;

    Ok(df)
}

#[cfg(test)]
mod tests {
    use crate::algorithm::chunkedarray::H3Resolution;
    use crate::algorithm::frame::{H3CompactDataframe, H3UncompactDataframe};
    use crate::algorithm::tests::make_cell_dataframe;
    use crate::AsH3CellChunked;
    use crate::NamedFromIndexes;
    use h3ron::{H3Cell, HasH3Resolution};
    use polars::prelude::{DataFrame, DataType, Series, SeriesOps};

    const CELL_COL_NAME: &str = "cell";

    fn compact_roundtrip_helper(value: Option<u32>) {
        let max_res = 8;
        let df = make_cell_dataframe(CELL_COL_NAME, max_res, value).unwrap();
        let shape_before = df.shape();

        let compacted = df.h3_compact_dataframe(CELL_COL_NAME, true).unwrap();

        assert!(shape_before.0 > compacted.shape().0);
        assert_eq!(shape_before.1, compacted.shape().1);
        assert_eq!(
            compacted.column(CELL_COL_NAME).unwrap().dtype(),
            &DataType::UInt64
        );

        let compacted_resolutions = compacted
            .column(CELL_COL_NAME)
            .unwrap()
            .u64()
            .unwrap()
            .h3cell()
            .h3_resolution();
        assert!(compacted_resolutions.len() > 1);
        for res in &compacted_resolutions {
            assert!(res.unwrap() <= max_res);
        }

        let uncompacted = compacted
            .h3_uncompact_dataframe(CELL_COL_NAME, max_res)
            .unwrap();
        assert_eq!(uncompacted.shape(), shape_before);
        assert_eq!(
            uncompacted.column(CELL_COL_NAME).unwrap().dtype(),
            &DataType::UInt64
        );

        let resolutions = uncompacted
            .column(CELL_COL_NAME)
            .unwrap()
            .u64()
            .unwrap()
            .h3cell()
            .h3_resolution();
        assert_eq!(uncompacted.shape().0, resolutions.len());
        for res in &resolutions {
            assert_eq!(res.unwrap(), max_res);
        }
    }

    #[test]
    fn compact_roundtrip_with_value() {
        compact_roundtrip_helper(Some(7))
    }

    #[test]
    fn compact_roundtrip_without_value() {
        compact_roundtrip_helper(None)
    }

    #[test]
    fn uncompact_subset() {
        let origin_cell = H3Cell::from_coordinate((12.0, 12.0).into(), 5).unwrap();

        let df = DataFrame::new(vec![Series::new_from_indexes(
            CELL_COL_NAME,
            origin_cell
                .grid_disk(12)
                .unwrap()
                .iter()
                .collect::<Vec<_>>(),
        )])
        .unwrap();

        let subset_origin = origin_cell.center_child(7).unwrap();
        let subset = {
            let mut subset = subset_origin
                .grid_disk(1)
                .unwrap()
                .iter()
                .collect::<Vec<_>>();
            subset.sort_unstable();
            subset
        };

        let subset_df = df
            .h3_uncompact_dataframe_subset_iter(
                CELL_COL_NAME,
                subset_origin.h3_resolution(),
                subset.as_slice(),
            )
            .unwrap();
        assert_eq!(subset_df.shape().0, subset.len());

        let subset_from_subset_df = {
            let mut sbs = subset_df
                .column(CELL_COL_NAME)
                .unwrap()
                .u64()
                .unwrap()
                .h3cell()
                .iter_indexes_validated()
                .flatten()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            sbs.sort();
            sbs
        };
        assert_eq!(subset, subset_from_subset_df);
    }
}
