use std::collections::HashMap;
use std::os::raw::c_int;

use geo::algorithm::euclidean_distance::EuclideanDistance;
use geo_types::{Coordinate, LineString, Polygon, Point};

use h3ron_h3_sys::{
    destroyLinkedPolygon,
    H3Index,
    h3SetToLinkedGeo,
    LinkedGeoPolygon,
    radsToDegs,
};

use crate::algorithm::smoothen_h3_linked_polygon;
use crate::collections::H3CompactedVec;
use crate::Index;

pub trait ToPolygon {
    fn to_polygon(&self) -> Polygon<f64>;
}

pub trait ToCoordinate {
    fn to_coordinate(&self) -> Coordinate<f64>;
}

/// join hexagon polygons to larger polygons where hexagons are touching each other
pub trait ToLinkedPolygons {
    fn to_linked_polygons(&self, smoothen: bool) -> Vec<Polygon<f64>>;
}

impl ToLinkedPolygons for Vec<Index> {
    fn to_linked_polygons(&self, smoothen: bool) -> Vec<Polygon<f64>> {
        let mut h3indexes: Vec<_> = self.iter().map(|i| i.h3index()).collect();
        h3indexes.sort_unstable();
        h3indexes.dedup();
        to_linked_polygons(&h3indexes, smoothen)
    }
}

impl ToLinkedPolygons for H3CompactedVec {
    fn to_linked_polygons(&self, smoothen: bool) -> Vec<Polygon<f64>> {
        if let Some(res) = self.finest_resolution_contained() {
            let mut h3indexes: Vec<_> = self.iter_uncompacted_indexes(res).collect();
            h3indexes.sort_unstable();
            h3indexes.dedup();
            to_linked_polygons(&h3indexes, smoothen)
        } else {
            vec![]
        }
    }
}

/// join hexagon polygons to larger polygons where hexagons are touching each other
///
/// The indexes will be grouped by the `align_to_h3_resolution`, so this will generate polygons
/// not exceeding the area of that parent resolution.
///
/// Corners will be aligned to the corners of the parent resolution when they are less than an
/// edge length away from them. This is to avoid gaps when `smoothen` is set to true.
///
/// This algorithm still needs some optimization to improve the runtime.
pub trait ToAlignedLinkedPolygons {
    fn to_aligned_linked_polygons(&self, align_to_h3_resolution: u8, smoothen: bool) -> Vec<Polygon<f64>>;
}

impl ToAlignedLinkedPolygons for Vec<Index> {
    fn to_aligned_linked_polygons(&self, align_to_h3_resolution: u8, smoothen: bool) -> Vec<Polygon<f64>> {
        let mut h3indexes_grouped = HashMap::new();
        for i in self.iter() {
            let parent = i.get_parent(align_to_h3_resolution);
            h3indexes_grouped.entry(parent)
                .or_insert_with(Vec::new)
                .push(i.h3index())
        }

        let mut polygons = Vec::new();
        for (parent_index, h3indexes) in h3indexes_grouped.drain() {
            if smoothen {
                //
                // align to the corners of the parent index
                //

                let parent_poly_vertices: Vec<_> = parent_index.to_polygon()
                    .exterior().0.iter()
                    .map(|c| Point::from(*c))
                    .collect();

                // edge length of the child indexes
                let edge_length = {
                    let ring= Index::from(h3indexes[0]).to_polygon();
                    let p1 = Point::from(ring.exterior().0[0]);
                    let p2 = Point::from(ring.exterior().0[1]);
                    p1.euclidean_distance(&p2)
                };

                for poly in to_linked_polygons(&h3indexes, true).drain(..) {
                    let points_new: Vec<_> = poly.exterior().0.iter().map(|c| {
                        let p = Point::from(*c);
                        if let Some(pv) = parent_poly_vertices.iter().find(|pv| p.euclidean_distance(*pv) < edge_length) {
                            pv.0
                        } else {
                            *c
                        }
                    })
                        .collect();
                    polygons.push(Polygon::new(LineString::from(points_new), poly.interiors().to_vec()));
                }
            } else {
                polygons.append(&mut to_linked_polygons(&h3indexes, false));
            }
        }
        polygons
    }
}

/// convert raw h3indexes to linked polygons
///
/// With `smoothen` an optional smoothing can be applied to the polygons to remove
/// H3 artifacts.
///
/// for this case, the slice must already be deduplicated, and all h3 indexes must be the same resolutions
pub fn to_linked_polygons(h3indexes: &[H3Index], smoothen: bool) -> Vec<Polygon<f64>> {
    if h3indexes.is_empty() {
        return vec![];
    }
    unsafe {
        let mut lgp = LinkedGeoPolygon {
            first: std::ptr::null_mut(),
            last: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
        };
        h3SetToLinkedGeo(h3indexes.as_ptr(), h3indexes.len() as c_int, &mut lgp);

        let mut polygons = vec![];
        let mut cur_linked_geo_polygon = Some(&lgp);
        while let Some(poly) = cur_linked_geo_polygon.as_ref() {
            let mut exterior = None;
            let mut interiors = vec![];
            let mut linked_loop_i = 0;
            let mut cur_linked_geo_loop = poly.first.as_ref();
            while let Some(linked_loop) = cur_linked_geo_loop {
                let mut coordinates = vec![];
                let mut cur_linked_geo_coord = linked_loop.first.as_ref();
                while let Some(linked_coord) = cur_linked_geo_coord {
                    coordinates.push((
                        radsToDegs(linked_coord.vertex.lon),
                        radsToDegs(linked_coord.vertex.lat),
                    ));
                    cur_linked_geo_coord = linked_coord.next.as_ref();
                }

                if coordinates.len() >= 3 {
                    let linestring = LineString::from(coordinates);
                    if linked_loop_i == 0 {
                        exterior = Some(linestring)
                    } else {
                        interiors.push(linestring)
                    }
                }

                linked_loop_i += 1;
                cur_linked_geo_loop = linked_loop.next.as_ref();
            }
            if let Some(ext) = exterior {
                let poly = Polygon::new(ext, interiors);
                if smoothen {
                    polygons.push(smoothen_h3_linked_polygon(&poly));
                } else {
                    polygons.push(poly);
                }
            }
            cur_linked_geo_polygon = poly.next.as_ref();
        }
        destroyLinkedPolygon(&mut lgp);
        polygons
    }
}

#[cfg(test)]
mod tests {
    use geo_types::Coordinate;

    use crate::{
        Index,
        ToLinkedPolygons,
    };

    #[test]
    fn donut_linked_polygon() {
        let ring = Index::from_coordinate(&Coordinate::from((23.3, 12.3)), 6)
            .hex_ring(1)
            .unwrap();
        let polygons = ring.to_linked_polygons(false);
        assert_eq!(polygons.len(), 1);
        assert_eq!(polygons[0].exterior().0.len(), 19);
        assert_eq!(polygons[0].interiors().len(), 1);
        assert_eq!(polygons[0].interiors()[0].0.len(), 7);
    }
}
