use std::convert::TryFrom;
use std::ops::Add;

use num_traits::Zero;

use h3ron::collections::ThreadPartitionedMap;
use h3ron::{H3Cell, HasH3Resolution};

use crate::error::Error;
use crate::graph::H3EdgeGraph;
use crate::node::{GetCellNode, NodeType};

pub struct RoutingH3EdgeGraph<W: Send + Sync> {
    pub graph: H3EdgeGraph<W>,
    graph_nodes: ThreadPartitionedMap<H3Cell, NodeType>,
}

unsafe impl<W: Sync + Send> Sync for RoutingH3EdgeGraph<W> {}

impl<W> HasH3Resolution for RoutingH3EdgeGraph<W>
where
    W: Send + Sync,
{
    fn h3_resolution(&self) -> u8 {
        self.graph.h3_resolution()
    }
}

impl<W: Send + Sync> GetCellNode for RoutingH3EdgeGraph<W> {
    fn get_node(&self, cell: &H3Cell) -> Option<&NodeType> {
        self.graph_nodes.get(cell)
    }
}

impl<W> TryFrom<H3EdgeGraph<W>> for RoutingH3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
{
    type Error = Error;

    fn try_from(graph: H3EdgeGraph<W>) -> std::result::Result<Self, Self::Error> {
        let graph_nodes = graph.nodes();
        Ok(Self { graph, graph_nodes })
    }
}
