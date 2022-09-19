use crate::collections::H3CellMap;
use std::os::raw::c_int;

use geo::algorithm::euclidean_distance::EuclideanDistance;
use geo_types::{Coordinate, Line, LineString, MultiLineString, Point, Polygon};

use h3ron_h3_sys::H3Index;

use crate::algorithm::smoothen_h3_linked_polygon;
use crate::collections::indexvec::IndexVec;
use crate::collections::CompactedCellVec;
use crate::{Error, H3Cell};

pub trait ToPolygon {
    type Error;

    fn to_polygon(&self) -> Result<Polygon<f64>, Self::Error>;
}

pub trait ToCoordinate {
    type Error;

    fn to_coordinate(&self) -> Result<Coordinate<f64>, Self::Error>;
}

pub trait ToLine {
    type Error;

    fn to_line(&self) -> Result<Line<f64>, Self::Error>;
}

pub trait ToLineString {
    type Error;

    fn to_linestring(&self) -> Result<LineString<f64>, Self::Error>;
}

pub trait ToMultiLineString {
    type Error;

    fn to_multilinestring(&self) -> Result<MultiLineString<f64>, Self::Error>;
}

/// join hexagon polygons to larger polygons where hexagons are touching each other
pub trait ToLinkedPolygons {
    type Error;

    fn to_linked_polygons(&self, smoothen: bool) -> Result<Vec<Polygon<f64>>, Self::Error>;
}

impl ToLinkedPolygons for Vec<H3Cell> {
    type Error = Error;

    fn to_linked_polygons(&self, smoothen: bool) -> Result<Vec<Polygon<f64>>, Self::Error> {
        let mut cells = self.clone();
        cells.sort_unstable();
        cells.dedup();
        to_linked_polygons(&cells, smoothen)
    }
}

impl ToLinkedPolygons for IndexVec<H3Cell> {
    type Error = Error;

    fn to_linked_polygons(&self, smoothen: bool) -> Result<Vec<Polygon<f64>>, Self::Error> {
        let mut cells = self.iter().collect::<Vec<_>>();
        cells.sort_unstable();
        cells.dedup();
        to_linked_polygons(&cells, smoothen)
    }
}

impl ToLinkedPolygons for CompactedCellVec {
    type Error = Error;

    fn to_linked_polygons(&self, smoothen: bool) -> Result<Vec<Polygon<f64>>, Self::Error> {
        match self.finest_resolution_contained() {
            Some(resolution) => {
                let mut cells: Vec<_> = self
                    .iter_uncompacted_cells(resolution)
                    .collect::<Result<Vec<_>, _>>()?;
                cells.sort_unstable();
                cells.dedup();
                to_linked_polygons(&cells, smoothen)
            }
            None => Ok(Vec::new()),
        }
    }
}

/// join hexagon polygons to larger polygons where hexagons are touching each other
///
/// The cells will be grouped by the `align_to_h3_resolution`, so this will generate polygons
/// not exceeding the area of that parent resolution.
///
/// Corners will be aligned to the corners of the parent resolution when they are less than an
/// edge length away from them. This is to avoid gaps when `smoothen` is set to true.
///
/// This algorithm still needs some optimization to improve the runtime.
pub trait ToAlignedLinkedPolygons {
    type Error;

    fn to_aligned_linked_polygons(
        &self,
        align_to_h3_resolution: u8,
        smoothen: bool,
    ) -> Result<Vec<Polygon<f64>>, Self::Error>;
}

impl ToAlignedLinkedPolygons for Vec<H3Cell> {
    type Error = Error;

    fn to_aligned_linked_polygons(
        &self,
        align_to_h3_resolution: u8,
        smoothen: bool,
    ) -> Result<Vec<Polygon<f64>>, Self::Error> {
        let mut cells_grouped = H3CellMap::default();
        for cell in self.iter() {
            let parent_cell = cell.get_parent(align_to_h3_resolution)?;
            cells_grouped
                .entry(parent_cell)
                .or_insert_with(Self::new)
                .push(*cell);
        }

        let mut polygons = Vec::new();
        for (parent_cell, cells) in cells_grouped.drain() {
            if smoothen {
                //
                // align to the corners of the parent index
                //

                let parent_poly_vertices: Vec<_> = parent_cell
                    .to_polygon()?
                    .exterior()
                    .0
                    .iter()
                    .map(|c| Point::from(*c))
                    .collect();

                // edge length of the child indexes
                let edge_length = {
                    let ring = cells[0].to_polygon()?;
                    let p1 = Point::from(ring.exterior().0[0]);
                    let p2 = Point::from(ring.exterior().0[1]);
                    p1.euclidean_distance(&p2)
                };

                for poly in to_linked_polygons(&cells, true)?.drain(..) {
                    let points_new: Vec<_> = poly
                        .exterior()
                        .0
                        .iter()
                        .map(|c| {
                            let p = Point::from(*c);
                            parent_poly_vertices
                                .iter()
                                .find(|pv| p.euclidean_distance(*pv) < edge_length)
                                .map_or_else(|| *c, |pv| pv.0)
                        })
                        .collect();
                    polygons.push(Polygon::new(
                        LineString::from(points_new),
                        poly.interiors().to_vec(),
                    ));
                }
            } else {
                polygons.append(&mut to_linked_polygons(&cells, false)?);
            }
        }
        Ok(polygons)
    }
}

/// convert cells to linked polygons
///
/// With `smoothen` an optional smoothing can be applied to the polygons to remove
/// H3 artifacts.
///
/// for this case, the slice must already be deduplicated, and all h3 cells must be the same resolutions
pub fn to_linked_polygons(cells: &[H3Cell], smoothen: bool) -> Result<Vec<Polygon<f64>>, Error> {
    if cells.is_empty() {
        return Ok(vec![]);
    }
    unsafe {
        let mut lgp = h3ron_h3_sys::LinkedGeoPolygon {
            first: std::ptr::null_mut(),
            last: std::ptr::null_mut(),
            next: std::ptr::null_mut(),
        };
        // the following requires `repr(transparent)` on H3Cell
        let h3index_slice =
            std::slice::from_raw_parts(cells.as_ptr().cast::<H3Index>(), cells.len());
        Error::check_returncode(h3ron_h3_sys::cellsToLinkedMultiPolygon(
            h3index_slice.as_ptr(),
            h3index_slice.len() as c_int,
            &mut lgp,
        ))?;

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
                        (linked_coord.vertex.lng as f64).to_degrees(),
                        (linked_coord.vertex.lat as f64).to_degrees(),
                    ));
                    cur_linked_geo_coord = linked_coord.next.as_ref();
                }

                if coordinates.len() >= 3 {
                    let linestring = LineString::from(coordinates);
                    if linked_loop_i == 0 {
                        exterior = Some(linestring);
                    } else {
                        interiors.push(linestring);
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
        h3ron_h3_sys::destroyLinkedMultiPolygon(&mut lgp);
        Ok(polygons)
    }
}

#[cfg(test)]
mod tests {
    use geo_types::Coordinate;

    use crate::{H3Cell, ToLinkedPolygons};

    #[test]
    fn donut_linked_polygon() {
        let ring = H3Cell::from_coordinate(Coordinate::from((23.3, 12.3)), 6)
            .unwrap()
            .grid_ring_unsafe(1)
            .unwrap();
        let polygons = ring.to_linked_polygons(false).unwrap();
        assert_eq!(polygons.len(), 1);
        assert_eq!(polygons[0].exterior().0.len(), 19);
        assert_eq!(polygons[0].interiors().len(), 1);
        assert_eq!(polygons[0].interiors()[0].0.len(), 7);
    }
}
