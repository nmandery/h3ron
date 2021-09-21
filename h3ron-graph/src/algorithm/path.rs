use std::cmp::Ordering;

use geo_types::{Geometry, LineString, Point};
use serde::{Deserialize, Serialize};

use h3ron::{H3Cell, H3Edge, Index, ToCoordinate};

use crate::error::Error;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Path<W> {
    /// cells of the route in the order origin -> destination
    pub cells: Vec<H3Cell>,

    /// the total cost of the route (= sum of all edge weights).
    pub cost: W,
}

impl<W> Path<W> {
    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
    pub fn len(&self) -> usize {
        self.cells.len()
    }
    pub fn origin_cell(&self) -> Result<H3Cell, Error> {
        self.cells.first().cloned().ok_or(Error::EmptyPath)
    }
    pub fn destination_cell(&self) -> Result<H3Cell, Error> {
        self.cells.last().cloned().ok_or(Error::EmptyPath)
    }
    pub fn geometry(&self) -> Geometry<f64> {
        match self.cells.len() {
            0 => unreachable!(),
            1 => Point::from(self.cells[0].to_coordinate()).into(),
            _ => LineString::from(
                self.cells
                    .iter()
                    .map(|cell| cell.to_coordinate())
                    .collect::<Vec<_>>(),
            )
            .into(),
        }
    }

    pub fn to_h3_edges(&self) -> Result<Vec<H3Edge>, Error> {
        self.cells
            .windows(2)
            .map(|wdow| wdow[0].unidirectional_edge_to(&wdow[1]))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.into())
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
    use h3ron::H3Cell;
    use h3ron::Index;

    use super::Path;

    #[test]
    fn path_deterministic_ordering() {
        let r1 = Path {
            cells: vec![H3Cell::new(0), H3Cell::new(5)],
            cost: 1,
        };
        let r2 = Path {
            cells: vec![H3Cell::new(1), H3Cell::new(2)],
            cost: 3,
        };
        let r3 = Path {
            cells: vec![H3Cell::new(1), H3Cell::new(3)],
            cost: 3,
        };
        let mut paths = vec![r3.clone(), r1.clone(), r2.clone()];
        paths.sort_unstable();
        assert_eq!(paths[0], r1);
        assert_eq!(paths[1], r2);
        assert_eq!(paths[2], r3);
    }
}
