use crate::collections::indexvec::IndexVec;
use crate::{Error, H3Cell, Index};
use std::borrow::Borrow;
use std::cmp::Ordering;

/// Returns an iterator to change the resolution of the given cells to the `output_h3_resolution`.
///
/// Also see [`change_resolution_tuple`].
pub fn change_resolution<I>(
    input_iter: I,
    output_h3_resolution: u8,
) -> ChangeResolutionIterator<<I as IntoIterator>::IntoIter>
where
    I: IntoIterator,
    I::Item: Borrow<H3Cell>,
{
    ChangeResolutionIterator {
        inner: input_iter.into_iter(),
        output_h3_resolution,
        current_output_batch: Default::default(),
    }
}

pub struct ChangeResolutionIterator<I> {
    inner: I,
    output_h3_resolution: u8,
    current_output_batch: IndexVec<H3Cell>,
}

impl<I> Iterator for ChangeResolutionIterator<I>
where
    I: Iterator,
    I::Item: Borrow<H3Cell>,
{
    type Item = Result<H3Cell, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(output_index) = self.current_output_batch.pop() {
            return Some(Ok(output_index));
        } else {
            for input_index_ref in self.inner.by_ref() {
                let input_index = *input_index_ref.borrow();
                match input_index.resolution().cmp(&self.output_h3_resolution) {
                    Ordering::Less => {
                        match input_index.borrow().get_children(self.output_h3_resolution) {
                            Ok(current_output_batch) => {
                                self.current_output_batch = current_output_batch;
                                if let Some(output_index) = self.current_output_batch.pop() {
                                    return Some(Ok(output_index));
                                }
                            }
                            Err(e) => return Some(Err(e)),
                        }
                    }
                    Ordering::Equal => return Some(Ok(input_index)),
                    Ordering::Greater => {
                        return Some(input_index.get_parent(self.output_h3_resolution))
                    }
                }
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// Returns an iterator to change the resolution of the given cells to the `output_h3_resolution`. The
/// iterator iterates over tuples of `(input_index, output_index)`, where `output_index` is the index
/// after the resolution change has been applied.
///
/// Also see [`change_resolution`].
pub fn change_resolution_tuple<I>(
    input_iter: I,
    output_h3_resolution: u8,
) -> ChangeResolutionTupleIterator<<I as IntoIterator>::IntoIter>
where
    I: IntoIterator,
    I::Item: Borrow<H3Cell>,
{
    ChangeResolutionTupleIterator {
        inner: input_iter.into_iter(),
        output_h3_resolution,
        current_output_batch: Default::default(),
    }
}

pub struct ChangeResolutionTupleIterator<I> {
    inner: I,
    output_h3_resolution: u8,
    current_output_batch: Option<(H3Cell, IndexVec<H3Cell>)>,
}

impl<I> Iterator for ChangeResolutionTupleIterator<I>
where
    I: Iterator,
    I::Item: Borrow<H3Cell>,
{
    type Item = Result<(H3Cell, H3Cell), Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some((current_input_index, current_output_batch)) =
            self.current_output_batch.as_mut()
        {
            if let Some(current_output_index) = current_output_batch.pop() {
                return Some(Ok((*current_input_index, current_output_index)));
            } else {
                self.current_output_batch = None;
            }
        }

        for input_index_ref in self.inner.by_ref() {
            let input_index = *input_index_ref.borrow();
            match input_index.resolution().cmp(&self.output_h3_resolution) {
                Ordering::Less => match input_index.get_children(self.output_h3_resolution) {
                    Ok(mut current_output_batch) => {
                        if let Some(output_index) = current_output_batch.pop() {
                            self.current_output_batch = Some((input_index, current_output_batch));
                            return Some(Ok((input_index, output_index)));
                        }
                    }
                    Err(e) => return Some(Err(e)),
                },
                Ordering::Equal => return Some(Ok((input_index, input_index))),
                Ordering::Greater => {
                    return Some(
                        input_index
                            .get_parent(self.output_h3_resolution)
                            .map(|parent| (input_index, parent)),
                    )
                }
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[cfg(test)]
mod tests {
    use std::iter::once;

    use geo_types::Coord;

    use crate::iter::change_resolution;
    use crate::iter::resolution::change_resolution_tuple;
    use crate::H3Cell;
    use crate::Index;

    #[test]
    fn test_change_h3_resolution_same_res() {
        let cell = H3Cell::from_coordinate(Coord::from((12.3, 45.4)), 6).unwrap();
        let changed = change_resolution(once(cell), 6)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0], cell);
    }

    #[test]
    fn test_change_h3_resolution_lower_res() {
        let cell = H3Cell::from_coordinate(Coord::from((12.3, 45.4)), 6).unwrap();
        let changed = change_resolution(once(cell), 5)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].resolution(), 5);
    }

    #[test]
    fn test_change_h3_resolution_higher_res() {
        let cell = H3Cell::from_coordinate(Coord::from((12.3, 45.4)), 6).unwrap();
        let changed = change_resolution(once(cell), 7)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(changed.len(), 7);
        assert_eq!(changed[0].resolution(), 7);
    }

    #[test]
    fn test_change_h3_resolution_higher_res_tuple() {
        let cell = H3Cell::from_coordinate(Coord::from((12.3, 45.4)), 6).unwrap();
        let changed = change_resolution_tuple(once(cell), 7)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(changed.len(), 7);
        assert_eq!(changed[0].1.resolution(), 7);
        assert_eq!(changed[0].0.resolution(), 6);
        assert_eq!(changed[0].0, cell);
    }
}
