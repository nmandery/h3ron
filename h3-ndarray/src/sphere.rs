use geo_types::{LineString, Coordinate, Rect};

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
        Coordinate { x: bounds.min().x, y: bounds.min().y },
        Coordinate { x: bounds.min().x, y: bounds.max().y },
        Coordinate { x: bounds.max().x, y: bounds.max().y },
        Coordinate { x: bounds.max().x, y: bounds.min().y },
        Coordinate { x: bounds.min().x, y: bounds.min().y },
    ]);
    area_linearring(&ring)
}
