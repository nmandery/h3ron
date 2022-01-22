use crate::collections::indexvec::IndexVec;
use crate::{FromH3Index, Index};
use std::borrow::Borrow;
use std::cmp::Ordering;

/// Returns an iterator to change the resolution of the given cells to the `target_resolution`
pub fn change_resolution<I, IX: FromH3Index + Index>(
    input_iter: I,
    target_h3_resolution: u8,
) -> ChangeResolutionIterator<<I as IntoIterator>::IntoIter, IX>
where
    I: IntoIterator,
    IX: Copy,
    I::Item: Borrow<IX>,
{
    ChangeResolutionIterator {
        inner: input_iter.into_iter(),
        target_h3_resolution,
        current_batch: Default::default(),
    }
}

pub struct ChangeResolutionIterator<I, IX: FromH3Index + Index> {
    inner: I,
    target_h3_resolution: u8,
    current_batch: IndexVec<IX>,
}

impl<I, IX: FromH3Index + Index> Iterator for ChangeResolutionIterator<I, IX>
where
    I: Iterator,
    IX: Copy,
    I::Item: Borrow<IX>,
{
    type Item = IX;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(cell) = self.current_batch.pop() {
            Some(cell)
        } else if let Some(cell) = self.inner.next() {
            match cell.borrow().resolution().cmp(&self.target_h3_resolution) {
                Ordering::Less => {
                    self.current_batch = cell.borrow().get_children(self.target_h3_resolution);
                    self.current_batch.pop()
                }
                Ordering::Equal => Some(*cell.borrow()),
                Ordering::Greater => Some(
                    cell.borrow()
                        .get_parent_unchecked(self.target_h3_resolution),
                ),
            }
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::iter::once;

    use geo::Coordinate;

    use crate::iter::change_resolution;
    use crate::Index;
    use crate::{H3Cell, H3Edge};

    #[test]
    fn test_change_h3_resolution_same_res() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let changed = change_resolution(once(cell), 6).collect::<Vec<_>>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0], cell);
    }

    #[test]
    fn test_change_h3_resolution_lower_res() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let changed = change_resolution(once(cell), 5).collect::<Vec<_>>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].resolution(), 5);
    }

    #[test]
    fn test_change_h3_resolution_higher_res() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let changed = change_resolution(once(cell), 7).collect::<Vec<_>>();
        assert_eq!(changed.len(), 7);
        assert_eq!(changed[0].resolution(), 7);
    }

    #[test]
    fn test_change_edge_res() {
        let edge = H3Edge::new(0x149283080ddbffff);
        let changed = change_resolution(once(edge), edge.resolution() + 1).collect::<Vec<_>>();
        assert_eq!(changed.len(), 7);
        assert_eq!(changed[0].resolution(), edge.resolution() + 1);
    }
}
