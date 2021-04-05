use geo_types::{Coordinate, LineString, Point};

use h3ron_h3_sys::{degsToRads, GeoCoord, H3Index};

use crate::index::Index;

/// filter out 0 values ( = positions in the vec not used to store h3indexes)
macro_rules! remove_zero_indexes_from_vec {
    ($h3_indexes:expr) => {
        $h3_indexes.retain(|h3i: &H3Index| *h3i != 0);
    };
}

#[inline]
pub(crate) fn drain_h3indexes_to_indexes(mut v: Vec<H3Index>) -> Vec<Index> {
    v.drain(..)
        .filter(|h3i| *h3i != 0)
        .map(Index::new)
        .collect()
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
