use std::iter::Iterator;
use std::os::raw::c_int;

use geo_types::{LineString, Polygon};

use h3ron_h3_sys::{GeoCoord, GeoPolygon, Geofence, H3Index};
pub use to_geo::{
    to_linked_polygons, ToAlignedLinkedPolygons, ToCoordinate, ToLinkedPolygons, ToPolygon,
};

use crate::error::check_same_resolution;
use crate::util::linestring_to_geocoords;
pub use {error::Error, h3_cell::H3Cell, h3_edge::H3Edge, index::Index, to_h3::ToH3Indexes};

#[macro_use]
mod util;
pub mod algorithm;
pub mod collections;
pub mod error;
pub mod experimental;
mod h3_cell;
mod h3_edge;
mod index;
mod to_geo;
mod to_h3;

pub const H3_MIN_RESOLUTION: u8 = 0_u8;
pub const H3_MAX_RESOLUTION: u8 = 15_u8;

pub trait FromH3Index {
    fn from_h3index(h3index: H3Index) -> Self;
}

impl FromH3Index for H3Index {
    fn from_h3index(h3index: H3Index) -> Self {
        h3index
    }
}

pub enum AreaUnits {
    M2,
    Km2,
    Radians2,
}

impl AreaUnits {
    pub fn hex_area_at_resolution(&self, resolution: u8) -> Result<f64, Error> {
        match self {
            AreaUnits::M2 => Ok(unsafe { h3ron_h3_sys::hexAreaM2(resolution as i32) }),
            AreaUnits::Km2 => Ok(unsafe { h3ron_h3_sys::hexAreaKm2(resolution as i32) }),
            _ => Err(Error::UnsupportedOperation),
        }
    }
}

unsafe fn to_geofence(ring: &mut Vec<GeoCoord>) -> Geofence {
    Geofence {
        numVerts: ring.len() as c_int,
        verts: ring.as_mut_ptr(),
    }
}

pub fn max_polyfill_size(poly: &Polygon<f64>, h3_resolution: u8) -> usize {
    unsafe {
        let mut exterior: Vec<GeoCoord> = linestring_to_geocoords(&poly.exterior());
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

pub fn polyfill(poly: &Polygon<f64>, h3_resolution: u8) -> Vec<H3Index> {
    let mut h3_indexes = unsafe {
        let mut exterior: Vec<GeoCoord> = linestring_to_geocoords(&poly.exterior());
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
        let mut h3_indexes: Vec<H3Index> = vec![0; num_hexagons as usize];

        h3ron_h3_sys::polyfill(&gp, h3_resolution as c_int, h3_indexes.as_mut_ptr());
        h3_indexes
    };
    remove_zero_indexes_from_vec!(h3_indexes);
    h3_indexes
}

///
/// the input vec must be deduplicated
pub fn compact(h3_indexes: &[H3Index]) -> Vec<H3Index> {
    let mut h3_indexes_out: Vec<H3Index> = vec![0; h3_indexes.len()];
    unsafe {
        h3ron_h3_sys::compact(
            h3_indexes.as_ptr(),
            h3_indexes_out.as_mut_ptr(),
            h3_indexes.len() as c_int,
        );
    }
    remove_zero_indexes_from_vec!(h3_indexes_out);
    h3_indexes_out
}

pub fn max_k_ring_size(k: u32) -> usize {
    unsafe { h3ron_h3_sys::maxKringSize(k as c_int) as usize }
}

/// Number of indexes in a line connecting two indexes
pub fn line_size(start: H3Index, end: H3Index) -> Result<usize, Error> {
    check_same_resolution(start, end)?;
    line_size_not_checked(start, end)
}

fn line_size_not_checked(start: H3Index, end: H3Index) -> Result<usize, Error> {
    let size = unsafe { h3ron_h3_sys::h3LineSize(start, end) };
    if size < 0 {
        Err(Error::LineNotComputable)
    } else {
        Ok(size as usize)
    }
}

fn line_between_indexes_not_checked(start: H3Index, end: H3Index) -> Result<Vec<H3Index>, Error> {
    let num_indexes = line_size_not_checked(start, end)?;
    let mut h3_indexes_out: Vec<H3Index> = vec![0; num_indexes];
    let retval = unsafe { h3ron_h3_sys::h3Line(start, end, h3_indexes_out.as_mut_ptr()) };
    if retval != 0 {
        return Err(Error::LineNotComputable);
    }
    remove_zero_indexes_from_vec!(h3_indexes_out);
    Ok(h3_indexes_out)
}

/// Line of h3 indexes connecting two indexes
pub fn line_between_indexes(start: H3Index, end: H3Index) -> Result<Vec<H3Index>, Error> {
    check_same_resolution(start, end)?;
    line_between_indexes_not_checked(start, end)
}

/// Generate h3 indexes along the given linestring
///
/// The returned indexes are ordered sequentially, there may
/// be duplicates caused by multiple line seqments
pub fn line(linestring: &LineString<f64>, h3_resolution: u8) -> Result<Vec<H3Index>, Error> {
    let mut h3_indexes_out = vec![];
    for coords in linestring.0.windows(2) {
        let start_index = H3Cell::from_coordinate(&coords[0], h3_resolution)?;
        let end_index = H3Cell::from_coordinate(&coords[1], h3_resolution)?;

        let mut segment_indexes =
            line_between_indexes_not_checked(start_index.h3index(), end_index.h3index())?;
        if segment_indexes.is_empty() {
            continue;
        }
        if !h3_indexes_out.is_empty()
            && h3_indexes_out[h3_indexes_out.len() - 1] == segment_indexes[0]
        {
            h3_indexes_out.remove(h3_indexes_out.len() - 1);
        }
        h3_indexes_out.append(&mut segment_indexes);
    }
    Ok(h3_indexes_out)
}

#[cfg(test)]
mod tests {
    use geo::Coordinate;
    use geo_types::LineString;

    use crate::{line, line_between_indexes};

    #[test]
    fn line_across_multiple_faces() {
        // ported from H3s testH3Line.c
        let start = 0x85285aa7fffffff_u64;
        let end = 0x851d9b1bfffffff_u64;

        // Line not computable across multiple icosa faces
        assert!(line_between_indexes(start, end).is_err());
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
        assert!(line(&ls, 5).unwrap().len() > 200)
    }
}
