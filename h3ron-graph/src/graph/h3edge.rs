use std::ops::Add;

use geo_types::MultiPolygon;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::algorithm::covered_area::{cells_covered_area, CoveredArea};
use h3ron::collections::{H3CellMap, ThreadPartitionedMap};
use h3ron::{H3Cell, H3Edge, HasH3Resolution, Index};

use crate::error::Error;
use crate::graph::node::NodeType;
use crate::graph::GetStats;

use super::GraphStats;

#[derive(Serialize, Deserialize, Clone)]
pub struct H3EdgeGraph<W: Send + Sync> {
    pub edges: ThreadPartitionedMap<H3Edge, W>,
    pub h3_resolution: u8,
}

impl<W> H3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Send + Sync,
{
    pub fn new(h3_resolution: u8) -> Self {
        Self {
            h3_resolution,
            edges: Default::default(),
        }
    }

    ///
    /// This has to generate the node list first, so its rather
    /// expensive to call.
    pub fn num_nodes(&self) -> usize {
        self.nodes().len()
    }

    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    pub fn edge_weight(&self, edge: &H3Edge) -> Option<&W> {
        self.edges.get(edge)
    }

    /// get all edges in the graph leading from this edge to neighbors
    pub fn edges_from_cell(&self, cell: &H3Cell) -> Vec<(&H3Edge, &W)> {
        cell.unidirectional_edges()
            .iter()
            .filter_map(|edge| self.edges.get_key_value(&edge))
            .collect()
    }

    /// get all edges in the graph leading to this cell from its neighbors
    pub fn edges_to_cell(&self, cell: &H3Cell) -> Vec<(&H3Edge, &W)> {
        cell.k_ring(1)
            .drain()
            .filter(|ring_cell| ring_cell != cell)
            .flat_map(|ring_cell| {
                ring_cell
                    .unidirectional_edges()
                    .drain()
                    .filter_map(|edge| self.edges.get_key_value(&edge))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn add_edge_using_cells(
        &mut self,
        cell_from: H3Cell,
        cell_to: H3Cell,
        weight: W,
    ) -> Result<(), Error> {
        let edge = cell_from.unidirectional_edge_to(&cell_to)?;
        self.add_edge(edge, weight)
    }

    pub fn add_edge_using_cells_bidirectional(
        &mut self,
        cell_from: H3Cell,
        cell_to: H3Cell,
        weight: W,
    ) -> Result<(), Error> {
        self.add_edge_using_cells(cell_from, cell_to, weight)?;
        self.add_edge_using_cells(cell_to, cell_from, weight)
    }

    pub fn add_edge(&mut self, edge: H3Edge, weight: W) -> Result<(), Error> {
        self.edges
            .insert_or_modify(edge, weight, edge_weight_selector);
        Ok(())
    }

    pub fn try_add(&mut self, mut other: H3EdgeGraph<W>) -> Result<(), Error> {
        if self.h3_resolution != other.h3_resolution {
            return Err(Error::MixedH3Resolutions(
                self.h3_resolution,
                other.h3_resolution,
            ));
        }
        for mut partition in other.edges.take_partitions().drain(..) {
            self.edges
                .insert_or_modify_many(partition.drain(), |old, new| {
                    *old = edge_weight_selector(old, new)
                });
        }
        Ok(())
    }

    /// cells which are valid targets to route to
    ///
    /// This is a rather expensive operation as nodes are not stored anywhere
    /// and need to be extracted from the edges.
    pub fn nodes(&self) -> ThreadPartitionedMap<H3Cell, NodeType> {
        log::debug!(
            "extracting nodes from the graph edges @ r={}",
            self.h3_resolution
        );
        let mut partition_cells: Vec<_> = self
            .edges
            .partitions()
            .par_iter()
            .map(|partition| {
                let mut cells = H3CellMap::with_capacity(partition.len());
                for edge in partition.keys() {
                    if let Ok(cell_from) = edge.origin_index() {
                        cells
                            .entry(cell_from)
                            .and_modify(|node_type| *node_type += NodeType::Origin)
                            .or_insert(NodeType::Origin);
                    }
                    if let Ok(cell_to) = edge.destination_index() {
                        cells
                            .entry(cell_to)
                            .and_modify(|node_type| *node_type += NodeType::Destination)
                            .or_insert(NodeType::Destination);
                    }
                }
                cells
            })
            .collect();
        let mut tpm = ThreadPartitionedMap::new();
        for mut pcs in partition_cells.drain(..) {
            tpm.insert_or_modify_many(pcs.drain(), |old, new| *old += new);
        }
        tpm
    }
}

impl<W> HasH3Resolution for H3EdgeGraph<W>
where
    W: Send + Sync,
{
    fn h3_resolution(&self) -> u8 {
        self.h3_resolution
    }
}

impl<W> GetStats for H3EdgeGraph<W>
where
    W: Send + Sync + PartialEq + PartialOrd + Add + Copy,
{
    fn get_stats(&self) -> GraphStats {
        GraphStats {
            h3_resolution: self.h3_resolution,
            num_nodes: self.num_nodes(),
            num_edges: self.num_edges(),
        }
    }
}

impl<W> CoveredArea for H3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Send + Sync,
{
    fn covered_area(&self, reduce_resolution_by: u8) -> Result<MultiPolygon<f64>, Error> {
        cells_covered_area(
            self.nodes().iter().map(|(cell, _)| cell),
            self.h3_resolution(),
            reduce_resolution_by,
        )
    }
}

#[inline]
fn edge_weight_selector<W: PartialOrd + Copy>(old: &W, new: W) -> W {
    // lower weight takes precedence
    if *old < new {
        *old
    } else {
        new
    }
}

/// change the resolution of a graph to a lower resolution
///
/// the `weight_selector_fn` decides which weight is assigned to a downsampled edge
/// by selecting a weight from all full-resolution edges crossing the new cells boundary.
///
/// This has the potential to change the graphs topology as multiple edges get condensed into one.
/// So for example routing results may differ in parts, but the computation time will be reduced by
/// the reduced number of nodes and edges.
pub fn downsample_graph<W, F>(
    graph: &H3EdgeGraph<W>,
    target_h3_resolution: u8,
    weight_selector_fn: F,
) -> Result<H3EdgeGraph<W>, Error>
where
    W: Sync + Send + Copy,
    F: Fn(W, W) -> W + Sync + Send,
{
    if target_h3_resolution >= graph.h3_resolution {
        return Err(Error::TooHighH3Resolution(target_h3_resolution));
    }
    log::debug!(
        "downsampling graph from r={} to r={}",
        graph.h3_resolution(),
        target_h3_resolution
    );
    let mut cross_cell_edges = graph
        .edges
        .partitions()
        .par_iter()
        .map(|partition| {
            partition
                .iter()
                .filter_map(|(edge, weight)| {
                    let cell_from = edge
                        .origin_index_unchecked()
                        .get_parent_unchecked(target_h3_resolution);
                    let cell_to = edge
                        .destination_index_unchecked()
                        .get_parent_unchecked(target_h3_resolution);
                    if cell_from == cell_to {
                        None
                    } else {
                        Some(
                            cell_from
                                .unidirectional_edge_to(&cell_to)
                                .map(|downsamled_edge| (downsamled_edge, *weight)),
                        )
                    }
                })
                .collect::<Vec<_>>()
        })
        .flatten()
        .collect::<Result<Vec<_>, _>>()?;

    let mut downsampled_edges = ThreadPartitionedMap::with_capacity(cross_cell_edges.len() / 2);
    downsampled_edges.insert_or_modify_many(cross_cell_edges.drain(..), |old, new| {
        *old = weight_selector_fn(*old, new)
    });

    Ok(H3EdgeGraph {
        edges: downsampled_edges,
        h3_resolution: target_h3_resolution,
    })
}

pub trait H3EdgeGraphBuilder<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Send + Sync,
{
    fn build_graph(self) -> Result<H3EdgeGraph<W>, Error>;
}

#[cfg(test)]
mod tests {
    use std::cmp::min;

    use geo_types::{Coordinate, LineString};

    use h3ron::H3Cell;

    use super::{downsample_graph, H3EdgeGraph, NodeType};

    #[test]
    fn test_downsample() {
        let full_h3_res = 8;
        let cells: Vec<_> = h3ron::line(
            &LineString::from(vec![
                Coordinate::from((23.3, 12.3)),
                Coordinate::from((24.2, 12.2)),
            ]),
            full_h3_res,
        )
        .unwrap()
        .into();
        assert!(cells.len() > 100);

        let mut graph = H3EdgeGraph::new(full_h3_res);
        for w in cells.windows(2) {
            graph.add_edge_using_cells(w[0], w[1], 20).unwrap();
        }
        assert!(graph.num_edges() > 50);
        let downsampled_graph =
            downsample_graph(&graph, full_h3_res.saturating_sub(3), min).unwrap();
        assert!(downsampled_graph.num_edges() > 0);
        assert!(downsampled_graph.num_edges() < 20);
    }

    #[test]
    fn test_graph_nodes() {
        let res = 8;
        let origin = H3Cell::from_coordinate(&Coordinate::from((23.3, 12.3)), res).unwrap();
        let edges: Vec<_> = origin
            .unidirectional_edges()
            .drain()
            .map(|edge| (edge, edge.destination_index_unchecked()))
            .collect();

        let mut graph = H3EdgeGraph::new(res);
        graph.add_edge(edges[0].0, 1).unwrap();
        graph.add_edge(edges[1].0, 1).unwrap();

        let edges2: Vec<_> = edges[1]
            .1
            .unidirectional_edges()
            .drain()
            .map(|edge| (edge, edge.destination_index_unchecked()))
            .collect();
        graph.add_edge(edges2[0].0, 1).unwrap();

        let nodes = graph.nodes();
        assert_eq!(nodes.len(), 4);
        assert_eq!(nodes.get(&origin), Some(&NodeType::Origin));
        assert_eq!(nodes.get(&edges[0].1), Some(&NodeType::Destination));
        assert_eq!(
            nodes.get(&edges[1].1),
            Some(&NodeType::OriginAndDestination)
        );
        assert_eq!(nodes.get(&edges2[0].1), Some(&NodeType::Destination));
    }
}
