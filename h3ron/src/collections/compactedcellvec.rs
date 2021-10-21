use std::borrow::Borrow;
use std::iter::FromIterator;
use std::ops::RangeInclusive;

#[cfg(feature = "use-serde")]
use serde::{Deserialize, Serialize};

use crate::collections::indexvec::IndexVec;
use crate::collections::H3CellSet;
use crate::collections::HashSet;
use crate::H3Cell;
use crate::{compact, Index, H3_MAX_RESOLUTION, H3_MIN_RESOLUTION};

const H3_RESOLUTION_RANGE_USIZE: RangeInclusive<usize> =
    (H3_MIN_RESOLUTION as usize)..=(H3_MAX_RESOLUTION as usize);

/// structure to keep compacted h3ron cells to allow more or less efficient
/// adding of further cells
#[derive(PartialEq, Debug)]
#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
pub struct CompactedCellVec {
    /// cells by their resolution. The index of the array is the resolution for the referenced vec
    cells_by_resolution: [Vec<H3Cell>; H3_MAX_RESOLUTION as usize + 1],
}

impl<'a> CompactedCellVec {
    pub fn new() -> CompactedCellVec {
        CompactedCellVec {
            cells_by_resolution: Default::default(),
        }
    }

    /// append the contents of another Stack to this one
    ///
    /// Indexes get moved, see [`Vec::append`]
    ///
    /// will trigger a re-compacting when compact is true
    pub fn append(&mut self, other: &mut Self, compact: bool) {
        let mut resolutions_touched = Vec::new();
        for resolution in H3_RESOLUTION_RANGE_USIZE {
            if !other.cells_by_resolution[resolution].is_empty() {
                resolutions_touched.push(resolution);
                let mut cells = std::mem::take(&mut other.cells_by_resolution[resolution]);
                self.cells_by_resolution[resolution].append(&mut cells);
            }
        }
        if compact {
            if let Some(max_res) = resolutions_touched.iter().max() {
                self.compact_from_resolution_up(*max_res, &resolutions_touched);
            }
        }
    }

    pub fn compact(&mut self) {
        self.compact_from_resolution_up(
            H3_MAX_RESOLUTION as usize,
            &H3_RESOLUTION_RANGE_USIZE.collect::<Vec<_>>(),
        )
    }

    /// append the contents of a vector. The caller is responsible to ensure that
    /// the append cells all are at resolution `resolution`.
    ///
    /// Cells get moved, this is the same API as [`Vec::append`].
    pub fn append_to_resolution(&mut self, resolution: u8, cells: &mut Vec<H3Cell>, compact: bool) {
        self.cells_by_resolution[resolution as usize].append(cells);
        if compact {
            self.compact_from_resolution_up(resolution as usize, &[]);
        }
    }

    /// shrink the underlying vec to fit using [`Vec::shrink_to_fit`].
    pub fn shrink_to_fit(&mut self) {
        self.cells_by_resolution
            .iter_mut()
            .for_each(|cells| cells.shrink_to_fit())
    }

    pub fn len(&self) -> usize {
        self.cells_by_resolution
            .iter()
            .fold(0, |acc, cells| acc + cells.len())
    }

    /// length of the vectors for all resolutions. The index of the vec is the resolution
    pub fn len_resolutions(&self) -> Vec<usize> {
        self.cells_by_resolution.iter().map(|v| v.len()).collect()
    }

    pub fn is_empty(&self) -> bool {
        !self
            .cells_by_resolution
            .iter()
            .any(|cells| !cells.is_empty())
    }

    /// check if the stack contains the cell or any of its parents
    ///
    /// This function is pretty inefficient.
    pub fn contains(&self, mut cell: H3Cell) -> bool {
        if self.is_empty() {
            return false;
        }
        for r in cell.resolution()..=H3_MIN_RESOLUTION {
            cell = match cell.get_parent(r) {
                Ok(i) => i,
                Err(_) => continue,
            };
            if self.cells_by_resolution[r as usize].contains(&cell) {
                return true;
            }
        }
        false
    }

    /// add a single h3 cell
    ///
    /// will trigger a re-compacting when `compact` is set
    #[inline]
    pub fn add_cell(&mut self, cell: H3Cell, compact: bool) {
        let res = cell.resolution() as usize;
        self.cells_by_resolution[res].push(cell);
        if compact {
            self.compact_from_resolution_up(res, &[]);
        }
    }

    ///
    ///
    pub fn add_cells<I>(&mut self, cells: I, compact: bool)
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell> + Index,
    {
        let mut resolutions_touched = HashSet::new();
        for cell in cells {
            let res = cell.resolution() as usize;
            resolutions_touched.insert(res);
            self.cells_by_resolution[res].push(*(cell.borrow()));
        }

        if compact {
            let recompact_res = resolutions_touched.iter().max();
            if let Some(rr) = recompact_res {
                self.compact_from_resolution_up(
                    *rr,
                    &resolutions_touched.drain().collect::<Vec<usize>>(),
                );
            }
        }
    }

    /// iterate over the compacted (or not, depending on if `compact` was called) contents
    pub fn iter_compacted_cells(&self) -> CompactedCellVecCompactedIterator {
        CompactedCellVecCompactedIterator {
            compacted_vec: self,
            current_resolution: H3_MIN_RESOLUTION as usize,
            current_pos: 0,
        }
    }

    /// get the compacted cells of the given resolution
    ///
    /// parent cells at lower resolutions will not be uncompacted
    pub fn get_compacted_cells_at_resolution(&self, resolution: u8) -> &[H3Cell] {
        &self.cells_by_resolution[resolution as usize]
    }

    /// iterate over the uncompacted cells.
    ///
    /// cells at lower resolutions will be decompacted, cells at higher resolutions will be
    /// ignored.
    pub fn iter_uncompacted_cells(
        &self,
        resolution: u8,
    ) -> CompactedCellVecUncompactedIterator<'_> {
        CompactedCellVecUncompactedIterator {
            compacted_vec: self,
            current_resolution: H3_MIN_RESOLUTION as usize,
            current_pos: 0,
            current_uncompacted: Default::default(),
            iteration_resolution: resolution as usize,
        }
    }

    /// deduplicate the internal cell vectors
    pub fn dedup(&mut self) {
        self.cells_by_resolution.iter_mut().for_each(|cells| {
            cells.sort_unstable();
            cells.dedup();
        });
        self.purge_children();
    }

    /// the finest resolution contained
    pub fn finest_resolution_contained(&self) -> Option<u8> {
        for resolution in H3_RESOLUTION_RANGE_USIZE.rev() {
            if !self.cells_by_resolution[resolution].is_empty() {
                return Some(resolution as u8);
            }
        }
        None
    }

    /// compact all resolution from the given to 0
    ///
    /// resolutions are skipped when the compacting of the
    /// former finer resolution added no new cells to
    /// the parent resolution unless include_resolutions
    /// forces the recompacting of a given resolution
    fn compact_from_resolution_up(&mut self, resolution: usize, include_resolutions: &[usize]) {
        let mut resolutions_touched = include_resolutions.iter().cloned().collect::<HashSet<_>>();
        resolutions_touched.insert(resolution);

        for res in ((H3_MIN_RESOLUTION as usize)..=resolution).rev() {
            if !resolutions_touched.contains(&res) {
                // no new cells have been added to this resolution
                continue;
            }

            let mut cells_to_compact = std::mem::take(&mut self.cells_by_resolution[res]);
            cells_to_compact.sort_unstable();
            cells_to_compact.dedup();
            let compacted = compact(&cells_to_compact);
            for cell in compacted.iter() {
                let res = cell.resolution() as usize;
                resolutions_touched.insert(res);
                self.cells_by_resolution[res].push(cell);
            }
        }
        self.purge_children();
    }

    /// purge children of cells already contained in lower resolutions
    fn purge_children(&mut self) {
        let mut lowest_resolution = None;
        for (r, cells) in self.cells_by_resolution.iter().enumerate() {
            if lowest_resolution.is_none() && !cells.is_empty() {
                lowest_resolution = Some(r);
                break;
            }
        }

        if let Some(lowest_res) = lowest_resolution {
            let mut known_cells = self.cells_by_resolution[lowest_res]
                .iter()
                .cloned()
                .collect::<H3CellSet>();

            for r in (lowest_res + 1)..=(H3_MAX_RESOLUTION as usize) {
                let mut orig_cells = std::mem::take(&mut self.cells_by_resolution[r]);
                orig_cells.drain(..).for_each(|cell| {
                    let is_parent_known = (lowest_res..r).any(|parent_res| {
                        known_cells.contains(&cell.get_parent_unchecked(parent_res as u8))
                    });
                    if !is_parent_known {
                        known_cells.insert(cell);
                        self.cells_by_resolution[r].push(cell);
                    }
                });
            }
        }
    }
}

impl Default for CompactedCellVec {
    fn default() -> Self {
        CompactedCellVec::new()
    }
}

impl FromIterator<H3Cell> for CompactedCellVec {
    fn from_iter<T: IntoIterator<Item = H3Cell>>(iter: T) -> Self {
        let mut cv = Self::new();
        cv.add_cells(iter, false);
        cv.compact();
        cv
    }
}

impl From<Vec<H3Cell>> for CompactedCellVec {
    fn from(mut in_vec: Vec<H3Cell>) -> Self {
        let mut cv = Self::new();
        cv.add_cells(in_vec.drain(..), false);
        cv.compact();
        cv
    }
}

pub struct CompactedCellVecCompactedIterator<'a> {
    compacted_vec: &'a CompactedCellVec,
    current_resolution: usize,
    current_pos: usize,
}

impl<'a> Iterator for CompactedCellVecCompactedIterator<'a> {
    type Item = H3Cell;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_resolution <= (H3_MAX_RESOLUTION as usize) {
            if let Some(value) = self.compacted_vec.cells_by_resolution[self.current_resolution]
                .get(self.current_pos)
            {
                self.current_pos += 1;
                return Some(*value);
            } else {
                self.current_pos = 0;
                self.current_resolution += 1;
            }
        }
        None
    }
}

pub struct CompactedCellVecUncompactedIterator<'a> {
    compacted_vec: &'a CompactedCellVec,
    current_resolution: usize,
    current_pos: usize,
    current_uncompacted: IndexVec<H3Cell>,
    iteration_resolution: usize,
}

impl<'a> Iterator for CompactedCellVecUncompactedIterator<'a> {
    type Item = H3Cell;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_resolution <= self.iteration_resolution {
            if self.current_resolution == self.iteration_resolution {
                let value = self.compacted_vec.cells_by_resolution[self.current_resolution]
                    .get(self.current_pos);
                self.current_pos += 1;
                return value.cloned();
            } else if let Some(next) = self.current_uncompacted.pop() {
                return Some(next);
            } else if let Some(next_parent) = self.compacted_vec.cells_by_resolution
                [self.current_resolution]
                .get(self.current_pos)
            {
                self.current_uncompacted =
                    next_parent.get_children(self.iteration_resolution as u8);
                self.current_pos += 1;
                continue;
            } else {
                self.current_resolution += 1;
                self.current_pos = 0;
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    #[cfg(feature = "use-serde")]
    use bincode::{deserialize, serialize};

    use crate::collections::CompactedCellVec;

    #[test]
    fn compactedvec_is_empty() {
        let mut cv = CompactedCellVec::new();
        assert!(cv.is_empty());
        assert_eq!(cv.len(), 0);
        cv.add_cell(0x89283080ddbffff_u64.try_into().unwrap(), false);
        assert!(!cv.is_empty());
        assert_eq!(cv.len(), 1);
    }

    #[cfg(feature = "use-serde")]
    #[test]
    fn compactedvec_serde_roundtrip() {
        let mut cv = CompactedCellVec::new();
        cv.add_cell(0x89283080ddbffff_u64.try_into().unwrap(), false);
        let serialized_data = serialize(&cv).unwrap();

        let cv_2: CompactedCellVec = deserialize(&serialized_data).unwrap();
        assert_eq!(cv, cv_2);
    }
}
