use std::ops::Add;

use num_traits::Zero;

use h3ron::collections::H3CellMap;
use h3ron::H3Cell;

use crate::algorithm::dijkstra::edge_dijkstra_weight_threshold;
use crate::graph::GetEdge;

/// Find all cells connected to the graph within a given threshold
pub trait WithinWeightThreshold<W> {
    /// Find all cells connected to the graph within a given `weight_threshold` around the
    /// given `origin_cell`
    fn cells_within_weight_threshold(
        &self,
        origin_cell: H3Cell,
        weight_threshold: W,
    ) -> H3CellMap<W>;
}

impl<W, G> WithinWeightThreshold<W> for G
where
    G: GetEdge<WeightType = W>,
    W: Zero + Ord + Copy + Add,
{
    fn cells_within_weight_threshold(
        &self,
        origin_cell: H3Cell,
        weight_threshold: W,
    ) -> H3CellMap<W> {
        edge_dijkstra_weight_threshold(self, &origin_cell, weight_threshold)
    }
}
