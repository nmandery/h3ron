use std::convert::TryFrom;
use std::ops::Add;

use geo::MultiPolygon;
use num_traits::Zero;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use h3ron::collections::ThreadPartitionedMap;
use h3ron::iter::H3EdgesBuilder;
use h3ron::{H3Cell, H3Edge, HasH3Resolution};

use crate::algorithm::covered_area::{cells_covered_area, CoveredArea};
use crate::error::Error;
use crate::graph::longedge::LongEdge;
use crate::graph::node::NodeType;
use crate::graph::{EdgeValue, GetEdge, GetNodeType, GetStats, GraphStats, H3EdgeGraph};

#[derive(Serialize, Deserialize, Clone)]
pub struct OwnedEdgeValue<W: Send + Sync> {
    weight: W,
    longedge: Option<(LongEdge, W)>,
}

fn to_longedge_edges<W>(
    input_graph: H3EdgeGraph<W>,
    min_longedge_length: usize,
) -> Result<ThreadPartitionedMap<H3Edge, OwnedEdgeValue<W>>, Error>
where
    W: PartialOrd + PartialEq + Add<Output = W> + Copy + Send + Sync,
{
    if min_longedge_length < 2 {
        return Err(Error::Other(
            "minimum longedge length must be >= 2".to_string(),
        ));
    }

    let mut edges: ThreadPartitionedMap<_, _> = Default::default();

    let mut parts: Vec<_> = input_graph
        .edges
        .partitions()
        .par_iter()
        .map::<_, Result<_, Error>>(|partition| {
            let mut new_edges = Vec::with_capacity(partition.len());
            let mut edge_builder = H3EdgesBuilder::new();

            for (edge, weight) in partition.iter() {
                let mut graph_entry = OwnedEdgeValue {
                    weight: *weight,
                    longedge: None,
                };

                // number of upstream edges leading to this one
                let num_edges_leading_to_this_one = edge_builder
                    .from_origin_cell(&edge.origin_index_unchecked())
                    .filter(|new_edge| new_edge != edge) // ignore the backwards edge
                    .map(|new_edge| new_edge.reversed_unchecked())
                    .filter(|new_edge| input_graph.edges.contains(new_edge))
                    .count();

                // attempt to build a longedge when this edge is either the end of a path, or a path
                // starting after a conjunction of multiple edges
                if num_edges_leading_to_this_one != 1 {
                    let mut edge_path = vec![*edge];
                    let mut longedge_weight = *weight;

                    let mut last_edge = *edge;
                    loop {
                        let last_edge_reverse = last_edge.reversed_unchecked();
                        // follow the edges until the end or a conjunction is reached
                        let following_edges: Vec<_> = edge_builder
                            .from_origin_cell(&last_edge.destination_index_unchecked())
                            .filter_map(|this_edge| {
                                if this_edge != last_edge_reverse {
                                    input_graph.edges.get_key_value(&this_edge)
                                } else {
                                    None
                                }
                            })
                            .collect();

                        // found no further continuing edge or conjunction
                        if following_edges.len() != 1 {
                            break;
                        }
                        let following_edge = *(following_edges[0].0);

                        // stop when encountering circles
                        if edge_path.contains(&following_edge) {
                            break;
                        }

                        edge_path.push(following_edge);
                        longedge_weight = *(following_edges[0].1) + longedge_weight;
                        // find the next following edge in the next iteration of the loop
                        last_edge = following_edge;
                    }

                    if edge_path.len() >= min_longedge_length {
                        graph_entry.longedge =
                            Some((LongEdge::try_from(edge_path)?, longedge_weight));
                    }
                }
                new_edges.push((*edge, graph_entry));
            }
            Ok(new_edges)
        })
        .collect::<Result<Vec<_>, _>>()?;

    for mut part in parts.drain(..) {
        edges.insert_many(part.drain(..))
    }
    Ok(edges)
}

/// a prepared graph which can be used for routing
#[derive(Serialize, Deserialize, Clone)]
pub struct PreparedH3EdgeGraph<W: Send + Sync> {
    edges: ThreadPartitionedMap<H3Edge, OwnedEdgeValue<W>>,
    h3_resolution: u8,
    graph_nodes: ThreadPartitionedMap<H3Cell, NodeType>,
}

unsafe impl<W: Sync + Send> Sync for PreparedH3EdgeGraph<W> {}

impl<W: Sync + Send> PreparedH3EdgeGraph<W> {
    pub fn num_long_edges(&self) -> usize {
        self.edges
            .iter()
            .map(|(_, edge_value)| if edge_value.longedge.is_some() { 1 } else { 0 })
            .sum()
    }
}

impl<W> HasH3Resolution for PreparedH3EdgeGraph<W>
where
    W: Send + Sync,
{
    fn h3_resolution(&self) -> u8 {
        self.h3_resolution
    }
}

impl<W> GetStats for PreparedH3EdgeGraph<W>
where
    W: Send + Sync,
{
    fn get_stats(&self) -> GraphStats {
        GraphStats {
            h3_resolution: self.h3_resolution,
            num_nodes: self.graph_nodes.len(),
            num_edges: self.edges.len(),
        }
    }
}

impl<W: Send + Sync> GetNodeType for PreparedH3EdgeGraph<W> {
    fn get_node_type(&self, cell: &H3Cell) -> Option<&NodeType> {
        self.graph_nodes.get(cell)
    }
}

impl<W: Send + Sync + Copy> GetEdge for PreparedH3EdgeGraph<W> {
    type WeightType = W;

    fn get_edge(&self, edge: &H3Edge) -> Option<EdgeValue<Self::WeightType>> {
        self.edges.get(edge).map(|owned_edge_value| EdgeValue {
            weight: owned_edge_value.weight,
            longedge: owned_edge_value.longedge.as_ref().map(|l| (&l.0, l.1)),
        })
    }
}

pub fn prepare_h3edgegraph<W>(graph: H3EdgeGraph<W>) -> Result<PreparedH3EdgeGraph<W>, Error>
where
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
{
    let h3_resolution = graph.h3_resolution();
    let graph_nodes = graph.nodes();
    let edges = to_longedge_edges(graph, 3)?;
    Ok(PreparedH3EdgeGraph {
        edges,
        graph_nodes,
        h3_resolution,
    })
}

impl<W> TryFrom<H3EdgeGraph<W>> for PreparedH3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
{
    type Error = Error;

    fn try_from(graph: H3EdgeGraph<W>) -> std::result::Result<Self, Self::Error> {
        prepare_h3edgegraph(graph)
    }
}

impl<W> From<PreparedH3EdgeGraph<W>> for H3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
{
    fn from(mut prepared_graph: PreparedH3EdgeGraph<W>) -> Self {
        H3EdgeGraph {
            edges: prepared_graph
                .edges
                .drain()
                .map(|(edge, edge_value)| (edge, edge_value.weight))
                .collect(),
            h3_resolution: prepared_graph.h3_resolution,
        }
    }
}

impl<W> CoveredArea for PreparedH3EdgeGraph<W>
where
    W: Send + Sync,
{
    fn covered_area(&self, reduce_resolution_by: u8) -> Result<MultiPolygon<f64>, Error> {
        cells_covered_area(
            self.graph_nodes.iter().map(|(cell, _)| cell),
            self.h3_resolution(),
            reduce_resolution_by,
        )
    }
}
