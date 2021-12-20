use std::cmp::Ordering;
use std::collections::BinaryHeap;

use indexmap::map::Entry::{Occupied, Vacant};
use indexmap::map::IndexMap;
use num_traits::Zero;

use h3ron::collections::{H3CellSet, H3Treemap, HashMap, RandomState};
use h3ron::iter::H3EdgesBuilder;
use h3ron::{H3Cell, H3Edge};

use crate::algorithm::path::Path;
use crate::graph::longedge::LongEdge;
use crate::graph::GetEdge;

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
    const fn last_edge(&self) -> H3Edge {
        match self {
            Self::Single(h3edge) => *h3edge,
            Self::Long(longedge) => longedge.out_edge,
        }
    }

    #[allow(dead_code)]
    const fn first_edge(&self) -> H3Edge {
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
pub fn edge_dijkstra<'a, G, W, PM, O>(
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
            if destinations_reached.insert(*cell)
                && destinations_reached.len() >= num_destinations_to_reach
            {
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

    edge_dijkstra_assemble_paths(start_cell, parents_map, destinations_reached, path_map_fn)
}

fn edge_dijkstra_assemble_paths<'a, W, PM, O>(
    start_cell: &H3Cell,
    parents_map: HashMap<H3Cell, (&'a H3Cell, &DijkstraEntry<'a, W>)>,
    destinations_reached: H3CellSet,
    path_map_fn: &PM,
) -> Vec<O>
where
    W: Zero + Ord + Copy,
    PM: Fn(Path<W>) -> O,
{
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
            // dijkstra_edge and the contained longedge is already in the correct order in
            // itself and does not need to be reversed
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
        // sort by priority, lowest values have the highest priority
        other.weight.cmp(&self.weight)
    }
}

#[cfg(test)]
mod tests {
    use crate::algorithm::dijkstra::SmallestHolder;

    #[test]
    fn smallest_holder_partial_eq() {
        let sh1 = SmallestHolder {
            weight: 10u8,
            index: 4,
        };
        let sh2 = SmallestHolder {
            weight: 10u8,
            index: 10,
        };
        assert!(sh2 == sh1);
    }

    #[test]
    fn smallest_holder_partial_ord() {
        let sh1 = SmallestHolder {
            weight: 10u8,
            index: 4,
        };
        let sh2 = SmallestHolder {
            weight: 7u8,
            index: 4,
        };
        assert!(sh2 > sh1);
    }
}
