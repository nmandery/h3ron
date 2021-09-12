use crate::iter::KRingBuilder;
use crate::H3Cell;
use std::borrow::Borrow;

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

    /// iterates over the neighbors of the current cell which are still to visit.
    k_ring_builder: KRingBuilder,
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
                for (neighbor_cell, neighbor_k) in &mut self.k_ring_builder {
                    if let Some(neighbor_value) =
                        (self.get_cell_value_fn)(&neighbor_cell).or(self.neighbor_default_value)
                    {
                        return Some(NeighborCell {
                            cell: *cell,
                            cell_value: *value,
                            neighbor_cell,
                            neighbor_value,
                            k: neighbor_k,
                        });
                    }
                }
                self.current_cell_key_value = None;
            }
            if let Some(cell) = self.cell_iter.next() {
                if let Some(cell_value) = (self.get_cell_value_fn)(cell.borrow()) {
                    self.current_cell_key_value = Some((*cell.borrow(), cell_value));
                    self.k_ring_builder.build_k_ring(cell.borrow());
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
    CellNeighborsIterator {
        cell_iter,
        get_cell_value_fn,
        current_cell_key_value: None,
        neighbor_default_value,
        k_ring_builder: KRingBuilder::new(k_min, k_max),
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

    use super::{neighbors_within_distance_window, neighbors_within_distance_window_or_default};
    use crate::H3Cell;

    #[test]
    fn test_neighbors_within_distance_window() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((12.3, 45.4)), 6).unwrap();
        let hm = cell
            .k_ring(2) // one k more than required
            .drain()
            .map(|cell| (cell, 6))
            .collect::<HashMap<_, _>>();

        let mut n_neighbors = 0_usize;
        for neighbor in neighbors_within_distance_window(once(cell), |cell| hm.get(cell), 1, 1) {
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
            once(cell),
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
            once(cell),
            |cell| hm.get(cell),
            1,
            1,
            Some(&6),
        )
        .count();
        assert_eq!(n_neighbors, 0);
    }
}
