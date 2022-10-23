use crate::{Error, FromIndexIterator, IndexChunked};
use h3ron::collections::CompactedCellVec;
use h3ron::H3Cell;
use polars_core::prelude::UInt64Chunked;

/// Compacts `H3Cell` using the H3 resolution hierarchy.
pub trait H3CompactCells {
    /// Compacts `H3Cell` using the H3 resolution hierarchy.
    ///
    /// Invalid cells are ignored.
    fn h3_compact_cells(&self) -> Result<UInt64Chunked, Error>;
}

impl<'a> H3CompactCells for IndexChunked<'a, H3Cell> {
    fn h3_compact_cells(&self) -> Result<UInt64Chunked, Error> {
        let mut ccv = CompactedCellVec::new();
        ccv.add_cells(self.iter_indexes_nonvalidated().flatten(), true)?;

        Ok(UInt64Chunked::from_index_iter(ccv.iter_compacted_cells()))
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithm::H3CompactCells;
    use crate::{AsH3CellChunked, FromIndexIterator};
    use h3ron::H3Cell;
    use polars_core::prelude::{TakeRandom, UInt64Chunked};

    #[test]
    fn cell_compact() {
        let cell = H3Cell::from_coordinate((4.5, 1.3).into(), 6).unwrap();

        let ca = UInt64Chunked::from_index_iter(&cell.get_children(7).unwrap());
        assert_eq!(ca.len(), 7);

        let changed = ca.h3cell().h3_compact_cells().unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed.h3cell().get(0), Some(cell));
    }
}
