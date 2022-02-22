use std::cmp::Ordering;

use geo_types::LineString;
use serde::{Deserialize, Serialize};

use h3ron::to_geo::{ToLineString, ToMultiLineString};
use h3ron::{H3Cell, H3DirectedEdge, Index};

use crate::error::Error;

/// [DirectedEdgePath] describes a path between a cell and another.
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum DirectedEdgePath {
    /// path is empty as origin and destination are the same.
    OriginIsDestination(H3Cell),

    /// a sequence of edges describing the path.
    ///
    /// The edges in the vec are expected to be consecutive.
    ///
    /// The cost is the total cost summed for all of the edges.
    DirectedEdgeSequence(Vec<H3DirectedEdge>),
}

impl DirectedEdgePath {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::OriginIsDestination(_) => true,
            Self::DirectedEdgeSequence(edges) => edges.is_empty(),
        }
    }

    /// Length of the path in number of edges
    pub fn len(&self) -> usize {
        match self {
            Self::OriginIsDestination(_) => 0,
            Self::DirectedEdgeSequence(edges) => edges.len(),
        }
    }

    pub fn origin_cell(&self) -> Result<H3Cell, Error> {
        match self {
            Self::OriginIsDestination(cell) => Ok(*cell),
            Self::DirectedEdgeSequence(edges) => {
                if let Some(edge) = edges.first() {
                    Ok(edge.origin_cell()?)
                } else {
                    Err(Error::EmptyPath)
                }
            }
        }
    }

    pub fn destination_cell(&self) -> Result<H3Cell, Error> {
        match self {
            Self::OriginIsDestination(cell) => Ok(*cell),
            Self::DirectedEdgeSequence(edges) => {
                if let Some(edge) = edges.last() {
                    Ok(edge.destination_cell()?)
                } else {
                    Err(Error::EmptyPath)
                }
            }
        }
    }

    pub fn to_linestring(&self) -> Result<LineString<f64>, Error> {
        match self {
            Self::OriginIsDestination(_) => Err(Error::InsufficientNumberOfEdges),
            Self::DirectedEdgeSequence(edges) => match edges.len() {
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

    pub fn edges(&self) -> &[H3DirectedEdge] {
        match self {
            Self::DirectedEdgeSequence(edges) => edges.as_slice(),
            Self::OriginIsDestination(_) => &[],
        }
    }

    /// return a vec of all [`H3Cell`] the path passes through.
    pub fn cells(&self) -> Result<Vec<H3Cell>, Error> {
        match self {
            Self::OriginIsDestination(cell) => Ok(vec![*cell]),
            Self::DirectedEdgeSequence(edges) => {
                let mut cells = Vec::with_capacity(edges.len() * 2);
                for edge in edges.iter() {
                    cells.push(edge.origin_cell()?);
                    cells.push(edge.destination_cell()?);
                }
                cells.dedup();
                cells.shrink_to_fit();
                Ok(cells)
            }
        }
    }

    /// calculate the length of the path in meters using the exact length of the
    /// contained edges
    pub fn length_m(&self) -> Result<f64, Error> {
        match self {
            Self::OriginIsDestination(_) => Ok(0.0),
            Self::DirectedEdgeSequence(edges) => {
                let mut length_m = 0.0;
                for edge in edges {
                    length_m += edge.exact_length_m()?;
                }
                Ok(length_m)
            }
        }
    }
}

/// [Path] describes a path between a cell and another with an associated cost
#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Path<W> {
    /// The cell the path starts at.
    ///
    /// This is the cell the path was calculated from. The actual start cell of the
    /// path may differ in case `origin_cell` is not directly connected to the graph
    pub origin_cell: H3Cell,

    /// The cell the path ends at.
    ///
    /// This is the cell the path was calculated to. The actual end cell of the
    /// path may differ in case `destination_cell` is not directly connected to the graph
    pub destination_cell: H3Cell,

    pub cost: W,

    /// describes the path
    pub directed_edge_path: DirectedEdgePath,
}

impl<W> Path<W> {
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.directed_edge_path.is_empty()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.directed_edge_path.len()
    }
}

impl<W> TryFrom<(DirectedEdgePath, W)> for Path<W> {
    type Error = Error;

    fn try_from((path_directed_edges, cost): (DirectedEdgePath, W)) -> Result<Self, Self::Error> {
        let origin_cell = path_directed_edges.origin_cell()?;
        let destination_cell = path_directed_edges.destination_cell()?;
        Ok(Self {
            origin_cell,
            destination_cell,
            cost,
            directed_edge_path: path_directed_edges,
        })
    }
}

impl PartialOrd<Self> for DirectedEdgePath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DirectedEdgePath {
    fn cmp(&self, other: &Self) -> Ordering {
        let cmp_origin = index_or_zero(self.origin_cell()).cmp(&index_or_zero(other.origin_cell()));
        if cmp_origin == Ordering::Equal {
            index_or_zero(self.destination_cell()).cmp(&index_or_zero(other.destination_cell()))
        } else {
            cmp_origin
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
        let cmp_cost = self.cost.cmp(&other.cost);
        if cmp_cost == Ordering::Equal {
            self.directed_edge_path.cmp(&other.directed_edge_path)
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
    use h3ron::{H3DirectedEdge, Index};

    use super::{DirectedEdgePath, Path};

    #[test]
    fn pathdirectededges_deterministic_ordering() {
        let r1 =
            DirectedEdgePath::DirectedEdgeSequence(vec![H3DirectedEdge::new(0x1176b49474ffffff)]);
        let r2 =
            DirectedEdgePath::DirectedEdgeSequence(vec![H3DirectedEdge::new(0x1476b49474ffffff)]);
        let mut paths = vec![r2.clone(), r1.clone()];
        paths.sort_unstable();
        assert_eq!(paths[0], r1);
        assert_eq!(paths[1], r2);
    }

    #[test]
    fn paths_deterministic_ordering() {
        let r1: Path<_> = (
            DirectedEdgePath::DirectedEdgeSequence(vec![H3DirectedEdge::new(0x1176b49474ffffff)]),
            1,
        )
            .try_into()
            .unwrap();
        let r2: Path<_> = (
            DirectedEdgePath::DirectedEdgeSequence(vec![H3DirectedEdge::new(0x1476b49474ffffff)]),
            3,
        )
            .try_into()
            .unwrap();
        let r3: Path<_> = (
            DirectedEdgePath::DirectedEdgeSequence(vec![H3DirectedEdge::new(0x1476b4b2c2ffffff)]),
            3,
        )
            .try_into()
            .unwrap();
        let mut paths = vec![r3.clone(), r1.clone(), r2.clone()];
        paths.sort_unstable();
        assert_eq!(paths[0], r1);
        assert_eq!(paths[1], r2);
        assert_eq!(paths[2], r3);
    }
}
