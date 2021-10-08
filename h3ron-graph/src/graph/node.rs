use std::ops::{Add, AddAssign};

use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use h3ron::H3Cell;

use crate::graph::GetNode;

#[derive(PartialEq, Debug, Copy, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Origin,
    Destination,
    OriginAndDestination,
}

impl NodeType {
    pub fn is_origin(&self) -> bool {
        match self {
            NodeType::Origin => true,
            NodeType::Destination => false,
            NodeType::OriginAndDestination => true,
        }
    }

    pub fn is_destination(&self) -> bool {
        match self {
            NodeType::Origin => false,
            NodeType::Destination => true,
            NodeType::OriginAndDestination => true,
        }
    }
}

impl Add<NodeType> for NodeType {
    type Output = NodeType;

    fn add(self, rhs: NodeType) -> Self::Output {
        if rhs == self {
            self
        } else {
            Self::OriginAndDestination
        }
    }
}

impl AddAssign<NodeType> for NodeType {
    fn add_assign(&mut self, rhs: NodeType) {
        if self != &rhs {
            *self = Self::OriginAndDestination
        }
    }
}

/// the type of membershipt a cell has within a graph
pub enum GapBridgedCellNode {
    /// the cell is connected to the graph
    DirectConnection(H3Cell),

    /// cell `self.0` is not connected to the graph, but the next best neighbor `self.1` is
    WithGap(H3Cell, H3Cell),

    /// cell is outside of the graph
    NoConnection(H3Cell),
}

impl GapBridgedCellNode {
    pub fn cell(&self) -> H3Cell {
        match self {
            Self::DirectConnection(cell) => *cell,
            Self::WithGap(cell, _) => *cell,
            Self::NoConnection(cell) => *cell,
        }
    }

    pub fn corresponding_cell_in_graph(&self) -> Option<H3Cell> {
        match self {
            Self::DirectConnection(cell) => Some(*cell),
            Self::WithGap(_, cell) => Some(*cell),
            _ => None,
        }
    }
}

pub trait GetGapBridgedCellNodes {
    /// TODO: This function should probably take an iterator instead of a vec
    fn gap_bridged_cell_nodes<B, F>(
        &self,
        cells: Vec<H3Cell>,
        nodetype_filter_fn: F,
        num_gap_cells_to_graph: u32,
    ) -> B
    where
        B: FromParallelIterator<GapBridgedCellNode>,
        F: Fn(&NodeType) -> bool + Send + Sync + Copy;
}

impl<G> GetGapBridgedCellNodes for G
where
    G: GetNode + Sync,
{
    fn gap_bridged_cell_nodes<B, F>(
        &self,
        mut cells: Vec<H3Cell>,
        nodetype_filter_fn: F,
        num_gap_cells_to_graph: u32,
    ) -> B
    where
        B: FromParallelIterator<GapBridgedCellNode>,
        F: Fn(&NodeType) -> bool + Send + Sync + Copy,
    {
        cells.sort_unstable();
        cells.dedup();
        cells
            .par_iter()
            .map(|cell: &H3Cell| {
                if self.get_node(cell).map(nodetype_filter_fn).unwrap_or(false) {
                    GapBridgedCellNode::DirectConnection(*cell)
                } else if num_gap_cells_to_graph > 0 {
                    // attempt to find the next neighboring cell which is part of the graph
                    let mut neighbors = cell.k_ring_distances(1, num_gap_cells_to_graph.max(1));
                    neighbors.sort_unstable_by_key(|neighbor| neighbor.0);

                    // possible improvement: choose the neighbor with the best connectivity or
                    // the edge with the smallest weight
                    let mut selected_neighbor: Option<H3Cell> = None;
                    for neighbor in neighbors {
                        if self
                            .get_node(&neighbor.1)
                            .map(nodetype_filter_fn)
                            .unwrap_or(false)
                        {
                            selected_neighbor = Some(neighbor.1);
                            break;
                        }
                    }
                    selected_neighbor
                        .map(|neighbor| GapBridgedCellNode::WithGap(*cell, neighbor))
                        .unwrap_or_else(|| GapBridgedCellNode::NoConnection(*cell))
                } else {
                    GapBridgedCellNode::NoConnection(*cell)
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::graph::node::NodeType;

    #[test]
    fn test_nodetype_add() {
        assert_eq!(NodeType::Origin, NodeType::Origin + NodeType::Origin);
        assert_eq!(
            NodeType::Destination,
            NodeType::Destination + NodeType::Destination
        );
        assert_eq!(
            NodeType::OriginAndDestination,
            NodeType::Origin + NodeType::Destination
        );
        assert_eq!(
            NodeType::OriginAndDestination,
            NodeType::OriginAndDestination + NodeType::Destination
        );
        assert_eq!(
            NodeType::OriginAndDestination,
            NodeType::Destination + NodeType::Origin
        );
    }

    #[test]
    fn test_nodetype_addassign() {
        let mut n1 = NodeType::Origin;
        n1 += NodeType::Origin;
        assert_eq!(n1, NodeType::Origin);

        let mut n2 = NodeType::Origin;
        n2 += NodeType::OriginAndDestination;
        assert_eq!(n2, NodeType::OriginAndDestination);

        let mut n3 = NodeType::Destination;
        n3 += NodeType::OriginAndDestination;
        assert_eq!(n3, NodeType::OriginAndDestination);

        let mut n4 = NodeType::Destination;
        n4 += NodeType::Origin;
        assert_eq!(n4, NodeType::OriginAndDestination);
    }
}
