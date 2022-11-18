use std::ops::Add;

use geo_types::MultiPolygon;
use serde::{Deserialize, Serialize};

use crate::algorithm::covered_area::{cells_covered_area, CoveredArea};
use h3ron::collections::hashbrown::hash_map::Entry;
use h3ron::collections::{H3CellMap, H3EdgeMap, RandomState};
use h3ron::{H3Cell, H3DirectedEdge, HasH3Resolution};

use crate::error::Error;
use crate::graph::node::NodeType;
use crate::graph::{EdgeWeight, GetEdge, GetStats};

use super::GraphStats;

#[derive(Serialize, Deserialize, Clone)]
pub struct H3EdgeGraph<W> {
    pub edges: H3EdgeMap<W>,
    pub h3_resolution: u8,
}

impl<W> H3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy,
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
        match self.edges.entry(edge) {
            Entry::Occupied(mut occ) => {
                if &weight < occ.get() {
                    // lower weight takes precedence
                    occ.insert(weight);
                }
            }
            Entry::Vacant(vac) => {
                vac.insert(weight);
            }
        }
        Ok(())
    }

    pub fn try_add(&mut self, mut other: Self) -> Result<(), Error> {
        if self.h3_resolution != other.h3_resolution {
            return Err(Error::MixedH3Resolutions(
                self.h3_resolution,
                other.h3_resolution,
            ));
        }
        for (edge, weight) in other.edges.drain() {
            self.add_edge(edge, weight)?;
        }
        Ok(())
    }

    /// cells which are valid targets to route to
    ///
    /// This is a rather expensive operation as nodes are not stored anywhere
    /// and need to be extracted from the edges.
    pub fn nodes(&self) -> Result<H3CellMap<NodeType>, Error> {
        log::debug!(
            "extracting nodes from the graph edges @ r={}",
            self.h3_resolution
        );
        extract_nodes(&self.edges)
    }

    pub fn iter_edges(&self) -> impl Iterator<Item = (H3DirectedEdge, &W)> {
        self.edges.iter().map(|(edge, weight)| (*edge, weight))
    }
}

fn extract_nodes<W>(partition: &H3EdgeMap<W>) -> Result<H3CellMap<NodeType>, Error> {
    let mut cells = H3CellMap::with_capacity_and_hasher(partition.len(), RandomState::default());
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

impl<W> HasH3Resolution for H3EdgeGraph<W> {
    fn h3_resolution(&self) -> u8 {
        self.h3_resolution
    }
}

impl<W> GetStats for H3EdgeGraph<W>
where
    W: PartialEq + PartialOrd + Add + Copy,
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
    W: Copy,
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
    W: PartialOrd + PartialEq + Add + Copy,
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
        graph.h3_resolution,
        target_h3_resolution
    );

    let mut downsampled_edges = H3EdgeMap::with_capacity_and_hasher(
        graph.edges.len()
            / (graph.h3_resolution.saturating_sub(target_h3_resolution) as usize).pow(7),
        RandomState::default(),
    );

    for (edge, weight) in graph.edges.iter() {
        let edge_cells = edge.cells()?;
        let cell_from = edge_cells.origin.get_parent(target_h3_resolution)?;
        let cell_to = edge_cells.destination.get_parent(target_h3_resolution)?;
        if cell_from != cell_to {
            let downsampled_edge = cell_from.directed_edge_to(cell_to)?;

            match downsampled_edges.entry(downsampled_edge) {
                Entry::Occupied(mut occ) => {
                    occ.insert(weight_selector_fn(*occ.get(), *weight));
                }
                Entry::Vacant(vac) => {
                    vac.insert(*weight);
                }
            }
        }
    }
    Ok(H3EdgeGraph {
        edges: downsampled_edges,
        h3_resolution: target_h3_resolution,
    })
}

pub trait H3EdgeGraphBuilder<W>
where
    W: PartialOrd + PartialEq + Add + Copy,
{
    fn build_graph(self) -> Result<H3EdgeGraph<W>, Error>;
}

#[cfg(test)]
mod tests {
    use std::cmp::min;

    use geo_types::{Coord, LineString};

    use h3ron::H3Cell;

    use super::{downsample_graph, H3EdgeGraph, NodeType};

    #[test]
    fn test_downsample() {
        let full_h3_res = 8;
        let cells: Vec<_> = h3ron::line(
            &LineString::from(vec![Coord::from((23.3, 12.3)), Coord::from((24.2, 12.2))]),
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
        let origin = H3Cell::from_coordinate(Coord::from((23.3, 12.3)), res).unwrap();
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
