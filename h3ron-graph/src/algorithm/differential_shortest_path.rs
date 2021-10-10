use std::borrow::Borrow;
use std::ops::Add;

use num_traits::Zero;

use h3ron::collections::{ContainsIndex, H3CellMap, H3Treemap, HashMap};
use h3ron::{H3Cell, HasH3Resolution, Index};

use crate::algorithm::path::Path;
use crate::algorithm::shortest_path::{ShortestPathManyToMany, ShortestPathOptions};
use crate::error::Error;
use crate::graph::modifiers::ExcludeCells;
use crate::graph::node::GetGapBridgedCellNodes;
use crate::graph::{GetEdge, GetNodeType};

pub struct Diff<T> {
    pub with_excluded_cells: Vec<T>,
    pub without_excluded_cells: Vec<T>,
}

/// "Differential" routing calculates the shortest path from (multiple) origin cells
/// to the `N` nearest destinations.
/// This done once to the un-modified graph, and once the the graph with a set of nodes
/// being removed, the `exclude_cells` parameter.
pub trait DifferentialShortestPath<W>
where
    W: Send + Sync + Ord + Copy,
{
    fn differential_shortest_path<I, OPT>(
        &self,
        origin_cells: I,
        destination_cells: I,
        exclude_cells: &H3Treemap<H3Cell>,
        options: &OPT,
    ) -> Result<HashMap<H3Cell, Diff<Path<W>>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
        OPT: ShortestPathOptions + Send + Sync,
    {
        self.differential_shortest_path_map(
            origin_cells,
            destination_cells,
            exclude_cells,
            options,
            |path| path,
        )
    }

    fn differential_shortest_path_map<I, OPT, PM, O>(
        &self,
        origin_cells: I,
        destination_cells: I,
        exclude_cells: &H3Treemap<H3Cell>,
        options: &OPT,
        path_map_fn: PM,
    ) -> Result<HashMap<H3Cell, Diff<O>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
        OPT: ShortestPathOptions + Send + Sync,
        O: Send + Ord + Clone,
        PM: Fn(Path<W>) -> O + Send + Sync;
}

impl<G, W> DifferentialShortestPath<W> for G
where
    W: PartialOrd + PartialEq + Add + Copy + Send + Ord + Zero + Sync,
    G: GetEdge<WeightType = W>
        + GetNodeType
        + HasH3Resolution
        + GetGapBridgedCellNodes
        + Sync
        + ShortestPathManyToMany<W>,
{
    fn differential_shortest_path_map<I, OPT, PM, O>(
        &self,
        origin_cells: I,
        destination_cells: I,
        exclude_cells: &H3Treemap<H3Cell>,
        options: &OPT,
        path_map_fn: PM,
    ) -> Result<HashMap<H3Cell, Diff<O>>, Error>
    where
        I: IntoIterator,
        I::Item: Borrow<H3Cell>,
        OPT: ShortestPathOptions + Send + Sync,
        O: Send + Ord + Clone,
        PM: Fn(Path<W>) -> O + Send + Sync,
    {
        if exclude_cells.is_empty() {
            return Err(Error::Other("exclude_cells must not be empty".to_string()));
        };
        let origin_cells = check_resolution_and_collect(
            origin_cells.into_iter().filter(|c| {
                // exclude the cells of the disturbance itself from routing
                !exclude_cells.contains_index(c.borrow())
            }),
            self.h3_resolution(),
        )?;
        let destination_cells =
            check_resolution_and_collect(destination_cells, self.h3_resolution())?;

        let mut paths_before = self.shortest_path_many_to_many_map(
            &origin_cells,
            &destination_cells,
            options,
            &path_map_fn,
        )?;

        let exclude_wrapper = ExcludeCells::new(self, exclude_cells);
        let mut paths_after = exclude_wrapper.shortest_path_many_to_many_map(
            &origin_cells,
            &destination_cells,
            options,
            path_map_fn,
        )?;

        let mut out_diffs = H3CellMap::with_capacity(paths_before.len());
        for (cell, paths) in paths_before.drain() {
            out_diffs.insert(
                cell,
                Diff {
                    with_excluded_cells: paths,
                    without_excluded_cells: paths_after.remove(&cell).unwrap_or_default(),
                },
            );
        }
        Ok(out_diffs)
    }
}

fn check_resolution_and_collect<I>(in_cells: I, h3_resolution: u8) -> Result<Vec<H3Cell>, Error>
where
    I: IntoIterator,
    I::Item: Borrow<H3Cell>,
{
    let mut out_cells = in_cells
        .into_iter()
        .map(|cell| {
            if cell.borrow().resolution() != h3_resolution {
                Err(Error::MixedH3Resolutions(
                    h3_resolution,
                    cell.borrow().resolution(),
                ))
            } else {
                Ok(*cell.borrow())
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    out_cells.sort_unstable();
    out_cells.dedup();
    Ok(out_cells)
}
