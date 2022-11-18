use std::ops::Add;

use geo::bounding_rect::BoundingRect;
use geo::concave_hull::ConcaveHull;
use geo_types::{Coord, MultiPoint, MultiPolygon, Point, Polygon, Rect};
use num_traits::Zero;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use smallvec::{smallvec, SmallVec};

use h3ron::collections::compressed::Decompressor;
use h3ron::collections::hashbrown::hash_map::Entry;
use h3ron::collections::{H3Treemap, HashMap};
use h3ron::iter::H3DirectedEdgesBuilder;
use h3ron::{H3Cell, H3DirectedEdge, HasH3Resolution, ToCoordinate};

use crate::algorithm::covered_area::{cells_covered_area, CoveredArea};
use crate::error::Error;
use crate::graph::longedge::LongEdge;
use crate::graph::node::NodeType;
use crate::graph::{
    EdgeWeight, GetCellEdges, GetCellNode, GetStats, GraphStats, H3EdgeGraph, IterateCellNodes,
};

#[derive(Serialize, Deserialize, Clone)]
struct OwnedEdgeValue<W> {
    pub weight: W,

    /// the longedge is a shortcut which includes many consequent edges while
    /// allowing to visit each of then individually.
    ///
    /// The Box takes care of allocating the LongEdge on the heap. That reduces
    /// the footprint of the OwnedEdgeValue - when W = f32 to nearly 10% compared to
    /// allocating the LongEdge on the stack.
    pub longedge: Option<Box<(LongEdge, W)>>,
}

impl<'a, W> From<&'a OwnedEdgeValue<W>> for EdgeWeight<'a, W>
where
    W: Copy,
{
    fn from(owned_edge_value: &'a OwnedEdgeValue<W>) -> Self {
        EdgeWeight {
            weight: owned_edge_value.weight,
            longedge: owned_edge_value
                .longedge
                .as_ref()
                .map(|boxed| (&boxed.0, boxed.1)),
        }
    }
}

type OwnedEdgeTuple<W> = (H3DirectedEdge, OwnedEdgeValue<W>);

/// A smallvec with an array length of 2 allows storing the - probably - most common
/// number of edges on the heap
type OwnedEdgeTupleList<W> = SmallVec<[OwnedEdgeTuple<W>; 2]>;

/// A prepared graph which can be used with a few algorithms.
///
/// Consequent H3DirectedEdges without forks get extended by a `LongEdge` to allow
/// skipping the individual `H3DirectedEdge` values for a more efficient graph
/// traversal.
///
/// <p>
#[doc=include_str!("../../doc/images/edges-and-longedges.svg")]
/// </p>
///
/// <p>
#[doc=include_str!("../../doc/images/prepared_h3_edge_graph.svg")]
/// </p>
///
#[derive(Serialize, Deserialize, Clone)]
pub struct PreparedH3EdgeGraph<W> {
    outgoing_edges: HashMap<H3Cell, OwnedEdgeTupleList<W>>,
    h3_resolution: u8,
    graph_nodes: HashMap<H3Cell, NodeType>,
}

unsafe impl<W> Sync for PreparedH3EdgeGraph<W> where W: Sync {}

impl<W> PreparedH3EdgeGraph<W> {
    /// count the number of edges in the graph
    ///
    /// The returned tuple is (`num_edges`, `num_long_edges`)
    pub fn count_edges(&self) -> (usize, usize) {
        let mut num_edges = 0usize;
        let mut num_long_edges = 0usize;

        for (_cell, oevs) in self.outgoing_edges.iter() {
            num_edges += oevs.len();
            num_long_edges += oevs
                .iter()
                .filter(|(_, oev)| oev.longedge.is_some())
                .count();
        }
        (num_edges, num_long_edges)
    }
}

impl<W> PreparedH3EdgeGraph<W>
where
    W: Copy,
{
    /// iterate over all edges of the graph
    pub fn iter_edges(&self) -> impl Iterator<Item = (H3DirectedEdge, EdgeWeight<W>)> {
        self.outgoing_edges
            .iter()
            .flat_map(|(_, oevs)| oevs.iter().map(|(edge, oev)| (*edge, oev.into())))
    }

    /// iterate over all edges of the graph, while skipping simple `H3DirectedEdge`
    /// which are already covered in other `LongEdge` instances of the graph.
    ///
    /// This function iterates the graph twice - the first time to collect
    /// all edges which are part of long-edges.
    pub fn iter_edges_non_overlapping(
        &self,
    ) -> Result<impl Iterator<Item = (H3DirectedEdge, EdgeWeight<W>)>, Error> {
        let mut covered_edges = H3Treemap::<H3DirectedEdge>::default();
        let mut decompressor = Decompressor::default();
        for (_, owned_edge_values) in self.outgoing_edges.iter() {
            for (_, owned_edge_value) in owned_edge_values.iter() {
                if let Some(boxed_longedge) = owned_edge_value.longedge.as_ref() {
                    for edge in decompressor
                        .decompress_block(&boxed_longedge.0.edge_path)?
                        .skip(1)
                    {
                        covered_edges.insert(edge);
                    }
                }
            }
        }
        Ok(self.iter_edges().filter_map(move |(edge, weight)| {
            if covered_edges.contains(&edge) {
                None
            } else {
                Some((edge, weight))
            }
        }))
    }
}

impl<W> HasH3Resolution for PreparedH3EdgeGraph<W> {
    fn h3_resolution(&self) -> u8 {
        self.h3_resolution
    }
}

impl<W> GetStats for PreparedH3EdgeGraph<W> {
    fn get_stats(&self) -> Result<GraphStats, Error> {
        Ok(GraphStats {
            h3_resolution: self.h3_resolution,
            num_nodes: self.graph_nodes.len(),
            num_edges: self.count_edges().0,
        })
    }
}

impl<W> GetCellNode for PreparedH3EdgeGraph<W> {
    fn get_cell_node(&self, cell: &H3Cell) -> Option<NodeType> {
        self.graph_nodes.get(cell).copied()
    }
}

impl<W: Copy> GetCellEdges for PreparedH3EdgeGraph<W> {
    type EdgeWeightType = W;

    fn get_edges_originating_from(
        &self,
        cell: &H3Cell,
    ) -> Result<Vec<(H3DirectedEdge, EdgeWeight<Self::EdgeWeightType>)>, Error> {
        let mut out_vec = Vec::with_capacity(7);
        if let Some(edges_with_weights) = self.outgoing_edges.get(cell) {
            out_vec.extend(
                edges_with_weights
                    .iter()
                    .map(|(edge, owv)| (*edge, owv.into())),
            );
        }
        Ok(out_vec)
    }
}

const MIN_LONGEDGE_LENGTH: usize = 3;

fn to_longedge_edges<W>(
    input_graph: H3EdgeGraph<W>,
    min_longedge_length: usize,
) -> Result<HashMap<H3Cell, OwnedEdgeTupleList<W>>, Error>
where
    W: PartialOrd + PartialEq + Add<Output = W> + Copy + Send + Sync,
{
    if min_longedge_length < MIN_LONGEDGE_LENGTH {
        return Err(Error::Other(format!(
            "minimum longedge length must be >= {}",
            MIN_LONGEDGE_LENGTH
        )));
    }

    let outgoing_edge_vecs = input_graph
        .edges
        .par_iter()
        .try_fold(
            || (Vec::new(), H3DirectedEdgesBuilder::new()),
            |(mut output_vec, mut edge_builder), (edge, weight)| {
                assemble_edge_with_longedge(
                    &input_graph.edges,
                    min_longedge_length,
                    edge,
                    weight,
                    &mut edge_builder,
                )
                .map(|cell_edge| {
                    output_vec.push(cell_edge);
                    (output_vec, edge_builder)
                })
            },
        )
        .collect::<Result<Vec<_>, _>>()?;

    let mut outgoing_edges: HashMap<H3Cell, OwnedEdgeTupleList<W>> = Default::default();
    for (outgoing_edge_vec, _) in outgoing_edge_vecs.into_iter() {
        for (cell, edge_with_weight) in outgoing_edge_vec.into_iter() {
            match outgoing_edges.entry(cell) {
                Entry::Occupied(mut occ) => occ.get_mut().push(edge_with_weight),
                Entry::Vacant(vac) => {
                    vac.insert(smallvec![edge_with_weight]);
                }
            }
        }
    }

    // remove duplicates if there are any. Ignores any differences in weights
    outgoing_edges
        .par_iter_mut()
        .for_each(|(_cell, edges_with_weights)| {
            edges_with_weights.sort_unstable_by_key(|eww| eww.0);
            edges_with_weights.dedup_by(|a, b| a.0 == b.0);
        });

    Ok(outgoing_edges)
}

fn assemble_edge_with_longedge<W>(
    input_edges: &HashMap<H3DirectedEdge, W>,
    min_longedge_length: usize,
    edge: &H3DirectedEdge,
    weight: &W,
    edge_builder: &mut H3DirectedEdgesBuilder,
) -> Result<(H3Cell, OwnedEdgeTuple<W>), Error>
where
    W: PartialOrd + PartialEq + Add<Output = W> + Copy,
{
    let mut graph_entry = OwnedEdgeValue {
        weight: *weight,
        longedge: None,
    };

    let origin_cell = edge.origin_cell()?;

    // number of upstream edges leading to this one
    let num_edges_leading_to_this_one = edge_builder
        .from_origin_cell(&origin_cell)?
        .filter(|new_edge| new_edge != edge) // ignore the backwards edge
        .filter(|new_edge| {
            new_edge
                .reversed()
                .ok()
                .map(|rev_edge| input_edges.get(&rev_edge).is_some())
                .unwrap_or(false)
        })
        .count();

    // attempt to build a longedge when this edge is either the end of a path, or a path
    // starting after a conjunction of multiple edges
    if num_edges_leading_to_this_one != 1 {
        let mut edge_path = vec![*edge];
        let mut longedge_weight = *weight;

        let mut last_edge = *edge;
        loop {
            let last_edge_reverse = last_edge.reversed()?;
            // follow the edges until the end or a conjunction is reached
            let following_edges: Vec<_> = edge_builder
                .from_origin_cell(&last_edge.destination_cell()?)?
                .filter_map(|this_edge| {
                    if this_edge != last_edge_reverse {
                        input_edges.get_key_value(&this_edge)
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
                Some(Box::new((LongEdge::try_from(edge_path)?, longedge_weight)));
        }
    }
    Ok((origin_cell, (*edge, graph_entry)))
}

impl<W> PreparedH3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Ord + Zero + Send + Sync,
{
    pub fn from_h3edge_graph(
        graph: H3EdgeGraph<W>,
        min_longedge_length: usize,
    ) -> Result<Self, Error> {
        let h3_resolution = graph.h3_resolution();
        let graph_nodes = graph.nodes()?;
        let outgoing_edges = to_longedge_edges(graph, min_longedge_length)?;
        Ok(Self {
            graph_nodes,
            h3_resolution,
            outgoing_edges,
        })
    }
}

impl<W> TryFrom<H3EdgeGraph<W>> for PreparedH3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Ord + Zero + Send + Sync,
{
    type Error = Error;

    fn try_from(graph: H3EdgeGraph<W>) -> Result<Self, Self::Error> {
        Self::from_h3edge_graph(graph, 4)
    }
}

impl<W> From<PreparedH3EdgeGraph<W>> for H3EdgeGraph<W>
where
    W: PartialOrd + PartialEq + Add + Copy + Ord + Zero,
{
    fn from(prepared_graph: PreparedH3EdgeGraph<W>) -> Self {
        Self {
            edges: prepared_graph
                .iter_edges()
                .map(|(edge, edge_value)| (edge, edge_value.weight))
                .collect(),
            h3_resolution: prepared_graph.h3_resolution,
        }
    }
}

impl<W> CoveredArea for PreparedH3EdgeGraph<W> {
    type Error = Error;

    fn covered_area(&self, reduce_resolution_by: u8) -> Result<MultiPolygon<f64>, Self::Error> {
        cells_covered_area(
            self.graph_nodes.iter().map(|(cell, _)| cell),
            self.h3_resolution(),
            reduce_resolution_by,
        )
    }
}

impl<'a, W> IterateCellNodes<'a> for PreparedH3EdgeGraph<W> {
    type CellNodeIterator = h3ron::collections::hashbrown::hash_map::Iter<'a, H3Cell, NodeType>;

    fn iter_cell_nodes(&'a self) -> Self::CellNodeIterator {
        self.graph_nodes.iter()
    }
}

impl<W> ConcaveHull for PreparedH3EdgeGraph<W> {
    type Scalar = f64;

    /// concave hull - this implementation leaves out invalid cells
    fn concave_hull(&self, concavity: Self::Scalar) -> Polygon<Self::Scalar> {
        let mpoint = MultiPoint::from(
            self.iter_cell_nodes()
                .filter_map(|(cell, _)| cell.to_coordinate().ok().map(Point::from))
                .collect::<Vec<_>>(),
        );
        mpoint.concave_hull(concavity)
    }
}

impl<W> BoundingRect<f64> for PreparedH3EdgeGraph<W> {
    type Output = Option<Rect<f64>>;

    fn bounding_rect(&self) -> Self::Output {
        let mut iter = self.iter_cell_nodes();
        let mut rect = {
            // consume until encountering the first valid cell
            if let Some(coord) = iter.find_map(|(cell, _)| cell.to_coordinate().ok()) {
                Point::from(coord).bounding_rect()
            } else {
                return None;
            }
        };

        for (cell, _) in iter {
            if let Ok(coord) = cell.to_coordinate() {
                rect = Rect::new(
                    Coord {
                        x: if coord.x < rect.min().x {
                            coord.x
                        } else {
                            rect.min().x
                        },
                        y: if coord.y < rect.min().y {
                            coord.y
                        } else {
                            rect.min().y
                        },
                    },
                    Coord {
                        x: if coord.x > rect.max().x {
                            coord.x
                        } else {
                            rect.max().x
                        },
                        y: if coord.y > rect.max().y {
                            coord.y
                        } else {
                            rect.max().y
                        },
                    },
                );
            }
        }
        Some(rect)
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use geo_types::{Coord, LineString};

    use crate::graph::{H3EdgeGraph, PreparedH3EdgeGraph};

    fn build_line_prepared_graph() -> PreparedH3EdgeGraph<u32> {
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
            graph.add_edge_using_cells(w[0], w[1], 20u32).unwrap();
        }
        assert!(graph.num_edges() > 50);
        let prep_graph: PreparedH3EdgeGraph<_> = graph.try_into().unwrap();
        assert_eq!(prep_graph.count_edges().1, 1);
        prep_graph
    }

    #[test]
    fn test_iter_edges() {
        let graph = build_line_prepared_graph();
        assert!(graph.iter_edges().count() > 50);
    }

    #[test]
    fn test_iter_non_overlapping_edges() {
        let graph = build_line_prepared_graph();
        assert_eq!(graph.iter_edges_non_overlapping().unwrap().count(), 1);
    }
}
