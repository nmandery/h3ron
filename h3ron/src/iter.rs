//! Iterator functionalites
//!
//! Most iterators in this module are implemented as generic functions. This allow to not just use them
//! with the collections in `std::collections`, but also to apply them to custom data structures.
//!
//! # Resolution handling
//!
//! * [`change_cell_resolution`]
//!
//! # Grid traversal
//!
//! * [`neighbors_within_distance_window_or_default`]
//! * [`neighbors_within_distance_window`]
//! * [`neighbors_within_distance`]
//!
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::os::raw::c_int;

use h3ron_h3_sys::H3Index;

use crate::{max_k_ring_size, H3Cell, Index};

/// Returns an iterator to change the resolution of the given cells to the `target_resolution`
pub fn change_cell_resolution<I>(
    cell_iter: I,
    target_h3_resolution: u8,
) -> ChangeCellResolutionIterator<<I as IntoIterator>::IntoIter>
where
    I: IntoIterator,
    I::Item: Borrow<H3Cell>,
{
    ChangeCellResolutionIterator {
        inner: cell_iter.into_iter(),
        target_h3_resolution,
        current_batch: Default::default(),
    }
}

pub struct ChangeCellResolutionIterator<I> {
    inner: I,
    target_h3_resolution: u8,
    current_batch: Vec<H3Cell>,
}

impl<I> Iterator for ChangeCellResolutionIterator<I>
where
    I: Iterator,
    I::Item: Borrow<H3Cell>,
{
    type Item = H3Cell;

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

/// A `H3Cell` and one of its neighboring cells, combined with associated generic values.
pub struct NeighborCell<'a, T> {
    /// The current cell of the iterator
    pub cell: H3Cell,

    /// Value of the current cell
    pub cell_value: &'a T,

    /// A neighbor of the current cell
    pub neighbor_cell: H3Cell,

    /// The value of the neighbor of the current cell
    pub neighbor_value: &'a T,

    /// The distance between `cell` and `neighbor_cell` in cells. See [`H3Cell::k_ring`].
    pub k: u32,
}

/// The iterator implementation returned by [`neighbors_within_distance_window_or_default`].
pub struct CellNeighborsIterator<'a, I, F, T> {
    /// Iterator for the cells to visit.
    cell_iter: I,

    /// Function to return the values associated with cells.
    get_cell_value_fn: F,

    /// The current cell and its value.
    current_cell_key_value: Option<(H3Cell, &'a T)>,

    /// The default value to use in case `get_cell_value_fn` returns no value for
    /// a neighbor.
    neighbor_default_value: Option<&'a T>,

    /// The neighbors of the current cell which are still to visit.
    next_neighbor_indexes: Vec<H3Index>,
    next_neighbor_k: Vec<c_int>,
    current_neighbor_i: usize,

    k_min: u32,
    k_max: u32,
    k_ring_size: usize,
}

/// See [`neighbors_within_distance_window_or_default`].
impl<'a, I, F, T> Iterator for CellNeighborsIterator<'a, I, F, T>
where
    I: Iterator,
    I::Item: Borrow<H3Cell>,
    I: 'a,
    F: Fn(&H3Cell) -> Option<&'a T>,
    F: 'a,
    T: 'a,
{
    type Item = NeighborCell<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some((cell, value)) = self.current_cell_key_value.as_ref() {
                // advance until we find the next existing neighbor
                while self.current_neighbor_i < self.k_ring_size {
                    let this_current_neighbor_i = self.current_neighbor_i;
                    self.current_neighbor_i += 1;
                    if let (Some(neighbor_h3index), Some(neighbor_k)) = (
                        self.next_neighbor_indexes.get(this_current_neighbor_i),
                        self.next_neighbor_k.get(this_current_neighbor_i),
                    ) {
                        // invalid h3index or `k` smaller the requested `k_min`, so it gets ignored
                        if (*neighbor_h3index == 0) || (*neighbor_k as u32) < self.k_min {
                            continue;
                        }

                        let neighbor_cell = H3Cell::new(*neighbor_h3index);
                        if let Some(neighbor_value) =
                            (self.get_cell_value_fn)(&neighbor_cell).or(self.neighbor_default_value)
                        {
                            return Some(NeighborCell {
                                cell: *cell,
                                cell_value: *value,
                                neighbor_cell,
                                neighbor_value,
                                k: *neighbor_k as u32,
                            });
                        }
                    }
                }
                self.current_cell_key_value = None;
            }
            if let Some(cell) = self.cell_iter.next() {
                if let Some(cell_value) = (self.get_cell_value_fn)(cell.borrow()) {
                    self.current_cell_key_value = Some((*cell.borrow(), cell_value));

                    // clear the pre-allocated vectors to ensure no values from the former run
                    // are left
                    self.next_neighbor_indexes
                        .iter_mut()
                        .for_each(|h3index| *h3index = 0);
                    self.next_neighbor_k.iter_mut().for_each(|k| *k = 0);

                    // populate the pre-allocated vectors with the new neighbors
                    unsafe {
                        h3ron_h3_sys::kRingDistances(
                            cell.borrow().h3index(),
                            self.k_max as c_int,
                            self.next_neighbor_indexes.as_mut_ptr(),
                            self.next_neighbor_k.as_mut_ptr(),
                        )
                    };
                    // reset to start over from the beginning
                    self.current_neighbor_i = 0;
                }
            } else {
                return None;
            }
        }
    }
}

/// Returns an iterator to visit all neighbors of the cells of `cell_iter` by accessing the values
/// using the `get_cell_value_fn` function. Neighbors of cells which do not have a
/// value returned by that function will not be visited.
///
/// The `neighbor_default_value` will be returned for neighbor cells which are not found by
/// `get_cell_value_fn`. In case `neighbor_default_value` is `None`, that neighbor will be skipped.
///
/// `k_min` and `k_max` control the radius in which the neighbors will be iterated. Also
/// see [`H3Cell::k_ring`].
///
/// This implementation trys to avoid memory allocations during iteration.
pub fn neighbors_within_distance_window_or_default<'a, I, F, T>(
    cell_iter: I,
    get_cell_value_fn: F,
    k_min: u32,
    k_max: u32,
    neighbor_default_value: Option<&'a T>,
) -> CellNeighborsIterator<'a, I, F, T>
where
    I: Iterator,
    I::Item: Borrow<H3Cell>,
    I: 'a,
    F: Fn(&H3Cell) -> Option<&'a T>,
    F: 'a,
{
    let k_ring_size = max_k_ring_size(k_max);

    // pre-allocate the output vecs for k_ring_distances so we do not
    // need to allocate during iteration.
    let next_neighbor_indexes = vec![0; k_ring_size];
    let next_neighbor_k = vec![0; k_ring_size];

    CellNeighborsIterator {
        cell_iter,
        get_cell_value_fn,
        current_cell_key_value: None,
        neighbor_default_value,
        next_neighbor_indexes,
        next_neighbor_k,
        current_neighbor_i: 0,
        k_min,
        k_max,
        k_ring_size,
    }
}

/// Simplified wrapper for [`neighbors_within_distance_window_or_default`].
#[inline]
pub fn neighbors_within_distance_window<'a, I, F, T>(
    cell_iter: I,
    get_cell_value_fn: F,
    k_min: u32,
    k_max: u32,
) -> CellNeighborsIterator<'a, I, F, T>
where
    I: Iterator,
    I::Item: Borrow<H3Cell>,
    I: 'a,
    F: Fn(&H3Cell) -> Option<&'a T>,
    F: 'a,
{
    neighbors_within_distance_window_or_default(cell_iter, get_cell_value_fn, k_min, k_max, None)
}

/// Simplified wrapper for [`neighbors_within_distance_window_or_default`].
#[inline]
pub fn neighbors_within_distance<'a, I, F, T>(
    cell_iter: I,
    get_cell_value_fn: F,
    k_max: u32,
) -> CellNeighborsIterator<'a, I, F, T>
where
    I: Iterator,
    I::Item: Borrow<H3Cell>,
    I: 'a,
    F: Fn(&H3Cell) -> Option<&'a T>,
    F: 'a,
{
    neighbors_within_distance_window_or_default(
        cell_iter,
        get_cell_value_fn,
        1, /* exclude the cell itself */
        k_max,
        None,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::iter::once;

    use geo::Coordinate;

    use crate::iter::{
        change_cell_resolution, neighbors_within_distance_window,
        neighbors_within_distance_window_or_default,
    };
    use crate::H3Cell;
    use crate::Index;

    #[test]
    fn test_change_h3_resolution_same_res() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let changed = change_cell_resolution(once(cell), 6).collect::<Vec<_>>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0], cell);
    }

    #[test]
    fn test_change_h3_resolution_lower_res() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let changed = change_cell_resolution(once(cell), 5).collect::<Vec<_>>();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].resolution(), 5);
    }

    #[test]
    fn test_change_h3_resolution_higher_res() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let changed = change_cell_resolution(once(cell), 7).collect::<Vec<_>>();
        assert_eq!(changed.len(), 7);
        assert_eq!(changed[0].resolution(), 7);
    }

    #[test]
    fn test_neighbors_within_distance_window() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let hm = cell
            .k_ring(2) // one k more than required
            .drain(..)
            .map(|cell| (cell, 6))
            .collect::<HashMap<_, _>>();

        let mut n_neighbors = 0_usize;
        for neighbor in
            neighbors_within_distance_window(std::iter::once(cell), |cell| hm.get(cell), 1, 1)
        {
            n_neighbors += 1;
            assert_eq!(neighbor.cell, cell);
            assert_ne!(cell, neighbor.neighbor_cell);
            assert!(hm.contains_key(&neighbor.neighbor_cell));
        }
        assert_eq!(n_neighbors, 6);
    }

    #[test]
    fn test_neighbors_within_distance_window_or_default() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let mut hm: HashMap<H3Cell, u32> = Default::default();
        hm.insert(cell, 4_u32);

        let mut n_neighbors = 0_usize;
        for neighbor in neighbors_within_distance_window_or_default(
            std::iter::once(cell),
            |cell| hm.get(cell),
            1,
            1,
            Some(&6),
        ) {
            n_neighbors += 1;
            assert_eq!(neighbor.cell, cell);
            assert_ne!(cell, neighbor.neighbor_cell);
            assert!(!hm.contains_key(&neighbor.neighbor_cell));
            assert_eq!(neighbor.neighbor_value, &6_u32);
            assert_eq!(neighbor.cell_value, &4_u32);
        }
        assert_eq!(n_neighbors, 6);
    }

    #[test]
    fn test_neighbors_within_distance_window_or_default_empty() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let hm: HashMap<H3Cell, u32> = Default::default();

        let n_neighbors = neighbors_within_distance_window_or_default(
            std::iter::once(cell),
            |cell| hm.get(cell),
            1,
            1,
            Some(&6),
        )
        .count();
        assert_eq!(n_neighbors, 0);
    }
}
