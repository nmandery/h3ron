//! Dijkstra shortest-path routing.
//!
use std::borrow::Borrow;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::ops::Add;

use indexmap::map::Entry::{Occupied, Vacant};
use indexmap::map::IndexMap;
use num_traits::Zero;
use rayon::prelude::*;

use h3ron::collections::{H3CellMap, H3CellSet, H3Treemap, HashMap, RandomState};
use h3ron::iter::{change_cell_resolution, H3EdgesBuilder};
use h3ron::{H3Cell, H3Edge, HasH3Resolution};

use crate::algorithm::path::Path;
use crate::error::Error;
use crate::graph::longedge::LongEdge;
use crate::graph::node::GetGapBridgedCellNodes;
use crate::graph::{GetEdge, GetNodeType};

///
/// Generic type parameters:
/// * `W`: The weight used in the graph.
pub trait ShortestPathOptions {
    /// Number of cells to be allowed to be missing between
    /// a cell and the graph while the cell is still counted as being connected
    /// to the graph
    fn num_gap_cells_to_graph(&self) -> u32 {
        0
    }

    /// number of destinations to reach.
    /// Routing for the origin cell will stop when this number of destinations are reached. When not set,
    /// routing will continue until all destinations are reached
    fn num_destinations_to_reach(&self) -> Option<usize> {
        None
    }
}

/// Default implementation of a type implementing the `ShortestPathOptions`
/// trait.
pub struct DefaultShortestPathOptions {}

impl ShortestPathOptions for DefaultShortestPathOptions {}

impl Default for DefaultShortestPathOptions {
    fn default() -> Self {
        Self {}
    }
}

impl DefaultShortestPathOptions {
    pub fn new() -> Self {
        Default::default()
    }
}

/// Implements a simple Dijkstra shortest path route finding.
///
/// While this is not the most efficient routing algorithm, it has the
/// benefit of finding the nearest destinations first. So it can be used
/// to answer questions like "which are the N nearest destinations" using a
/// large amount of possible destinations.
pub trait ShortestPath<W> {
    fn shortest_path<I, OPT: ShortestPathOptions>(
        &self,
        origin_cell: H3Cell,
        destination_cells: I,
        options: &OPT,
    ) -> Result<Vec<Path<W>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>;
}

/// Variant of the [`ShortestPath`] trait routing from multiple
/// origins in parallel.
pub trait ShortestPathManyToMany<W>
where
    W: Send + Sync + Ord + Copy,
{
    /// Returns found paths keyed by the origin cell.
    ///
    /// All cells must be in the h3 resolution of the graph.
    #[inline]
    fn shortest_path_many_to_many<I, OPT>(
        &self,
        origin_cells: I,
        destination_cells: I,
        options: &OPT,
    ) -> Result<H3CellMap<Vec<Path<W>>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
        OPT: ShortestPathOptions + Send + Sync,
    {
        self.shortest_path_many_to_many_map(origin_cells, destination_cells, options, |path| path)
    }

    /// Returns found paths, transformed by the `path_map_fn` and keyed by the
    /// origin cell.
    ///
    /// `path_map_fn` can be used to directly convert the paths to a less memory intensive
    /// type.
    ///
    /// All cells must be in the h3 resolution of the graph.
    fn shortest_path_many_to_many_map<I, OPT, PM, O>(
        &self,
        origin_cells: I,
        destination_cells: I,
        options: &OPT,
        path_map_fn: PM,
    ) -> Result<H3CellMap<Vec<O>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
        OPT: ShortestPathOptions + Send + Sync,
        PM: Fn(Path<W>) -> O + Send + Sync,
        O: Send + Ord + Clone;
}

impl<W, G> ShortestPathManyToMany<W> for G
where
    G: GetEdge<WeightType = W> + GetNodeType + HasH3Resolution + GetGapBridgedCellNodes + Sync,
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
{
    fn shortest_path_many_to_many_map<I, OPT, PM, O>(
        &self,
        origin_cells: I,
        destination_cells: I,
        options: &OPT,
        path_map_fn: PM,
    ) -> Result<H3CellMap<Vec<O>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
        OPT: ShortestPathOptions + Send + Sync,
        PM: Fn(Path<W>) -> O + Send + Sync,
        O: Send + Ord + Clone,
    {
        let filtered_origin_cells =
            filtered_origin_cells(self, options.num_gap_cells_to_graph(), origin_cells);
        if filtered_origin_cells.is_empty() {
            return Ok(Default::default());
        }

        let filtered_destination_cells = {
            let origins_treemap: H3Treemap<H3Cell> =
                filtered_origin_cells.iter().map(|(k, _)| *k).collect();

            filtered_destination_cells(
                self,
                options.num_gap_cells_to_graph(),
                destination_cells,
                &origins_treemap,
            )?
        };

        log::debug!(
            "shortest_path many-to-many: from {} cells to {} cells at resolution {} with num_gap_cells_to_graph = {}",
            filtered_origin_cells.len(),
            filtered_destination_cells.len(),
            self.h3_resolution(),
            options.num_gap_cells_to_graph()
        );
        let paths = filtered_origin_cells
            .par_iter()
            .map(|(graph_connected_origin_cell, output_origin_cells)| {
                let paths = edge_dijkstra(
                    self,
                    graph_connected_origin_cell,
                    &filtered_destination_cells,
                    options.num_destinations_to_reach(),
                    &path_map_fn,
                );

                output_origin_cells
                    .iter()
                    .map(|out_cell| (*out_cell, paths.clone()))
                    .collect::<Vec<_>>()
            })
            .flatten()
            .collect::<H3CellMap<_>>();
        Ok(paths)
    }
}

impl<W, G> ShortestPath<W> for G
where
    G: GetEdge<WeightType = W> + GetNodeType + HasH3Resolution + GetGapBridgedCellNodes,
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
{
    fn shortest_path<I, OPT>(
        &self,
        origin_cell: H3Cell,
        destination_cells: I,
        options: &OPT,
    ) -> Result<Vec<Path<W>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
        OPT: ShortestPathOptions,
    {
        let graph_connected_origin_cell = {
            let filtered_origin_cells = filtered_origin_cells(
                self,
                options.num_gap_cells_to_graph(),
                std::iter::once(origin_cell),
            );
            if let Some(first_fo) = filtered_origin_cells.first() {
                first_fo.0
            } else {
                return Ok(Default::default());
            }
        };

        let destination_treemap = {
            let mut origins_treemap: H3Treemap<H3Cell> = Default::default();
            origins_treemap.insert(graph_connected_origin_cell);
            filtered_destination_cells(
                self,
                options.num_gap_cells_to_graph(),
                destination_cells,
                &origins_treemap,
            )?
        };

        if destination_treemap.is_empty() {
            return Ok(Default::default());
        }
        let paths = edge_dijkstra(
            self,
            &graph_connected_origin_cell,
            &destination_treemap,
            options.num_destinations_to_reach(),
            &(|path| path),
        );
        Ok(paths)
    }
}

/// finds the corresponding cells in the graph for the given
/// destinations. When no corresponding cell is found, that destination
/// is filtered out.
///
/// The cell resolution is changed to the resolution of the graph.
///
/// There must be at least one destination to get Result::Ok, otherwise
/// the complete graph would be traversed.
fn filtered_destination_cells<G, I>(
    graph: &G,
    num_gap_cells_to_graph: u32,
    destination_cells: I,
    origins_treemap: &H3Treemap<H3Cell>,
) -> Result<H3Treemap<H3Cell>, Error>
where
    G: GetNodeType + GetGapBridgedCellNodes + HasH3Resolution,
    I: IntoIterator,
    I::Item: Borrow<H3Cell>,
{
    let mut destinations: H3Treemap<H3Cell> = Default::default();
    for destination in change_cell_resolution(destination_cells, graph.h3_resolution()) {
        let gap_bridged = graph.gap_bridged_corresponding_node_filtered(
            &destination,
            num_gap_cells_to_graph,
            // destinations which are origins at the same time are always allowed as they can
            // always be reached even when they are not a destination in the graph.
            |graph_cell, node_type| {
                node_type.is_destination() || origins_treemap.contains(graph_cell)
            },
        );
        if let Some(graph_cell) = gap_bridged.corresponding_cell_in_graph() {
            destinations.insert(graph_cell);
        }
    }

    if destinations.is_empty() {
        return Err(Error::DestinationsNotInGraph);
    }
    Ok(destinations)
}

/// Locates the corresponding cells for the given ones in the graph.
///
/// The returned hashmap maps cells, which are members of the graph to all
/// surrounding cells which are not directly part of the graph. This depends
/// on the gap-bridging in the options. With no gap bridging, cells are only mapped
/// to themselves.
///
/// The cell resolution is changed to the resolution of the graph.
fn filtered_origin_cells<G, I>(
    graph: &G,
    num_gap_cells_to_graph: u32,
    origin_cells: I,
) -> Vec<(H3Cell, Vec<H3Cell>)>
where
    G: GetNodeType + GetGapBridgedCellNodes + HasH3Resolution,
    I: IntoIterator,
    I::Item: Borrow<H3Cell>,
{
    // maps cells to their closest found neighbors in the graph
    let mut origin_cell_map = H3CellMap::default();

    for cell in change_cell_resolution(origin_cells, graph.h3_resolution()) {
        let gap_bridged = graph.gap_bridged_corresponding_node_filtered(
            &cell,
            num_gap_cells_to_graph,
            |_, node_type| node_type.is_origin(),
        );
        if let Some(graph_cell) = gap_bridged.corresponding_cell_in_graph() {
            origin_cell_map
                .entry(graph_cell)
                .and_modify(|ccs: &mut Vec<H3Cell>| ccs.push(gap_bridged.cell()))
                .or_insert_with(|| vec![gap_bridged.cell()]);
        }
    }
    origin_cell_map.drain().collect()
}

#[derive(Clone)]
enum DijkstraEdge<'a> {
    Single(H3Edge),
    Long(&'a LongEdge),
}

impl<'a> DijkstraEdge<'a> {
    #[allow(dead_code)]
    fn origin_cell(&self) -> H3Cell {
        match self {
            Self::Single(h3edge) => h3edge.origin_index_unchecked(),
            Self::Long(longedge) => longedge.origin_cell(),
        }
    }

    fn destination_cell(&self) -> H3Cell {
        match self {
            Self::Single(h3edge) => h3edge.destination_index_unchecked(),
            Self::Long(longedge) => longedge.destination_cell(),
        }
    }

    #[allow(dead_code)]
    fn last_edge(&self) -> H3Edge {
        match self {
            Self::Single(h3edge) => *h3edge,
            Self::Long(longedge) => longedge.out_edge,
        }
    }

    #[allow(dead_code)]
    fn first_edge(&self) -> H3Edge {
        match self {
            Self::Single(h3edge) => *h3edge,
            Self::Long(longedge) => longedge.in_edge,
        }
    }
}

struct DijkstraEntry<'a, W> {
    weight: W,
    index: usize,

    /// the edge which lead to that cell.
    /// using an option here as the start_cell will not have an edge
    edge: Option<DijkstraEdge<'a>>,
}

/// Dijkstra shortest path using h3 edges
///
/// Adapted from the `run_dijkstra` function of the `pathfinding` crate.
fn edge_dijkstra<'a, G, W, PM, O>(
    graph: &'a G,
    start_cell: &H3Cell,
    destinations: &H3Treemap<H3Cell>,
    num_destinations_to_reach: Option<usize>,
    path_map_fn: &PM,
) -> Vec<O>
where
    G: GetEdge<WeightType = W>,
    W: Zero + Ord + Copy,
    PM: Fn(Path<W>) -> O,
{
    // this is the main exit condition. Stop after this many destinations have been reached or
    // the complete graph has been traversed.
    let num_destinations_to_reach = num_destinations_to_reach
        .unwrap_or_else(|| destinations.len())
        .min(destinations.len());

    let mut edge_builder = H3EdgesBuilder::new();
    let mut to_see = BinaryHeap::new();
    let mut parents: IndexMap<H3Cell, DijkstraEntry<W>, RandomState> = IndexMap::default();
    let mut destinations_reached = H3CellSet::default();

    to_see.push(SmallestHolder {
        weight: W::zero(),
        index: 0,
    });
    parents.insert(
        *start_cell,
        DijkstraEntry {
            weight: W::zero(),
            index: usize::MAX,
            edge: None,
        },
    );
    while let Some(SmallestHolder { weight, index }) = to_see.pop() {
        let (cell, dijkstra_entry) = parents.get_index(index).unwrap();
        if destinations.contains(cell) {
            destinations_reached.insert(*cell);
            if destinations_reached.len() >= num_destinations_to_reach {
                break;
            }
        }

        // We may have inserted a node several time into the binary heap if we found
        // a better way to access it. Ensure that we are currently dealing with the
        // best path and discard the others.
        if weight > dijkstra_entry.weight {
            continue;
        }

        for succeeding_edge in edge_builder.from_origin_cell(cell) {
            if let Some(succeeding_edge_value) = graph.get_edge(&succeeding_edge) {
                // use the longedge if it does not contain any destination. If it would
                // contain a destination we would "jump over" it when we would use the longedge.
                let (dijkstra_edge, new_weight) =
                    if let Some((longedge, longedge_weight)) = succeeding_edge_value.longedge {
                        if longedge.is_disjoint(destinations) {
                            (DijkstraEdge::Long(longedge), longedge_weight + weight)
                        } else {
                            (
                                DijkstraEdge::Single(succeeding_edge),
                                succeeding_edge_value.weight + weight,
                            )
                        }
                    } else {
                        (
                            DijkstraEdge::Single(succeeding_edge),
                            succeeding_edge_value.weight + weight,
                        )
                    };

                let n;
                match parents.entry(dijkstra_edge.destination_cell()) {
                    Vacant(e) => {
                        n = e.index();
                        e.insert(DijkstraEntry {
                            weight: new_weight,
                            index,
                            edge: Some(dijkstra_edge),
                        });
                    }
                    Occupied(mut e) => {
                        if e.get().weight > new_weight {
                            n = e.index();
                            e.insert(DijkstraEntry {
                                weight: new_weight,
                                index,
                                edge: Some(dijkstra_edge),
                            });
                        } else {
                            continue;
                        }
                    }
                }
                to_see.push(SmallestHolder {
                    weight: new_weight,
                    index: n,
                });
            }
        }
    }

    let parents_map: HashMap<_, _> = parents
        .iter()
        .skip(1)
        .map(|(cell, dijkstra_entry)| {
            (
                *cell,
                (
                    parents.get_index(dijkstra_entry.index).unwrap().0,
                    dijkstra_entry,
                ),
            )
        })
        .collect();

    // assemble the paths
    let mut paths = Vec::with_capacity(destinations_reached.len());
    for destination_cell in destinations_reached {
        // start from the destination and collect all edges up to the origin

        let mut rev_dijkstra_edges: Vec<&DijkstraEdge> = vec![];
        let mut next = destination_cell;
        let mut total_weight: Option<W> = None;
        while let Some((parent_cell, parent_edge_value)) = parents_map.get(&next) {
            if total_weight.is_none() {
                total_weight = Some(parent_edge_value.weight);
            }
            if let Some(dijkstra_edge) = parent_edge_value.edge.as_ref() {
                rev_dijkstra_edges.push(dijkstra_edge);
            }
            next = **parent_cell;
        }

        // reverse order to go from origin to destination
        rev_dijkstra_edges.reverse();

        let mut h3edges = vec![];
        for dijkstra_edge in rev_dijkstra_edges.drain(..) {
            // dijkstra_edge is already in the correct order in itself and
            // does not need to be reversed
            match dijkstra_edge {
                DijkstraEdge::Single(h3edge) => h3edges.push(*h3edge),
                DijkstraEdge::Long(longedge) => h3edges.append(&mut longedge.h3edge_path()),
            }
        }
        let path = if h3edges.is_empty() {
            Path::OriginIsDestination(*start_cell, total_weight.unwrap_or_else(W::zero))
        } else {
            Path::EdgeSequence(h3edges, total_weight.unwrap_or_else(W::zero))
        };
        paths.push(path);
    }

    // return sorted from lowest to highest cost, use destination cell as second criteria
    // to make path vecs directly comparable using this deterministic order
    paths.sort_unstable();

    // ensure the sorted order is correct by sorting path instances before applying
    // the `path_map_fn`.
    paths.drain(..).map(path_map_fn).collect()
}

struct SmallestHolder<W> {
    weight: W,
    index: usize,
}

impl<W: PartialEq> PartialEq for SmallestHolder<W> {
    fn eq(&self, other: &Self) -> bool {
        self.weight == other.weight
    }
}

impl<W: PartialEq> Eq for SmallestHolder<W> {}

impl<W: Ord> PartialOrd for SmallestHolder<W> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<W: Ord> Ord for SmallestHolder<W> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.weight.cmp(&self.weight)
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithm::shortest_path::{DefaultShortestPathOptions, ShortestPathManyToMany};
    use crate::graph::{H3EdgeGraph, PreparedH3EdgeGraph};
    use geo_types::Coordinate;
    use h3ron::H3Cell;
    use std::convert::TryInto;

    #[test]
    fn test_shortest_path_same_origin_and_destination() {
        let res = 8;
        let origin = H3Cell::from_coordinate(&Coordinate::from((23.3, 12.3)), res).unwrap();
        let edge = origin.unidirectional_edges().first().unwrap();
        let destination = edge.destination_index_unchecked();

        // build a micro-graph
        let prepared_graph: PreparedH3EdgeGraph<_> = {
            let mut graph = H3EdgeGraph::new(res);
            graph.add_edge(edge, 5_u32).unwrap();
            graph.try_into().unwrap()
        };

        let paths = prepared_graph
            .shortest_path_many_to_many(
                &vec![origin],
                // find the path to the origin cell itself, and to the neighbor
                &vec![origin, destination],
                &DefaultShortestPathOptions::default(),
            )
            .unwrap();

        assert_eq!(paths.len(), 1);
        let path_vec = paths.get(&origin).unwrap();
        assert_eq!(path_vec.len(), 2);
        for path in path_vec.iter() {
            let path_destination = path.destination_cell().unwrap();
            if path_destination == origin {
                assert!(path.is_empty());
                assert_eq!(path.cost(), &0);
            } else if path_destination == destination {
                assert!(!path.is_empty());
                assert_eq!(path.cost(), &5);
            } else {
                unreachable!()
            }
        }
    }
}
