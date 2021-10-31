use geo_types::{Coordinate, LineString, Point};

use h3ron_h3_sys::{degsToRads, GeoBoundary, GeoCoord};

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

pub(crate) fn geoboundary_to_coordinates(gb: &GeoBoundary) -> Vec<Coordinate<f64>> {
    let mut nodes = Vec::with_capacity(gb.numVerts as usize);
    for i in 0..(gb.numVerts as usize) {
        nodes.push(Coordinate::from((
            unsafe { h3ron_h3_sys::radsToDegs(gb.verts[i].lon) },
            unsafe { h3ron_h3_sys::radsToDegs(gb.verts[i].lat) },
        )));
    }
    nodes
}
