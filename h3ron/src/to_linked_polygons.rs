use std::os::raw::c_int;

use geo_types::{LineString, Polygon};

use h3ron_h3_sys::{
    destroyLinkedPolygon,
    H3Index,
    h3SetToLinkedGeo,
    LinkedGeoPolygon,
    radsToDegs,
};

use crate::collections::H3CompactedVec;
use crate::Index;

pub trait ToLinkedPolygons {
    fn to_linked_polygons(&self) -> Vec<Polygon<f64>>;
}

impl ToLinkedPolygons for Vec<Index> {
    fn to_linked_polygons(&self) -> Vec<Polygon<f64>> {
        let mut h3indexes: Vec<_> = self.iter().map(|i| i.h3index()).collect();
        h3indexes.sort_unstable();
        h3indexes.dedup();
        to_linked_polygons(&h3indexes)
    }
}

impl ToLinkedPolygons for H3CompactedVec {
    fn to_linked_polygons(&self) -> Vec<Polygon<f64>> {
        if let Some(res) = self.finest_resolution_contained() {
            let mut h3indexes: Vec<_> = self.iter_uncompacted_indexes(res).collect();
            h3indexes.sort_unstable();
            h3indexes.dedup();
            to_linked_polygons(&h3indexes)
        } else {
            vec![]
        }
    }
}

/// convert raw h3indexes to linked polygons
///
/// for this case, the slice must already be deduplicated
pub fn to_linked_polygons(h3indexes: &[H3Index]) -> Vec<Polygon<f64>> {
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
                polygons.push(Polygon::new(ext, interiors));
            }
            cur_linked_geo_polygon = poly.next.as_ref();
        }
        destroyLinkedPolygon(&mut lgp);
        polygons
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        Index,
        ToLinkedPolygons,
    };
    use geo_types::Coordinate;

    #[test]
    fn donut_linked_polygon() {
        let ring = Index::from_coordinate(&Coordinate::from((23.3, 12.3)), 6)
            .hex_ring(1)
            .unwrap();
        let polygons = ring.to_linked_polygons();
        assert_eq!(polygons.len(), 1);
        assert_eq!(polygons[0].exterior().0.len(), 19);
        assert_eq!(polygons[0].interiors().len(), 1);
        assert_eq!(polygons[0].interiors()[0].0.len(), 7);
    }
}
