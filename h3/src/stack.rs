use std::collections::HashSet;

use h3_sys::H3Index;

use crate::compact;
use crate::index::Index;

/// structure to keep compacted h3 indexes to allow more or less efficient
/// adding of further indexes
pub struct H3IndexStack {
    pub indexes_by_resolution: [Vec<H3Index>; 16],
}

impl<'a> H3IndexStack {
    pub fn new() -> H3IndexStack {
        H3IndexStack {
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

    /// indexes must be of the same resolution
    ///
    /// will trigger a re-compacting when `compact` is set
    pub fn add_indexes(&mut self, h3_indexes: &[H3Index], compact: bool) {
        if h3_indexes.is_empty() {
            return;
        }
        let resolution = Index::from(*h3_indexes.first().unwrap()).resolution() as usize;
        h3_indexes.iter().for_each(|h| self.indexes_by_resolution[resolution].push(*h));
        if compact {
            self.compact_from_resolution_up(resolution, &[]);
        }
    }

    ///
    ///
    pub fn add_indexes_mixed_resolutions(&mut self, h3_indexes: &[H3Index], compact: bool) {
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

    pub fn dedup(&mut self) {
        self.indexes_by_resolution.iter_mut().for_each(|indexes| {
            indexes.sort_unstable();
            indexes.dedup();
        });
        self.purge_children();
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
            let mut known_indexes = self.indexes_by_resolution[lowest_res].iter().cloned().collect::<HashSet<_>>();

            for r in (lowest_res + 1)..=15 {
                let mut orig_h3indexes = std::mem::take(&mut self.indexes_by_resolution[r]);
                orig_h3indexes.drain(..).for_each(|h3index| {
                    let index = Index::from(h3index);
                    if ! (lowest_res..r).any(|parent_res| known_indexes.contains(&index.get_parent(parent_res as u8).h3index())) {
                        known_indexes.insert(h3index);
                        self.indexes_by_resolution[r].push(h3index);
                    }
                });
            }
        }
    }
}

impl Default for H3IndexStack {
    fn default() -> Self {
        H3IndexStack::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::stack::H3IndexStack;

    #[test]
    fn test_compactedindexstack_is_empty() {
        let mut stack = H3IndexStack::new();
        assert!(stack.is_empty());
        assert_eq!(stack.len(), 0);
        stack.add_indexes(vec![0x89283080ddbffff_u64].as_ref(), false);
        assert!(!stack.is_empty());
        assert_eq!(stack.len(), 1);
    }
}
