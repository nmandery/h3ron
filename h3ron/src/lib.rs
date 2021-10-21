//!
//! # Features
//!
//! * **io**: Convenience serialization helpers of the `h3ron::io` module. These are not really related to h3, but helpful for utilities
//! during development.
//! * **use-serde**: serde support and also enables the `h3ron::collections::compressed` module.
//! * **use-rayon**: Enables [`collections::ThreadPartitionedMap`].
//! * **use-roaring**: Enables [`collections::H3Treemap`] based on the `roaring` crate.
use std::iter::Iterator;
use std::os::raw::c_int;

use geo_types::{LineString, Polygon};

use h3ron_h3_sys::{GeoCoord, GeoPolygon, Geofence, H3Index};
pub use to_geo::{
    to_linked_polygons, ToAlignedLinkedPolygons, ToCoordinate, ToLinkedPolygons, ToPolygon,
};
pub use {
    error::Error, h3_cell::H3Cell, h3_direction::H3Direction, h3_edge::H3Edge,
    index::HasH3Resolution, index::Index, to_h3::ToH3Cells,
};

use crate::collections::indexvec::IndexVec;
use crate::error::check_same_resolution;
use crate::util::linestring_to_geocoords;

#[macro_use]
mod util;
pub mod algorithm;
pub mod collections;
pub mod error;
pub mod experimental;
mod h3_cell;
mod h3_direction;
mod h3_edge;
mod index;
#[cfg(feature = "io")]
pub mod io;
pub mod iter;
pub mod to_geo;
pub mod to_h3;

pub const H3_MIN_RESOLUTION: u8 = 0_u8;
pub const H3_MAX_RESOLUTION: u8 = 15_u8;

/// trait for types which can be created from an H3Index
pub trait FromH3Index {
    fn from_h3index(h3index: H3Index) -> Self;
}

impl FromH3Index for H3Index {
    fn from_h3index(h3index: H3Index) -> Self {
        h3index
    }
}

/// trait for types with a measurable area
pub trait ExactArea {
    /// Retrieves the exact area of `self` in square meters
    fn exact_area_m2(&self) -> f64;

    /// Retrieves the exact area of `self` in square kilometers
    fn exact_area_km2(&self) -> f64;

    /// Retrieves the exact area of `self` in square radians
    fn exact_area_rads2(&self) -> f64;
}

/// trait for types with a measurable length
pub trait ExactLength {
    /// Retrieves the exact length of `self` in meters
    fn exact_length_m(&self) -> f64;

    /// Retrieves the exact length of `self` in kilometers
    fn exact_length_km(&self) -> f64;

    /// Retrieves the exact length of `self` in radians
    fn exact_length_rads(&self) -> f64;
}

unsafe fn to_geofence(ring: &mut Vec<GeoCoord>) -> Geofence {
    Geofence {
        numVerts: ring.len() as c_int,
        verts: ring.as_mut_ptr(),
    }
}

pub fn max_polyfill_size(poly: &Polygon<f64>, h3_resolution: u8) -> usize {
    unsafe {
        let mut exterior: Vec<GeoCoord> = linestring_to_geocoords(poly.exterior());
        let mut interiors: Vec<Vec<GeoCoord>> = poly
            .interiors()
            .iter()
            .map(|ls| linestring_to_geocoords(ls))
            .collect();

        let mut holes: Vec<Geofence> = interiors.iter_mut().map(|ring| to_geofence(ring)).collect();

        let gp = GeoPolygon {
            geofence: to_geofence(&mut exterior),
            numHoles: holes.len() as c_int,
            holes: holes.as_mut_ptr(),
        };

        h3ron_h3_sys::maxPolyfillSize(&gp, h3_resolution as c_int) as usize
    }
}

pub fn polyfill(poly: &Polygon<f64>, h3_resolution: u8) -> IndexVec<H3Cell> {
    unsafe {
        let mut exterior: Vec<GeoCoord> = linestring_to_geocoords(poly.exterior());
        let mut interiors: Vec<Vec<GeoCoord>> = poly
            .interiors()
            .iter()
            .map(|ls| linestring_to_geocoords(ls))
            .collect();

        let mut holes: Vec<Geofence> = interiors.iter_mut().map(|ring| to_geofence(ring)).collect();

        let gp = GeoPolygon {
            geofence: to_geofence(&mut exterior),
            numHoles: holes.len() as c_int,
            holes: holes.as_mut_ptr(),
        };

        let num_hexagons = h3ron_h3_sys::maxPolyfillSize(&gp, h3_resolution as c_int);

        // pre-allocate for the expected number of hexagons
        let mut index_vec = IndexVec::with_length(num_hexagons as usize);

        h3ron_h3_sys::polyfill(&gp, h3_resolution as c_int, index_vec.as_mut_ptr());
        index_vec
    }
}

///
/// the input vec must be deduplicated and all cells must be at the same resolution
pub fn compact(cells: &[H3Cell]) -> IndexVec<H3Cell> {
    let mut index_vec = IndexVec::with_length(cells.len());
    unsafe {
        // the following requires `repr(transparent)` on H3Cell
        let h3index_slice =
            std::slice::from_raw_parts(cells.as_ptr() as *const H3Index, cells.len());
        h3ron_h3_sys::compact(
            h3index_slice.as_ptr(),
            index_vec.as_mut_ptr(),
            cells.len() as c_int,
        );
    }
    index_vec
}

/// maximum number of cells needed for the k_ring
#[inline]
pub fn max_k_ring_size(k: u32) -> usize {
    unsafe { h3ron_h3_sys::maxKringSize(k as c_int) as usize }
}

/// Number of cells in a line connecting two cells
pub fn line_size(start: H3Cell, end: H3Cell) -> Result<usize, Error> {
    check_same_resolution(start, end)?;
    line_size_not_checked(start, end)
}

fn line_size_not_checked(start: H3Cell, end: H3Cell) -> Result<usize, Error> {
    let size = unsafe { h3ron_h3_sys::h3LineSize(start.h3index(), end.h3index()) };
    if size < 0 {
        Err(Error::LineNotComputable)
    } else {
        Ok(size as usize)
    }
}

fn line_between_cells_not_checked(start: H3Cell, end: H3Cell) -> Result<IndexVec<H3Cell>, Error> {
    let num_indexes = line_size_not_checked(start, end)?;
    let mut index_vec = IndexVec::with_length(num_indexes);
    let retval =
        unsafe { h3ron_h3_sys::h3Line(start.h3index(), end.h3index(), index_vec.as_mut_ptr()) };
    if retval != 0 {
        return Err(Error::LineNotComputable);
    }
    Ok(index_vec)
}

/// Line of h3 indexes connecting two indexes
pub fn line_between_cells(start: H3Cell, end: H3Cell) -> Result<IndexVec<H3Cell>, Error> {
    check_same_resolution(start, end)?;
    line_between_cells_not_checked(start, end)
}

/// Generate h3 cells along the given linestring
///
/// The returned cells are ordered sequentially, there are no
/// duplicates caused by the start and endpoints of multiple line segments.
pub fn line(linestring: &LineString<f64>, h3_resolution: u8) -> Result<IndexVec<H3Cell>, Error> {
    let mut cells_out = IndexVec::new();
    for coords in linestring.0.windows(2) {
        let start_index = H3Cell::from_coordinate(&coords[0], h3_resolution)?;
        let end_index = H3Cell::from_coordinate(&coords[1], h3_resolution)?;

        let mut segment_indexes = line_between_cells_not_checked(start_index, end_index)?;
        if segment_indexes.is_empty() {
            continue;
        }
        cells_out.append(&mut segment_indexes);
    }
    cells_out.dedup();
    Ok(cells_out)
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use geo::Coordinate;
    use geo_types::LineString;

    use crate::{line, line_between_cells, H3Cell};

    #[test]
    fn line_across_multiple_faces() {
        // ported from H3s testH3Line.c
        let start = H3Cell::try_from(0x85285aa7fffffff_u64).unwrap();
        let end = H3Cell::try_from(0x851d9b1bfffffff_u64).unwrap();

        // Line not computable across multiple icosa faces
        assert!(line_between_cells(start, end).is_err());
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
        assert!(line(&ls, 5).unwrap().count() > 200)
    }
}
