use std::cmp::min;

use geo::algorithm::area::Area;
use geo::algorithm::simplifyvw::SimplifyVW;
use geo_types::{Coordinate, LineString, Polygon, Triangle};

fn is_closed(ls: &[Coordinate<f64>]) -> bool {
    if ls.len() < 2 {
        false
    } else {
        ls.first() == ls.last()
    }
}

/// Smoothen a linestring to remove some of the artifacts
/// of the h3indexes left after creating a h3 linkedpolygon.
pub(crate) fn smoothen_h3_coordinates(in_coords: &[Coordinate<f64>]) -> Vec<Coordinate<f64>> {
    let closed = is_closed(in_coords);
    let mut out = Vec::with_capacity(in_coords.len() + if closed { 2 } else { 0 });
    if in_coords.len() >= 3 {
        // The algorithm in this block is essentially an adaptation of
        // [Chaikins smoothing algorithm](http://www.idav.ucdavis.edu/education/CAGDNotes/Chaikins-Algorithm/Chaikins-Algorithm.html)
        // taking advantage of hexagon-polygons having all edges the
        // same length while avoiding the vertex duplication of chaikins algorithm.

        if !closed {
            // preserve the unmodified starting coordinate
            out.push(*in_coords.first().unwrap());
        }
        let apply_window = |c1: &Coordinate<f64>, c2: &Coordinate<f64>| Coordinate {
            x: 0.5f64.mul_add(c1.x, 0.5 * c2.x),
            y: 0.5f64.mul_add(c1.y, 0.5 * c2.y),
        };
        in_coords.windows(2).for_each(|window| {
            out.push(apply_window(&window[0], &window[1]));
        });

        if closed {
            //apply to first and last coordinate of linestring to not loose the closing point
            out.push(apply_window(&in_coords[in_coords.len() - 1], &in_coords[0]));

            // rotate a bit to improve the simplification result at the start/end of the ring
            let rotation_n = min(out.len(), 4);
            out.rotate_right(rotation_n);
        } else {
            // preserve the unmodified end coordinate
            out.push(*in_coords.last().unwrap());
        }
    } else {
        out = in_coords.to_vec();
    }

    if in_coords.len() >= 3 {
        // now remove redundant vertices which are, more or less, on the same straight line. the
        // are covered by three point must be less than the triangle of three points of a hexagon
        let out_ls = LineString::from(out);
        let hexagon_corner_area =
            Triangle::from([in_coords[0], in_coords[1], in_coords[2]]).unsigned_area();
        out_ls.simplifyvw(&(hexagon_corner_area * 0.75)).0
    } else {
        out
    }
}

/// Smoothen a polygon to remove some of the artifacts of the h3indexes left after creating a h3 linkedpolygon.
pub fn smoothen_h3_linked_polygon(in_poly: &Polygon<f64>) -> Polygon<f64> {
    Polygon::new(
        LineString::from(smoothen_h3_coordinates(&in_poly.exterior().0)),
        in_poly
            .interiors()
            .iter()
            .map(|ring| LineString::from(smoothen_h3_coordinates(&ring.0)))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use geo::algorithm::coords_iter::CoordsIter;
    use geo_types::Coordinate;

    use crate::algorithm::smoothen_h3_linked_polygon;
    use crate::{H3Cell, ToLinkedPolygons};

    #[test]
    fn smooth_donut_linked_polygon() {
        let ring = H3Cell::from_coordinate(&Coordinate::from((23.3, 12.3)), 6)
            .unwrap()
            .hex_ring(4)
            .unwrap();
        let polygons = ring.to_linked_polygons(false);
        assert_eq!(polygons.len(), 1);

        //let gj_in = geojson::Value::from(&polygons[0]);
        //println!("{}", gj_in);

        let smoothed = smoothen_h3_linked_polygon(&polygons[0]);

        //let gj_smoothed = geojson::Value::from(&smoothed);
        //println!("{}", gj_smoothed);

        assert!(smoothed.exterior().coords_count() < 10);
        assert_eq!(smoothed.interiors().len(), 1);
        assert!(smoothed.interiors()[0].coords_count() < 10);
    }
}
