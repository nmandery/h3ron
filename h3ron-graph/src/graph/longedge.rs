use std::borrow::Borrow;
use std::convert::TryFrom;

use serde::{Deserialize, Serialize};

use h3ron::collections::compressed::CompressedIndexVec;
use h3ron::collections::H3Treemap;
use h3ron::{H3Cell, H3Edge};

use crate::error::Error;

/// `h3dge_path` is a iterator of `H3Edge` where the edges form a continuous path
fn h3edge_path_to_h3cell_path<I>(h3edge_path: I) -> Vec<H3Cell>
where
    I: IntoIterator,
    I::Item: Borrow<H3Edge>,
{
    let mut iter = h3edge_path.into_iter();
    let mut out_vec = Vec::with_capacity(iter.size_hint().0 + 1);
    if let Some(h3edge) = iter.next() {
        out_vec.push(h3edge.borrow().origin_index_unchecked());
        out_vec.push(h3edge.borrow().destination_index_unchecked());
    }
    for h3edge in iter {
        out_vec.push(h3edge.borrow().destination_index_unchecked());
    }
    out_vec
}

/// A `LongEdge` is an artificial construct to combine a continuous path
/// of [`H3Edge`] values into a single edge.
///
/// This intended to be used to compress longer paths into a single edge to
/// reduce the number of nodes to visit during routing.
#[derive(Serialize, Deserialize, Clone)]
pub struct LongEdge {
    pub in_edge: H3Edge,
    pub out_edge: H3Edge,

    /// the path of the longedge described by multiple, successive
    /// `H3Edge` values.
    edge_path: CompressedIndexVec<H3Edge>,

    /// provides an efficient lookup to check for intersection of
    /// the edge with `H3Cell` values.
    cell_lookup: H3Treemap<H3Cell>,
}

impl LongEdge {
    pub fn destination_cell(&self) -> H3Cell {
        self.out_edge.destination_index_unchecked()
    }

    pub fn origin_cell(&self) -> H3Cell {
        self.in_edge.origin_index_unchecked()
    }

    pub fn is_disjoint(&self, celltreemap: &H3Treemap<H3Cell>) -> bool {
        self.cell_lookup.is_disjoint(celltreemap)
    }

    /// length of `self` as the number of contained h3edges
    pub fn h3edges_len(&self) -> usize {
        (self.edge_path.len() as usize).saturating_sub(1)
    }

    /// the path of the longedge described by multiple, successive `H3Edge` values
    pub fn h3edge_path(&self) -> Vec<H3Edge> {
        self.edge_path.to_vec()
    }
}

/// construct an longedge from a vec of `H3Edges`.
///
/// The `H3Edge` must be sorted according to the path they describe
impl TryFrom<Vec<H3Edge>> for LongEdge {
    type Error = Error;

    fn try_from(mut h3edges: Vec<H3Edge>) -> Result<Self, Self::Error> {
        h3edges.dedup();
        h3edges.shrink_to_fit();
        if h3edges.len() >= 2 {
            let cell_lookup: H3Treemap<_> = h3edge_path_to_h3cell_path(&h3edges).iter().collect();
            Ok(LongEdge {
                in_edge: h3edges[0],
                out_edge: *h3edges.last().unwrap(),
                edge_path: h3edges.into(),
                cell_lookup,
            })
        } else {
            Err(Error::InsufficientNumberOfEdges)
        }
    }
}
