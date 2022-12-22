//! Spatial search indexes
//!
//! For some background on spatial search algorithms see [A dive into spatial search algorithms](https://blog.mapbox.com/a-dive-into-spatial-search-algorithms-ebd0c5e39d2a).
//!
//! Available implementations:
//! * `KDTreeIndex`: Fast to create and query, works only on centroids.
//! * `PackedHilbertRTreeIndex`: Still fast to create and query, works on envelopes. Creation is a bit slower than `KDTreeIndex`
//! * `RTreeIndex`: Also envelope based.
//!
//! All of the above spatial indexes provide a second stage which can perform fine-grained filtering
//! by doing geometry intersections on the returned index-matches using the exact geometry of the indexed
//! entity and the queried `Polygon` or `MultiPolygon`. See the [SpatialIndexGeomOp] trait.
//! This of course comes with an additional runtime cost.
//!
//! For a more detailed comparison of the runtime characteristics see the included `spatialindex` benchmark.

#[cfg(feature = "si_kdtree")]
pub mod kdtree;

#[cfg(feature = "si_rtree")]
pub mod rtree;

#[cfg(feature = "si_packed_hilbert_rtree")]
pub mod packed_hilbert_rtree;

#[cfg(test)]
pub(crate) mod tests;

use crate::{Error, IndexChunked, IndexValue};
use geo::bounding_rect::BoundingRect;
use geo::{Contains, Intersects};
use geo_types::{Coord, MultiPolygon, Polygon, Rect};
use h3ron::to_geo::ToLine;
use h3ron::{H3Cell, H3DirectedEdge, ToCoordinate, ToPolygon};
use polars::export::arrow::array::BooleanArray;
use polars::export::arrow::bitmap::{Bitmap, MutableBitmap};
use polars::prelude::BooleanChunked;
use polars_core::prelude::{FromData, TakeRandom, UInt64Chunked};

#[cfg(feature = "si_kdtree")]
pub use crate::spatial_index::kdtree::*;

#[cfg(feature = "si_rtree")]
pub use crate::spatial_index::rtree::*;

#[cfg(feature = "si_packed_hilbert_rtree")]
pub use crate::spatial_index::packed_hilbert_rtree::*;

/// marker trait to restrict on what kind of geometries a spatial index
/// operates.
pub trait SIKind {}

/// Marks spatial indexes operating on [Coord] points
pub struct CoordinateSIKind {}
impl SIKind for CoordinateSIKind {}

/// Marks spatial indexes operating on [Rect] envelopes
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

    /// The envelope of the indexed elements is with `distance` of the given [Coord] `coord`.
    fn envelopes_within_distance(&self, coord: Coord, distance: f64) -> BooleanChunked;
}

pub trait SpatialIndexGeomOp<IX: IndexValue, Kind: SIKind> {
    /// The geometry of the indexed elements is with in the given [Rect]
    fn geometries_intersect(&self, rect: &Rect) -> BooleanChunked;

    /// The geometry of the indexed elements is with in the given [Polygon]
    fn geometries_intersect_polygon(&self, polygon: &Polygon) -> BooleanChunked;

    /// The geometry of the indexed elements is with in the given [MultiPolygon]
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
        geometries_intersect_polygon(self, polygon, validate_coordinate_containment)
    }

    fn geometries_intersect_multipolygon(&self, multipolygon: &MultiPolygon) -> BooleanChunked {
        geometries_intersect_multipolygon(self, multipolygon, validate_coordinate_containment)
    }
}

impl<T, IX: IndexValue> SpatialIndexGeomOp<IX, RectSIKind> for T
where
    T: SpatialIndex<IX, RectSIKind>,
    IX: RectIndexable,
{
    fn geometries_intersect(&self, rect: &Rect) -> BooleanChunked {
        let mask = self.envelopes_intersect_impl(rect);
        let ic = self.h3indexchunked();
        finish_mask(
            validate_geometry_intersection(mask, &ic, &rect.to_polygon()).into(),
            &ic,
        )
    }

    fn geometries_intersect_polygon(&self, polygon: &Polygon) -> BooleanChunked {
        geometries_intersect_polygon(self, polygon, validate_geometry_intersection)
    }

    fn geometries_intersect_multipolygon(&self, multipolygon: &MultiPolygon) -> BooleanChunked {
        geometries_intersect_multipolygon(self, multipolygon, validate_geometry_intersection)
    }
}

pub trait CoordinateIndexable {
    /// coordinate to use for spatial indexing
    fn spatial_index_coordinate(&self) -> Result<Coord, Error>;
}

impl CoordinateIndexable for H3Cell {
    fn spatial_index_coordinate(&self) -> Result<Coord, Error> {
        self.to_coordinate().map_err(Error::from)
    }
}

impl CoordinateIndexable for H3DirectedEdge {
    fn spatial_index_coordinate(&self) -> Result<Coord, Error> {
        let cells = self.cells()?;
        let c1 = cells.destination.to_coordinate()?;
        let c2 = cells.origin.to_coordinate()?;
        Ok(((c1.x + c2.x) / 2.0, (c1.y + c2.y) / 2.0).into())
    }
}

pub trait RectIndexable {
    fn spatial_index_rect(&self) -> Result<Option<Rect>, Error>;
    fn intersects_with_polygon(&self, poly: &Polygon) -> Result<bool, Error>;
}

impl RectIndexable for H3Cell {
    fn spatial_index_rect(&self) -> Result<Option<Rect>, Error> {
        Ok(self.to_polygon()?.bounding_rect())
    }

    fn intersects_with_polygon(&self, poly: &Polygon) -> Result<bool, Error> {
        Ok(poly.intersects(&self.to_polygon()?))
    }
}

impl RectIndexable for H3DirectedEdge {
    fn spatial_index_rect(&self) -> Result<Option<Rect>, Error> {
        Ok(Some(self.to_line()?.bounding_rect()))
    }

    fn intersects_with_polygon(&self, poly: &Polygon) -> Result<bool, Error> {
        Ok(poly.intersects(&self.to_line()?))
    }
}

pub(crate) fn negative_mask(ca: &UInt64Chunked) -> MutableBitmap {
    let mut mask = MutableBitmap::new();
    mask.extend_constant(ca.len(), false);
    mask
}

pub(crate) fn finish_mask<IX: IndexValue>(mask: Bitmap, ic: &IndexChunked<IX>) -> BooleanChunked {
    let validites = ic.validity_bitmap();
    let bool_arr = BooleanArray::from_data_default(mask, Some(validites));
    BooleanChunked::from(bool_arr)
}

fn geometries_intersect_polygon<IX: IndexValue, SI, Kind, Validator>(
    spatial_index: &SI,
    polygon: &Polygon,
    validator: Validator,
) -> BooleanChunked
where
    SI: SpatialIndex<IX, Kind>,
    Kind: SIKind,
    Validator: Fn(MutableBitmap, &IndexChunked<IX>, &Polygon) -> MutableBitmap,
{
    let mask = if let Some(rect) = polygon.bounding_rect() {
        let mask = spatial_index.envelopes_intersect_impl(&rect);
        validator(mask, &spatial_index.h3indexchunked(), polygon)
    } else {
        negative_mask(spatial_index.h3indexchunked().chunked_array)
    };
    finish_mask(mask.into(), &spatial_index.h3indexchunked())
}

fn geometries_intersect_multipolygon<IX: IndexValue, SI, Kind, Validator>(
    spatial_index: &SI,
    multipolygon: &MultiPolygon,
    validator: Validator,
) -> BooleanChunked
where
    SI: SpatialIndex<IX, Kind>,
    Kind: SIKind,
    Validator: Fn(MutableBitmap, &IndexChunked<IX>, &Polygon) -> MutableBitmap,
{
    let mask = multipolygon
        .0
        .iter()
        .filter_map(|polygon| {
            if let Some(rect) = polygon.bounding_rect() {
                let mask = spatial_index.envelopes_intersect_impl(&rect);
                Some(validator(mask, &spatial_index.h3indexchunked(), polygon))
            } else {
                None
            }
        })
        .fold(
            negative_mask(spatial_index.h3indexchunked().chunked_array),
            |acc_mask, mask| acc_mask | &(mask.into()),
        );
    finish_mask(mask.into(), &spatial_index.h3indexchunked())
}

pub(crate) fn validate_geometry_intersection<IX>(
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
                if let Ok(true) = index.intersects_with_polygon(polygon) {
                    mask.set(i, true)
                }
            }
        }
    }
    mask
}

pub(crate) fn validate_coordinate_containment<IX>(
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
                if let Ok(coord) = index.spatial_index_coordinate() {
                    if polygon.contains(&coord) {
                        mask.set(i, true)
                    }
                }
            }
        }
    }
    mask
}
