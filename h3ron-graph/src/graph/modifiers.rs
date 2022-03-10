use std::marker::PhantomData;

use crate::error::Error;
use h3ron::collections::H3Treemap;
use h3ron::{H3Cell, H3DirectedEdge, HasH3Resolution};

use crate::graph::node::NodeType;
use crate::graph::{EdgeWeight, GetCellEdges, GetCellNode};

/// wrapper to exclude cells from traversal during routing
pub struct ExcludeCells<'a, G, W> {
    cells_to_exclude: &'a H3Treemap<H3Cell>,
    inner_graph: &'a G,
    phantom_weight: PhantomData<W>,
}

impl<'a, G, W> ExcludeCells<'a, G, W>
where
    G: GetCellNode + GetCellEdges<EdgeWeightType = W> + HasH3Resolution,
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
    G: GetCellNode,
{
    fn get_cell_node(&self, cell: &H3Cell) -> Option<NodeType> {
        if self.cells_to_exclude.contains(cell) {
            None
        } else {
            self.inner_graph.get_cell_node(cell)
        }
    }
}

impl<'a, G, W> GetCellEdges for ExcludeCells<'a, G, W>
where
    G: GetCellEdges<EdgeWeightType = W>,
{
    type EdgeWeightType = G::EdgeWeightType;

    fn get_edges_originating_at(
        &self,
        cell: &H3Cell,
    ) -> Result<Vec<(H3DirectedEdge, EdgeWeight<Self::EdgeWeightType>)>, Error> {
        if self.cells_to_exclude.contains(cell) {
            Ok(vec![])
        } else {
            let found = self.inner_graph.get_edges_originating_at(cell)?;
            let mut not_excluded = Vec::with_capacity(found.len());
            for (edge, edge_value) in found {
                if self.cells_to_exclude.contains(&edge.destination_cell()?) {
                    continue;
                }

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

                not_excluded.push((
                    edge,
                    EdgeWeight {
                        weight: edge_value.weight,
                        longedge: filtered_longedge_opt,
                    },
                ));
            }
            Ok(not_excluded)
        }
    }
}

impl<'a, G, W> HasH3Resolution for ExcludeCells<'a, G, W>
where
    G: HasH3Resolution,
{
    fn h3_resolution(&self) -> u8 {
        self.inner_graph.h3_resolution()
    }
}
