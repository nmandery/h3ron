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
    ///
    /// The edges in the vec are expected to be consecutive.
    ///
    /// The cost is the total cost summed for all of the edges.
    EdgeSequence(Vec<H3Edge>, W),
}

impl<W> Path<W> {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::OriginIsDestination(_, _) => true,
            Self::EdgeSequence(edges, _) => edges.is_empty(),
        }
    }

    /// Length of the path in number of edges
    pub fn len(&self) -> usize {
        match self {
            Self::OriginIsDestination(_, _) => 0,
            Self::EdgeSequence(edges, _) => edges.len(),
        }
    }

    pub fn origin_cell(&self) -> Result<H3Cell, Error> {
        match self {
            Self::OriginIsDestination(cell, _) => Ok(*cell),
            Self::EdgeSequence(edges, _) => edges
                .first()
                .map(|edge| edge.origin_index_unchecked())
                .ok_or(Error::EmptyPath),
        }
    }

    pub fn destination_cell(&self) -> Result<H3Cell, Error> {
        match self {
            Self::OriginIsDestination(cell, _) => Ok(*cell),
            Self::EdgeSequence(edges, _) => edges
                .last()
                .map(|edge| edge.destination_index_unchecked())
                .ok_or(Error::EmptyPath),
        }
    }

    pub fn to_linestring(&self) -> Result<LineString<f64>, Error> {
        match self {
            Self::OriginIsDestination(_, _) => Err(Error::InsufficientNumberOfEdges),
            Self::EdgeSequence(edges, _) => match edges.len() {
                0 => Err(Error::InsufficientNumberOfEdges),
                1 => Ok(edges[0].to_linestring()?),
                _ => {
                    let mut multilinesstring = edges.to_multilinestring()?;
                    match multilinesstring.0.len() {
                        0 => Err(Error::InsufficientNumberOfEdges),
                        1 => Ok(multilinesstring.0.remove(0)),
                        _ => Err(Error::SegmentedPath),
                    }
                }
            },
        }
    }

    pub fn cost(&self) -> &W {
        match self {
            Self::OriginIsDestination(_, c) => c,
            Self::EdgeSequence(_, c) => c,
        }
    }

    pub fn edges(&self) -> &[H3Edge] {
        match self {
            Self::EdgeSequence(edges, _) => edges.as_slice(),
            Self::OriginIsDestination(_, _) => &[],
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

    use super::Path;

    #[test]
    fn paths_deterministic_ordering() {
        let r1 = Path::EdgeSequence(vec![H3Edge::new(0x1176b49474ffffff)], 1);
        let r2 = Path::EdgeSequence(vec![H3Edge::new(0x1476b49474ffffff)], 3);
        let r3 = Path::EdgeSequence(vec![H3Edge::new(0x1476b4b2c2ffffff)], 3);
        let mut paths = vec![r3.clone(), r1.clone(), r2.clone()];
        paths.sort_unstable();
        assert_eq!(paths[0], r1);
        assert_eq!(paths[1], r2);
        assert_eq!(paths[2], r3);
    }
}
