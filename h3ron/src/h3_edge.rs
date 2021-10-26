use std::convert::TryFrom;
use std::ffi::CString;
use std::fmt::{self, Debug, Formatter};
use std::iter::FromIterator;
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::os::raw::c_int;
use std::str::FromStr;

use geo::{LineString, MultiLineString};
#[cfg(feature = "use-serde")]
use serde::{Deserialize, Serialize};

use h3ron_h3_sys::H3Index;

use crate::index::{HasH3Resolution, Index};
use crate::to_geo::{ToLineString, ToMultiLineString};
use crate::util::geoboundary_to_coordinates;
use crate::{Error, ExactLength, FromH3Index, H3Cell, ToCoordinate};

/// H3 Index representing an Unidirectional H3 edge
#[derive(PartialOrd, PartialEq, Clone, Hash, Eq, Ord, Copy)]
#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct H3Edge(H3Index);

impl Debug for H3Edge {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "H3Edge({})", self.to_string())
    }
}

/// convert to index including validation
impl TryFrom<u64> for H3Edge {
    type Error = Error;

    fn try_from(h3index: H3Index) -> Result<Self, Self::Error> {
        let index = Self::new(h3index);
        index.validate()?;
        Ok(index)
    }
}

impl H3Edge {
    pub fn is_edge_valid(&self) -> bool {
        unsafe { h3ron_h3_sys::h3UnidirectionalEdgeIsValid(self.h3index()) != 0 }
    }

    /// Gets the average length of an edge in kilometers at `resolution`.
    /// This is the length of the cell boundary segment represented by the edge.
    pub fn edge_length_km(resolution: u8) -> f64 {
        unsafe { h3ron_h3_sys::edgeLengthKm(resolution as c_int) }
    }

    /// Gets the average length of an edge in meters at `resolution`.
    /// This is the length of the cell boundary segment represented by the edge.
    pub fn edge_length_m(resolution: u8) -> f64 {
        unsafe { h3ron_h3_sys::edgeLengthM(resolution as c_int) }
    }

    /// The approximate distance between the centroids of two neighboring cells
    /// at the given `resolution`.
    ///
    /// Based on the approximate edge length. See [`cell_centroid_distance_m`] for a
    /// more exact variant of this function.
    pub fn cell_centroid_distance_m_at_resolution(resolution: u8) -> f64 {
        cell_centroid_distance_m_by_edge_length(Self::edge_length_m(resolution))
    }

    /// The approximate distance between the centroids of two neighboring cells
    /// at the given `resolution`.
    ///
    /// Based on the exact edge length. See [`cell_centroid_distance_at_resolution`]
    /// for a resolution based variant.
    pub fn cell_centroid_distance_m(&self) -> f64 {
        cell_centroid_distance_m_by_edge_length(self.exact_length_m())
    }

    /// Retrieves the destination H3 Cell of `self`
    ///
    /// # Returns
    /// The built index may be invalid.
    /// Use the `destination_index` method for validity check.
    pub fn destination_index_unchecked(&self) -> H3Cell {
        let index =
            unsafe { h3ron_h3_sys::getDestinationH3IndexFromUnidirectionalEdge(self.h3index()) };
        H3Cell::new(index)
    }

    /// Retrieves the destination H3 Cell of `self`
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    /// Use the `destination_index_unchecked` to avoid error.
    pub fn destination_index(&self) -> Result<H3Cell, Error> {
        let res = self.destination_index_unchecked();
        res.validate()?;
        Ok(res)
    }

    /// Retrieves the origin H3 Cell of `self`
    ///
    /// # Returns
    /// The built index may be invalid.
    /// Use the `origin_index` method for validity check.
    pub fn origin_index_unchecked(&self) -> H3Cell {
        let index = unsafe { h3ron_h3_sys::getOriginH3IndexFromUnidirectionalEdge(self.h3index()) };
        H3Cell::new(index)
    }

    /// Retrieves the origin H3 Cell of `self`
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    /// Use the `origin_index_unchecked` to avoid error.
    pub fn origin_index(&self) -> Result<H3Cell, Error> {
        let res = self.origin_index_unchecked();
        res.validate()?;
        Ok(res)
    }

    /// Retrieves a `H3EdgeCells` of the origin and destination cell of the
    /// edge.
    pub fn cell_indexes_unchecked(&self) -> H3EdgeCells {
        let mut out: [H3Index; 2] = [0, 0];
        unsafe {
            h3ron_h3_sys::getH3IndexesFromUnidirectionalEdge(self.h3index(), out.as_mut_ptr())
        }
        H3EdgeCells {
            origin: H3Cell::new(out[0]),
            destination: H3Cell::new(out[1]),
        }
    }

    /// Retrieves a `H3EdgeCells` struct of the origin and destination cell of the
    /// edge.
    ///
    /// # Returns
    /// If the built indexes are invalid, returns an Error.
    /// Use the `cell_indexes_unchecked` to avoid error.
    pub fn cell_indexes(&self) -> Result<H3EdgeCells, Error> {
        let cells = self.cell_indexes_unchecked();
        cells.origin.validate()?;
        cells.destination.validate()?;
        Ok(cells)
    }

    /// Retrieves the corresponding edge in the reversed direction.
    pub fn reversed_unchecked(&self) -> H3Edge {
        let edge_cells = self.cell_indexes_unchecked();
        edge_cells
            .destination
            .unidirectional_edge_to_unchecked(&edge_cells.origin)
    }

    /// Retrieves the corresponding edge in the reversed direction.
    ///
    /// # Returns
    /// If the built edge is invalid, returns an Error.
    /// Use the `reversed_unchecked` to avoid error.
    pub fn reversed(&self) -> Result<H3Edge, Error> {
        let edge_cells = self.cell_indexes()?;
        edge_cells
            .destination
            .unidirectional_edge_to(&edge_cells.origin)
    }

    /// Retrieves the [`LineString`] which forms the boundary between
    /// two cells.
    pub fn boundary_linestring(&self) -> LineString<f64> {
        let gb = unsafe {
            let mut mu = MaybeUninit::<h3ron_h3_sys::GeoBoundary>::uninit();
            h3ron_h3_sys::getH3UnidirectionalEdgeBoundary(self.0, mu.as_mut_ptr());
            mu.assume_init()
        };

        LineString::from_iter(geoboundary_to_coordinates(&gb).drain(0..(gb.numVerts as usize)))
    }
}

/// Measures the length of a edge.
/// This is the length of the cell boundary segment represented by the edge.
impl ExactLength for H3Edge {
    /// Retrieves the exact length of `self` in meters
    /// This is the length of the cell boundary segment represented by the edge.
    fn exact_length_m(&self) -> f64 {
        unsafe { h3ron_h3_sys::exactEdgeLengthM(self.h3index()) }
    }

    /// Retrieves the exact length of `self` in kilometers
    /// This is the length of the cell boundary segment represented by the edge.
    fn exact_length_km(&self) -> f64 {
        unsafe { h3ron_h3_sys::exactEdgeLengthKm(self.h3index()) }
    }

    /// Retrieves the exact length of `self` in radians
    /// This is the length of the cell boundary segment represented by the edge.
    fn exact_length_rads(&self) -> f64 {
        unsafe { h3ron_h3_sys::exactEdgeLengthRads(self.h3index()) }
    }
}

impl FromH3Index for H3Edge {
    fn from_h3index(h3index: H3Index) -> Self {
        H3Edge::new(h3index)
    }
}

impl Index for H3Edge {
    fn h3index(&self) -> H3Index {
        self.0
    }

    fn new(h3index: H3Index) -> Self {
        Self(h3index)
    }

    fn validate(&self) -> Result<(), Error> {
        if !self.is_edge_valid() {
            Err(Error::InvalidH3Edge(self.h3index()))
        } else {
            Ok(())
        }
    }
}

impl HasH3Resolution for H3Edge {
    fn h3_resolution(&self) -> u8 {
        self.resolution()
    }
}

impl ToString for H3Edge {
    fn to_string(&self) -> String {
        format!("{:x}", self.0)
    }
}

impl FromStr for H3Edge {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let h3index: H3Index = CString::new(s)
            .map(|cs| unsafe { h3ron_h3_sys::stringToH3(cs.as_ptr()) })
            .map_err(|_| Error::InvalidInput)?;
        Self::try_from(h3index)
    }
}

impl ToLineString for H3Edge {
    /// Create a linestring from the origin index to the destination index
    ///
    /// ```
    /// use h3ron::{H3Edge, Index};
    /// use h3ron::to_geo::{ToLineString, ToCoordinate};
    ///
    /// let edge = H3Edge::new(0x149283080ddbffff);
    /// let ls = edge.to_linestring().unwrap();
    /// assert_eq!(ls.0.len(), 2);
    /// assert_eq!(ls.0[0], edge.origin_index_unchecked().to_coordinate());
    /// assert_eq!(ls.0[1], edge.destination_index_unchecked().to_coordinate());
    /// ```
    fn to_linestring(&self) -> Result<LineString<f64>, Error> {
        let edge_cells = self.cell_indexes()?;
        Ok(LineString::from(vec![
            edge_cells.origin.to_coordinate(),
            edge_cells.destination.to_coordinate(),
        ]))
    }

    /// Create a linestring from the origin index to the destination index.
    ///
    /// Also see [`H3Edge::to_linestring`] for an related example.
    fn to_linestring_unchecked(&self) -> LineString<f64> {
        let edge_cells = self.cell_indexes_unchecked();
        LineString::from(vec![
            edge_cells.origin.to_coordinate(),
            edge_cells.destination.to_coordinate(),
        ])
    }
}

/// converts `&[H3Edge]` slices to `MultiLineString` while attempting
/// to combine consequent `H3Edge` values into a single `LineString<f64>`
impl ToMultiLineString for &[H3Edge] {
    fn to_multilinestring(&self) -> Result<MultiLineString<f64>, Error> {
        let cell_tuples = self
            .iter()
            .map(
                |edge| match (edge.origin_index(), edge.destination_index()) {
                    (Ok(origin_cell), Ok(destination_cell)) => Ok((origin_cell, destination_cell)),
                    (Err(e), _) => Err(e),
                    (_, Err(e)) => Err(e),
                },
            )
            .collect::<Result<Vec<_>, _>>()?;
        Ok(celltuples_to_multlinestring(cell_tuples))
    }

    fn to_multilinestring_unchecked(&self) -> MultiLineString<f64> {
        celltuples_to_multlinestring(self.iter().map(|edge| {
            (
                edge.origin_index_unchecked(),
                edge.destination_index_unchecked(),
            )
        }))
    }
}

impl Deref for H3Edge {
    type Target = H3Index;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[inline]
fn cell_centroid_distance_m_by_edge_length(edge_length: f64) -> f64 {
    // the height of two triangles
    2.0 * (edge_length / 2.0) * 3.0_f64.sqrt()
}

/// convert an iterator of subsequent H3Cell-tuples `(origin_cell, destination_cell)` generated
/// from `H3Edge` values to a multilinestring
fn celltuples_to_multlinestring<I>(iter: I) -> MultiLineString<f64>
where
    I: IntoIterator<Item = (H3Cell, H3Cell)>,
{
    let mut linestrings = vec![];
    let mut last_destination_cell: Option<H3Cell> = None;
    let mut coordinates = vec![];
    for (origin_cell, destination_cell) in iter {
        if coordinates.is_empty() {
            coordinates.push(origin_cell.to_coordinate());
            coordinates.push(destination_cell.to_coordinate());
        } else {
            if last_destination_cell != Some(origin_cell) {
                // create a new linestring
                linestrings.push(LineString::from(std::mem::take(&mut coordinates)));
                coordinates.push(origin_cell.to_coordinate());
            }
            coordinates.push(destination_cell.to_coordinate())
        }
        last_destination_cell = Some(destination_cell)
    }
    if !coordinates.is_empty() {
        linestrings.push(LineString::from(coordinates));
    }
    MultiLineString(linestrings)
}

impl ToMultiLineString for Vec<H3Edge> {
    fn to_multilinestring(&self) -> Result<MultiLineString<f64>, Error> {
        self.as_slice().to_multilinestring()
    }

    fn to_multilinestring_unchecked(&self) -> MultiLineString<f64> {
        self.as_slice().to_multilinestring_unchecked()
    }
}

pub struct H3EdgeCells {
    /// origin cell of the edge
    pub origin: H3Cell,

    /// destination cell of the edge
    pub destination: H3Cell,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[should_panic(expected = "InvalidH3Edge")]
    #[test]
    fn checks_both_validity() {
        let edge = H3Edge::new(0x149283080ddbffff);
        assert!(edge.validate().is_ok());
        let edge = H3Edge::new(0x89283080ddbffff_u64);
        edge.validate().unwrap();
    }

    #[test]
    fn can_find_parent() {
        let edge = H3Edge::new(0x149283080ddbffff);
        assert_eq!(edge.resolution(), 9);
        let parent_8 = edge.get_parent(8).unwrap();
        assert_eq!(parent_8.resolution(), 8);
        assert!(edge.is_child_of(&parent_8));
        let parent_1 = edge.get_parent(1).unwrap();
        assert_eq!(parent_1.resolution(), 1);
        assert!(edge.is_child_of(&parent_1));
    }

    #[test]
    fn debug_hexadecimal() {
        let edge = H3Edge::new(0x149283080ddbffff);
        assert_eq!(
            format!("{:?}", edge),
            "H3Edge(149283080ddbffff)".to_string()
        )
    }

    #[test]
    fn reversed() {
        let edge = H3Edge::new(0x149283080ddbffff);
        let rev_edge = edge.reversed_unchecked();
        assert_ne!(edge, rev_edge);
        assert_eq!(
            edge.origin_index_unchecked(),
            rev_edge.destination_index_unchecked()
        );
        assert_eq!(
            edge.destination_index_unchecked(),
            rev_edge.origin_index_unchecked()
        );
    }

    #[test]
    fn boundary_linestring() {
        let edge = H3Edge::new(0x149283080ddbffff);
        dbg!(edge.boundary_linestring());
        dbg!(edge.to_linestring().unwrap());
    }

    #[test]
    fn test_cell_centroid_distance_m() {
        let edge = H3Edge::new(0x149283080ddbffff);
        assert!(edge.exact_length_m() < edge.cell_centroid_distance_m());
        assert!((2.0 * edge.exact_length_m()) > edge.cell_centroid_distance_m());
    }
}
