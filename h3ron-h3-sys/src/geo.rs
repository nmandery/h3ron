use crate::LatLng;
use geo_types::{Coordinate, Point};

impl From<LatLng> for Coordinate<f64> {
    fn from(lat_lng: LatLng) -> Self {
        Coordinate {
            x: (lat_lng.lng as f64).to_degrees(),
            y: (lat_lng.lat as f64).to_degrees(),
        }
    }
}

impl From<Coordinate<f64>> for LatLng {
    fn from(coord: Coordinate<f64>) -> Self {
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
