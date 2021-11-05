use std::ops::{Add, AddAssign};

use serde::{Deserialize, Serialize};

use h3ron::H3Cell;

use crate::graph::GetNodeType;

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
    /// get the closest corresponding node in the graph to the
    /// given cell
    fn gap_bridged_corresponding_node(
        &self,
        cell: &H3Cell,
        num_gap_cells_to_graph: u32,
    ) -> GapBridgedCellNode {
        self.gap_bridged_corresponding_node_filtered(cell, num_gap_cells_to_graph, |_, _| true)
    }

    fn gap_bridged_corresponding_node_filtered<F>(
        &self,
        cell: &H3Cell,
        num_gap_cells_to_graph: u32,
        nodetype_filter_fn: F,
    ) -> GapBridgedCellNode
    where
        F: Fn(&H3Cell, &NodeType) -> bool + Send + Sync + Copy;
}

impl<G> GetGapBridgedCellNodes for G
where
    G: GetNodeType + Sync,
{
    fn gap_bridged_corresponding_node_filtered<F>(
        &self,
        cell: &H3Cell,
        num_gap_cells_to_graph: u32,
        nodetype_filter_fn: F,
    ) -> GapBridgedCellNode
    where
        F: Fn(&H3Cell, &NodeType) -> bool + Send + Sync + Copy,
    {
        if self
            .get_node_type(cell)
            .map(|node_type| nodetype_filter_fn(cell, node_type))
            .unwrap_or(false)
        {
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
                    .get_node_type(&neighbor.1)
                    .map(|node_type| nodetype_filter_fn(&neighbor.1, node_type))
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
