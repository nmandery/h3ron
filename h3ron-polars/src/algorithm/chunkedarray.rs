use crate::from::FromIndexIterator;
use crate::{Error, IndexChunked, IndexValue};
use geo::BoundingRect as GeoBoundingRect;
use geo_types::{coord, CoordNum, Rect};
use h3ron::collections::CompactedCellVec;
use h3ron::error::check_valid_h3_resolution;
use h3ron::iter::change_resolution;
use h3ron::to_geo::ToLineString;
use h3ron::{H3Cell, H3DirectedEdge, ToPolygon};
use polars::prelude::{BooleanChunked, IntoSeries, ListChunked, UInt64Chunked, UInt8Chunked};
use std::iter::once;

pub trait BoundingRect {
    fn bounding_rect(&self) -> Result<Option<Rect>, Error>;
}

impl BoundingRect for H3Cell {
    fn bounding_rect(&self) -> Result<Option<Rect>, Error> {
        Ok(self.to_polygon()?.bounding_rect())
    }
}

impl BoundingRect for H3DirectedEdge {
    fn bounding_rect(&self) -> Result<Option<Rect>, Error> {
        Ok(self.to_linestring()?.bounding_rect())
    }
}

impl<'a, IX: IndexValue> BoundingRect for IndexChunked<'a, IX>
where
    IX: BoundingRect,
{
    fn bounding_rect(&self) -> Result<Option<Rect>, Error> {
        let mut rect = None;
        for maybe_index in self.iter_indexes_validated().flatten() {
            let new_rect = maybe_index?.bounding_rect()?;

            match (rect.as_mut(), new_rect) {
                (None, Some(r)) => rect = Some(r),
                (Some(agg), Some(this)) => *agg = bounding_rect_merge(agg, &this),
                _ => (),
            }
        }
        Ok(rect)
    }
}

// Return a new rectangle that encompasses the provided rectangles
//
// taken from `geo` crate
fn bounding_rect_merge<T: CoordNum>(a: &Rect<T>, b: &Rect<T>) -> Rect<T> {
    Rect::new(
        coord! {
            x: partial_min(a.min().x, b.min().x),
            y: partial_min(a.min().y, b.min().y),
        },
        coord! {
            x: partial_max(a.max().x, b.max().x),
            y: partial_max(a.max().y, b.max().y),
        },
    )
}

// The Rust standard library has `max` for `Ord`, but not for `PartialOrd`
pub fn partial_max<T: PartialOrd>(a: T, b: T) -> T {
    if a > b {
        a
    } else {
        b
    }
}

// The Rust standard library has `min` for `Ord`, but not for `PartialOrd`
pub fn partial_min<T: PartialOrd>(a: T, b: T) -> T {
    if a < b {
        a
    } else {
        b
    }
}

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
}

impl<'a, IX: IndexValue> H3IsValid for IndexChunked<'a, IX> {
    fn h3_is_valid(&self) -> BooleanChunked {
        BooleanChunked::from_iter(
            self.iter_indexes_nonvalidated()
                .map(|maybe_index| maybe_index.map(|index| index.is_valid())),
        )
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
        ccv.add_cells(
            self.iter_indexes_validated()
                .filter_map(|c_opt| c_opt.and_then(|c| c.ok())),
            true,
        )?;

        Ok(UInt64Chunked::from_index_iter(ccv.iter_compacted_cells()))
    }
}

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

fn list_map_cells<F>(cc: &IndexChunked<H3Cell>, map_fn: F) -> Result<ListChunked, Error>
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
