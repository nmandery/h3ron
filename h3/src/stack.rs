use std::collections::{HashMap, HashSet};

use h3_sys::H3Index;

use crate::compact;
use crate::index::Index;

/// structure to keep compacted h3 indexes to allow more or less efficient
/// adding of further indexes
pub struct H3IndexStack {
    pub indexes_by_resolution: HashMap<u8, Vec<H3Index>>,
}

impl<'a> H3IndexStack {
    pub fn new() -> H3IndexStack {
        H3IndexStack {
            indexes_by_resolution: HashMap::new()
        }
    }

    /// append the contents of another Stack to this one
    ///
    /// Indexes get moved, see Vec::append
    ///
    /// will trigger a re-compacting
    pub fn append(&mut self, other: &mut Self, compact: bool) {
        let mut resolutions_touched = Vec::new();
        for (resolution, h3indexes) in other.indexes_by_resolution.iter_mut() {
            resolutions_touched.push(*resolution);
            self.indexes_by_resolution.entry(*resolution)
                .or_insert_with(Vec::new)
                .append(h3indexes);
        }
        if compact {
            if let Some(max_res) = resolutions_touched.iter().max() {
                self.compact_from_resolution_up(*max_res, resolutions_touched);
            }
        }
    }

    pub fn compact(&mut self) {
        let max_res = self.indexes_by_resolution.keys().max().copied();
        if let Some(r) = max_res {
            self.compact_from_resolution_up(r, vec![])
        }
    }

    /// append the contents of a vector
    ///
    /// Indexes get moved, see Vec::append
    pub fn append_to_resolution(&mut self, resolution: u8, h3indexes: &mut Vec<H3Index>, compact: bool) {
        self.indexes_by_resolution.entry(resolution)
            .or_insert_with(Vec::new)
            .append(h3indexes);
        if compact {
            self.compact_from_resolution_up(resolution, vec![]);
        }
    }

    pub fn len(&self) -> usize {
        self.indexes_by_resolution.values()
            .fold(0, |acc, h3indexes| acc + h3indexes.len())
    }

    pub fn is_empty(&self) -> bool {
        !self.indexes_by_resolution.values()
            .any(|h3indexes| !h3indexes.is_empty())
    }

    /// indexes must be of the same resolution
    ///
    /// will trigger a re-compacting
    pub fn add_indexes(&mut self, h3_indexes: &[H3Index], compact: bool) {
        if h3_indexes.is_empty() {
            return;
        }
        let resolution = Index::from(*h3_indexes.first().unwrap()).resolution();
        let res_vec = self.indexes_by_resolution.entry(resolution)
            .or_insert_with(Vec::new);
        h3_indexes.iter().for_each(|h| res_vec.push(*h));
        if compact {
            self.compact_from_resolution_up(resolution, vec![]);
        }
    }

    ///
    ///
    pub fn add_indexes_mixed_resolutions(&mut self, h3_indexes: &[H3Index], compact: bool) {
        let mut resolutions_touched = HashSet::new();
        for h3_index in h3_indexes {
            let res = Index::from(*h3_index).resolution();
            resolutions_touched.insert(res);
            self.indexes_by_resolution.entry(res)
                .or_insert_with(Vec::new)
                .push(*h3_index);
        }

        if compact {
            let recompact_res = resolutions_touched.iter().max();
            if let Some(rr) = recompact_res {
                self.compact_from_resolution_up(*rr, resolutions_touched.drain().collect::<Vec<u8>>());
            }
        }
    }

    /// compact all resolution from the given to 0
    ///
    /// resolutions are skipped when the compating of the
    /// former finer resolution added no new indexes to
    /// the parent resolution unless include_resolutions
    /// forces the recompacting of a given resolution
    fn compact_from_resolution_up(&mut self, resolution: u8, include_resolutions: Vec<u8>) {
        let mut resolutions_touched = HashSet::new();
        resolutions_touched.insert(resolution);
        for include_res in include_resolutions {
            resolutions_touched.insert(include_res);
        }

        for res in (0..=resolution).rev() {
            if !resolutions_touched.contains(&res) {
                // no new indexes have been added to this resolution
                continue;
            }

            if let Some(indexes_to_compact) = self.indexes_by_resolution.remove(&res) {
                let compacted = compact(&indexes_to_compact);
                for h3_index in compacted {
                    let res = Index::from(h3_index).resolution();
                    resolutions_touched.insert(res);
                    self.indexes_by_resolution.entry(res)
                        .or_insert_with(Vec::new)
                        .push(h3_index);
                }
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
