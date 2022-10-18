use crate::{Error, IndexChunked, IndexValue};
use geo::BoundingRect as GeoBoundingRect;
use geo_types::{coord, CoordNum, Rect};
use h3ron::to_geo::ToLineString;
use h3ron::{H3Cell, H3DirectedEdge, ToPolygon};

pub trait BoundingRect {
    fn bounding_rect(&self) -> Result<Option<Rect>, Error>;
}

impl BoundingRect for H3Cell {
    fn bounding_rect(&self) -> Result<Option<Rect>, Error> {
        Ok(self.to_polygon()?.bounding_rect())
    }
}

impl BoundingRect for H3DirectedEdge {
    fn bounding_rect(&self) -> Result<Option<Rect>, Error> {
        Ok(self.to_linestring()?.bounding_rect())
    }
}

impl<'a, IX: IndexValue> BoundingRect for IndexChunked<'a, IX>
where
    IX: BoundingRect,
{
    fn bounding_rect(&self) -> Result<Option<Rect>, Error> {
        let mut rect = None;
        for maybe_index in self.iter_indexes_validated().flatten() {
            let new_rect = maybe_index?.bounding_rect()?;

            match (rect.as_mut(), new_rect) {
                (None, Some(r)) => rect = Some(r),
                (Some(agg), Some(this)) => *agg = bounding_rect_merge(agg, &this),
                _ => (),
            }
        }
        Ok(rect)
    }
}

// Return a new rectangle that encompasses the provided rectangles
//
// taken from `geo` crate
fn bounding_rect_merge<T: CoordNum>(a: &Rect<T>, b: &Rect<T>) -> Rect<T> {
    Rect::new(
        coord! {
            x: partial_min(a.min().x, b.min().x),
            y: partial_min(a.min().y, b.min().y),
        },
        coord! {
            x: partial_max(a.max().x, b.max().x),
            y: partial_max(a.max().y, b.max().y),
        },
    )
}

// The Rust standard library has `max` for `Ord`, but not for `PartialOrd`
pub fn partial_max<T: PartialOrd>(a: T, b: T) -> T {
    if a > b {
        a
    } else {
        b
    }
}

// The Rust standard library has `min` for `Ord`, but not for `PartialOrd`
pub fn partial_min<T: PartialOrd>(a: T, b: T) -> T {
    if a < b {
        a
    } else {
        b
    }
}
