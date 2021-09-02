use std::ops::{Add, AddAssign};

use geo::algorithm::simplify::Simplify;
use geo_types::{MultiPolygon, Polygon};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};

use h3ron::collections::{H3CellMap, H3CellSet, ThreadPartitionedMap};
use h3ron::{H3Cell, H3Edge, HasH3Resolution, Index, ToLinkedPolygons};

use crate::error::Error;

#[derive(Serialize)]
pub struct GraphStats {
    pub h3_resolution: u8,
    pub num_nodes: usize,
    pub num_edges: usize,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct H3EdgeGraph<T: Send + Sync> {
    pub edges: ThreadPartitionedMap<H3Edge, T>,
    pub h3_resolution: u8,
}

impl<T> H3EdgeGraph<T>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Sync,
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

    pub fn edge_weight(&self, edge: &H3Edge) -> Option<&T> {
        self.edges.get(edge)
    }

    /// get all edges in the graph leading from this edge to neighbors
    pub fn edges_from_cell(&self, cell: &H3Cell) -> Vec<(&H3Edge, &T)> {
        cell.unidirectional_edges()
            .iter()
            .filter_map(|edge| self.edges.get_key_value(edge))
            .collect()
    }

    /// get all edges in the graph leading to this cell from its neighbors
    pub fn edges_to_cell(&self, cell: &H3Cell) -> Vec<(&H3Edge, &T)> {
        cell.k_ring(1)
            .drain(..)
            .filter(|ring_cell| ring_cell != cell)
            .flat_map(|ring_cell| {
                ring_cell
                    .unidirectional_edges()
                    .drain(..)
                    .filter_map(|edge| self.edges.get_key_value(&edge))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    pub fn add_edge_using_cells(
        &mut self,
        cell_from: H3Cell,
        cell_to: H3Cell,
        weight: T,
    ) -> Result<(), Error> {
        let edge = cell_from.unidirectional_edge_to(&cell_to)?;
        self.add_edge(edge, weight)
    }

    pub fn add_edge_using_cells_bidirectional(
        &mut self,
        cell_from: H3Cell,
        cell_to: H3Cell,
        weight: T,
    ) -> Result<(), Error> {
        self.add_edge_using_cells(cell_from, cell_to, weight)?;
        self.add_edge_using_cells(cell_to, cell_from, weight)
    }

    pub fn add_edge(&mut self, edge: H3Edge, weight: T) -> Result<(), Error> {
        self.edges
            .insert_or_modify(edge, weight, edge_weight_selector);
        Ok(())
    }

    pub fn try_add(&mut self, mut other: H3EdgeGraph<T>) -> Result<(), Error> {
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

    pub fn stats(&self) -> GraphStats {
        GraphStats {
            h3_resolution: self.h3_resolution,
            num_nodes: self.num_nodes(),
            num_edges: self.num_edges(),
        }
    }

    /// generate a - simplified and overestimating - multipolygon of the area
    /// covered by the graph.
    pub fn covered_area(&self) -> Result<MultiPolygon<f64>, Error> {
        let t_res = self.h3_resolution.saturating_sub(3);
        let mut cells = H3CellSet::default();
        for cell in self.nodes().keys() {
            cells.insert(cell.get_parent(t_res)?);
        }
        let cell_vec: Vec<_> = cells.drain().collect();
        let mp = MultiPolygon::from(
            cell_vec
                // remove the number of vertices by smoothing
                .to_linked_polygons(true)
                .drain(..)
                // reduce the number of vertices again and discard all holes
                .map(|p| Polygon::new(p.exterior().simplify(&0.000001), vec![]))
                .collect::<Vec<_>>(),
        );
        Ok(mp)
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

impl<T> HasH3Resolution for H3EdgeGraph<T>
where
    T: Send + Sync,
{
    fn h3_resolution(&self) -> u8 {
        self.h3_resolution
    }
}

#[derive(PartialEq, Debug, Copy, Clone)]
pub enum NodeType {
    Origin,
    Destination,
    OriginAndDestination,
}

impl NodeType {
    pub fn is_origin(&self) -> bool {
        match self {
            NodeType::Origin => true,
            NodeType::Destination => false,
            NodeType::OriginAndDestination => true,
        }
    }

    pub fn is_destination(&self) -> bool {
        match self {
            NodeType::Origin => false,
            NodeType::Destination => true,
            NodeType::OriginAndDestination => true,
        }
    }
}

impl Add<NodeType> for NodeType {
    type Output = NodeType;

    fn add(self, rhs: NodeType) -> Self::Output {
        if rhs == self {
            self
        } else {
            Self::OriginAndDestination
        }
    }
}

impl AddAssign<NodeType> for NodeType {
    fn add_assign(&mut self, rhs: NodeType) {
        if self != &rhs {
            *self = Self::OriginAndDestination
        }
    }
}

#[inline]
fn edge_weight_selector<T: PartialOrd + Copy>(old: &T, new: T) -> T {
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
pub fn downsample_graph<T, F>(
    graph: &H3EdgeGraph<T>,
    target_h3_resolution: u8,
    weight_selector_fn: F,
) -> Result<H3EdgeGraph<T>, Error>
where
    T: Sync + Send + Copy,
    F: Fn(T, T) -> T + Sync + Send,
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

pub trait H3EdgeGraphBuilder<T>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Sync,
{
    fn build_graph(self) -> Result<H3EdgeGraph<T>, Error>;
}

#[cfg(test)]
mod tests {
    use std::cmp::min;

    use geo_types::{Coordinate, LineString};

    use h3ron::H3Cell;

    use crate::graph::{downsample_graph, H3EdgeGraph, NodeType};

    #[test]
    fn test_downsample() {
        let full_h3_res = 8;
        let cells = h3ron::line(
            &LineString::from(vec![
                Coordinate::from((23.3, 12.3)),
                Coordinate::from((24.2, 12.2)),
            ]),
            full_h3_res,
        )
        .unwrap();
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
    fn test_nodetype_add() {
        assert_eq!(NodeType::Origin, NodeType::Origin + NodeType::Origin);
        assert_eq!(
            NodeType::Destination,
            NodeType::Destination + NodeType::Destination
        );
        assert_eq!(
            NodeType::OriginAndDestination,
            NodeType::Origin + NodeType::Destination
        );
        assert_eq!(
            NodeType::OriginAndDestination,
            NodeType::OriginAndDestination + NodeType::Destination
        );
        assert_eq!(
            NodeType::OriginAndDestination,
            NodeType::Destination + NodeType::Origin
        );
    }

    #[test]
    fn test_nodetype_addassign() {
        let mut n1 = NodeType::Origin;
        n1 += NodeType::Origin;
        assert_eq!(n1, NodeType::Origin);

        let mut n2 = NodeType::Origin;
        n2 += NodeType::OriginAndDestination;
        assert_eq!(n2, NodeType::OriginAndDestination);

        let mut n3 = NodeType::Destination;
        n3 += NodeType::OriginAndDestination;
        assert_eq!(n3, NodeType::OriginAndDestination);

        let mut n4 = NodeType::Destination;
        n4 += NodeType::Origin;
        assert_eq!(n4, NodeType::OriginAndDestination);
    }

    #[test]
    fn test_graph_nodes() {
        let res = 8;
        let origin = H3Cell::from_coordinate(&Coordinate::from((23.3, 12.3)), res).unwrap();
        let edges: Vec<_> = origin
            .unidirectional_edges()
            .drain(0..2)
            .map(|edge| (edge, edge.destination_index_unchecked()))
            .collect();

        let mut graph = H3EdgeGraph::new(res);
        graph.add_edge(edges[0].0, 1).unwrap();
        graph.add_edge(edges[1].0, 1).unwrap();

        let edges2: Vec<_> = edges[1]
            .1
            .unidirectional_edges()
            .drain(0..1)
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
