extern crate h3_sys;
extern crate geo_types;

use std::collections::HashMap;
use std::os::raw::c_int;

use geo_types::{LineString, Point, Polygon};
use h3_sys::{degsToRads, GeoCoord, Geofence, GeoPolygon, H3Index};
use std::iter::Iterator;

pub fn get_resolution(i: H3Index) -> i32 {
    unsafe { h3_sys::h3GetResolution(i) }
}

pub fn get_parent(i: H3Index, resolution: i32) -> H3Index {
    unsafe { h3_sys::h3ToParent(i, resolution as c_int) }
}

unsafe fn point_to_geocoord(pt: &Point<f64>) -> GeoCoord {
    GeoCoord {
        lat: degsToRads(pt.y()),
        lon: degsToRads(pt.x()),
    }
}

unsafe fn linestring_to_geocoords(ls: &LineString<f64>) -> Vec<GeoCoord> {
    ls.points_iter()
        .map(|p| point_to_geocoord(&p))
        .collect()
}

pub fn polyfill_polygon(poly: &Polygon<f64>, h3_resolution: i32) -> Vec<H3Index> {
    let mut h3_indexes = unsafe {
        let mut exterior: Vec<GeoCoord> = linestring_to_geocoords(&poly.exterior());
        let mut interiors: Vec<Vec<GeoCoord>> = poly.interiors().iter()
            .map(|ls| linestring_to_geocoords(ls))
            .collect();

        fn to_geofence(ring: &mut Vec<GeoCoord>) -> Geofence {
            Geofence {
                numVerts: ring.len() as c_int,
                verts: ring.as_mut_ptr(),
            }
        }

        let mut holes: Vec<Geofence> = interiors
            .iter_mut()
            .map(|ring| to_geofence(ring))
            .collect();

        let gp = GeoPolygon {
            geofence: to_geofence(&mut exterior),
            numHoles: holes.len() as c_int,
            holes: holes.as_mut_ptr(),
        };

        let num_hexagons = h3_sys::maxPolyfillSize(&gp, h3_resolution as c_int);

        // pre-allocate for the expected number of hexagons
        let mut h3_indexes: Vec<H3Index> = vec![0; num_hexagons as usize];

        h3_sys::polyfill(&gp, h3_resolution as c_int, h3_indexes.as_mut_ptr());

        h3_indexes
    };

    // filter out 0 values ( = positions in the vec not used to store h3indexes)
    h3_indexes.retain(|h3i: &H3Index| *h3i != 0);
    h3_indexes
}

pub fn point_to_h3index(pt: &Point<f64>, h3_resolution: i32) -> H3Index {
    unsafe {
        let gc = point_to_geocoord(pt);
        h3_sys::geoToH3(&gc, h3_resolution as c_int)
    }
}

pub fn compact(h3_indexes: &[H3Index]) -> Vec<H3Index> {
    let mut h3_indexes_out: Vec<H3Index> = vec![0; h3_indexes.len()];
    unsafe {
        h3_sys::compact(h3_indexes.as_ptr(), h3_indexes_out.as_mut_ptr(), h3_indexes.len() as c_int);
    }
    // filter out 0 values ( = unused values of the vec )
    h3_indexes_out.retain(|h3i: &H3Index| *h3i != 0);
    h3_indexes_out
}

/// group indexes by their resolution
pub fn group_h3indexes_by_resolution(h3_indexes: &[H3Index]) -> HashMap<i32, Vec<H3Index>> {
    let mut m = HashMap::new();
    h3_indexes.iter().for_each(|idx: &H3Index| {
        m.entry(get_resolution(*idx))
            .or_insert_with(Vec::new)
            .push(*idx);
    });
    m
}
