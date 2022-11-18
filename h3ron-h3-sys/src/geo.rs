use crate::LatLng;
use geo_types::{Coord, Point};

impl From<LatLng> for Coord<f64> {
    fn from(lat_lng: LatLng) -> Self {
        Coord {
            x: lat_lng.lng.to_degrees(),
            y: lat_lng.lat.to_degrees(),
        }
    }
}

impl From<Coord<f64>> for LatLng {
    fn from(coord: Coord<f64>) -> Self {
        LatLng {
            lat: coord.y.to_radians(),
            lng: coord.x.to_radians(),
        }
    }
}

impl From<Point<f64>> for LatLng {
    fn from(pt: Point<f64>) -> Self {
        pt.0.into()
    }
}
