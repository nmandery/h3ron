use geo_types::{Coordinate, LineString, Point};

use crate::Index;
use h3ron_h3_sys::{degsToRads, GeoCoord, H3Index};

#[inline]
pub(crate) fn drain_h3indexes_to_indexes<T: Index>(mut v: Vec<H3Index>) -> Vec<T> {
    // filters out 0 values ( = positions in the vec not used to store h3indexes, created by polyfill, k_ring, ...)
    v.drain(..).filter(|h3i| *h3i != 0).map(T::new).collect()
}

pub(crate) unsafe fn coordinate_to_geocoord(c: &Coordinate<f64>) -> GeoCoord {
    GeoCoord {
        lat: degsToRads(c.y),
        lon: degsToRads(c.x),
    }
}

pub(crate) unsafe fn linestring_to_geocoords(ls: &LineString<f64>) -> Vec<GeoCoord> {
    ls.points_iter().map(|p| point_to_geocoord(&p)).collect()
}

pub(crate) unsafe fn point_to_geocoord(pt: &Point<f64>) -> GeoCoord {
    GeoCoord {
        lat: degsToRads(pt.y()),
        lon: degsToRads(pt.x()),
    }
}
