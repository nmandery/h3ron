use crate::{Error, IndexChunked, IndexValue};
use geo::{BoundingRect, Contains};
use geo_types::{Coordinate, MultiPolygon, Polygon, Rect};
use h3ron::{H3Cell, H3DirectedEdge, ToCoordinate};
use kdbush::{KDBush, PointReader};
use polars::datatypes::ArrowDataType;
use polars::export::arrow::array::BooleanArray;
use polars::export::arrow::bitmap::{Bitmap, MutableBitmap};
use polars::prelude::{BooleanChunked, TakeRandom};

pub trait KDBushCoordinate {
    fn kdbush_coordinate(&self) -> Result<Coordinate, Error>;
}

impl KDBushCoordinate for H3Cell {
    fn kdbush_coordinate(&self) -> Result<Coordinate, Error> {
        Ok(self.to_coordinate()?)
    }
}

impl KDBushCoordinate for H3DirectedEdge {
    fn kdbush_coordinate(&self) -> Result<Coordinate, Error> {
        let cells = self.cells()?;
        let c1 = cells.destination.to_coordinate()?;
        let c2 = cells.origin.to_coordinate()?;
        Ok(((c1.x + c2.x) / 2.0, (c1.y + c2.y) / 2.0).into())
    }
}

impl<'a, IX> PointReader for IndexChunked<'a, IX>
where
    IX: IndexValue + KDBushCoordinate,
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
                if let Ok(coord) = point.kdbush_coordinate() {
                    visitor(id, coord.x, coord.y);
                }
            }
        }
    }
}

pub trait BuildKDBushIndex<'a, IX>
where
    IX: IndexValue + KDBushCoordinate,
{
    /// Build a [`KDBushIndex`] using the default parameters.
    ///
    /// # Example
    ///
    /// ```
    /// use geo_types::Rect;
    /// use polars::prelude::UInt64Chunked;
    /// use polars_core::prelude::TakeRandom;
    /// use h3ron::{H3Cell, Index};
    /// use h3ron_polars::{AsH3CellChunked, NamedFromIndexes};
    /// use h3ron_polars::spatial_index::BuildKDBushIndex;
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
    /// let kdbush = ca.h3cell().kdbush_index();
    /// let mask = kdbush.within_rect(&Rect::new((40.0, 40.0), (50.0, 50.0)));
    ///
    /// assert_eq!(mask.len(), 3);
    /// assert_eq!(mask.get(0), Some(true));
    /// assert_eq!(mask.get(1), Some(false));
    /// assert_eq!(mask.get(2), Some(false));
    /// ```
    fn kdbush_index(&self) -> KDBushIndex<'a, IX> {
        self.kdbush_index_with_node_size(64)
    }

    /// Build a [`KDBushIndex`] using custom parameters.
    ///
    /// `node_size` - Size of the KD-tree node, 64 by default. Higher means faster indexing but slower search, and vise versa
    fn kdbush_index_with_node_size(&self, node_size: u8) -> KDBushIndex<'a, IX>;
}

impl<'a, IX: IndexValue> BuildKDBushIndex<'a, IX> for IndexChunked<'a, IX>
where
    Self: PointReader,
    IX: KDBushCoordinate,
{
    fn kdbush_index_with_node_size(&self, node_size: u8) -> KDBushIndex<'a, IX> {
        KDBushIndex {
            index_chunked: self.clone(),
            kdbush: KDBush::create((*self).clone(), node_size),
        }
    }
}

/// A very fast static spatial index for 2D points based on a flat KD-tree.
///
/// Operates on the centroid coordinate of [`H3Cell`] and [`H3DirectedEdge`] values.
pub struct KDBushIndex<'a, IX: IndexValue> {
    /// this reference prevents the underlying array from being mutated while
    /// this index exists
    index_chunked: IndexChunked<'a, IX>,

    kdbush: KDBush,
}

impl<'a, IX: IndexValue> KDBushIndex<'a, IX>
where
    IX: KDBushCoordinate,
{
    fn build_negative_mask(&self) -> MutableBitmap {
        let mut mask = MutableBitmap::new();
        mask.extend_constant(self.index_chunked.len(), false);
        mask
    }

    fn finish_mask(&self, mask: Bitmap) -> BooleanChunked {
        let validites = self.index_chunked.validity_bitmap();
        let bool_arr = BooleanArray::from_data(ArrowDataType::Boolean, mask, Some(validites));
        BooleanChunked::from(bool_arr)
    }

    fn within_rect_impl(&self, rect: &Rect) -> MutableBitmap {
        let mut mask = self.build_negative_mask();
        self.kdbush.range(
            rect.min().x,
            rect.min().y,
            rect.max().x,
            rect.max().y,
            |id| mask.set(id, true),
        );
        mask
    }

    pub fn within_rect(&self, rect: &Rect) -> BooleanChunked {
        self.finish_mask(self.within_rect_impl(rect).into())
    }

    pub fn within_distance(&self, coord: Coordinate, distance: f64) -> BooleanChunked {
        let mut mask = self.build_negative_mask();
        self.kdbush
            .within(coord.x, coord.y, distance, |id| mask.set(id, true));
        self.finish_mask(mask.into())
    }

    pub fn within_polygon(&self, polygon: &Polygon) -> Result<BooleanChunked, Error> {
        let mask =
            index_mask_within_polygon(self, polygon).unwrap_or_else(|| self.build_negative_mask());
        Ok(self.finish_mask(mask.into()))
    }

    pub fn within_multipolygon(
        &self,
        multipolygon: &MultiPolygon,
    ) -> Result<BooleanChunked, Error> {
        let mask = multipolygon
            .0
            .iter()
            .map(|poly| index_mask_within_polygon(self, poly))
            .reduce(|a, b| match (a, b) {
                (None, Some(b)) => Some(b),
                (Some(a), None) => Some(a),
                (None, None) => None,
                (Some(a), Some(b)) => Some(a | &(b.into())),
            })
            .flatten()
            .unwrap_or_else(|| self.build_negative_mask());
        Ok(self.finish_mask(mask.into()))
    }
}

fn index_mask_within_polygon<IX>(kdbi: &KDBushIndex<IX>, polygon: &Polygon) -> Option<MutableBitmap>
where
    IX: KDBushCoordinate + IndexValue,
{
    if let Some(rect) = polygon.bounding_rect() {
        let mut mask = kdbi.within_rect_impl(&rect);
        for i in 0..mask.len() {
            if mask.get(i) {
                if let Some(coord) = kdbi
                    .index_chunked
                    .get(i)
                    .and_then(|cell| cell.kdbush_coordinate().ok())
                {
                    mask.set(i, polygon.contains(&coord))
                }
            }
        }
        Some(mask)
    } else {
        None
    }
}

#[cfg(test)]
mod test {
    use crate::from::NamedFromIndexes;
    use crate::spatial_index::kdbush::BuildKDBushIndex;
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
        let kd = ca.h3cell().kdbush_index();
        let mask = kd.within_distance((-60.0, -60.0).into(), 2.0);

        assert_eq!(mask.len(), 4);
        assert_eq!(mask.get(0), Some(false));
        assert_eq!(mask.get(1), Some(true));
        assert_eq!(mask.get(2), Some(false));
        assert_eq!(mask.get(3), None);
    }

    #[test]
    fn cell_within_rect() {
        let ca = build_cell_ca();
        let kd = ca.h3cell().kdbush_index();
        let mask = kd.within_rect(&Rect::new((40.0, 40.0), (50.0, 50.0)));

        assert_eq!(mask.len(), 4);
        assert_eq!(mask.get(0), Some(true));
        assert_eq!(mask.get(1), Some(false));
        assert_eq!(mask.get(2), Some(false));
        assert_eq!(mask.get(3), None);
    }

    #[test]
    fn cell_within_polygon() {
        let ca = build_cell_ca();
        let kd = ca.h3cell().kdbush_index();
        let mask = kd
            .within_polygon(&polygon!(exterior: [
                coord! {x: 40.0, y: 40.0},
                coord! {x: 40.0, y: 50.0},
                coord! {x: 49.0, y: 50.0},
                coord! {x: 49.0, y: 40.0},
                coord! {x: 40.0, y: 40.0},
            ], interiors: []))
            .unwrap();

        assert_eq!(mask.len(), 4);
        assert_eq!(mask.get(0), Some(true));
        assert_eq!(mask.get(1), Some(false));
        assert_eq!(mask.get(2), Some(false));
        assert_eq!(mask.get(3), None);
    }
}
