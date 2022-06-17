use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::ops::Add;

use indexmap::map::Entry::{Occupied, Vacant};
use indexmap::map::IndexMap;
use num_traits::Zero;

use h3ron::collections::compressed::Decompressor;
use h3ron::collections::{H3CellMap, H3CellSet, H3Treemap, HashMap, RandomState};
use h3ron::{H3Cell, H3DirectedEdge, Index};

use crate::algorithm::path::{DirectedEdgePath, Path};
use crate::error::Error;
use crate::graph::longedge::LongEdge;
use crate::graph::GetCellEdges;

#[derive(Clone)]
enum DijkstraEdge<'a> {
    Single(H3DirectedEdge),
    Long(&'a LongEdge),
}

impl<'a> DijkstraEdge<'a> {
    #[allow(dead_code)]
    fn origin_cell(&self) -> Result<H3Cell, Error> {
        let cell = match self {
            Self::Single(h3edge) => h3edge.origin_cell()?,
            Self::Long(longedge) => longedge.origin_cell()?,
        };
        Ok(cell)
    }

    fn destination_cell(&self) -> Result<H3Cell, Error> {
        let cell = match self {
            Self::Single(h3edge) => h3edge.destination_cell()?,
            Self::Long(longedge) => longedge.destination_cell()?,
        };
        Ok(cell)
    }

    #[allow(dead_code)]
    const fn last_edge(&self) -> H3DirectedEdge {
        match self {
            Self::Single(h3edge) => *h3edge,
            Self::Long(longedge) => longedge.out_edge,
        }
    }

    #[allow(dead_code)]
    const fn first_edge(&self) -> H3DirectedEdge {
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

/// follow the edges of the graph until the aggregated weights reach `threshold_weight`.
/// Returns a hashmap of all traversed cells and the weight.
///
/// This function does not make usage of longedges.
pub fn edge_dijkstra_weight_threshold<G, W>(
    graph: &G,
    origin_cell: &H3Cell,
    threshold_weight: W,
    // TODO: optional bitmap/set of cells we are interested in
) -> Result<H3CellMap<W>, Error>
where
    G: GetCellEdges<EdgeWeightType = W>,
    W: Zero + Ord + Copy + Add,
{
    let mut to_see = BinaryHeap::new();
    let mut parents: IndexMap<H3Cell, W, RandomState> = IndexMap::default();

    to_see.push(SmallestHolder {
        weight: W::zero(),
        index: 0,
    });
    parents.insert(*origin_cell, W::zero());

    while let Some(SmallestHolder { weight, index }) = to_see.pop() {
        let (cell, weight_from_parents) = parents.get_index(index).unwrap();

        // We may have inserted a node several time into the binary heap if we found
        // a better way to access it. Ensure that we are currently dealing with the
        // best path and discard the others.
        if weight > *weight_from_parents {
            continue;
        }

        for (succeeding_edge, succeeding_edge_value) in graph.get_edges_originating_from(cell)? {
            // TODO: make use of longedges in case a subset-of-interest is set

            let new_weight = weight + succeeding_edge_value.weight;

            // skip following this edge when the threshold is reached.
            if new_weight > threshold_weight {
                continue;
            }

            let n;
            match parents.entry(succeeding_edge.destination_cell()?) {
                Vacant(e) => {
                    n = e.index();
                    e.insert(new_weight);
                }
                Occupied(mut e) => {
                    if e.get() > &new_weight {
                        n = e.index();
                        e.insert(new_weight);
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
    Ok(parents.drain(..).collect())
}

/// Dijkstra shortest path using h3 edges
///
/// Adapted from the `run_dijkstra` function of the `pathfinding` crate.
pub fn edge_dijkstra<'a, G, W>(
    graph: &'a G,
    origin_cell: &H3Cell,
    destinations: &H3Treemap<H3Cell>,
    num_destinations_to_reach: Option<usize>,
) -> Result<Vec<Path<W>>, Error>
where
    G: GetCellEdges<EdgeWeightType = W>,
    W: Zero + Ord + Copy + Add,
{
    // this is the main exit condition. Stop after this many destinations have been reached or
    // the complete graph has been traversed.
    let num_destinations_to_reach = num_destinations_to_reach
        .unwrap_or_else(|| destinations.len())
        .min(destinations.len());

    let mut to_see = BinaryHeap::new();
    let mut parents: IndexMap<H3Cell, DijkstraEntry<W>, RandomState> = IndexMap::default();
    let mut destinations_reached = H3CellSet::default();

    to_see.push(SmallestHolder {
        weight: W::zero(),
        index: 0,
    });
    parents.insert(
        *origin_cell,
        DijkstraEntry {
            weight: W::zero(),
            index: usize::MAX,
            edge: None,
        },
    );
    while let Some(SmallestHolder { weight, index }) = to_see.pop() {
        let (cell, dijkstra_entry) = parents.get_index(index).unwrap();
        if destinations.contains(cell)
            && destinations_reached.insert(*cell)
            && destinations_reached.len() >= num_destinations_to_reach
        {
            break;
        }

        // We may have inserted a node several time into the binary heap if we found
        // a better way to access it. Ensure that we are currently dealing with the
        // best path and discard the others.
        if weight > dijkstra_entry.weight {
            continue;
        }

        for (succeeding_edge, succeeding_edge_value) in graph.get_edges_originating_from(cell)? {
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
            match parents.entry(dijkstra_edge.destination_cell()?) {
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

    edge_dijkstra_assemble_paths(origin_cell, parents_map, destinations_reached)
}

fn edge_dijkstra_assemble_paths<'a, W>(
    origin_cell: &H3Cell,
    parents_map: HashMap<H3Cell, (&'a H3Cell, &DijkstraEntry<'a, W>)>,
    destinations_reached: H3CellSet,
) -> Result<Vec<Path<W>>, Error>
where
    W: Zero + Ord + Copy,
{
    let mut decompressor = Decompressor::default();

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
        for dijkstra_edge in rev_dijkstra_edges.into_iter() {
            // dijkstra_edge and the contained longedge is already in the correct order in
            // itself and does not need to be reversed
            match dijkstra_edge {
                DijkstraEdge::Single(h3edge) => h3edges.push(*h3edge),
                DijkstraEdge::Long(longedge) => {
                    for h3edge in decompressor.decompress_block(&longedge.edge_path)? {
                        h3edge.validate()?;
                        h3edges.push(h3edge);
                    }
                }
            }
        }
        let path_directed_edges = if h3edges.is_empty() {
            DirectedEdgePath::OriginIsDestination(*origin_cell)
        } else {
            DirectedEdgePath::DirectedEdgeSequence(h3edges)
        };

        paths.push((path_directed_edges, total_weight.unwrap_or_else(W::zero)).try_into()?);
    }

    // return sorted from lowest to highest cost, use destination cell as second criteria
    // to make path vecs directly comparable using this deterministic order
    paths.sort_unstable();

    Ok(paths)
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
