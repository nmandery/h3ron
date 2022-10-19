use crate::algorithm::chunkedarray::util::list_map_cells;
use crate::{Error, FromIndexIterator, IndexChunked, IndexValue};
use h3ron::error::check_valid_h3_resolution;
use h3ron::iter::change_resolution;
use h3ron::H3Cell;
use polars_core::prelude::{ListChunked, UInt64Chunked, UInt8Chunked};
use std::iter::once;

/// Obtain the H3 Resolutions at the array positions where
/// the contained `Index` values are valid.
pub trait H3Resolution {
    /// Obtain the H3 Resolutions at the array positions where
    /// the contained `Index` values are valid.
    fn h3_resolution(&self) -> UInt8Chunked;
}

impl<'a, IX: IndexValue> H3Resolution for IndexChunked<'a, IX> {
    fn h3_resolution(&self) -> UInt8Chunked {
        UInt8Chunked::from_iter(self.iter_indexes_validated().map(
            |maybe_index| match maybe_index {
                Some(Ok(index)) => Some(index.resolution()),
                _ => None,
            },
        ))
    }
}

/// Changes the resolution of the contained `H3Cell` values.
pub trait H3ChangeResolution {
    /// Changes the resolution of the contained `H3Cell` values
    ///
    /// For each cell of the input array a list of cells is produced. This list may
    /// only contain a single element for cases where `target_resolution` is <= the array
    /// elements resolution.
    fn h3_change_resolution(&self, target_resolution: u8) -> Result<ListChunked, Error>;
}

impl<'a> H3ChangeResolution for IndexChunked<'a, H3Cell> {
    fn h3_change_resolution(&self, target_resolution: u8) -> Result<ListChunked, Error> {
        check_valid_h3_resolution(target_resolution)?;
        list_map_cells(self, |cell| {
            Ok(UInt64Chunked::from_index_iter(
                change_resolution(once(cell), target_resolution)
                    // todo: This error should not be hidden
                    .filter_map(|cell| cell.ok()),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithm::{H3ChangeResolution, H3Resolution};
    use crate::{AsH3CellChunked, FromIndexIterator, NamedFromIndexes};
    use h3ron::{H3Cell, Index};
    use polars_core::prelude::{ChunkExplode, TakeRandom, UInt64Chunked};

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
}
