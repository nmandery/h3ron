//! This crate depends on the `h3ron-h3-sys` crate, which includes the C sources for libh3. So
//! compiling requires a C toolchain and the `cmake` build tool.
//!
//! # Features
//!
//! * **use-serde**: serde serialization/deserialization for most types of this crate.
//! * **use-rayon**
//! * **roaring**: Enables `collections::H3Treemap` based on the `roaring` crate.
//! * **parse**: Parse [`H3Cell`] from different string representations using `H3Cell::from_str`.
//!
#![warn(nonstandard_style)]
#![allow(clippy::redundant_pub_crate)]
extern crate core;

use geo_types::LineString;

use h3ron_h3_sys::H3Index;
pub use to_geo::{
    to_linked_polygons, ToAlignedLinkedPolygons, ToCoordinate, ToLinkedPolygons, ToPolygon,
};
pub use {
    cell::H3Cell, directed_edge::H3DirectedEdge, direction::H3Direction, error::Error,
    index::HasH3Resolution, index::Index, localij::CoordIj, to_h3::ToH3Cells,
};

use crate::collections::indexvec::IndexVec;

#[macro_use]
pub mod algorithm;
mod cell;
pub mod collections;
mod directed_edge;
mod direction;
pub mod error;
mod index;
pub mod iter;
pub mod localij;
pub mod to_geo;
pub mod to_h3;

pub const H3_MIN_RESOLUTION: u8 = 0_u8;
pub const H3_MAX_RESOLUTION: u8 = 15_u8;

/// trait for types which can be created from an `H3Index`
pub trait FromH3Index {
    fn from_h3index(h3index: H3Index) -> Self;
}

impl FromH3Index for H3Index {
    fn from_h3index(h3index: H3Index) -> Self {
        h3index
    }
}

///
/// the input vec must be deduplicated and all cells must be at the same resolution
pub fn compact_cells(cells: &[H3Cell]) -> Result<IndexVec<H3Cell>, Error> {
    let mut index_vec = IndexVec::with_length(cells.len());
    Error::check_returncode(unsafe {
        // the following requires `repr(transparent)` on H3Cell
        let h3index_slice =
            std::slice::from_raw_parts(cells.as_ptr().cast::<H3Index>(), cells.len());
        h3ron_h3_sys::compactCells(
            h3index_slice.as_ptr(),
            index_vec.as_mut_ptr(),
            cells.len() as i64,
        )
    })?;
    Ok(index_vec)
}

/// maximum number of cells needed for the `k_ring`
pub fn max_grid_disk_size(k: u32) -> Result<usize, Error> {
    let mut max_size: i64 = 0;
    Error::check_returncode(unsafe { h3ron_h3_sys::maxGridDiskSize(k as i32, &mut max_size) })?;
    Ok(max_size as usize)
}

/// Number of cells in a line connecting two cells
pub fn grid_path_cells_size(start: H3Cell, end: H3Cell) -> Result<usize, Error> {
    let mut cells_size: i64 = 0;
    Error::check_returncode(unsafe {
        h3ron_h3_sys::gridPathCellsSize(start.h3index(), end.h3index(), &mut cells_size)
    })?;
    Ok(cells_size as usize)
}

/// Line of h3 indexes connecting two cells
///
/// # Arguments
///
/// * `start`- start cell
/// * `end` - end cell
///
pub fn grid_path_cells(start: H3Cell, end: H3Cell) -> Result<IndexVec<H3Cell>, Error> {
    let cells_size = grid_path_cells_size(start, end)?;
    let mut index_vec = IndexVec::with_length(cells_size);

    Error::check_returncode(unsafe {
        h3ron_h3_sys::gridPathCells(start.h3index(), end.h3index(), index_vec.as_mut_ptr())
    })?;

    Ok(index_vec)
}

/// Generate h3 cells along the given linestring
///
/// The returned cells are ordered sequentially, there are no
/// duplicates caused by the start and endpoints of multiple line segments.
///
/// # Errors
///
/// The function may fail if invalid indexes are built from the given coordinates.
pub fn line(linestring: &LineString<f64>, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
    let mut cells_out = IndexVec::new();
    for coords in linestring.0.windows(2) {
        let start_index = H3Cell::from_coordinate(coords[0], h3_resolution)?;
        let end_index = H3Cell::from_coordinate(coords[1], h3_resolution)?;

        for cell in grid_path_cells(start_index, end_index)?.iter() {
            cells_out.push(cell)
        }
    }
    cells_out.dedup();
    Ok(cells_out)
}

/// `res0_cell_count` returns the number of resolution 0 indexes
pub fn res0_cell_count() -> u8 {
    unsafe { h3ron_h3_sys::res0CellCount() as u8 }
}

/// provides all base cells in H3Index format
pub fn res0_cells() -> IndexVec<H3Cell> {
    let mut index_vec = IndexVec::with_length(res0_cell_count() as usize);
    unsafe { h3ron_h3_sys::getRes0Cells(index_vec.as_mut_ptr()) };
    index_vec
}

#[cfg(test)]
mod tests {
    use geo_types::{Coordinate, LineString};

    use crate::{grid_path_cells, line, res0_cell_count, res0_cells, H3Cell};

    #[test]
    fn line_across_multiple_faces() {
        // ported from H3s testH3Line.c
        let start = H3Cell::try_from(0x85285aa7fffffff_u64).unwrap();
        let end = H3Cell::try_from(0x851d9b1bfffffff_u64).unwrap();

        // Line not computable across multiple icosa faces
        assert!(grid_path_cells(start, end).is_err());
    }

    #[test]
    fn linestring() {
        let ls = LineString::from(vec![
            Coordinate::from((11.60, 37.16)),
            Coordinate::from((3.86, 39.63)),
            Coordinate::from((-4.57, 35.17)),
            Coordinate::from((-20.74, 34.88)),
            Coordinate::from((-23.55, 48.92)),
        ]);
        assert!(line(&ls, 5).unwrap().count() > 200);
    }

    #[test]
    fn test_res0_index_count() {
        assert_eq!(res0_cell_count(), 122);
    }

    #[test]
    fn test_res0_indexes() {
        assert_eq!(res0_cells().iter().count(), res0_cell_count() as usize);
    }
}
