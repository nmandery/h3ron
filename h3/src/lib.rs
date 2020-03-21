extern crate geo_types;
extern crate h3_sys;

use std::collections::HashMap;
use std::iter::Iterator;
use std::mem::MaybeUninit;
use std::os::raw::c_int;

use geo_types::{Coordinate, LineString, Point, Polygon};

use h3_sys::{degsToRads, GeoCoord, Geofence, GeoPolygon, H3Index};

#[macro_use]
mod util;
pub mod compact;

pub fn get_resolution(i: H3Index) -> i32 {
    unsafe { h3_sys::h3GetResolution(i) }
}

pub fn get_parent(i: H3Index, resolution: i32) -> H3Index {
    unsafe { h3_sys::h3ToParent(i, resolution as c_int) }
}

pub enum AreaUnits {
    M2,
    Km2,
}

pub fn hex_area_at_resolution(resolution: i32, units: AreaUnits) -> f64 {
    match units {
        AreaUnits::M2 => unsafe { h3_sys::hexAreaM2(resolution) },
        AreaUnits::Km2 => unsafe { h3_sys::hexAreaKm2(resolution) },
    }
}

/*
impl Into<Coordinate<f64>> for H3Index {
    fn into(self) -> Coordinate<f64> {
        unsafe {
            let geocoord: GeoCoord = GeoCoord {x: 0.0, y:0.0};
            h3_sys::h3ToGeo(h3index, geocoord.as_mut_ptr());

            Coordinate {
                x: radsToDegs(geocoord.lon),
                y: radsToDegs(geocoord.lat),
            }
        }
    }
}
*/

unsafe fn point_to_geocoord(pt: &Point<f64>) -> GeoCoord {
    GeoCoord {
        lat: degsToRads(pt.y()),
        lon: degsToRads(pt.x()),
    }
}

unsafe fn coordinate_to_geocoord(c: &Coordinate<f64>) -> GeoCoord {
    GeoCoord {
        lat: degsToRads(c.y),
        lon: degsToRads(c.x),
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
    remove_zero_indexes_from_vec!(h3_indexes);
    h3_indexes
}

pub fn point_to_h3index(pt: &Point<f64>, h3_resolution: i32) -> H3Index {
    unsafe {
        let gc = point_to_geocoord(pt);
        h3_sys::geoToH3(&gc, h3_resolution as c_int)
    }
}

pub fn polygon_from_h3index(h3index: H3Index) -> Option<Polygon<f64>> {
    let gb = unsafe {
        let mut mu = MaybeUninit::<h3_sys::GeoBoundary>::uninit();
        h3_sys::h3ToGeoBoundary(h3index, mu.as_mut_ptr());
        mu.assume_init()
    };

    if gb.numVerts > 0 {
        let mut nodes = vec![];
        for i in 0..gb.numVerts {
            nodes.push((
                unsafe { h3_sys::radsToDegs(gb.verts[i as usize].lon) },
                unsafe { h3_sys::radsToDegs(gb.verts[i as usize].lat) },
            ));
        }
        nodes.push((*nodes.first().unwrap()).clone());
        Some(Polygon::new(LineString::from(nodes), vec![]))
    } else {
        None
    }
}

pub fn coordinate_to_h3index(c: &Coordinate<f64>, h3_resolution: i32) -> H3Index {
    unsafe {
        let gc = coordinate_to_geocoord(c);
        h3_sys::geoToH3(&gc, h3_resolution as c_int)
    }
}

pub fn coordinate_from_h3index(h3index: H3Index) -> Coordinate<f64> {
    unsafe {
        let mut gc = GeoCoord {
            lat: 0.0,
            lon: 0.0,
        };
        h3_sys::h3ToGeo(h3index, &mut gc);

        Coordinate {
            x: h3_sys::radsToDegs(gc.lon),
            y: h3_sys::radsToDegs(gc.lat),
        }
    }
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

pub fn k_ring(h3_index: H3Index, k: i32) -> Vec<H3Index> {
    let max_size = unsafe { h3_sys::maxKringSize(k) as usize };
    let mut h3_indexes_out: Vec<H3Index> = vec![0; max_size];

    unsafe {
        h3_sys::kRing(h3_index, k as c_int, h3_indexes_out.as_mut_ptr());
    }
    remove_zero_indexes_from_vec!(h3_indexes_out);
    h3_indexes_out
}


pub fn h3_to_string(h3_index: H3Index) -> String {
    format!("{:x}", h3_index)
}

pub fn is_valid(h3_index: H3Index) -> bool {
    unsafe { h3_sys::h3IsValid(h3_index) != 0 }
}

#[cfg(test)]
mod tests {
    use crate::{h3_to_string, is_valid};

    #[test]
    fn test_h3_to_string() {
        let h3index = 0x89283080ddbffff_u64;
        let h3str = h3_to_string(h3index);
        assert_eq!(h3str, "89283080ddbffff".to_string());
    }

    #[test]
    fn test_is_valid() {
        assert_eq!(is_valid(0x89283080ddbffff_u64), true);
        assert_eq!(is_valid(0_u64), false);
    }
}
