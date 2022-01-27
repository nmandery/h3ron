use crate::graph::node::NodeType;
use crate::graph::GetCellNode;
use h3ron::H3Cell;

/// find the nearest nodes in the graph
pub trait NearestGraphNodes {
    /// get an iterator over the closest corresponding nodes in the graph to the
    /// given cell. The iterator will return all nodes with the
    /// same, smallest `k` <= `max_distance_k` which are part of the graph.
    fn nearest_graph_nodes(
        &self,
        cell: &H3Cell,
        max_distance_k: u32,
    ) -> NearestGraphNodesGetCellIter<Self>
    where
        Self: Sized;
}

pub struct NearestGraphNodesGetCellIter<'a, G> {
    graph: &'a G,
    neighbors_reversed: Vec<(u32, H3Cell)>,
    found_max_k: u32,
}

impl<'a, G> Iterator for NearestGraphNodesGetCellIter<'a, G>
where
    G: GetCellNode,
{
    type Item = (H3Cell, NodeType, u32);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((neighbor_k, neighbor_cell)) = self.neighbors_reversed.pop() {
            if neighbor_k > self.found_max_k {
                break;
            }

            if let Some(node_type) = self.graph.get_cell_node(&neighbor_cell) {
                self.found_max_k = neighbor_k;
                return Some((neighbor_cell, node_type, neighbor_k));
            }
        }
        None
    }
}

impl<G> NearestGraphNodes for G
where
    G: GetCellNode + Sync,
{
    fn nearest_graph_nodes(
        &self,
        cell: &H3Cell,
        max_distance_k: u32,
    ) -> NearestGraphNodesGetCellIter<G> {
        let mut neighbors = cell.k_ring_distances(0, max_distance_k);

        // reverse the order to gave the nearest neighbors first
        neighbors.sort_unstable_by_key(|neighbor| max_distance_k - neighbor.0);

        NearestGraphNodesGetCellIter {
            graph: self,
            neighbors_reversed: neighbors,
            found_max_k: max_distance_k,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithm::NearestGraphNodes;
    use crate::graph::node::NodeType;
    use crate::graph::GetCellNode;
    use h3ron::collections::H3CellSet;
    use h3ron::{H3Cell, Index};

    impl GetCellNode for H3CellSet {
        fn get_cell_node(&self, cell: &H3Cell) -> Option<NodeType> {
            self.get(cell).map(|_| NodeType::OriginAndDestination)
        }
    }

    #[test]
    fn nearest_finds_given_cell_first() {
        let cell = H3Cell::new(0x89283080ddbffff_u64);
        let mut cellset = H3CellSet::new();
        for ring_cell in cell.k_ring(3).iter() {
            cellset.insert(ring_cell);
        }
        assert_eq!(cellset.nearest_graph_nodes(&cell, 3).count(), 1);
        assert_eq!(
            cellset.nearest_graph_nodes(&cell, 3).next(),
            Some((cell, NodeType::OriginAndDestination, 0))
        );
    }

    #[test]
    fn nearest_finds_all_with_same_k() {
        let cell = H3Cell::new(0x89283080ddbffff_u64);
        let mut cellset = H3CellSet::new();
        let mut expected = H3CellSet::new();
        for (_, ring_cell) in cell.k_ring_distances(2, 3).iter().take(2) {
            cellset.insert(*ring_cell);
            expected.insert(*ring_cell);
        }
        for (_, ring_cell) in cell.k_ring_distances(4, 5).iter().take(2) {
            cellset.insert(*ring_cell);
        }
        assert_eq!(cellset.nearest_graph_nodes(&cell, 8).count(), 2);
        for (nearest_cell, _, _) in cellset.nearest_graph_nodes(&cell, 8) {
            assert!(expected.contains(&nearest_cell));
        }
    }
}
