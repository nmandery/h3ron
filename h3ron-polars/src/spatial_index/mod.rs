//! Spatial search indexes
//!
//! For some background on spatial search algorithms see [A dive into spatial search algorithms](https://blog.mapbox.com/a-dive-into-spatial-search-algorithms-ebd0c5e39d2a).
//!

#[cfg(feature = "si_kdtree")]
pub mod kdtree;

#[cfg(feature = "si_rtree")]
pub mod rtree;

#[cfg(feature = "si_packed_hilbert_rtree")]
pub mod packed_hilbert_rtree;

use crate::{Error, IndexChunked, IndexValue};
use geo::bounding_rect::BoundingRect;
use geo::{Contains, Intersects};
use geo_types::{Coordinate, Line, MultiPolygon, Polygon, Rect};
use h3ron::to_geo::ToLine;
use h3ron::{H3Cell, H3DirectedEdge, ToCoordinate, ToPolygon};
use polars::export::arrow::array::BooleanArray;
use polars::export::arrow::bitmap::{Bitmap, MutableBitmap};
use polars::prelude::{ArrowDataType, BooleanChunked};
use polars_core::prelude::{TakeRandom, UInt64Chunked};

#[cfg(feature = "si_kdtree")]
pub use crate::spatial_index::kdtree::*;

#[cfg(feature = "si_rtree")]
pub use crate::spatial_index::rtree::*;

#[cfg(feature = "si_packed_hilbert_rtree")]
pub use crate::spatial_index::packed_hilbert_rtree::*;

/// marker trait to restrict on what kind of geometries a spatial index
/// operates.
pub trait SIKind {}

pub struct CoordinateSIKind {}
impl SIKind for CoordinateSIKind {}

pub struct RectSIKind {}
impl SIKind for RectSIKind {}

pub trait SpatialIndex<IX: IndexValue, Kind: SIKind> {
    fn h3indexchunked(&self) -> IndexChunked<IX>;

    /// internal
    fn envelopes_intersect_impl(&self, rect: &Rect) -> MutableBitmap;

    /// The envelope of the indexed elements has some overlap with the given `rect`
    fn envelopes_intersect(&self, rect: &Rect) -> BooleanChunked {
        finish_mask(
            self.envelopes_intersect_impl(rect).into(),
            &self.h3indexchunked(),
        )
    }

    /// The envelope of the indexed elements is with `distance` of the given `Coordinate` `coord`.
    fn envelopes_within_distance(&self, coord: Coordinate, distance: f64) -> BooleanChunked;
}

pub trait SpatialIndexGeomOp<IX: IndexValue, Kind: SIKind> {
    /// The geometry of the indexed elements is with in the given `Rect`
    fn geometries_intersect(&self, rect: &Rect) -> BooleanChunked;

    /// The geometry of the indexed elements is with in the given `Polygon`
    fn geometries_intersect_polygon(&self, polygon: &Polygon) -> BooleanChunked;

    /// The geometry of the indexed elements is with in the given `MultiPolygon`
    fn geometries_intersect_multipolygon(&self, multipolygon: &MultiPolygon) -> BooleanChunked;
}

impl<T, IX: IndexValue> SpatialIndexGeomOp<IX, CoordinateSIKind> for T
where
    T: SpatialIndex<IX, CoordinateSIKind>,
    IX: CoordinateIndexable,
{
    fn geometries_intersect(&self, rect: &Rect) -> BooleanChunked {
        self.envelopes_intersect(rect) // index only works with points, so this is the same
    }

    fn geometries_intersect_polygon(&self, polygon: &Polygon) -> BooleanChunked {
        let mask = if let Some(rect) = polygon.bounding_rect() {
            let mask = self.envelopes_intersect_impl(&rect);
            intersect_coordinates_with_polygon(mask, &self.h3indexchunked(), polygon)
        } else {
            negative_mask(self.h3indexchunked().chunked_array)
        };
        finish_mask(mask.into(), &self.h3indexchunked())
    }

    fn geometries_intersect_multipolygon(&self, multipolygon: &MultiPolygon) -> BooleanChunked {
        let mask = multipolygon
            .0
            .iter()
            .map(|polygon| {
                if let Some(rect) = polygon.bounding_rect() {
                    let mask = self.envelopes_intersect_impl(&rect);
                    Some(intersect_coordinates_with_polygon(
                        mask,
                        &self.h3indexchunked(),
                        polygon,
                    ))
                } else {
                    None
                }
            })
            .fold(
                negative_mask(self.h3indexchunked().chunked_array),
                |acc_mask, mask| match mask {
                    Some(mask) => acc_mask | &(mask.into()),
                    None => acc_mask,
                },
            );
        finish_mask(mask.into(), &self.h3indexchunked())
    }
}

impl<T, IX: IndexValue> SpatialIndexGeomOp<IX, RectSIKind> for T
where
    T: SpatialIndex<IX, RectSIKind>,
    IX: RectIndexable,
{
    fn geometries_intersect(&self, rect: &Rect) -> BooleanChunked {
        // todo: comparing directly with rect is probably cheaper than polygon
        self.geometries_intersect_polygon(&rect.to_polygon())
    }

    fn geometries_intersect_polygon(&self, polygon: &Polygon) -> BooleanChunked {
        let mask = if let Some(rect) = polygon.bounding_rect() {
            let mask = self.envelopes_intersect_impl(&rect);
            intersect_geometries_with_polygon(mask, &self.h3indexchunked(), polygon)
        } else {
            negative_mask(self.h3indexchunked().chunked_array)
        };
        finish_mask(mask.into(), &self.h3indexchunked())
    }

    fn geometries_intersect_multipolygon(&self, multipolygon: &MultiPolygon) -> BooleanChunked {
        let mask = multipolygon
            .0
            .iter()
            .map(|polygon| {
                if let Some(rect) = polygon.bounding_rect() {
                    let mask = self.envelopes_intersect_impl(&rect);
                    Some(intersect_geometries_with_polygon(
                        mask,
                        &self.h3indexchunked(),
                        polygon,
                    ))
                } else {
                    None
                }
            })
            .fold(
                negative_mask(self.h3indexchunked().chunked_array),
                |acc_mask, mask| match mask {
                    Some(mask) => acc_mask | &(mask.into()),
                    None => acc_mask,
                },
            );
        finish_mask(mask.into(), &self.h3indexchunked())
    }
}

pub trait CoordinateIndexable {
    /// coordinate to use for spatial indexing
    fn spatial_index_coordinate(&self) -> Result<Coordinate, Error>;
}

impl CoordinateIndexable for H3Cell {
    fn spatial_index_coordinate(&self) -> Result<Coordinate, Error> {
        self.to_coordinate().map_err(Error::from)
    }
}

impl CoordinateIndexable for H3DirectedEdge {
    fn spatial_index_coordinate(&self) -> Result<Coordinate, Error> {
        let cells = self.cells()?;
        let c1 = cells.destination.to_coordinate()?;
        let c2 = cells.origin.to_coordinate()?;
        Ok(((c1.x + c2.x) / 2.0, (c1.y + c2.y) / 2.0).into())
    }
}

pub trait RectIndexable {
    type GeomType;

    fn spatial_index_rect(&self) -> Result<Option<Rect>, Error>;
    fn spatial_index_geometry(&self) -> Result<Self::GeomType, Error>;
    fn intersects_with_polygon(&self, poly: &Polygon) -> Result<bool, Error>;
}

impl RectIndexable for H3Cell {
    type GeomType = Polygon;

    fn spatial_index_rect(&self) -> Result<Option<Rect>, Error> {
        Ok(self.spatial_index_geometry()?.bounding_rect())
    }

    fn spatial_index_geometry(&self) -> Result<Self::GeomType, Error> {
        Ok(self.to_polygon()?)
    }

    fn intersects_with_polygon(&self, poly: &Polygon) -> Result<bool, Error> {
        Ok(poly.intersects(&self.spatial_index_geometry()?))
    }
}

impl RectIndexable for H3DirectedEdge {
    type GeomType = Line;

    fn spatial_index_rect(&self) -> Result<Option<Rect>, Error> {
        Ok(Some(self.spatial_index_geometry()?.bounding_rect()))
    }

    fn spatial_index_geometry(&self) -> Result<Self::GeomType, Error> {
        Ok(self.to_line()?)
    }

    fn intersects_with_polygon(&self, poly: &Polygon) -> Result<bool, Error> {
        Ok(poly.intersects(&self.spatial_index_geometry()?))
    }
}

pub(crate) fn negative_mask(ca: &UInt64Chunked) -> MutableBitmap {
    let mut mask = MutableBitmap::new();
    mask.extend_constant(ca.len(), false);
    mask
}

pub(crate) fn finish_mask<IX: IndexValue>(mask: Bitmap, ic: &IndexChunked<IX>) -> BooleanChunked {
    let validites = ic.validity_bitmap();
    let bool_arr = BooleanArray::from_data(ArrowDataType::Boolean, mask, Some(validites));
    BooleanChunked::from(bool_arr)
}

pub(crate) fn intersect_geometries_with_polygon<IX>(
    mut mask: MutableBitmap,
    indexchunked: &IndexChunked<IX>,
    polygon: &Polygon,
) -> MutableBitmap
where
    IX: RectIndexable + IndexValue,
{
    for i in 0..mask.len() {
        if mask.get(i) {
            if let Some(index) = indexchunked.get(i) {
                match index.intersects_with_polygon(polygon) {
                    Ok(true) => mask.set(i, true),
                    _ => (),
                }
            }
        }
    }
    mask
}

pub(crate) fn intersect_coordinates_with_polygon<IX>(
    mut mask: MutableBitmap,
    indexchunked: &IndexChunked<IX>,
    polygon: &Polygon,
) -> MutableBitmap
where
    IX: CoordinateIndexable + IndexValue,
{
    for i in 0..mask.len() {
        if mask.get(i) {
            if let Some(index) = indexchunked.get(i) {
                match index.spatial_index_coordinate() {
                    Ok(coord) => {
                        if polygon.contains(&coord) {
                            mask.set(i, true)
                        }
                    }
                    _ => (),
                }
            }
        }
    }
    mask
}
