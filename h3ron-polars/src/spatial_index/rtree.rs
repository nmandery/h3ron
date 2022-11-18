use crate::spatial_index::{finish_mask, negative_mask, RectIndexable, RectSIKind, SpatialIndex};
use crate::{AsH3IndexChunked, IndexChunked, IndexValue};
use geo_types::{Coord, Rect};
use polars::export::arrow::bitmap::MutableBitmap;
use polars::prelude::UInt64Chunked;
use polars_core::datatypes::BooleanChunked;
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{RTree, AABB};
use std::marker::PhantomData;

// todo: Use the Line type supported by rtree instead of Rectangle to index H3DirectedEdges

type RTreeCoord = [f64; 2];
type RTreeBBox = Rectangle<RTreeCoord>;
type LocatedArrayPosition = GeomWithData<RTreeBBox, usize>;

/// [R-Tree](https://en.wikipedia.org/wiki/R-tree) spatial index
pub struct RTreeIndex<IX: IndexValue> {
    index_phantom: PhantomData<IX>,
    chunked_array: UInt64Chunked,
    pub rtree: RTree<LocatedArrayPosition>,
}

#[inline]
fn to_coord(coord: Coord) -> RTreeCoord {
    [coord.x, coord.y]
}

#[inline]
fn to_bbox(rect: &Rect) -> RTreeBBox {
    RTreeBBox::from_corners(to_coord(rect.min()), to_coord(rect.max()))
}

pub trait BuildRTreeIndex<'a, IX>
where
    IX: IndexValue + RectIndexable,
{
    fn rtree_index(&self) -> RTreeIndex<IX>;
}

impl<'a, IX: IndexValue> BuildRTreeIndex<'a, IX> for IndexChunked<'a, IX>
where
    IX: RectIndexable,
{
    /// Build a [R-Tree](https://en.wikipedia.org/wiki/R-tree) spatial index
    ///
    /// # Example
    ///
    /// ```
    /// use geo_types::Rect;
    /// use polars::prelude::UInt64Chunked;
    /// use polars_core::prelude::TakeRandom;
    /// use h3ron::{H3Cell, Index};
    /// use h3ron_polars::{AsH3CellChunked, NamedFromIndexes};
    /// use h3ron_polars::spatial_index::{BuildRTreeIndex, SpatialIndex, SpatialIndexGeomOp};
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
    /// let rtree = ca.h3cell().rtree_index();
    /// let mask = rtree.geometries_intersect(&Rect::new((40.0, 40.0), (50.0, 50.0)));
    ///
    /// assert_eq!(mask.len(), 3);
    /// assert_eq!(mask.get(0), Some(true));
    /// assert_eq!(mask.get(1), Some(false));
    /// assert_eq!(mask.get(2), Some(false));
    /// ```
    fn rtree_index(&self) -> RTreeIndex<IX> {
        let entries: Vec<_> = self
            .iter_indexes_nonvalidated()
            .enumerate()
            .filter_map(|(pos, maybe_index)| match maybe_index {
                Some(index) => index.spatial_index_rect().ok().and_then(|maybe_rect| {
                    maybe_rect.map(|rect| LocatedArrayPosition::new(to_bbox(&rect), pos))
                }),
                _ => None,
            })
            .collect();

        RTreeIndex {
            index_phantom: PhantomData::<IX>::default(),
            chunked_array: self.chunked_array.clone(),
            rtree: RTree::bulk_load(entries),
        }
    }
}

impl<IX: IndexValue> SpatialIndex<IX, RectSIKind> for RTreeIndex<IX>
where
    IX: RectIndexable,
{
    fn h3indexchunked(&self) -> IndexChunked<IX> {
        self.chunked_array.h3indexchunked()
    }

    fn envelopes_intersect_impl(&self, rect: &Rect) -> MutableBitmap {
        let mut mask = negative_mask(&self.chunked_array);
        let envelope = AABB::from_corners(to_coord(rect.min()), to_coord(rect.max()));
        let locator = self.rtree.locate_in_envelope_intersecting(&envelope);
        for located_array_position in locator {
            mask.set(located_array_position.data, true);
        }
        mask
    }

    fn envelopes_within_distance(&self, coord: Coord, distance: f64) -> BooleanChunked {
        let mut mask = negative_mask(&self.chunked_array);

        let locator = self.rtree.locate_within_distance(to_coord(coord), distance);
        for located_array_position in locator {
            mask.set(located_array_position.data, true);
        }

        finish_mask(mask.into(), &self.h3indexchunked())
    }
}

#[cfg(test)]
mod test {
    use crate::spatial_index::{BuildRTreeIndex, RTreeIndex};
    use crate::IndexChunked;
    use h3ron::H3Cell;

    fn build_index(cc: &IndexChunked<H3Cell>) -> RTreeIndex<H3Cell> {
        cc.rtree_index()
    }
    crate::spatial_index::tests::impl_std_tests!(build_index);
}
