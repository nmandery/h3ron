use serde::Serialize;

pub use h3edge::{H3EdgeGraph, H3EdgeGraphBuilder};
use h3ron::{H3Cell, H3Edge};
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
    fn get_stats(&self) -> GraphStats;
}

pub trait GetNodeType {
    fn get_node_type(&self, cell: &H3Cell) -> Option<&NodeType>;
}

#[derive(Clone)]
pub struct EdgeValue<'a, W> {
    pub weight: W,

    pub longedge: Option<(&'a LongEdge, W)>,
}

pub trait GetEdge {
    type WeightType;

    fn get_edge(&self, edge: &H3Edge) -> Option<EdgeValue<Self::WeightType>>;
}
