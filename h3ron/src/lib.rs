use std::iter::Iterator;
use std::os::raw::c_int;

use geo_types::Polygon;

use h3ron_h3_sys::{GeoCoord, Geofence, GeoPolygon, H3Index};

pub use crate::index::Index;
pub use crate::error::Error;

#[macro_use]
mod util;
mod to_geo;
pub mod collections;
pub mod experimental;
pub mod algorithm;
pub mod error;
mod index;

pub use to_geo::{
    ToPolygon,
    ToCoordinate,
    ToLinkedPolygons,
    to_linked_polygons,
    ToAlignedLinkedPolygons,
};
use crate::util::linestring_to_geocoords;

pub const H3_MIN_RESOLUTION: u8 = 0_u8;
pub const H3_MAX_RESOLUTION: u8 = 15_u8;

pub enum AreaUnits {
    M2,
    Km2,
}

impl AreaUnits {
    pub fn hex_area_at_resolution(&self, resolution: u8) -> f64 {
        match self {
            AreaUnits::M2 => unsafe { h3ron_h3_sys::hexAreaM2(resolution as i32) },
            AreaUnits::Km2 => unsafe { h3ron_h3_sys::hexAreaKm2(resolution as i32) },
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
        let mut interiors: Vec<Vec<GeoCoord>> = poly.interiors().iter()
            .map(|ls| linestring_to_geocoords(ls))
            .collect();

        let mut holes: Vec<Geofence> = interiors
            .iter_mut()
            .map(|ring| to_geofence(ring))
            .collect();

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
        let mut interiors: Vec<Vec<GeoCoord>> = poly.interiors().iter()
            .map(|ls| linestring_to_geocoords(ls))
            .collect();

        let mut holes: Vec<Geofence> = interiors
            .iter_mut()
            .map(|ring| to_geofence(ring))
            .collect();

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
        h3ron_h3_sys::compact(h3_indexes.as_ptr(), h3_indexes_out.as_mut_ptr(), h3_indexes.len() as c_int);
    }
    remove_zero_indexes_from_vec!(h3_indexes_out);
    h3_indexes_out
}

pub fn max_k_ring_size(k: u32) -> usize {
    unsafe { h3ron_h3_sys::maxKringSize(k as c_int) as usize }
}