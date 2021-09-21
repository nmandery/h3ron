use std::convert::TryFrom;
use std::fmt::Debug;
use std::ops::Add;

use num_traits::Zero;
use rayon::prelude::*;

use crate::error::Error;
use crate::graph::{H3EdgeGraph, NodeType};
use h3ron::collections::ThreadPartitionedMap;
use h3ron::{H3Cell, HasH3Resolution};

pub struct RoutingH3EdgeGraph<W: Send + Sync> {
    pub graph: H3EdgeGraph<W>,
    graph_nodes: ThreadPartitionedMap<H3Cell, NodeType>,
}

impl<W> HasH3Resolution for RoutingH3EdgeGraph<W>
where
    W: Send + Sync,
{
    fn h3_resolution(&self) -> u8 {
        self.graph.h3_resolution()
    }
}

// TODO: move to graph.rs, make public
pub(crate) enum CellGraphMembership {
    /// the cell is connected to the graph
    DirectConnection(H3Cell),

    /// cell `self.0` is not connected to the graph, but the next best neighbor `self.1` is
    WithGap(H3Cell, H3Cell),

    /// cell is outside of the graph
    NoConnection(H3Cell),
}

impl CellGraphMembership {
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

///
///
/// All routing methods will silently ignore origin and destination cells which are not
/// part of the graph.
impl<W> RoutingH3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync + Debug,
{
    pub(crate) fn filtered_graph_membership<B, F>(
        &self,
        mut cells: Vec<H3Cell>,
        nodetype_filter_fn: F,
        num_gap_cells_to_graph: u32,
    ) -> B
    where
        B: FromParallelIterator<CellGraphMembership>,
        F: Fn(&NodeType) -> bool + Send + Sync + Copy,
    {
        // TODO: This function should probably take an iterator instead of a vec
        cells.sort_unstable();
        cells.dedup();
        cells
            .par_iter()
            .map(|cell: &H3Cell| {
                if self
                    .graph_nodes
                    .get(cell)
                    .map(nodetype_filter_fn)
                    .unwrap_or(false)
                {
                    CellGraphMembership::DirectConnection(*cell)
                } else if num_gap_cells_to_graph > 0 {
                    // attempt to find the next neighboring cell which is part of the graph
                    let mut neighbors = cell.k_ring_distances(1, num_gap_cells_to_graph.max(1));
                    neighbors.sort_unstable_by_key(|neighbor| neighbor.0);

                    // possible improvement: choose the neighbor with the best connectivity or
                    // the edge with the smallest weight
                    let mut selected_neighbor: Option<H3Cell> = None;
                    for neighbor in neighbors {
                        if self
                            .graph_nodes
                            .get(&neighbor.1)
                            .map(nodetype_filter_fn)
                            .unwrap_or(false)
                        {
                            selected_neighbor = Some(neighbor.1);
                            break;
                        }
                    }
                    selected_neighbor
                        .map(|neighbor| CellGraphMembership::WithGap(*cell, neighbor))
                        .unwrap_or_else(|| CellGraphMembership::NoConnection(*cell))
                } else {
                    CellGraphMembership::NoConnection(*cell)
                }
            })
            .collect()
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
