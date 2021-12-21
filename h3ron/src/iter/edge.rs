use crate::collections::indexvec::{IndexVec, UncheckedIter};
use crate::{Error, H3Cell, H3Edge, Index};
use std::borrow::Borrow;

/// Creates H3Edges from cells while only requiring a single memory allocation
/// when the struct is created.
pub struct H3EdgesBuilder {
    index_vec: IndexVec<H3Edge>,
}

impl Default for H3EdgesBuilder {
    fn default() -> Self {
        Self {
            index_vec: IndexVec::with_length(6),
        }
    }
}

impl H3EdgesBuilder {
    pub fn new() -> Self {
        Default::default()
    }

    /// Create an iterator for iterating over all [`H3Edge`] leading away from the given [`H3Cell`].
    pub fn from_origin_cell(
        &mut self,
        cell: &H3Cell,
    ) -> crate::collections::indexvec::UncheckedIter<'_, H3Edge> {
        unsafe {
            h3ron_h3_sys::getH3UnidirectionalEdgesFromHexagon(
                cell.h3index(),
                self.index_vec.as_mut_ptr(),
            )
        };
        self.index_vec.iter()
    }

    /// get an iterator over all edges leading to the origin of the input `edge` except the reverse of input
    pub fn previous_edges_leading_to_origin(&mut self, edge: &H3Edge) -> EdgeIter<'_> {
        EdgeIter {
            index_iter: self.from_origin_cell(&edge.origin_index_unchecked()),
            do_reverse: true,
            exclude_edge: *edge,
        }
    }

    /// get all following edges leading away from the destination of the input `edge`, except
    /// the reverse of the input.
    pub fn following_edges_leading_from_destination(&mut self, edge: &H3Edge) -> EdgeIter<'_> {
        EdgeIter {
            index_iter: self.from_origin_cell(&edge.destination_index_unchecked()),
            do_reverse: false,
            exclude_edge: edge.reversed_unchecked(),
        }
    }
}

pub struct EdgeIter<'a> {
    index_iter: UncheckedIter<'a, H3Edge>,
    do_reverse: bool,

    /// edge to exclude. the exclusion is done before the reversing
    exclude_edge: H3Edge,
}

impl<'a> Iterator for EdgeIter<'a> {
    type Item = H3Edge;

    fn next(&mut self) -> Option<Self::Item> {
        for edge in &mut self.index_iter {
            if edge == self.exclude_edge {
                continue;
            }
            return if self.do_reverse {
                Some(edge.reversed_unchecked())
            } else {
                Some(edge)
            };
        }
        None
    }
}

/// convert an iterator of continuous (= neighboring) cells to edges connecting
/// consecutive cells from the iterator.
pub fn continuous_cells_to_edges<I>(cells: I) -> CellsToEdgesIter<<I as IntoIterator>::IntoIter>
where
    I: IntoIterator,
    I::Item: Borrow<H3Cell>,
{
    let iter = cells.into_iter();
    CellsToEdgesIter {
        last_cell: None,
        iter,
    }
}

pub struct CellsToEdgesIter<I> {
    last_cell: Option<H3Cell>,
    iter: I,
}

impl<I> Iterator for CellsToEdgesIter<I>
where
    I: Iterator,
    I::Item: Borrow<H3Cell>,
{
    type Item = Result<H3Edge, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        for cell_item in self.iter.by_ref() {
            let cell = *cell_item.borrow();
            if self.last_cell.is_none() {
                self.last_cell = Some(cell);
                continue;
            }
            let edge = self.last_cell.unwrap().unidirectional_edge_to(&cell);
            self.last_cell = Some(cell);
            return Some(edge);
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (remaining, upper_bound) = self.iter.size_hint();
        (
            remaining.saturating_sub(1),
            upper_bound.map(|ub| ub.saturating_sub(1)),
        )
    }
}

#[cfg(test)]
mod tests {
    use geo::Coordinate;
    use geo_types::{Geometry, Line};

    use crate::iter::{continuous_cells_to_edges, H3EdgesBuilder};
    use crate::{H3Cell, ToH3Cells};

    #[test]
    fn from_origin_cell() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((34.2, 30.5)), 7).unwrap();
        let mut edge_builder = H3EdgesBuilder::new();
        assert_eq!(edge_builder.from_origin_cell(&cell).count(), 6);
    }

    #[test]
    fn following_edges_leading_from_destination() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((34.2, 30.5)), 7).unwrap();
        let mut edge_builder = H3EdgesBuilder::new();
        let edge = edge_builder.from_origin_cell(&cell).next().unwrap();

        let other_edges: Vec<_> = edge_builder
            .following_edges_leading_from_destination(&edge)
            .collect();
        assert_eq!(other_edges.len(), 5);
        assert!(!other_edges.contains(&edge));
        for other_edge in other_edges {
            assert_eq!(
                edge.destination_index_unchecked(),
                other_edge.origin_index_unchecked()
            );
        }
    }

    #[test]
    fn previous_edges_leading_to_origin() {
        let cell = H3Cell::from_coordinate(&Coordinate::from((34.2, 30.5)), 7).unwrap();
        let mut edge_builder = H3EdgesBuilder::new();
        let edge = edge_builder.from_origin_cell(&cell).next().unwrap();

        let other_edges: Vec<_> = edge_builder
            .previous_edges_leading_to_origin(&edge)
            .collect();
        assert_eq!(other_edges.len(), 5);
        assert!(!other_edges.contains(&edge));
        for other_edge in other_edges {
            assert_eq!(cell, other_edge.destination_index_unchecked());
        }
    }

    #[test]
    fn test_continuous_cells_to_edges() {
        let h3_resolution = 4;
        let cell_sequence: Vec<_> = Geometry::Line(Line {
            start: (10.0f64, 20.0f64).into(),
            end: (20., 20.).into(),
        })
        .to_h3_cells(h3_resolution)
        .unwrap()
        .iter()
        .collect();
        assert!(cell_sequence.len() > 20);

        let edges = continuous_cells_to_edges(&cell_sequence)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(cell_sequence.len(), edges.len() + 1);
        assert_eq!(cell_sequence[0], edges[0].origin_index_unchecked());
        assert_eq!(
            *cell_sequence.last().unwrap(),
            edges.last().unwrap().destination_index_unchecked()
        );
    }

    #[should_panic(expected = "InvalidH3Edge")]
    #[test]
    fn test_continuous_cells_to_edges_non_continuous() {
        let h3_resolution = 4;
        let cells = vec![
            H3Cell::from_coordinate(&(10.0, 20.0).into(), h3_resolution).unwrap(),
            H3Cell::from_coordinate(&(20.0, 20.0).into(), h3_resolution).unwrap(),
        ];
        let _edges = continuous_cells_to_edges(&cells)
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
    }
}
