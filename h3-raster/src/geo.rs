use geo_types::{Coordinate, CoordinateType, Polygon, Rect, LineString};

pub fn rect_contains(rect: &Rect<f64>, coordinate: &Coordinate<f64>) -> bool {
    (rect.min.x <= coordinate.x)
        && (rect.min.y <= coordinate.y)
        && (rect.max.x >= coordinate.x)
        && (rect.max.y >= coordinate.y)
}

pub fn rect_from_coordinates(c1: Coordinate<f64>, c2: Coordinate<f64>) -> Rect<f64> {
    Rect::new(
        Coordinate {
            x: if c1.x > c2.x { c2.x } else { c1.x },
            y: if c1.y > c2.y { c2.y } else { c1.y },
        },
        Coordinate {
            x: if c1.x < c2.x { c2.x } else { c1.x },
            y: if c1.y < c2.y { c2.y } else { c1.y },
        },
    )
}


/// calculate the approximate area of the given
/// linestring ring  (wgs84) in square meters
pub fn area_linearring(ring: &LineString<f64>) -> f64 {
    // roughly taken from https://gis.stackexchange.com/questions/711/how-can-i-measure-area-from-geographic-coordinates
    // full paper at: https://www.semanticscholar.org/paper/Some-algorithms-for-polygons-on-a-sphere.-Chamberlain-Duquette/79668c0fe32788176758a2285dd674fa8e7b8fa8
    ring.0.windows(2)
        .map(|coords| {
            (coords[1].x - coords[0].x).to_radians()
                * (2.0 + coords[0].y.to_radians().sin() + coords[1].y.to_radians().sin())
        })
        .sum::<f64>().abs() * 6_378_137_f64.powi(2) / 2.0
}

/// calculate the approximate area of the given
/// rect (wgs84) in square meters
pub fn area_rect(bounds: &Rect<f64>) -> f64 {
    let ring = LineString::from(vec![
        Coordinate { x: bounds.min.x, y: bounds.min.y },
        Coordinate { x: bounds.min.x, y: bounds.max.y },
        Coordinate { x: bounds.max.x, y: bounds.max.y },
        Coordinate { x: bounds.max.x, y: bounds.min.y },
        Coordinate { x: bounds.min.x, y: bounds.min.y },
    ]);
    area_linearring(&ring)
}

/// return true when the given Polygon (in WGS84 projection)
/// wraps around the dateline
///
/// this is not a generic implementation as it takes advantage
/// of the maximum polygon sizes implied by the H3 hexagons
pub fn polygon_has_dateline_wrap<T: CoordinateType>(poly: &Polygon<T>) -> bool {
    let ext_ring = poly.exterior();
    if ext_ring.num_coords() == 0 {
        return false;
    }
    let x_first = ext_ring.0.first().unwrap().x;
    let (xmin, xmax) = ext_ring.points_iter().fold(
        (x_first, x_first),
        |(xmin, xmax), p| {
            (
                if xmin < p.0.x { xmin } else { p.0.x },
                if xmax > p.0.x { xmax } else { p.0.x }
            )
        });
    (xmax - xmin) > T::from(270.0).unwrap()
}
