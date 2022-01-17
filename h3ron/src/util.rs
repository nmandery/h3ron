use geo_types::{Coordinate, LineString, Point};

use h3ron_h3_sys::GeoCoord;

#[inline(always)]
pub(crate) fn coordinate_to_geocoord(c: &Coordinate<f64>) -> GeoCoord {
    GeoCoord {
        lat: c.y.to_radians(),
        lon: c.x.to_radians(),
    }
}

pub(crate) unsafe fn linestring_to_geocoords(ls: &LineString<f64>) -> Vec<GeoCoord> {
    ls.points_iter().map(|p| point_to_geocoord(&p)).collect()
}

#[inline(always)]
pub(crate) fn point_to_geocoord(pt: &Point<f64>) -> GeoCoord {
    GeoCoord {
        lat: pt.y().to_radians(),
        lon: pt.x().to_radians(),
    }
}
