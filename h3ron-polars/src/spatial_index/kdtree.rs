use crate::spatial_index::{
    finish_mask, negative_mask, CoordinateIndexable, CoordinateSIKind, SpatialIndex,
};
use crate::{AsH3IndexChunked, IndexChunked, IndexValue};
use geo_types::{Coordinate, Rect};
use kdbush::{KDBush, PointReader};
use polars::export::arrow::bitmap::MutableBitmap;
use polars::prelude::BooleanChunked;
use polars_core::prelude::UInt64Chunked;
use std::marker::PhantomData;

struct Points(Vec<(usize, Coordinate)>);

impl PointReader for Points {
    fn size_hint(&self) -> usize {
        self.0.len()
    }

    fn visit_all<F>(&self, mut visitor: F)
    where
        F: FnMut(usize, f64, f64),
    {
        for (pos, coord) in self.0.iter() {
            visitor(*pos, coord.x, coord.y)
        }
    }
}

pub trait BuildKDTreeIndex<'a, IX>
where
    IX: IndexValue + CoordinateIndexable,
{
    /// Build a [KDTreeIndex] using the default parameters.
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

    /// Build a [KDTreeIndex] using custom parameters.
    ///
    /// `node_size` - Size of the KD-tree node, 64 by default. Higher means faster indexing but slower search, and vise versa
    fn kdtree_index_with_node_size(&self, node_size: u8) -> KDTreeIndex<IX>;
}

impl<'a, IX: IndexValue> BuildKDTreeIndex<'a, IX> for IndexChunked<'a, IX>
where
    IX: CoordinateIndexable,
{
    fn kdtree_index_with_node_size(&self, node_size: u8) -> KDTreeIndex<IX> {
        // KDBush requires at least one entry to be successful build, so we need to inspect
        // the data first.
        let entries: Vec<_> = self
            .iter_indexes_nonvalidated()
            .enumerate()
            .filter_map(|(pos, maybe_index)| match maybe_index {
                Some(index) => index.spatial_index_coordinate().ok().map(|c| (pos, c)),
                _ => None,
            })
            .collect();

        let kdbush = if entries.is_empty() {
            None
        } else {
            Some(KDBush::create(Points(entries), node_size))
        };

        KDTreeIndex {
            index_phantom: PhantomData::<IX>::default(),

            // clones of arrow-backed arrays are cheap, so we clone this for the benefit of not
            // requiring a lifetime dependency
            chunked_array: self.chunked_array.clone(),

            kdbush,
        }
    }
}

/// A very fast static spatial index for 2D points based on a flat [KD-tree](https://en.wikipedia.org/wiki/K-d_tree).
///
/// Operates only the centroid coordinate, not on envelopes / bounding boxes.
///
/// # Example
///
/// ```
/// use polars::prelude::UInt64Chunked;
/// use h3ron::H3Cell;
/// use h3ron_polars::{AsH3CellChunked, NamedFromIndexes};
/// use h3ron_polars::spatial_index::BuildKDTreeIndex;
///
/// let uc = UInt64Chunked::new_from_indexes(
///     "",
///     vec![
///         H3Cell::from_coordinate((45.5, 45.5).into(), 7).unwrap(),
///         H3Cell::from_coordinate((-60.5, -60.5).into(), 7).unwrap(),
///         H3Cell::from_coordinate((120.5, 70.5).into(), 7).unwrap(),
///     ],
/// );
///
/// let idx = uc.h3cell().kdtree_index();
/// ```
pub struct KDTreeIndex<IX: IndexValue> {
    index_phantom: PhantomData<IX>,

    chunked_array: UInt64Chunked,

    pub kdbush: Option<KDBush>,
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
        if let Some(kdbush) = self.kdbush.as_ref() {
            kdbush.range(
                rect.min().x,
                rect.min().y,
                rect.max().x,
                rect.max().y,
                |id| mask.set(id, true),
            );
        }
        mask
    }

    fn envelopes_within_distance(&self, coord: Coordinate, distance: f64) -> BooleanChunked {
        let mut mask = negative_mask(&self.chunked_array);
        if let Some(kdbush) = self.kdbush.as_ref() {
            kdbush.within(coord.x, coord.y, distance, |id| mask.set(id, true));
        }
        finish_mask(mask.into(), &self.h3indexchunked())
    }
}

#[cfg(test)]
mod test {
    use crate::spatial_index::{BuildKDTreeIndex, KDTreeIndex};
    use crate::IndexChunked;
    use h3ron::H3Cell;

    fn build_index(cc: &IndexChunked<H3Cell>) -> KDTreeIndex<H3Cell> {
        cc.kdtree_index()
    }
    crate::spatial_index::tests::impl_std_tests!(build_index);
}
