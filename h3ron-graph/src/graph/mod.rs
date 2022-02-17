use serde::Serialize;

use crate::error::Error;
pub use h3edge::{H3EdgeGraph, H3EdgeGraphBuilder};
use h3ron::{H3Cell, H3DirectedEdge};
use node::NodeType;
pub use prepared::PreparedH3EdgeGraph;

use crate::graph::longedge::LongEdge;

pub mod h3edge;
pub mod longedge;
pub mod modifiers;
pub mod node;
pub mod prepared;

#[derive(Serialize)]
pub struct GraphStats {
    pub h3_resolution: u8,
    pub num_nodes: usize,
    pub num_edges: usize,
}

pub trait GetStats {
    fn get_stats(&self) -> Result<GraphStats, Error>;
}

pub trait GetCellNode {
    fn get_cell_node(&self, cell: &H3Cell) -> Option<NodeType>;
}

pub trait IterateCellNodes<'a> {
    type CellNodeIterator;
    fn iter_cell_nodes(&'a self) -> Self::CellNodeIterator;
}

pub trait GetEdge {
    type EdgeWeightType;

    fn get_edge(
        &self,
        edge: &H3DirectedEdge,
    ) -> Result<Option<EdgeWeight<Self::EdgeWeightType>>, Error>;
}

#[derive(Clone)]
pub struct EdgeWeight<'a, W> {
    pub weight: W,

    pub longedge: Option<(&'a LongEdge, W)>,
}

impl<'a, W> From<W> for EdgeWeight<'a, W> {
    fn from(weight: W) -> Self {
        EdgeWeight {
            weight,
            longedge: None,
        }
    }
}
