use crate::spatial_index::{
    finish_mask, negative_mask, CoordinateIndexable, CoordinateSIKind, SpatialIndex,
};
use crate::{AsH3IndexChunked, Error, IndexChunked, IndexValue};
use geo::{BoundingRect, Contains};
use geo_types::{Coordinate, MultiPolygon, Polygon, Rect};
use h3ron::{H3Cell, H3DirectedEdge, ToCoordinate};
use kdbush::{KDBush, PointReader};
use polars::datatypes::ArrowDataType;
use polars::export::arrow::array::BooleanArray;
use polars::export::arrow::bitmap::{Bitmap, MutableBitmap};
use polars::prelude::{BooleanChunked, TakeRandom};
use polars_core::prelude::UInt64Chunked;
use std::marker::PhantomData;

impl<'a, IX> PointReader for IndexChunked<'a, IX>
where
    IX: IndexValue + CoordinateIndexable,
{
    fn size_hint(&self) -> usize {
        self.len()
    }

    fn visit_all<F>(&self, mut visitor: F)
    where
        F: FnMut(usize, f64, f64),
    {
        for (id, maybe_point) in self.iter_indexes_validated().enumerate() {
            if let Some(Ok(point)) = maybe_point {
                if let Ok(coord) = point.spatial_index_coordinate() {
                    visitor(id, coord.x, coord.y);
                }
            }
        }
    }
}

pub trait BuildKDTreeIndex<'a, IX>
where
    IX: IndexValue + CoordinateIndexable,
{
    /// Build a [`KDTreeIndex`] using the default parameters.
    ///
    /// # Example
    ///
    /// ```
    /// use geo_types::Rect;
    /// use polars::prelude::UInt64Chunked;
    /// use polars_core::prelude::TakeRandom;
    /// use h3ron::{H3Cell, Index};
    /// use h3ron_polars::{AsH3CellChunked, NamedFromIndexes};
    /// use h3ron_polars::spatial_index::{BuildKDTreeIndex, SpatialIndex, SpatialIndexGeomOp};
    ///
    /// let ca = UInt64Chunked::new_from_indexes(
    ///     "",
    ///     vec![
    ///         H3Cell::from_coordinate((45.5, 45.5).into(), 7).unwrap(),
    ///         H3Cell::from_coordinate((-60.5, -60.5).into(), 7).unwrap(),
    ///         H3Cell::from_coordinate((120.5, 70.5).into(), 7).unwrap(),
    ///     ],
    /// );
    ///
    /// let kdbush = ca.h3cell().kdtree_index();
    /// let mask = kdbush.geometries_intersect(&Rect::new((40.0, 40.0), (50.0, 50.0)));
    ///
    /// assert_eq!(mask.len(), 3);
    /// assert_eq!(mask.get(0), Some(true));
    /// assert_eq!(mask.get(1), Some(false));
    /// assert_eq!(mask.get(2), Some(false));
    /// ```
    fn kdtree_index(&self) -> KDTreeIndex<IX> {
        self.kdtree_index_with_node_size(64)
    }

    /// Build a [`KDTreeIndex`] using custom parameters.
    ///
    /// `node_size` - Size of the KD-tree node, 64 by default. Higher means faster indexing but slower search, and vise versa
    fn kdtree_index_with_node_size(&self, node_size: u8) -> KDTreeIndex<IX>;
}

impl<'a, IX: IndexValue> BuildKDTreeIndex<'a, IX> for IndexChunked<'a, IX>
where
    Self: PointReader,
    IX: CoordinateIndexable,
{
    fn kdtree_index_with_node_size(&self, node_size: u8) -> KDTreeIndex<IX> {
        KDTreeIndex {
            index_phantom: PhantomData::<IX>::default(),

            // clones of arrow-backed arrays are cheap, so we clone this for the benefit of not
            // requiring a lifetime dependency
            chunked_array: self.chunked_array.clone(),

            kdbush: KDBush::create((*self).clone(), node_size),
        }
    }
}

/// A very fast static spatial index for 2D points based on a flat KD-tree.
///
/// Operates on the centroid coordinate of [`H3Cell`] and [`H3DirectedEdge`] values.
pub struct KDTreeIndex<IX: IndexValue> {
    index_phantom: PhantomData<IX>,

    chunked_array: UInt64Chunked,

    pub kdbush: KDBush,
}

impl<IX: IndexValue> SpatialIndex<IX, CoordinateSIKind> for KDTreeIndex<IX>
where
    IX: CoordinateIndexable,
{
    fn h3indexchunked(&self) -> IndexChunked<IX> {
        self.chunked_array.h3indexchunked()
    }

    fn envelopes_intersect_impl(&self, rect: &Rect) -> MutableBitmap {
        let mut mask = negative_mask(&self.chunked_array);
        self.kdbush.range(
            rect.min().x,
            rect.min().y,
            rect.max().x,
            rect.max().y,
            |id| mask.set(id, true),
        );
        mask
    }

    fn envelopes_within_distance(&self, coord: Coordinate, distance: f64) -> BooleanChunked {
        let mut mask = negative_mask(&self.chunked_array);
        self.kdbush
            .within(coord.x, coord.y, distance, |id| mask.set(id, true));
        finish_mask(mask.into(), &self.h3indexchunked())
    }
}

#[cfg(test)]
mod test {
    use crate::from::NamedFromIndexes;
    use crate::spatial_index::kdtree::BuildKDTreeIndex;
    use crate::spatial_index::{SpatialIndex, SpatialIndexGeomOp};
    use crate::AsH3CellChunked;
    use geo_types::{coord, polygon, Rect};
    use h3ron::{H3Cell, Index};
    use polars::prelude::{TakeRandom, UInt64Chunked};

    fn build_cell_ca() -> UInt64Chunked {
        UInt64Chunked::new_from_indexes(
            "",
            vec![
                H3Cell::from_coordinate((45.5, 45.5).into(), 7).unwrap(),
                H3Cell::from_coordinate((-60.5, -60.5).into(), 7).unwrap(),
                H3Cell::from_coordinate((120.5, 70.5).into(), 7).unwrap(),
                H3Cell::new(55), // invalid
            ],
        )
    }

    #[test]
    fn cell_within_distance() {
        let ca = build_cell_ca();
        let kd = ca.h3cell().kdtree_index();
        let mask = kd.envelopes_within_distance((-60.0, -60.0).into(), 2.0);

        assert_eq!(mask.len(), 4);
        assert_eq!(mask.get(0), Some(false));
        assert_eq!(mask.get(1), Some(true));
        assert_eq!(mask.get(2), Some(false));
        assert_eq!(mask.get(3), None);
    }

    #[test]
    fn cell_within_rect() {
        let ca = build_cell_ca();
        let kd = ca.h3cell().kdtree_index();
        let mask = kd.geometries_intersect(&Rect::new((40.0, 40.0), (50.0, 50.0)));

        assert_eq!(mask.len(), 4);
        assert_eq!(mask.get(0), Some(true));
        assert_eq!(mask.get(1), Some(false));
        assert_eq!(mask.get(2), Some(false));
        assert_eq!(mask.get(3), None);
    }

    #[test]
    fn cell_within_polygon() {
        let ca = build_cell_ca();
        let kd = ca.h3cell().kdtree_index();
        let mask = kd.geometries_intersect_polygon(&polygon!(exterior: [
                coord! {x: 40.0, y: 40.0},
                coord! {x: 40.0, y: 50.0},
                coord! {x: 49.0, y: 50.0},
                coord! {x: 49.0, y: 40.0},
                coord! {x: 40.0, y: 40.0},
            ], interiors: []));

        assert_eq!(mask.len(), 4);
        assert_eq!(mask.get(0), Some(true));
        assert_eq!(mask.get(1), Some(false));
        assert_eq!(mask.get(2), Some(false));
        assert_eq!(mask.get(3), None);
    }
}
