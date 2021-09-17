use std::borrow::Borrow;
use std::fmt::Debug;
use std::ops::Add;

use num_traits::Zero;
use rayon::prelude::*;

use h3ron::collections::{H3CellMap, H3CellSet, HashMap};
use h3ron::iter::{change_cell_resolution, H3EdgesBuilder};
use h3ron::{H3Cell, HasH3Resolution};

use crate::algorithm::dijkstra::{
    build_path_with_cost, dijkstra_partial, DijkstraSuccessorsGenerator,
};
use crate::algorithm::path::Path;
use crate::error::Error;
use crate::routing::RoutingH3EdgeGraph;
use std::cell::RefCell;
use std::rc::Rc;

pub trait ShortestPathOptions {
    /// cells which are not allowed to be used for routing
    fn exclude_cells(&self) -> Option<H3CellSet> {
        None
    }

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

pub trait ShortestPath<T: Ord + Send + Clone> {
    fn shortest_path<OPT: ShortestPathOptions>(
        &self,
        origin_cell: H3Cell,
        destination_cell: H3Cell,
        options: &OPT,
    ) -> Result<Option<Path<T>>, Error>
    where
        OPT: ShortestPathOptions;
}

pub trait ShortestPathManyToMany<T: Ord + Send + Clone> {
    /// Returns found paths keyed by the origin cell.
    ///
    /// All cells must be in the h3 resolution of the graph.
    #[inline]
    fn shortest_path_many_to_many<I, OPT>(
        &self,
        origin_cells: I,
        destination_cells: I,
        options: &OPT,
    ) -> Result<H3CellMap<Vec<Path<T>>>, Error>
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
        PM: Fn(Path<T>) -> O + Send + Sync,
        O: Send + Ord + Clone;
}

impl<T> ShortestPathManyToMany<T> for RoutingH3EdgeGraph<T>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync + Debug,
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
        PM: Fn(Path<T>) -> O + Send + Sync,
        O: Send + Ord + Clone,
    {
        let filtered_origin_cells =
            self.filtered_origin_cells(options.num_gap_cells_to_graph(), origin_cells);
        if filtered_origin_cells.is_empty() {
            return Ok(Default::default());
        }

        let filtered_destination_cells =
            self.filtered_destination_cells(options.num_gap_cells_to_graph(), destination_cells)?;

        log::debug!(
            "shortest_path many-to-many: from {} cells to {} cells at resolution {} with num_gap_cells_to_graph = {}",
            filtered_origin_cells.len(),
            filtered_destination_cells.len(),
            self.h3_resolution(),
            options.num_gap_cells_to_graph()
        );
        let exclude_cells = options.exclude_cells();
        let paths = filtered_origin_cells
            .par_iter()
            .map(|(origin_cell, output_origin_cells)| {
                let mut destination_cells_reached = H3CellSet::default();

                let mut successors_gen = SuccessorsGenerator::new(self, &exclude_cells);

                // Possible improvement: add timeout to avoid continuing routing forever
                let (routemap, _) = dijkstra_partial(
                    // start cell
                    origin_cell,
                    // successor cells
                    &mut successors_gen,
                    // stop condition
                    |graph_cell| {
                        if let Some(cell) = filtered_destination_cells.get(graph_cell) {
                            destination_cells_reached.insert(*cell);

                            // stop when enough destination cells are reached
                            destination_cells_reached.len()
                                >= options
                                    .num_destinations_to_reach()
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

impl<T> ShortestPath<T> for RoutingH3EdgeGraph<T>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync + Debug,
{
    fn shortest_path<OPT>(
        &self,
        origin_cell: H3Cell,
        destination_cell: H3Cell,
        options: &OPT,
    ) -> Result<Option<Path<T>>, Error>
    where
        OPT: ShortestPathOptions,
    {
        let filtered_origin_cells = self.filtered_origin_cells(
            options.num_gap_cells_to_graph(),
            std::iter::once(origin_cell),
        );
        let filtered_origin_cell = if let Some(first_fo) = filtered_origin_cells.first() {
            first_fo.0
        } else {
            return Ok(None);
        };

        let (_, graph_destination_cell) = self
            .filtered_destination_cells(
                options.num_gap_cells_to_graph(),
                std::iter::once(destination_cell),
            )?
            .drain()
            .next()
            .unwrap();

        let exclude_cells = options.exclude_cells();
        let mut successors_gen = SuccessorsGenerator::new(self, &exclude_cells);

        // Possible improvement: add timeout to avoid continuing routing forever
        let (routemap, _) = dijkstra_partial(
            // start cell
            &filtered_origin_cell,
            // successor cells
            &mut successors_gen,
            // stop condition
            |graph_cell| *graph_cell == graph_destination_cell,
        );

        let (path_cells, cost) = build_path_with_cost(&graph_destination_cell, &routemap);
        if path_cells.is_empty() {
            Ok(None)
        } else {
            Ok(Some(Path {
                cells: path_cells,
                cost,
            }))
        }
    }
}

impl<T> RoutingH3EdgeGraph<T>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync + Debug,
{
    /// maps the requested cells to the directly to the graph connected cells in
    /// graph where theses are used as a substitute. For direct graph members
    /// both cells are the same
    /// TODO: this should be a 1:n relationship in case multiple cells map to
    ///      the same cell in the graph
    ///
    /// The cell resolution is changed to the resolution of the graph.
    ///
    /// There must be at least one destination to get Result::Ok, otherwise
    /// the complete graph would be traversed.
    fn filtered_destination_cells<I>(
        &self,
        num_gap_cells_to_graph: u32,
        destination_cells: I,
    ) -> Result<HashMap<H3Cell, H3Cell>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
    {
        let destinations: HashMap<H3Cell, H3Cell> = self
            .filtered_graph_membership::<Vec<_>, _>(
                change_cell_resolution(destination_cells, self.h3_resolution()).collect(),
                |node_type| node_type.is_destination(),
                num_gap_cells_to_graph,
            )
            .drain(..)
            .filter_map(|graph_membership| {
                // ignore all non-connected destinations
                graph_membership
                    .corresponding_cell_in_graph()
                    .map(|graph_cell| (graph_cell, graph_membership.cell()))
            })
            .collect();

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
    fn filtered_origin_cells<I>(
        &self,
        num_gap_cells_to_graph: u32,
        origin_cells: I,
    ) -> Vec<(H3Cell, Vec<H3Cell>)>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
    {
        // maps cells to their closest found neighbors in the graph
        let mut origin_cell_map = H3CellMap::default();
        for gm in self
            .filtered_graph_membership::<Vec<_>, _>(
                change_cell_resolution(origin_cells, self.h3_resolution()).collect(),
                |node_type| node_type.is_origin(),
                num_gap_cells_to_graph,
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
    }
}

/// generates all successors of a cell for the shortest_path algorithm
///
/// This struct allocates the memory only once for all repeated calls. This
/// results in in runtime improvement of approx. -25% by avoiding
/// repeated allocations and deallocations during a benchmark
struct SuccessorsGenerator<'a, T: Send + Sync> {
    routing_edge_graph: &'a RoutingH3EdgeGraph<T>,
    exclude_cells: &'a Option<H3CellSet>,
    edges_builder: H3EdgesBuilder,
    // TODO: figure out the correct lifetimes to avoid having to use Rc and RefCell
    #[allow(clippy::type_complexity)]
    out_cells: Rc<RefCell<[Option<(H3Cell, T)>; 6]>>,
}

impl<'a, T: Send + Sync> SuccessorsGenerator<'a, T>
where
    T: Copy,
{
    pub fn new(
        routing_edge_graph: &'a RoutingH3EdgeGraph<T>,
        exclude_cells: &'a Option<H3CellSet>,
    ) -> Self {
        Self {
            routing_edge_graph,
            exclude_cells,
            edges_builder: Default::default(),
            out_cells: Rc::new(RefCell::new([None; 6])),
        }
    }
}

impl<'a, T: Send + Sync> DijkstraSuccessorsGenerator<'a, H3Cell, T> for SuccessorsGenerator<'a, T>
where
    T: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync + Debug,
{
    type IntoIter = SuccessorsGeneratorIter<T>;

    fn successors_iter(&mut self, node: &H3Cell) -> Self::IntoIter {
        let mut edges_iter = self.edges_builder.from_origin_cell(node);

        for out_cell in (*self.out_cells).borrow_mut().iter_mut() {
            *out_cell = match edges_iter.next() {
                Some(edge) => {
                    if let Some(weight) = self.routing_edge_graph.graph.edges.get(&edge) {
                        let destination_cell = edge.destination_index_unchecked();
                        let is_excluded = self
                            .exclude_cells
                            .as_ref()
                            .map(|exclude| exclude.contains(&destination_cell))
                            .unwrap_or(false);
                        if !is_excluded {
                            Some((destination_cell, *weight))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                None => None,
            }
        }
        Self::IntoIter {
            out_cells: self.out_cells.clone(),
            current_pos: 0,
        }
    }
}

struct SuccessorsGeneratorIter<T: Send + Sync> {
    #[allow(clippy::type_complexity)]
    out_cells: Rc<RefCell<[Option<(H3Cell, T)>; 6]>>,
    current_pos: usize,
}

impl<T: Send + Sync> Iterator for SuccessorsGeneratorIter<T>
where
    T: Copy,
{
    type Item = (H3Cell, T);

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_pos < (*self.out_cells).borrow().len() {
            if let Some(next) = (*self.out_cells).borrow()[self.current_pos] {
                self.current_pos += 1;
                return Some(next);
            } else {
                self.current_pos += 1;
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        ((*self.out_cells).borrow().len(), None)
    }
}
