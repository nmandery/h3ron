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

pub trait GetCellEdges {
    type EdgeWeightType;

    /// get all edges and their values originating from cell `cell`
    #[allow(clippy::complexity)]
    fn get_edges_originating_from(
        &self,
        cell: &H3Cell,
    ) -> Result<Vec<(H3DirectedEdge, EdgeWeight<Self::EdgeWeightType>)>, Error>;
}

pub trait GetEdge {
    type EdgeWeightType;

    fn get_edge(
        &self,
        edge: &H3DirectedEdge,
    ) -> Result<Option<EdgeWeight<Self::EdgeWeightType>>, Error>;
}

impl<G> GetEdge for G
where
    G: GetCellEdges,
{
    type EdgeWeightType = G::EdgeWeightType;

    fn get_edge(
        &self,
        edge: &H3DirectedEdge,
    ) -> Result<Option<EdgeWeight<Self::EdgeWeightType>>, Error> {
        let cell = edge.origin_cell()?;
        for (found_edge, value) in self.get_edges_originating_from(&cell)? {
            if edge == &found_edge {
                return Ok(Some(value));
            }
        }
        Ok(None)
    }
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
