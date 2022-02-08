use std::os::raw::c_int;

use h3ron_h3_sys::H3Index;

use crate::{max_grid_disk_size, Error, H3Cell, Index};

/// `GridDiskBuilder` allows building k-rings with allocations only on the creation
/// of the struct. This can be more efficient for large numbers of small (~ `k_max` <= 6) disks.
/// (See the included `k_ring_variants` benchmark)
///
/// After calling [`GridDiskBuilder::build_grid_disk`] the struct can be accessed
/// as a [`Iterator`] returning `(H3Cell, u32)` tuples. The [`H3Cell`] value is a
/// cell from within the grid disk and the `u32` distance `k` to the disks center.
///
/// TODO: find out why this method is slower for larger k-rings (see benchmark).
pub struct GridDiskBuilder {
    k_min: u32,
    k_max: u32,

    k_ring_indexes: Vec<H3Index>,
    k_ring_distances: Vec<c_int>,
    k_ring_size: usize,

    current_pos: usize,
}

impl GridDiskBuilder {
    /// `k_min` and `k_max` control the radius in which the neighbors will be iterated. Also
    /// see [`H3Cell::grid_disk`].
    pub fn create(k_min: u32, k_max: u32) -> Result<Self, Error> {
        let k_ring_size = max_grid_disk_size(k_max)?;

        // pre-allocate the output vecs for k_ring_distances so we do not
        // need to allocate during iteration.
        let k_ring_indexes = vec![0; k_ring_size];
        let k_ring_distances = vec![0; k_ring_size];
        Ok(Self {
            k_min,
            k_max,
            k_ring_indexes,
            k_ring_distances,
            k_ring_size,
            current_pos: k_ring_size, // nothing left to iterate over
        })
    }

    #[inline(always)]
    fn rewind_iterator(&mut self) {
        self.current_pos = 0;
    }

    /// Build the grid disk around the given [`H3Cell`]
    ///
    /// `k_min` and `k_max` control the radius in which the neighbors will be iterated. Also
    /// see [`H3Cell::grid_disk`].
    ///
    /// Building a grid disk resets the iterator to the start.
    pub fn build_grid_disk(&mut self, cell: &H3Cell) -> Result<&mut Self, Error> {
        // clear the pre-allocated vectors to ensure no values from the former run
        // are left
        Error::check_returncode(unsafe {
            // this essentially is `memset`
            std::ptr::write_bytes(self.k_ring_indexes.as_mut_ptr(), 0, self.k_ring_size);
            std::ptr::write_bytes(self.k_ring_distances.as_mut_ptr(), 0, self.k_ring_size);

            // populate the pre-allocated vectors with the new neighbors
            h3ron_h3_sys::gridDiskDistances(
                cell.h3index(),
                self.k_max as c_int,
                self.k_ring_indexes.as_mut_ptr(),
                self.k_ring_distances.as_mut_ptr(),
            )
        })?;
        self.rewind_iterator();
        Ok(self)
    }
}

impl Iterator for GridDiskBuilder {
    type Item = (H3Cell, u32);

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_pos < self.k_ring_size {
            let pos = self.current_pos;
            self.current_pos += 1;

            let h3index = self.k_ring_indexes[pos];
            let k = self.k_ring_distances[pos] as u32;
            if h3index == 0 || k < self.k_min {
                // invalid h3index or `k` smaller the requested `k_min`,
                // so it gets ignored.
                continue;
            }
            return Some((H3Cell::new(h3index), k));
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        // IMPROVE: this overestimates when k_min != 0
        (self.k_ring_size - self.current_pos, None)
    }
}
