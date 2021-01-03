use std::collections::HashSet;
use std::iter::FromIterator;
use std::slice::Iter;

use h3ron_h3_sys::H3Index;

use crate::compact;
use crate::index::Index;

/// structure to keep compacted h3ron indexes to allow more or less efficient
/// adding of further indexes
pub struct H3CompactedVec {
    indexes_by_resolution: [Vec<H3Index>; 16],
}

impl<'a> H3CompactedVec {
    pub fn new() -> H3CompactedVec {
        H3CompactedVec {
            indexes_by_resolution: Default::default()
        }
    }

    /// append the contents of another Stack to this one
    ///
    /// Indexes get moved, see Vec::append
    ///
    /// will trigger a re-compacting when compact is true
    pub fn append(&mut self, other: &mut Self, compact: bool) {
        let mut resolutions_touched = Vec::new();
        for resolution in 0..=15 {
            if !other.indexes_by_resolution[resolution].is_empty() {
                resolutions_touched.push(resolution);
                let mut h3indexes = std::mem::take(&mut other.indexes_by_resolution[resolution]);
                self.indexes_by_resolution[resolution].append(&mut h3indexes);
            }
        }
        if compact {
            if let Some(max_res) = resolutions_touched.iter().max() {
                self.compact_from_resolution_up(*max_res, &resolutions_touched);
            }
        }
    }

    pub fn compact(&mut self) {
        self.compact_from_resolution_up(15, &(0..=15).collect::<Vec<_>>())
    }

    /// append the contents of a vector
    ///
    /// Indexes get moved, see Vec::append
    pub fn append_to_resolution(&mut self, resolution: u8, h3indexes: &mut Vec<H3Index>, compact: bool) {
        self.indexes_by_resolution[resolution as usize].append(h3indexes);
        if compact {
            self.compact_from_resolution_up(resolution as usize, &[]);
        }
    }

    pub fn len(&self) -> usize {
        self.indexes_by_resolution.iter()
            .fold(0, |acc, h3indexes| acc + h3indexes.len())
    }

    pub fn is_empty(&self) -> bool {
        !self.indexes_by_resolution.iter()
            .any(|h3indexes| !h3indexes.is_empty())
    }

    /// check if the stack contains the index or any of its parents
    ///
    /// This function is pretty inefficient.
    pub fn contains(&self, h3index: H3Index) -> bool {
        if self.is_empty() {
            return false;
        }
        let mut index = Index::from(h3index);
        for r in index.resolution()..=0 {
            index = index.get_parent(r);
            if self.indexes_by_resolution[r as usize].contains(&index.h3index()) {
                return true;
            }
        }
        false
    }

    /// add a single h3ron index
    ///
    /// will trigger a re-compacting when `compact` is set
    pub fn add_index(&mut self, h3_index: H3Index, compact: bool) {
        let resolution = Index::from(h3_index).resolution();
        self.add_index_to_resolution(h3_index, resolution, compact);
    }

    /// add a single h3ron index
    ///
    /// the `resolution` parameter must match the resolution of the index. This method
    ///  only exists to skip the resolution check of `add_index`.
    pub fn add_index_to_resolution(&mut self, h3_index: H3Index, resolution: u8, compact:bool) {
        self.indexes_by_resolution[resolution as usize].push(h3_index);
        if compact {
            self.compact_from_resolution_up(resolution as usize, &[]);
        }
    }

    ///
    ///
    pub fn add_indexes(&mut self, h3_indexes: &[H3Index], compact: bool) {
        let mut resolutions_touched = HashSet::new();
        for h3_index in h3_indexes {
            let res = Index::from(*h3_index).resolution() as usize;
            resolutions_touched.insert(res);
            self.indexes_by_resolution[res].push(*h3_index);
        }

        if compact {
            let recompact_res = resolutions_touched.iter().max();
            if let Some(rr) = recompact_res {
                self.compact_from_resolution_up(*rr, &resolutions_touched.drain().collect::<Vec<usize>>());
            }
        }
    }

    pub fn add_indexes_from_iter<T: IntoIterator<Item=H3Index>>(&mut self, iter: T, compact: bool) {
        let mut cv = Self::new();
        for h3index in iter {
            self.add_index(h3index, false);
        }
        if compact {
            cv.compact();
        }
    }

    /// iterate over the compacted (or not, depending on if `compact` was called) contents
    pub fn iter_compacted_indexes(&self) -> H3CompactedVecCompactedIterator {
        H3CompactedVecCompactedIterator {
            compacted_vec: &self,
            current_resolution: 0,
            current_pos: 0,
        }
    }

    /// iterate over the compacted indexes at the given resolution
    ///
    /// parent indexes at lower resolutions will not be uncompacted
    pub fn iter_compacted_indexes_at_resolution(&self, resolution: u8) -> Iter<'_, H3Index> {
        self.indexes_by_resolution[resolution as usize].iter()
    }

    /// iterate over the uncompacted indexes.
    ///
    /// indexes at lower resolutions will be decompacted, indexes at higher resolutions will be
    /// ignored.
    pub fn iter_uncompacted_indexes(&self, resolution: u8) -> H3CompactedVecUncompactedIterator<'_> {
        H3CompactedVecUncompactedIterator {
            compacted_vec: self,
            current_resolution: 0,
            current_pos: 0,
            current_uncompacted: vec![],
            iteration_resolution: resolution as usize,
        }
    }

    pub fn dedup(&mut self) {
        self.indexes_by_resolution.iter_mut().for_each(|indexes| {
            indexes.sort_unstable();
            indexes.dedup();
        });
        self.purge_children();
    }

    /// the finest resolution contained
    pub fn finest_resolution_contained(&self) -> Option<u8> {
        for resolution in (0..=15).rev() {
            if !self.indexes_by_resolution[resolution].is_empty() {
                return Some(resolution as u8);
            }
        }
        None
    }

    /// compact all resolution from the given to 0
    ///
    /// resolutions are skipped when the compacting of the
    /// former finer resolution added no new indexes to
    /// the parent resolution unless include_resolutions
    /// forces the recompacting of a given resolution
    fn compact_from_resolution_up(&mut self, resolution: usize, include_resolutions: &[usize]) {
        let mut resolutions_touched = include_resolutions.iter().cloned().collect::<HashSet<_>>();
        resolutions_touched.insert(resolution);

        for res in (0..=resolution).rev() {
            if !resolutions_touched.contains(&res) {
                // no new indexes have been added to this resolution
                continue;
            }

            let mut indexes_to_compact = std::mem::take(&mut self.indexes_by_resolution[res]);
            indexes_to_compact.sort_unstable();
            indexes_to_compact.dedup();
            let compacted = compact(&indexes_to_compact);
            for h3_index in compacted {
                let res = Index::from(h3_index).resolution() as usize;
                resolutions_touched.insert(res);
                self.indexes_by_resolution[res].push(h3_index);
            }
        }
        self.purge_children();
    }

    /// purge children of h3indexes already contained in lower resolutions
    fn purge_children(&mut self) {
        let mut lowest_resolution = None;
        for (r, h3indexes) in self.indexes_by_resolution.iter().enumerate() {
            if lowest_resolution.is_none() && !h3indexes.is_empty() {
                lowest_resolution = Some(r);
                break;
            }
        }

        if let Some(lowest_res) = lowest_resolution {
            let mut known_indexes = self.indexes_by_resolution[lowest_res].iter()
                .cloned()
                .collect::<HashSet<_>>();

            for r in (lowest_res + 1)..=15 {
                let mut orig_h3indexes = std::mem::take(&mut self.indexes_by_resolution[r]);
                orig_h3indexes.drain(..).for_each(|h3index| {
                    let index = Index::from(h3index);
                    if !(lowest_res..r).any(|parent_res| known_indexes.contains(&index.get_parent(parent_res as u8).h3index())) {
                        known_indexes.insert(h3index);
                        self.indexes_by_resolution[r].push(h3index);
                    }
                });
            }
        }
    }
}

impl Default for H3CompactedVec {
    fn default() -> Self {
        H3CompactedVec::new()
    }
}

impl FromIterator<H3Index> for H3CompactedVec {
    fn from_iter<T: IntoIterator<Item=H3Index>>(iter: T) -> Self {
        let mut cv = Self::new();
        cv.add_indexes_from_iter(iter, true);
        cv
    }
}

impl FromIterator<Index> for H3CompactedVec {
    fn from_iter<T: IntoIterator<Item=Index>>(iter: T) -> Self {
        let mut cv = Self::new();
        for index in iter {
            cv.add_index(index.h3index(), false);
        }
        cv.compact();
        cv
    }
}

impl From<Vec<H3Index>> for H3CompactedVec {
    fn from(mut in_vec: Vec<H3Index>) -> Self {
        let mut cv = Self::new();
        for h3index in in_vec.drain(..) {
            cv.add_index(h3index, false);
        }
        cv.compact();
        cv
    }
}

pub struct H3CompactedVecCompactedIterator<'a> {
    compacted_vec: &'a H3CompactedVec,
    current_resolution: usize,
    current_pos: usize,
}

impl<'a> Iterator for H3CompactedVecCompactedIterator<'a> {
    type Item = H3Index;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_resolution <= 15 {
            if let Some(value) = self.compacted_vec.indexes_by_resolution[self.current_resolution].get(self.current_pos) {
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

pub struct H3CompactedVecUncompactedIterator<'a> {
    compacted_vec: &'a H3CompactedVec,
    current_resolution: usize,
    current_pos: usize,
    current_uncompacted: Vec<H3Index>,
    iteration_resolution: usize,
}

impl<'a> Iterator for H3CompactedVecUncompactedIterator<'a> {
    type Item = H3Index;

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_resolution <= self.iteration_resolution {
            if self.current_resolution == self.iteration_resolution {
                let value = self.compacted_vec.indexes_by_resolution[self.current_resolution].get(self.current_pos);
                self.current_pos += 1;
                return value.cloned();
            } else if let Some(next) = self.current_uncompacted.pop() {
                return Some(next);
            } else if let Some(next_parent) = self.compacted_vec.indexes_by_resolution[self.current_resolution].get(self.current_pos) {
                self.current_uncompacted = Index::from(*next_parent)
                    .get_children(self.iteration_resolution as u8)
                    .iter()
                    .map(|i| i.h3index())
                    .collect();
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
    use crate::collections::H3CompactedVec;

    #[test]
    fn test_compactedindexstack_is_empty() {
        let mut stack = H3CompactedVec::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
        stack.add_index(0x89283080ddbffff_u64, false);
        assert!(!stack.is_empty());
        assert_eq!(stack.len(), 1);
    }
}
