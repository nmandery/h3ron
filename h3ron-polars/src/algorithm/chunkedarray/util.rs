use crate::{Error, IndexChunked};
use h3ron::H3Cell;
use polars_core::prelude::{IntoSeries, ListChunked, UInt64Chunked};

pub(crate) fn list_map_cells<F>(cc: &IndexChunked<H3Cell>, map_fn: F) -> Result<ListChunked, Error>
where
    F: Fn(H3Cell) -> Result<UInt64Chunked, Error>,
{
    let mut series_vec = Vec::with_capacity(cc.chunked_array.len());
    for maybe_cell in cc.iter_indexes_validated() {
        let mapped = match maybe_cell {
            Some(Ok(cell)) => Some(map_fn(cell)?.into_series()),
            _ => None,
        };
        series_vec.push(mapped);
    }
    Ok(ListChunked::from_iter(series_vec))
}
