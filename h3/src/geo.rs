#[cfg(feature = "with-geo-types-0_4")]
use geo_types_04::{Coordinate, LineString, Point};
#[cfg(feature = "with-geo-types-0_6")]
use geo_types_06::{Coordinate, LineString, Point};

use h3_sys::{degsToRads, GeoCoord};

pub(crate) unsafe fn coordinate_to_geocoord(c: &Coordinate<f64>) -> GeoCoord {
    GeoCoord {
        lat: degsToRads(c.y),
        lon: degsToRads(c.x),
    }
}

pub(crate) unsafe fn linestring_to_geocoords(ls: &LineString<f64>) -> Vec<GeoCoord> {
    ls.points_iter()
        .map(|p| point_to_geocoord(&p))
        .collect()
}


pub(crate) unsafe fn point_to_geocoord(pt: &Point<f64>) -> GeoCoord {
    GeoCoord {
        lat: degsToRads(pt.y()),
        lon: degsToRads(pt.x()),
    }
}
