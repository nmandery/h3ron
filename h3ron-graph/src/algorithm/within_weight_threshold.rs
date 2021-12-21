use std::borrow::Borrow;
use std::ops::Add;

use num_traits::Zero;
use rayon::prelude::*;

use h3ron::collections::hashbrown::hash_map::Entry;
use h3ron::collections::H3CellMap;
use h3ron::H3Cell;

use crate::algorithm::dijkstra::edge_dijkstra_weight_threshold;
use crate::graph::GetEdge;

/// Find all cells connected to the graph around a origin cell within a given threshold
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

/// Find all cells connected to the graph around a origin cell within a given threshold
pub trait WithinWeightThresholdMany<W> {
    /// Find all cells connected to the graph within a given `weight_threshold` around the
    /// given `origin_cells`.
    ///
    /// The weights for cells which are traversed from multiple `origin_cells` are aggregated using
    /// `agg_fn`. This can be used - for example - to find the minimum or maximum weight for a cell.
    fn cells_within_weight_threshold_many<I, AGG>(
        &self,
        origin_cells: I,
        weight_threshold: W,
        agg_fn: AGG,
    ) -> H3CellMap<W>
    where
        I: IntoParallelIterator,
        I::Item: Borrow<H3Cell>,
        AGG: Fn(&mut W, W) + Sync;
}

impl<W, G> WithinWeightThresholdMany<W> for G
where
    G: GetEdge<WeightType = W> + WithinWeightThreshold<W> + Sync,
    W: Zero + Ord + Copy + Add + Send + Sync,
{
    fn cells_within_weight_threshold_many<I, AGG>(
        &self,
        origin_cells: I,
        weight_threshold: W,
        agg_fn: AGG,
    ) -> H3CellMap<W>
    where
        I: IntoParallelIterator,
        I::Item: Borrow<H3Cell>,
        AGG: Fn(&mut W, W) + Sync,
    {
        origin_cells
            .into_par_iter()
            .map(|item| self.cells_within_weight_threshold(*item.borrow(), weight_threshold))
            .reduce_with(|cellmap1, cellmap2| {
                // select the source and target maps, to move the contents of the map with fewer elements, to the map
                // with more elements. This should save quite a few hashing operations.
                let (source_cellmap, mut target_cellmap) = if cellmap1.len() < cellmap2.len() {
                    (cellmap1, cellmap2)
                } else {
                    (cellmap2, cellmap1)
                };

                for (cell, weight) in source_cellmap {
                    match target_cellmap.entry(cell) {
                        Entry::Occupied(mut entry) => {
                            agg_fn(entry.get_mut(), weight);
                        }
                        Entry::Vacant(entry) => {
                            entry.insert(weight);
                        }
                    };
                }
                target_cellmap
            })
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryInto;

    use geo_types::{Geometry, Line};
    use itertools::Itertools;

    use h3ron::iter::continuous_cells_to_edges;
    use h3ron::{H3Cell, ToH3Cells};

    use crate::algorithm::{WithinWeightThreshold, WithinWeightThresholdMany};
    use crate::graph::{GetStats, H3EdgeGraph, PreparedH3EdgeGraph};

    /// a simple graph consisting of a single line
    fn line_graph(default_weight: u32) -> (Vec<H3Cell>, PreparedH3EdgeGraph<u32>) {
        let h3_resolution = 4;
        let cell_sequence: Vec<_> = Geometry::Line(Line {
            start: (10.0f64, 20.0f64).into(),
            end: (20., 20.).into(),
        })
        .to_h3_cells(h3_resolution)
        .unwrap()
        .iter()
        .collect();

        let mut g = H3EdgeGraph::new(h3_resolution);
        for edge_result in continuous_cells_to_edges(&cell_sequence) {
            g.add_edge(edge_result.unwrap(), default_weight).unwrap();
        }
        (cell_sequence, g.try_into().unwrap())
    }

    #[test]
    fn test_cells_within_weight_threshold() {
        let (cell_sequence, prepared_graph) = line_graph(10);
        assert!(prepared_graph.get_stats().num_edges > 10);
        let within_threshold = prepared_graph.cells_within_weight_threshold(cell_sequence[0], 30);
        assert_eq!(within_threshold.len(), 4);
        let weights: Vec<_> = within_threshold.values().copied().collect();
        assert!(weights.contains(&0));
        assert!(weights.contains(&10));
        assert!(weights.contains(&20));
        assert!(weights.contains(&30));
    }

    #[test]
    fn test_cells_within_weight_threshold_many() {
        let (cell_sequence, prepared_graph) = line_graph(10);
        assert!(prepared_graph.get_stats().num_edges > 20);

        let origin_cells = vec![
            cell_sequence[0],
            cell_sequence[1], // overlaps with the previous cell
            cell_sequence[10],
        ];

        let within_threshold = prepared_graph.cells_within_weight_threshold_many(
            &origin_cells,
            30,
            // use the minimum weight encountered
            |existing, new| {
                if new < *existing {
                    *existing = new
                }
            },
        );
        assert_eq!(within_threshold.len(), 9);
        let weights_freq = within_threshold.iter().counts_by(|w| *w.1);
        assert_eq!(weights_freq[&0], 3);
        assert_eq!(weights_freq[&10], 2);
        assert_eq!(weights_freq[&20], 2);
        assert_eq!(weights_freq[&30], 2);
    }
}
