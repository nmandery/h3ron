use crate::spatial_index::{finish_mask, negative_mask, RectIndexable, RectSIKind, SpatialIndex};
use crate::{AsH3IndexChunked, IndexChunked, IndexValue};
use geo_types::{Coordinate, Rect};
use polars::export::arrow::bitmap::MutableBitmap;
use polars::prelude::UInt64Chunked;
use polars_core::datatypes::BooleanChunked;
use rstar::primitives::{GeomWithData, Rectangle};
use rstar::{RTree, AABB};
use std::marker::PhantomData;

type Coord = [f64; 2];
type BBox = Rectangle<Coord>;
type LocatedArrayPosition = GeomWithData<BBox, usize>;

pub struct RTreeIndex<IX: IndexValue> {
    index_phantom: PhantomData<IX>,
    chunked_array: UInt64Chunked,
    pub rtree: RTree<LocatedArrayPosition>,
}

#[inline]
fn to_coord(coord: Coordinate) -> Coord {
    [coord.x, coord.y]
}

#[inline]
fn to_bbox(rect: &Rect) -> BBox {
    BBox::from_corners(to_coord(rect.min()), to_coord(rect.max()))
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
    fn rtree_index(&self) -> RTreeIndex<IX> {
        let entries: Vec<_> = self
            .iter_indexes_validated()
            .enumerate()
            .filter_map(|(pos, maybe_index)| match maybe_index {
                Some(Ok(index)) => index.spatial_index_rect().ok().and_then(|maybe_rect| {
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

    fn envelopes_within_distance(&self, coord: Coordinate, distance: f64) -> BooleanChunked {
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
