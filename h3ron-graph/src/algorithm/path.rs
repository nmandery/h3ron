use std::cmp::Ordering;

use geo_types::LineString;
use serde::{Deserialize, Serialize};

use h3ron::to_geo::{ToLineString, ToMultiLineString};
use h3ron::{H3Cell, H3Edge, Index};

use crate::error::Error;

/// [Path] describes a path between a cell and another.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum Path<W> {
    /// path is empty as origin and destination are the same.
    OriginIsDestination(H3Cell, W),

    /// a sequence of edges describing the path.
    EdgeSequence(EdgeSequence<W>),
}

/// A path of continuous [`H3Edge`] values with an associated cost.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct EdgeSequence<W> {
    /// Vec of [`H3Edge`] values of the route in the order origin -> destination
    pub edges: Vec<H3Edge>,

    /// the total cost of the route (= sum of all edge weights).
    pub path_cost: W,
}

impl<W> EdgeSequence<W> {
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn len(&self) -> usize {
        self.edges.len()
    }

    pub fn origin_cell(&self) -> Result<H3Cell, Error> {
        self.edges
            .first()
            .map(|edge| edge.origin_index_unchecked())
            .ok_or(Error::EmptyPath)
    }

    pub fn destination_cell(&self) -> Result<H3Cell, Error> {
        self.edges
            .last()
            .map(|edge| edge.destination_index_unchecked())
            .ok_or(Error::EmptyPath)
    }

    pub fn to_linestring(&self) -> Result<LineString<f64>, Error> {
        match self.edges.len() {
            0 => Err(Error::InsufficientNumberOfEdges),
            1 => Ok(self.edges[0].to_linestring()?),
            _ => {
                let mut multilinesstring = self.edges.to_multilinestring()?;
                match multilinesstring.0.len() {
                    0 => Err(Error::InsufficientNumberOfEdges),
                    1 => Ok(multilinesstring.0.remove(0)),
                    _ => Err(Error::SegmentedPath),
                }
            }
        }
    }
}

impl<W> Path<W> {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::OriginIsDestination(_, _) => true,
            Self::EdgeSequence(es) => es.is_empty(),
        }
    }

    /// Length of the path in number of edges
    pub fn len(&self) -> usize {
        match self {
            Self::OriginIsDestination(_, _) => 0,
            Self::EdgeSequence(es) => es.len(),
        }
    }

    pub fn origin_cell(&self) -> Result<H3Cell, Error> {
        match self {
            Self::OriginIsDestination(cell, _) => Ok(*cell),
            Self::EdgeSequence(es) => es.origin_cell(),
        }
    }

    pub fn destination_cell(&self) -> Result<H3Cell, Error> {
        match self {
            Self::OriginIsDestination(cell, _) => Ok(*cell),
            Self::EdgeSequence(es) => es.destination_cell(),
        }
    }

    pub fn to_linestring(&self) -> Result<LineString<f64>, Error> {
        match self {
            Self::OriginIsDestination(_, _) => Err(Error::InsufficientNumberOfEdges),
            Self::EdgeSequence(es) => es.to_linestring(),
        }
    }

    pub fn cost(&self) -> &W {
        match self {
            Self::OriginIsDestination(_, c) => c,
            Self::EdgeSequence(es) => &es.path_cost,
        }
    }
}

/// order by cost, origin index and destination_index.
///
/// This ordering can used to bring `Vec`s of routes in a deterministic order to make them
/// comparable
impl<W> Ord for Path<W>
where
    W: Ord,
{
    fn cmp(&self, other: &Self) -> Ordering {
        let cmp_cost = self.cost().cmp(other.cost());
        if cmp_cost == Ordering::Equal {
            let cmp_origin =
                index_or_zero(self.origin_cell()).cmp(&index_or_zero(other.origin_cell()));
            if cmp_origin == Ordering::Equal {
                index_or_zero(self.destination_cell()).cmp(&index_or_zero(other.destination_cell()))
            } else {
                cmp_origin
            }
        } else {
            cmp_cost
        }
    }
}

impl<W> PartialOrd for Path<W>
where
    W: Ord,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[inline]
fn index_or_zero(cell: Result<H3Cell, Error>) -> u64 {
    cell.map(|c| c.h3index()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use h3ron::{H3Edge, Index};

    use crate::algorithm::path::EdgeSequence;

    use super::Path;

    #[test]
    fn paths_deterministic_ordering() {
        let r1 = Path::EdgeSequence(EdgeSequence {
            edges: vec![H3Edge::new(0x1176b49474ffffff)],
            path_cost: 1,
        });
        let r2 = Path::EdgeSequence(EdgeSequence {
            edges: vec![H3Edge::new(0x1476b49474ffffff)],
            path_cost: 3,
        });
        let r3 = Path::EdgeSequence(EdgeSequence {
            edges: vec![H3Edge::new(0x1476b4b2c2ffffff)],
            path_cost: 3,
        });
        let mut paths = vec![r3.clone(), r1.clone(), r2.clone()];
        paths.sort_unstable();
        assert_eq!(paths[0], r1);
        assert_eq!(paths[1], r2);
        assert_eq!(paths[2], r3);
    }
}
