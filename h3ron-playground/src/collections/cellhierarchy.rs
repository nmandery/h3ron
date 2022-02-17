use std::iter::repeat_with;

#[cfg(feature = "use-serde")]
use serde::{Deserialize, Serialize};

use h3ron::{H3Cell, H3Direction};

// there are only 122 res0 nodes, but the `Array` trait in tinyvec is only implemented for smaller counts
// and powers of two. So 128 is the next possible value.
const RES0_NODES_CAPACITY: usize = 128;

/// A map using a tree based on the H3 hierarchy levels internally.
///
/// This is a naive draft at best, things work, but the performance is far from
/// optimal. Maybe some collections from `std::collections` could be consulted
/// as examples for improvements.
///
/// This collections was implemented as a experiment, maybe a suitable use case arises
/// in the future.
#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    feature = "use-serde",
    serde(bound(serialize = "V: Serialize", deserialize = "V: Deserialize<'de>",))
)]
pub struct H3CellHierarchyMap<V> {
    res0_nodes: tinyvec::ArrayVec<[Option<Box<IndexHierarchyMapNode<V>>>; RES0_NODES_CAPACITY]>,
}

impl<V> Default for H3CellHierarchyMap<V> {
    fn default() -> Self {
        let mut res0_nodes = tinyvec::ArrayVec::new();
        res0_nodes.fill(repeat_with(|| None));
        Self { res0_nodes }
    }
}

#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[cfg_attr(
    feature = "use-serde",
    serde(bound(serialize = "V: Serialize", deserialize = "V: Deserialize<'de>",))
)]
struct IndexHierarchyMapNode<V> {
    /// Children of this node in all directions - if there are any.
    children: [Option<Box<IndexHierarchyMapNode<V>>>; 7],

    /// the value attached to this node - if there is any
    value: Option<V>,
}

impl<V> Default for IndexHierarchyMapNode<V> {
    fn default() -> Self {
        Self {
            children: Default::default(),
            value: None,
        }
    }
}

impl<V> IndexHierarchyMapNode<V> {
    /// count the number of values in this tree node
    pub fn count(&self) -> usize {
        let mut count = if self.value.is_some() { 1 } else { 0 };
        for child in self.children.iter().flatten() {
            count += child.count()
        }
        count
    }

    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// remove all empty child nodes
    ///
    /// Returns the number of children which have been removed
    pub fn prune(&mut self) -> usize {
        prune(self.children.iter_mut())
    }
}

// TODO: get-multiple using toposort
// TODO: https://github.com/serde-rs/serde/issues/1503

impl<V> H3CellHierarchyMap<V> {
    /// count the number of values in this tree
    pub fn count(&self) -> usize {
        let mut count = 0;
        for node in self.res0_nodes.iter().flatten() {
            count += node.count()
        }
        count
    }

    pub fn is_empty(&self) -> bool {
        self.count() == 0
    }

    /// get the value for the given index
    pub fn get(&self, cell: &H3Cell) -> Option<&V> {
        let mut node = self.res0_nodes[cell.get_base_cell_number() as usize].as_ref();
        let mut directions = H3Direction::iter_directions_over_resolutions(cell);
        loop {
            match node {
                Some(indexmapnode) => match directions.next() {
                    Some(direction) => {
                        node = indexmapnode.children[direction.unwrap() as usize].as_ref()
                    }
                    None => return indexmapnode.value.as_ref(),
                },
                None => return None,
            }
        }
    }

    fn replace(&mut self, cell: H3Cell, value: Option<V>) -> Option<V> {
        let mut node = {
            let base_cell_idx = cell.get_base_cell_number() as usize;
            match self.res0_nodes[base_cell_idx].as_mut() {
                Some(ims) => ims,
                None => {
                    self.res0_nodes[base_cell_idx] =
                        Some(Box::new(IndexHierarchyMapNode::default()));
                    self.res0_nodes[base_cell_idx].as_mut().unwrap()
                }
            }
        };
        let mut directions = H3Direction::iter_directions_over_resolutions(&cell);
        loop {
            match directions.next() {
                Some(direction) => {
                    node = {
                        let direction_idx = direction.unwrap() as usize;
                        if node.children[direction_idx].is_none() {
                            node.children[direction_idx] =
                                Some(Box::new(IndexHierarchyMapNode::default()));
                        }
                        node.children[direction_idx].as_mut().unwrap()
                    }
                }
                None => return std::mem::replace(&mut node.value, value),
            }
        }
    }

    pub fn insert(&mut self, index: H3Cell, value: V) -> Option<V> {
        self.replace(index, Some(value))
    }

    /// remove a value from the map
    ///
    /// After removing many values, the size of the whole map can be
    /// reduced by calling `prune`
    pub fn remove(&mut self, index: H3Cell) -> Option<V> {
        self.replace(index, None)
    }

    /// remove all empty child node
    ///
    /// Returns the number of children which have been removed
    pub fn prune(&mut self) -> usize {
        prune(self.res0_nodes.iter_mut())
    }
}

/// remove all empty child node
///
/// Returns the number of children which have been removed
fn prune<V>(it: core::slice::IterMut<Option<Box<IndexHierarchyMapNode<V>>>>) -> usize {
    let mut count = 0;
    for child in it {
        let mut prune_child = false;
        if let Some(child) = child {
            count += child.prune();
            if child.is_empty() {
                prune_child = true;
            }
        }
        if prune_child {
            *child = None;
            count += 1;
        }
    }
    count
}

impl<V> FromIterator<(H3Cell, V)> for H3CellHierarchyMap<V> {
    fn from_iter<I: IntoIterator<Item = (H3Cell, V)>>(iter: I) -> Self {
        let mut map = Self::default();
        for (k, v) in iter {
            map.insert(k, v);
        }
        map
    }
}

#[cfg(test)]
mod tests {
    use crate::collections::cellhierarchy::{H3CellHierarchyMap, RES0_NODES_CAPACITY};
    use h3ron::{res0_cell_count, H3Cell, Index};

    #[test]
    fn assert_res0_index_count_is_unchanged() {
        // ensure to be informed in case of the unlikely event the number
        // of base cells changes.
        assert!(res0_cell_count() as usize <= RES0_NODES_CAPACITY);
    }

    #[test]
    fn treemap_get_from_empty() {
        let map: H3CellHierarchyMap<u8> = H3CellHierarchyMap::default();
        assert_eq!(map.get(&H3Cell::new(0x89283080ddbffff_u64)), None);
    }

    #[test]
    fn map_insert_get_remove() {
        let cell = H3Cell::new(0x89283080ddbffff_u64);
        let mut map: H3CellHierarchyMap<u8> = H3CellHierarchyMap::default();
        assert_eq!(map.insert(cell, 54u8), None);
        assert_eq!(map.count(), 1);
        assert!(!map.is_empty());
        assert_eq!(map.get(&cell), Some(&54));
        assert_eq!(map.remove(cell), Some(54));
        assert_eq!(map.count(), 0);
        assert!(map.is_empty());
    }

    #[test]
    fn map_count_empty() {
        let map: H3CellHierarchyMap<u8> = H3CellHierarchyMap::default();
        assert_eq!(map.count(), 0);
        assert!(map.is_empty());
    }
}
