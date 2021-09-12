//! Compute a shortest path using the [Dijkstra search
//! algorithm](https://en.wikipedia.org/wiki/Dijkstra's_algorithm).
//!
//!
//! Parts of this file have been taken from the excellent `pathfinding` crate and has been modified
//! to use `ahash` for the reasons detailed in the introduction of this crates docs, and
//! to use the `DijkstraSuccessorsGenerator` trait.

use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::hash::Hash;
use std::ops::Add;
use std::usize;

use indexmap::map::Entry::{Occupied, Vacant};
use indexmap::IndexMap;
use num_traits::Zero;

use h3ron::collections::{HashMap, RandomState};

pub trait DijkstraSuccessorsGenerator<'a, N, C>
where
    N: Eq + Hash + Clone,
    C: Zero + Ord + Copy,
{
    type IntoIter: IntoIterator<Item = (N, C)>;

    /// return an iterator visiting the successors of thie given `node`
    fn successors_iter(&mut self, node: &N) -> Self::IntoIter;
}

/// Determine some reachable nodes from a starting point as well as the minimum cost to
/// reach them and a possible optimal parent node
/// using the [Dijkstra search algorithm](https://en.wikipedia.org/wiki/Dijkstra's_algorithm).
///
/// - `start` is the starting node.
/// - `successors_generator` implements the generator trait to create an iterator of successors from the node.
/// - `stop` is a function which is called every time a node is examined (including `start`).
///   A `true` return value will stop the algorithm.
///
/// The result is a map where every node examined before the algorithm stopped (not including
/// `start`) is associated with an optimal parent node and a cost, as well as the node which
/// caused the algorithm to stop if any.
///
/// The [`build_path_with_cost`] function can be used to build a full path from the starting point to one
/// of the reachable targets.
pub fn dijkstra_partial<'a, N, C, G, FS>(
    start: &N,
    successors_generator: &mut G,
    mut stop: FS,
) -> (HashMap<N, (N, C)>, Option<N>)
where
    N: Eq + Hash + Clone,
    C: Zero + Ord + Copy,
    G: DijkstraSuccessorsGenerator<'a, N, C>,
    FS: FnMut(&N) -> bool,
{
    let (parents, reached) = run_dijkstra(start, successors_generator, &mut stop);
    (
        parents
            .iter()
            .skip(1)
            .map(|(n, (p, c))| (n.clone(), (parents.get_index(*p).unwrap().0.clone(), *c)))
            .collect(),
        reached.map(|i| parents.get_index(i).unwrap().0.clone()),
    )
}

fn run_dijkstra<'a, N, C, G, FS>(
    start: &N,
    successors_generator: &mut G,
    stop: &mut FS,
) -> (IndexMap<N, (usize, C), RandomState>, Option<usize>)
where
    N: Eq + Hash + Clone,
    C: Zero + Ord + Copy,
    G: DijkstraSuccessorsGenerator<'a, N, C>,
    FS: FnMut(&N) -> bool,
{
    let mut to_see = BinaryHeap::new();
    to_see.push(SmallestHolder {
        cost: Zero::zero(),
        index: 0,
    });
    let mut parents: IndexMap<N, (usize, C), RandomState> = IndexMap::default();
    parents.insert(start.clone(), (usize::max_value(), Zero::zero()));
    let mut target_reached = None;
    while let Some(SmallestHolder { cost, index }) = to_see.pop() {
        let successors = {
            let (node, &(_, c)) = parents.get_index(index).unwrap();
            if stop(node) {
                target_reached = Some(index);
                break;
            }
            // We may have inserted a node several time into the binary heap if we found
            // a better way to access it. Ensure that we are currently dealing with the
            // best path and discard the others.
            if cost > c {
                continue;
            }
            successors_generator.successors_iter(node)
        };
        for (successor, move_cost) in successors {
            let new_cost = cost + move_cost;
            let n;
            match parents.entry(successor) {
                Vacant(e) => {
                    n = e.index();
                    e.insert((index, new_cost));
                }
                Occupied(mut e) => {
                    if e.get().1 > new_cost {
                        n = e.index();
                        e.insert((index, new_cost));
                    } else {
                        continue;
                    }
                }
            }

            to_see.push(SmallestHolder {
                cost: new_cost,
                index: n,
            });
        }
    }
    (parents, target_reached)
}

/// Build a path leading to a target according to a parents map, which must
/// contain no loop. This function can be used after [`dijkstra_partial`]
/// to build a path from a starting point to a reachable target.
///
/// - `target` is reachable target.
/// - `parents` is a map containing an optimal parent (and an associated
///    cost which is ignored here) for every reachable node.
///
/// This function returns a tuple of vector with a path from the farthest parent up to
/// `target`, including `target` itself and the total cost.
///
/// # Panics
///
/// If the `parents` map contains a loop, this function will attempt to build
/// a path of infinite length and panic when memory is exhausted.
#[allow(clippy::implicit_hasher)]
pub fn build_path_with_cost<N, C>(target: &N, parents: &HashMap<N, (N, C)>) -> (Vec<N>, C)
where
    N: Eq + Hash + Clone,
    C: Add + Zero + Clone,
{
    let mut rev = vec![target.clone()];
    let mut next = target.clone();
    let mut cost: Option<C> = None;
    while let Some((parent, node_cost)) = parents.get(&next) {
        if cost.is_none() {
            // the cost is already summed up after routing, so we just need the last
            // cost value
            cost = Some(node_cost.clone());
        }
        rev.push(parent.clone());
        next = parent.clone();
    }
    rev.reverse();
    (rev, cost.unwrap_or_else(C::zero))
}

struct SmallestHolder<K> {
    cost: K,
    index: usize,
}

impl<K: PartialEq> PartialEq for SmallestHolder<K> {
    fn eq(&self, other: &Self) -> bool {
        self.cost == other.cost
    }
}

impl<K: PartialEq> Eq for SmallestHolder<K> {}

impl<K: Ord> PartialOrd for SmallestHolder<K> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<K: Ord> Ord for SmallestHolder<K> {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost)
    }
}
