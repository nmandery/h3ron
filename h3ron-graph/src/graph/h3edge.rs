use std::ops::Add;

use geo_types::MultiPolygon;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use crate::algorithm::covered_area::{cells_covered_area, CoveredArea};
use h3ron::collections::{H3CellMap, HashMap, ThreadPartitionedMap};
use h3ron::{H3Cell, H3DirectedEdge, HasH3Resolution};

use crate::error::Error;
use crate::graph::node::NodeType;
use crate::graph::{EdgeWeight, GetEdge, GetStats};

use super::GraphStats;

#[derive(Serialize, Deserialize, Clone)]
pub struct H3EdgeGraph<W: Send + Sync> {
    pub edges: ThreadPartitionedMap<H3DirectedEdge, W, 4>,
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
    pub fn num_nodes(&self) -> Result<usize, Error> {
        Ok(self.nodes()?.len())
    }

    pub fn num_edges(&self) -> usize {
        self.edges.len()
    }

    pub fn edge_weight(&self, edge: &H3DirectedEdge) -> Option<&W> {
        self.edges.get(edge)
    }

    /// get all edges in the graph leading from this edge to neighbors
    pub fn edges_from_cell(&self, cell: &H3Cell) -> Result<Vec<(&H3DirectedEdge, &W)>, Error> {
        let edges = cell
            .directed_edges()?
            .iter()
            .filter_map(|edge| self.edges.get_key_value(&edge))
            .collect();
        Ok(edges)
    }

    /// get all edges in the graph leading to this cell from its neighbors
    pub fn edges_to_cell(&self, cell: &H3Cell) -> Result<Vec<(&H3DirectedEdge, &W)>, Error> {
        let mut edges = vec![];
        for disk_cell in cell.grid_disk(1)?.iter() {
            if &disk_cell == cell {
                continue;
            }
            edges.extend(
                disk_cell
                    .directed_edges()?
                    .iter()
                    .filter_map(|edge| self.edges.get_key_value(&edge)),
            )
        }
        Ok(edges)
    }

    pub fn add_edge_using_cells(
        &mut self,
        cell_from: H3Cell,
        cell_to: H3Cell,
        weight: W,
    ) -> Result<(), Error> {
        let edge = cell_from.directed_edge_to(cell_to)?;
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

    pub fn add_edge(&mut self, edge: H3DirectedEdge, weight: W) -> Result<(), Error> {
        self.edges
            .insert_or_modify(edge, weight, edge_weight_selector);
        Ok(())
    }

    pub fn try_add(&mut self, mut other: Self) -> Result<(), Error> {
        if self.h3_resolution != other.h3_resolution {
            return Err(Error::MixedH3Resolutions(
                self.h3_resolution,
                other.h3_resolution,
            ));
        }
        for partition in other.edges.partitions_mut() {
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
    pub fn nodes(&self) -> Result<ThreadPartitionedMap<H3Cell, NodeType, 4>, Error> {
        log::debug!(
            "extracting nodes from the graph edges @ r={}",
            self.h3_resolution
        );
        let mut partition_cell_maps = self
            .edges
            .partitions()
            .par_iter()
            .map(|partition| partition_nodes(partition))
            .collect::<Result<Vec<_>, _>>()?;
        let mut tpm = ThreadPartitionedMap::new();
        for mut pcs in partition_cell_maps.drain(..) {
            tpm.insert_or_modify_many(pcs.drain(), |old, new| *old += new);
        }
        Ok(tpm)
    }

    pub fn iter_edges(&self) -> impl Iterator<Item = (H3DirectedEdge, &W)> {
        self.edges.iter().map(|(edge, weight)| (*edge, weight))
    }
}

fn partition_nodes<W>(
    partition: &HashMap<H3DirectedEdge, W>,
) -> Result<HashMap<H3Cell, NodeType>, Error> {
    let mut cells = H3CellMap::with_capacity(partition.len());
    for edge in partition.keys() {
        let cell_from = edge.origin_cell()?;
        cells
            .entry(cell_from)
            .and_modify(|node_type| *node_type += NodeType::Origin)
            .or_insert(NodeType::Origin);

        let cell_to = edge.destination_cell()?;
        cells
            .entry(cell_to)
            .and_modify(|node_type| *node_type += NodeType::Destination)
            .or_insert(NodeType::Destination);
    }
    Ok(cells)
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
    fn get_stats(&self) -> Result<GraphStats, Error> {
        Ok(GraphStats {
            h3_resolution: self.h3_resolution,
            num_nodes: self.num_nodes()?,
            num_edges: self.num_edges(),
        })
    }
}

impl<W> GetEdge for H3EdgeGraph<W>
where
    W: Copy + Send + Sync,
{
    type EdgeWeightType = W;

    fn get_edge(
        &self,
        edge: &H3DirectedEdge,
    ) -> Result<Option<EdgeWeight<Self::EdgeWeightType>>, Error> {
        Ok(self.edges.get(edge).map(|w| EdgeWeight::from(*w)))
    }
}

impl<W> CoveredArea for H3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Send + Sync,
{
    type Error = Error;

    fn covered_area(&self, reduce_resolution_by: u8) -> Result<MultiPolygon<f64>, Self::Error> {
        cells_covered_area(
            self.nodes()?.iter().map(|(cell, _)| cell),
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
        .map(|partition| downsample_partition_edges(partition, target_h3_resolution))
        .collect::<Result<Vec<Vec<_>>, _>>()?;

    let mut downsampled_edges = ThreadPartitionedMap::with_capacity(cross_cell_edges.len() / 2);
    for mut partition in cross_cell_edges.drain(..) {
        downsampled_edges.insert_or_modify_many(partition.drain(..), |old, new| {
            *old = weight_selector_fn(*old, new)
        });
    }
    Ok(H3EdgeGraph {
        edges: downsampled_edges,
        h3_resolution: target_h3_resolution,
    })
}

fn downsample_partition_edges<W>(
    partition: &HashMap<H3DirectedEdge, W>,
    target_h3_resolution: u8,
) -> Result<Vec<(H3DirectedEdge, W)>, Error>
where
    W: Copy,
{
    let mut ds_edges = vec![];

    for (edge, weight) in partition {
        let cell_from = edge.origin_cell()?.get_parent(target_h3_resolution)?;
        let cell_to = edge.destination_cell()?.get_parent(target_h3_resolution)?;
        if cell_from != cell_to {
            ds_edges.push((cell_from.directed_edge_to(cell_to)?, *weight))
        }
    }
    Ok(ds_edges)
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
        let origin = H3Cell::from_coordinate(Coordinate::from((23.3, 12.3)), res).unwrap();
        let edges: Vec<_> = origin
            .directed_edges()
            .unwrap()
            .drain()
            .map(|edge| (edge, edge.destination_cell().unwrap()))
            .collect();

        let mut graph = H3EdgeGraph::new(res);
        graph.add_edge(edges[0].0, 1).unwrap();
        graph.add_edge(edges[1].0, 1).unwrap();

        let edges2: Vec<_> = edges[1]
            .1
            .directed_edges()
            .unwrap()
            .drain()
            .map(|edge| (edge, edge.destination_cell().unwrap()))
            .collect();
        graph.add_edge(edges2[0].0, 1).unwrap();

        let nodes = graph.nodes().unwrap();
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
