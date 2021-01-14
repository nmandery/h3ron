use geo::algorithm::area::Area;
use geo::algorithm::simplifyvw::SimplifyVW;
use geo_types::{Coordinate, LineString, Polygon, Triangle};
use std::cmp::min;

/// Smoothen a (closed) linestring to remove some of the artifacts
/// of the h3indexes left after creating a h3 linkedpolygon.
///
/// This function must only be used with closed linestrings
fn smoothen_h3_linked_ring(in_ring: &LineString<f64>) -> LineString<f64> {

    let mut out = Vec::with_capacity(in_ring.0.len());
    if in_ring.0.len() >= 3 {
        // The algorithm in this block is essentially an adaptation of
        // [Chaikins smoothing algorithm](http://www.idav.ucdavis.edu/education/CAGDNotes/Chaikins-Algorithm/Chaikins-Algorithm.html)
        // taking advantage of hexagon-polygons having all edges the
        // same length while avoiding the vertex duplication of chaikins algorithm.

        let apply_window = |c1: &Coordinate<f64>, c2: &Coordinate<f64>| {
            Coordinate {
                x: 0.5 * c1.x + 0.5 * c2.x,
                y: 0.5 * c1.y + 0.5 * c2.y,
            }
        };
        in_ring.0.windows(2).for_each(|window| {
            out.push(apply_window(&window[0], &window[1]));
        });

        //apply to first and last coordinate of linestring to not loose the closing point
        out.push(apply_window(&in_ring.0[in_ring.0.len() - 1], &in_ring.0[0]));

        // rotate a bit to improve the simplification result at the start/end of the ring
        let rotation_n = min(out.len(), 4);
        out.rotate_right(rotation_n);
    }

    let ring = LineString::from(out);

    // now remove redundant vertices which are, more or less, on the same straight line. the
    // are covered by three point must be less than the triangle of three points of a hexagon
    let hexagon_corner_area = Triangle::from([in_ring.0[0], in_ring.0[1], in_ring.0[2]])
        .unsigned_area();
    ring.simplifyvw(&(hexagon_corner_area * 0.75))
}


/// Smoothen a polygon to remove some of the artifacts of the h3indexes left after creating a h3 linkedpolygon.
pub fn smoothen_h3_linked_polygon(in_poly: &Polygon<f64>) -> Polygon<f64> {
    Polygon::new(
        smoothen_h3_linked_ring(in_poly.exterior()),
        in_poly.interiors().iter()
            .map(|ring| smoothen_h3_linked_ring(ring))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use crate::{
        Index,
        ToLinkedPolygons,
    };
    use geo_types::Coordinate;
    use crate::algorithm::smoothen_h3_linked_polygon;

    #[test]
    fn smooth_donut_linked_polygon() {
        let ring = Index::from_coordinate(&Coordinate::from((23.3, 12.3)), 6)
            .hex_ring(4)
            .unwrap();
        let polygons = ring.to_linked_polygons();
        assert_eq!(polygons.len(), 1);

        //let gj_in = geojson::Value::from(&polygons[0]);
        //println!("{}", gj_in);

        let smoothed = smoothen_h3_linked_polygon(&polygons[0]);

        //let gj_smoothed = geojson::Value::from(&smoothed);
        //println!("{}", gj_smoothed);

        assert!(smoothed.exterior().num_coords() < 10);
        assert_eq!(smoothed.interiors().len(), 1);
        assert!(smoothed.interiors()[0].num_coords() < 10);
    }
}
