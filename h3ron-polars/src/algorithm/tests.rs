use crate::from::FromIndexIterator;
use crate::Error;
use h3ron::H3Cell;
use polars::prelude::{DataFrame, NamedFrom, Series, UInt64Chunked};

pub(crate) fn make_cell_dataframe(
    cell_col_name: &str,
    h3_resolution: u8,
    value: Option<u32>,
) -> Result<DataFrame, Error> {
    let cells = UInt64Chunked::from_index_iter(
        H3Cell::from_coordinate((10.0, 20.0).into(), h3_resolution)?
            .grid_disk(10)?
            .iter()
            .chain(
                H3Cell::from_coordinate((45.0, 45.0).into(), h3_resolution)?
                    .grid_disk(3)?
                    .iter(),
            ),
    );

    let mut df = DataFrame::new(vec![Series::new(cell_col_name, cells)])?;
    if let Some(value) = value {
        df.with_column(Series::new(
            "value",
            (0..(df.shape().0)).map(|_| value).collect::<Vec<_>>(),
        ))?;
    }
    Ok(df)
}
