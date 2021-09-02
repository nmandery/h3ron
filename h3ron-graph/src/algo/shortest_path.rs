use std::borrow::Borrow;
use std::fmt::Debug;
use std::ops::Add;

use num_traits::Zero;
use rayon::prelude::*;

use crate::algo::dijkstra::{build_path_with_cost, dijkstra_partial};
use crate::algo::path::Path;
use crate::error::Error;
use crate::routing::RoutingH3EdgeGraph;
use h3ron::collections::{H3CellMap, H3CellSet, HashMap};
use h3ron::iter::change_cell_resolution;
use h3ron::{H3Cell, HasH3Resolution};

#[derive(Clone, Debug)]
pub struct ManyToManyOptions {
    /// number of destinations to reach.
    /// Routing for the origin cell will stop when this number of destinations are reached. When not set,
    /// routing will continue until all destinations are reached
    pub num_destinations_to_reach: Option<usize>,

    /// cells which are not allowed to be used for routing
    pub exclude_cells: Option<H3CellSet>,

    /// Number of cells to be allowed to be missing between
    /// a cell and the graph while the cell is still counted as being connected
    /// to the graph
    pub num_gap_cells_to_graph: u32,
}

impl Default for ManyToManyOptions {
    fn default() -> Self {
        Self {
            num_destinations_to_reach: None,
            exclude_cells: None,
            num_gap_cells_to_graph: 0,
        }
    }
}

pub trait ShortestPath<T: Ord + Send + Clone> {
    /// Returns found paths keyed by the origin cell.
    ///
    /// All cells must be in the h3 resolution of the graph.
    #[inline]
    fn shortest_path_many_to_many<I>(
        &self,
        origin_cells: I,
        destination_cells: I,
        options: &ManyToManyOptions,
    ) -> Result<H3CellMap<Vec<Path<T>>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
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
    fn shortest_path_many_to_many_map<I, F, O>(
        &self,
        origin_cells: I,
        destination_cells: I,
        options: &ManyToManyOptions,
        path_map_fn: F,
    ) -> Result<H3CellMap<Vec<O>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
        F: Fn(Path<T>) -> O + Send + Sync,
        O: Send + Ord + Clone;
}

impl<T> ShortestPath<T> for RoutingH3EdgeGraph<T>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync + Debug,
{
    fn shortest_path_many_to_many_map<I, F, O>(
        &self,
        origin_cells: I,
        destination_cells: I,
        options: &ManyToManyOptions,
        path_map_fn: F,
    ) -> Result<H3CellMap<Vec<O>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
        F: Fn(Path<T>) -> O + Send + Sync,
        O: Send + Ord + Clone,
    {
        let filtered_origin_cells: Vec<_> = {
            // maps cells to their closest found neighbors in the graph
            let mut origin_cell_map = H3CellMap::default();
            for gm in self
                .filtered_graph_membership::<Vec<_>, _>(
                    change_cell_resolution(origin_cells, self.h3_resolution()).collect(),
                    |node_type| node_type.is_origin(),
                    options.num_gap_cells_to_graph,
                )
                .drain(..)
            {
                if let Some(corr_cell) = gm.corresponding_cell_in_graph() {
                    origin_cell_map
                        .entry(corr_cell)
                        .and_modify(|ccs: &mut Vec<H3Cell>| ccs.push(gm.cell()))
                        .or_insert_with(|| vec![gm.cell()]);
                }
            }
            origin_cell_map.drain().collect()
        };

        if filtered_origin_cells.is_empty() {
            return Ok(Default::default());
        }

        // maps directly to the graph connected cells to the cells outside the
        // graph where they are used as a substitute. For direct graph members
        // both cells are the same
        // TODO: this should be a 1:n relationship in case multiple cells map to
        //      the same cell in the graph
        let filtered_destination_cells: HashMap<_, _> = self
            .filtered_graph_membership::<Vec<_>, _>(
                change_cell_resolution(destination_cells, self.h3_resolution()).collect(),
                |node_type| node_type.is_destination(),
                options.num_gap_cells_to_graph,
            )
            .drain(..)
            .filter_map(|connected_cell| {
                // ignore all non-connected destinations
                connected_cell
                    .corresponding_cell_in_graph()
                    .map(|cor_cell| (cor_cell, connected_cell.cell()))
            })
            .collect();

        if filtered_destination_cells.is_empty() {
            return Err(Error::DestinationsNotInGraph);
        }

        let is_excluded = |cell: H3Cell| {
            options
                .exclude_cells
                .as_ref()
                .map(|exclude| exclude.contains(&cell))
                .unwrap_or(false)
        };

        log::debug!(
            "shortest_path many-to-many: from {} cells to {} cells at resolution {} with num_gap_cells_to_graph = {}",
            filtered_origin_cells.len(),
            filtered_destination_cells.len(),
            self.h3_resolution(),
            options.num_gap_cells_to_graph
        );
        let paths = filtered_origin_cells
            .par_iter()
            .map(|(origin_cell, output_origin_cells)| {
                let mut destination_cells_reached = H3CellSet::default();

                // Possible improvement: add timeout to avoid continuing routing forever
                let (routemap, _) = dijkstra_partial(
                    // start cell
                    origin_cell,
                    // successor cells
                    |cell| {
                        let neighbors = cell
                            .unidirectional_edges()
                            .iter()
                            .filter_map(|edge| {
                                if let Some((edge, weight)) = self.graph.edges.get_key_value(edge) {
                                    let destination_cell = edge.destination_index_unchecked();
                                    if !is_excluded(destination_cell) {
                                        Some((destination_cell, *weight))
                                    } else {
                                        None
                                    }
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        neighbors
                    },
                    // stop condition
                    |graph_cell| {
                        if let Some(cell) = filtered_destination_cells.get(graph_cell) {
                            destination_cells_reached.insert(*cell);

                            // stop when enough destination cells are reached
                            destination_cells_reached.len()
                                >= options
                                    .num_destinations_to_reach
                                    .unwrap_or_else(|| filtered_destination_cells.len())
                        } else {
                            false
                        }
                    },
                );

                // build the paths
                let paths: Vec<_> = {
                    let mut paths = Vec::with_capacity(destination_cells_reached.len());

                    for dest in destination_cells_reached.iter() {
                        let (path_cells, cost) = build_path_with_cost(dest, &routemap);
                        paths.push(Path {
                            cells: path_cells,
                            cost,
                        })
                    }

                    // return sorted from lowest to highest cost, use destination cell as second criteria
                    // to make path vecs directly comparable using this deterministic order
                    paths.sort_unstable();

                    // ensure the sorted order is correct by sorting path instances before applying
                    // the `path_map_fn`.
                    paths.drain(..).map(|p| path_map_fn(p)).collect()
                };

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
