use crate::collections::indexvec::IndexVec;
use crate::{FromH3Index, Index};
use std::borrow::Borrow;
use std::cmp::Ordering;

/// Returns an iterator to change the resolution of the given cells to the `output_h3_resolution`.
///
/// Also see [`change_resolution_tuple`].
pub fn change_resolution<I, IX: FromH3Index + Index>(
    input_iter: I,
    output_h3_resolution: u8,
) -> ChangeResolutionIterator<<I as IntoIterator>::IntoIter, IX>
where
    I: IntoIterator,
    IX: Copy,
    I::Item: Borrow<IX>,
{
    ChangeResolutionIterator {
        inner: input_iter.into_iter(),
        output_h3_resolution,
        current_output_batch: Default::default(),
    }
}

pub struct ChangeResolutionIterator<I, IX: FromH3Index + Index> {
    inner: I,
    output_h3_resolution: u8,
    current_output_batch: IndexVec<IX>,
}

impl<I, IX: FromH3Index + Index> Iterator for ChangeResolutionIterator<I, IX>
where
    I: Iterator,
    IX: Copy,
    I::Item: Borrow<IX>,
{
    type Item = IX;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(output_index) = self.current_output_batch.pop() {
            return Some(output_index);
        } else {
            for input_index_ref in self.inner.by_ref() {
                let input_index = *input_index_ref.borrow();
                match input_index.resolution().cmp(&self.output_h3_resolution) {
                    Ordering::Less => {
                        self.current_output_batch =
                            input_index.borrow().get_children(self.output_h3_resolution);
                        if let Some(output_index) = self.current_output_batch.pop() {
                            return Some(output_index);
                        }
                    }
                    Ordering::Equal => return Some(input_index),
                    Ordering::Greater => {
                        return Some(input_index.get_parent_unchecked(self.output_h3_resolution))
                    }
                }
            }
        }
        None
    }
}

/// Returns an iterator to change the resolution of the given cells to the `output_h3_resolution`. The
/// iterator iterates over tuples of `(input_index, output_index)`, where `output_index` is the index
/// after the resolution change has been applied.
///
/// Also see [`change_resolution`].
pub fn change_resolution_tuple<I, IX: FromH3Index + Index>(
    input_iter: I,
    output_h3_resolution: u8,
) -> ChangeResolutionTupleIterator<<I as IntoIterator>::IntoIter, IX>
where
    I: IntoIterator,
    IX: Copy,
    I::Item: Borrow<IX>,
{
    ChangeResolutionTupleIterator {
        inner: input_iter.into_iter(),
        output_h3_resolution,
        current_output_batch: Default::default(),
    }
}

pub struct ChangeResolutionTupleIterator<I, IX: FromH3Index + Index> {
    inner: I,
    output_h3_resolution: u8,
    current_output_batch: Option<(IX, IndexVec<IX>)>,
}

impl<I, IX: FromH3Index + Index> Iterator for ChangeResolutionTupleIterator<I, IX>
where
    I: Iterator,
    IX: Copy,
    I::Item: Borrow<IX>,
{
    type Item = (IX, IX);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((current_input_index, current_output_batch)) =
            self.current_output_batch.as_mut()
        {
            if let Some(current_output_index) = current_output_batch.pop() {
                return Some((*current_input_index, current_output_index));
            } else {
                self.current_output_batch = None;
            }
        }

        for input_index_ref in self.inner.by_ref() {
            let input_index = *input_index_ref.borrow();
            match input_index.resolution().cmp(&self.output_h3_resolution) {
                Ordering::Less => {
                    let mut current_output_batch =
                        input_index.get_children(self.output_h3_resolution);
                    if let Some(output_index) = current_output_batch.pop() {
                        self.current_output_batch = Some((input_index, current_output_batch));
                        return Some((input_index, output_index));
                    }
                }
                Ordering::Equal => return Some((input_index, input_index)),
                Ordering::Greater => {
                    return Some((
                        input_index,
                        input_index.get_parent_unchecked(self.output_h3_resolution),
                    ))
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::iter::once;

    use geo::Coordinate;

    use crate::iter::change_resolution;
    use crate::iter::resolution::change_resolution_tuple;
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

    #[test]
    fn test_change_h3_resolution_higher_res_tuple() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let changed = change_resolution_tuple(once(cell), 7).collect::<Vec<_>>();
        assert_eq!(changed.len(), 7);
        assert_eq!(changed[0].1.resolution(), 7);
        assert_eq!(changed[0].0.resolution(), 6);
        assert_eq!(changed[0].0, cell);
    }
}
