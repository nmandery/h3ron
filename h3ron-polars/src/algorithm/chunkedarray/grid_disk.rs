use crate::algorithm::chunkedarray::util::list_map_cells;
use crate::{Error, FromIndexIterator, IndexChunked};
use h3ron::H3Cell;
use polars_core::prelude::{ListChunked, UInt64Chunked};

/// Produces all cells within k distance of the origin cell.
pub trait H3GridDisk {
    /// Produces all cells within k distance of the origin cell.
    ///
    /// k=0 is defined as the origin cell, k=1 is defined as k=0 + all
    /// neighboring cells, and so on.
    fn h3_grid_disk(&self, k: u32) -> Result<ListChunked, Error>;
}

impl<'a> H3GridDisk for IndexChunked<'a, H3Cell> {
    fn h3_grid_disk(&self, k: u32) -> Result<ListChunked, Error> {
        list_map_cells(self, |cell| {
            cell.grid_disk(k)
                .map(|cells| UInt64Chunked::from_index_iter(cells.into_iter()))
                .map_err(Error::from)
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithm::chunkedarray::{
        H3ChangeResolution, H3CompactCells, H3GridDisk, H3Resolution,
    };
    use crate::from::{FromIndexIterator, NamedFromIndexes};
    use crate::AsH3CellChunked;
    use h3ron::{H3Cell, Index};
    use polars::prelude::{ChunkExplode, TakeRandom, UInt64Chunked};

    #[test]
    fn cell_resolution() {
        let expected_res = 6;
        let cell = H3Cell::from_coordinate((4.5, 1.3).into(), expected_res).unwrap();
        let ca = UInt64Chunked::from_index_iter([
            Some(cell),
            Some(H3Cell::new(55)), // invalid
            None,
        ]);
        assert_eq!(ca.len(), 3);
        let resolution_ca = ca.h3cell().h3_resolution();

        assert_eq!(resolution_ca.get(0), Some(expected_res));
        assert_eq!(resolution_ca.get(1), None);
        assert_eq!(resolution_ca.get(2), None);
    }

    #[test]
    fn cell_change_resolution_to_child() {
        let cell = H3Cell::from_coordinate((4.5, 1.3).into(), 6).unwrap();
        let ca = UInt64Chunked::new_from_indexes("", vec![cell]);
        assert_eq!(ca.len(), 1);

        let changed = ca.h3cell().h3_change_resolution(7).unwrap();
        assert_eq!(changed.len(), 1);
        let exploded = changed.explode().unwrap().unique().unwrap();
        assert_eq!(exploded.len(), 7);
    }

    #[test]
    fn cell_compact() {
        let cell = H3Cell::from_coordinate((4.5, 1.3).into(), 6).unwrap();

        let ca = UInt64Chunked::from_index_iter(&cell.get_children(7).unwrap());
        assert_eq!(ca.len(), 7);

        let changed = ca.h3cell().h3_compact_cells().unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed.h3cell().get(0), Some(cell));
    }

    #[test]
    fn cell_grid_disk() {
        let cell = H3Cell::from_coordinate((4.5, 1.3).into(), 6).unwrap();

        let ca = UInt64Chunked::new_from_indexes("", vec![cell]);
        assert_eq!(ca.len(), 1);

        let disk: Vec<_> = ca
            .h3cell()
            .h3_grid_disk(1)
            .unwrap()
            .explode()
            .unwrap()
            .sort(false)
            .u64()
            .unwrap()
            .h3cell()
            .iter_indexes_nonvalidated()
            .flatten()
            .collect();

        let mut expected: Vec<_> = cell.grid_disk(1).unwrap().iter().collect();
        expected.sort_unstable();

        assert_eq!(disk, expected);
    }
}
