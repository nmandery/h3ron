use std::marker::PhantomData;
use std::ops::Add;

use num_traits::Zero;

use crate::error::Error;
use h3ron::collections::H3Treemap;
use h3ron::{H3Cell, H3DirectedEdge, HasH3Resolution};

use crate::graph::node::NodeType;
use crate::graph::{EdgeWeight, GetCellNode, GetEdge};

/// wrapper to exclude cells from traversal during routing
pub struct ExcludeCells<'a, G, W> {
    cells_to_exclude: &'a H3Treemap<H3Cell>,
    inner_graph: &'a G,
    phantom_weight: PhantomData<W>,
}

impl<'a, G, W> ExcludeCells<'a, G, W>
where
    G: GetCellNode + GetEdge<EdgeWeightType = W> + HasH3Resolution,
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
{
    pub fn new(inner_graph: &'a G, cells_to_exclude: &'a H3Treemap<H3Cell>) -> Self {
        Self {
            cells_to_exclude,
            inner_graph,
            phantom_weight: Default::default(),
        }
    }
}

impl<'a, G, W> GetCellNode for ExcludeCells<'a, G, W>
where
    G: GetCellNode + GetEdge<EdgeWeightType = W> + HasH3Resolution,
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
{
    fn get_cell_node(&self, cell: &H3Cell) -> Option<NodeType> {
        if self.cells_to_exclude.contains(cell) {
            None
        } else {
            self.inner_graph.get_cell_node(cell)
        }
    }
}

impl<'a, G, W> GetEdge for ExcludeCells<'a, G, W>
where
    G: GetCellNode + GetEdge<EdgeWeightType = W> + HasH3Resolution,
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
{
    type EdgeWeightType = G::EdgeWeightType;

    fn get_edge(
        &self,
        edge: &H3DirectedEdge,
    ) -> Result<Option<EdgeWeight<Self::EdgeWeightType>>, Error> {
        if self.cells_to_exclude.contains(&edge.destination_cell()?) {
            Ok(None)
        } else if let Some(edge_value) = self.inner_graph.get_edge(edge)? {
            // remove the longedge when it contains any excluded cell
            let filtered_longedge_opt =
                if let Some((longedge, longedge_weight)) = edge_value.longedge {
                    if longedge.is_disjoint(self.cells_to_exclude) {
                        Some((longedge, longedge_weight))
                    } else {
                        None
                    }
                } else {
                    None
                };

            Ok(Some(EdgeWeight {
                weight: edge_value.weight,
                longedge: filtered_longedge_opt,
            }))
        } else {
            Ok(None)
        }
    }
}

impl<'a, G, W> HasH3Resolution for ExcludeCells<'a, G, W>
where
    G: GetCellNode + GetEdge<EdgeWeightType = W> + HasH3Resolution,
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
{
    fn h3_resolution(&self) -> u8 {
        self.inner_graph.h3_resolution()
    }
}
