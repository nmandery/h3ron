use std::fmt::{self, Debug, Formatter};
use std::mem::MaybeUninit;
use std::ops::Deref;
use std::os::raw::c_int;
use std::str::FromStr;

use geo::{LineString, MultiLineString};
#[cfg(feature = "use-serde")]
use serde::{Deserialize, Serialize};

use h3ron_h3_sys::H3Index;

use crate::index::{index_from_str, Index};
use crate::iter::CellBoundaryIter;
use crate::to_geo::{ToLineString, ToMultiLineString};
use crate::{Error, FromH3Index, H3Cell, ToCoordinate};

/// H3 Index representing an directed H3 edge
#[derive(PartialOrd, PartialEq, Clone, Hash, Eq, Ord, Copy)]
#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct H3DirectedEdge(H3Index);

impl Debug for H3DirectedEdge {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "H3DirectedEdge({})", self.to_string())
    }
}

/// convert to index including validation
impl TryFrom<u64> for H3DirectedEdge {
    type Error = Error;

    fn try_from(h3index: H3Index) -> Result<Self, Self::Error> {
        let index = Self::new(h3index);
        index.validate()?;
        Ok(index)
    }
}

impl H3DirectedEdge {
    /// Gets the unidirectional edge from `origin_cell` to `destination_cell`
    pub fn from_cells(origin_cell: H3Cell, destination_cell: H3Cell) -> Result<Self, Error> {
        origin_cell.directed_edge_to(destination_cell)
    }

    pub fn is_edge_valid(&self) -> bool {
        self.validate().is_ok()
    }

    /// Gets the average length of an edge in kilometers at `resolution`.
    /// This is the length of the cell boundary segment represented by the edge.
    pub fn edge_length_avg_km(resolution: u8) -> Result<f64, Error> {
        let mut edge_length: f64 = 0.0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::getHexagonEdgeLengthAvgKm(c_int::from(resolution), &mut edge_length)
        })
        .map(|_| edge_length)
    }

    /// Gets the average length of an edge in meters at `resolution`.
    /// This is the length of the cell boundary segment represented by the edge.
    pub fn edge_length_avg_m(resolution: u8) -> Result<f64, Error> {
        let mut edge_length: f64 = 0.0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::getHexagonEdgeLengthAvgM(c_int::from(resolution), &mut edge_length)
        })
        .map(|_| edge_length)
    }

    /// The approximate distance between the centroids of two neighboring cells
    /// at the given `resolution`.
    ///
    /// Based on the approximate edge length. See [`H3DirectedEdge::cell_centroid_distance_m`] for a
    /// more exact variant of this function.
    pub fn cell_centroid_distance_avg_m_at_resolution(resolution: u8) -> Result<f64, Error> {
        Self::edge_length_avg_m(resolution).map(cell_centroid_distance_m_by_edge_length)
    }

    /// The approximate distance between the centroids of two neighboring cells
    /// at the given `resolution`.
    ///
    /// Based on the exact edge length. See [`H3DirectedEdge::cell_centroid_distance_avg_m_at_resolution`]
    /// for a resolution based variant.
    pub fn cell_centroid_distance_m(&self) -> Result<f64, Error> {
        self.exact_length_m()
            .map(cell_centroid_distance_m_by_edge_length)
    }

    /// Retrieves the destination H3 Cell of `self`
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    pub fn destination_cell(&self) -> Result<H3Cell, Error> {
        let mut cell_h3index: H3Index = 0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::getDirectedEdgeDestination(self.h3index(), &mut cell_h3index)
        })
        .map(|_| H3Cell::new(cell_h3index))
    }

    /// Retrieves the origin H3 Cell of `self`
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    pub fn origin_cell(&self) -> Result<H3Cell, Error> {
        let mut cell_h3index: H3Index = 0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::getDirectedEdgeOrigin(self.h3index(), &mut cell_h3index)
        })
        .map(|_| H3Cell::new(cell_h3index))
    }

    /// Retrieves a `H3EdgeCells` of the origin and destination cell of the
    /// edge.
    ///
    /// # Returns
    /// If the built indexes are invalid, returns an Error.
    pub fn cells(&self) -> Result<H3EdgeCells, Error> {
        let mut out: [H3Index; 2] = [0, 0];
        unsafe {
            h3ron_h3_sys::directedEdgeToCells(self.h3index(), out.as_mut_ptr());
        }
        let res = H3EdgeCells {
            origin: H3Cell::new(out[0]),
            destination: H3Cell::new(out[1]),
        };
        res.origin.validate()?;
        res.destination.validate()?;
        Ok(res)
    }

    /// Retrieves the corresponding edge in the reversed direction.
    ///
    /// # Returns
    /// If the built edge is invalid, returns an Error.
    pub fn reversed(&self) -> Result<Self, Error> {
        let edge_cells = self.cells()?;
        edge_cells.destination.directed_edge_to(edge_cells.origin)
    }

    /// Retrieves the [`LineString`] which forms the boundary between
    /// two cells.
    pub fn boundary_linestring(&self) -> Result<LineString<f64>, Error> {
        let cb = unsafe {
            let mut mu = MaybeUninit::<h3ron_h3_sys::CellBoundary>::uninit();
            Error::check_returncode(h3ron_h3_sys::directedEdgeToBoundary(
                self.0,
                mu.as_mut_ptr(),
            ))?;
            mu.assume_init()
        };
        Ok(CellBoundaryIter::new(&cb, false).collect())
    }

    /// Retrieves the exact length of `self` in meters
    /// This is the length of the cell boundary segment represented by the edge.
    pub fn exact_length_m(&self) -> Result<f64, Error> {
        let mut length: f64 = 0.0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::exactEdgeLengthM(self.h3index(), &mut length)
        })
        .map(|_| length)
    }

    /// Retrieves the exact length of `self` in kilometers
    /// This is the length of the cell boundary segment represented by the edge.
    pub fn exact_length_km(&self) -> Result<f64, Error> {
        let mut length: f64 = 0.0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::exactEdgeLengthKm(self.h3index(), &mut length)
        })
        .map(|_| length)
    }

    /// Retrieves the exact length of `self` in radians
    /// This is the length of the cell boundary segment represented by the edge.
    pub fn exact_length_rads(&self) -> Result<f64, Error> {
        let mut length: f64 = 0.0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::exactEdgeLengthRads(self.h3index(), &mut length)
        })
        .map(|_| length)
    }
}

impl FromH3Index for H3DirectedEdge {
    fn from_h3index(h3index: H3Index) -> Self {
        Self::new(h3index)
    }
}

impl Index for H3DirectedEdge {
    fn h3index(&self) -> H3Index {
        self.0
    }

    fn new(h3index: H3Index) -> Self {
        Self(h3index)
    }

    fn validate(&self) -> Result<(), Error> {
        if unsafe { h3ron_h3_sys::isValidDirectedEdge(self.h3index()) == 0 } {
            Err(Error::DirectedEdgeInvalid)
        } else {
            Ok(())
        }
    }
}

impl ToString for H3DirectedEdge {
    fn to_string(&self) -> String {
        format!("{:x}", self.0)
    }
}

impl FromStr for H3DirectedEdge {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        index_from_str(s)
    }
}

impl ToLineString for H3DirectedEdge {
    type Error = Error;

    /// Create a linestring from the origin index to the destination index
    ///
    /// ```
    /// use h3ron::{H3DirectedEdge, Index};
    /// use h3ron::to_geo::{ToLineString, ToCoordinate};
    ///
    /// let edge = H3DirectedEdge::new(0x149283080ddbffff);
    /// let ls = edge.to_linestring().unwrap();
    /// assert_eq!(ls.0.len(), 2);
    /// assert_eq!(ls.0[0], edge.origin_cell().unwrap().to_coordinate().unwrap());
    /// assert_eq!(ls.0[1], edge.destination_cell().unwrap().to_coordinate().unwrap());
    /// ```
    fn to_linestring(&self) -> Result<LineString<f64>, Self::Error> {
        let edge_cells = self.cells()?;
        Ok(LineString::from(vec![
            edge_cells.origin.to_coordinate()?,
            edge_cells.destination.to_coordinate()?,
        ]))
    }
}

/// converts `&[H3Edge]` slices to `MultiLineString` while attempting
/// to combine consequent `H3Edge` values into a single `LineString<f64>`
impl ToMultiLineString for &[H3DirectedEdge] {
    type Error = Error;

    fn to_multilinestring(&self) -> Result<MultiLineString<f64>, Self::Error> {
        let cell_tuples = self
            .iter()
            .map(|edge| match (edge.origin_cell(), edge.destination_cell()) {
                (Ok(origin_cell), Ok(destination_cell)) => Ok((origin_cell, destination_cell)),
                (Err(e), _) | (_, Err(e)) => Err(e),
            })
            .collect::<Result<Vec<_>, _>>()?;
        celltuples_to_multlinestring(cell_tuples)
    }
}

impl Deref for H3DirectedEdge {
    type Target = H3Index;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// avoid repeated calculations by using this constant of the
/// result of `3.0_f64.sqrt()`.
const F64_SQRT_3: f64 = 1.7320508075688772_f64;

/// the height of two equilateral triangles with a shared side calculated using
/// the `edge_length`.
///     .
///    /_\
///    \ /
///     `
///
/// For one triangle:  `h = (edge_length / 2.0) * 3.0.sqrt()`
#[inline(always)]
fn cell_centroid_distance_m_by_edge_length(edge_length: f64) -> f64 {
    edge_length * F64_SQRT_3
}

/// convert an iterator of subsequent H3Cell-tuples `(origin_cell, destination_cell)` generated
/// from `H3DirectedEdge` values to a multilinestring
fn celltuples_to_multlinestring<I>(iter: I) -> Result<MultiLineString<f64>, Error>
where
    I: IntoIterator<Item = (H3Cell, H3Cell)>,
{
    let mut linestrings = vec![];
    let mut last_destination_cell: Option<H3Cell> = None;
    let mut coordinates = vec![];
    for (origin_cell, destination_cell) in iter {
        if coordinates.is_empty() {
            coordinates.push(origin_cell.to_coordinate()?);
        } else if last_destination_cell != Some(origin_cell) {
            // create a new linestring
            linestrings.push(LineString::from(std::mem::take(&mut coordinates)));
            coordinates.push(origin_cell.to_coordinate()?);
        }
        coordinates.push(destination_cell.to_coordinate()?);
        last_destination_cell = Some(destination_cell);
    }
    if !coordinates.is_empty() {
        linestrings.push(LineString::from(coordinates));
    }
    Ok(MultiLineString(linestrings))
}

impl ToMultiLineString for Vec<H3DirectedEdge> {
    type Error = Error;

    fn to_multilinestring(&self) -> Result<MultiLineString<f64>, Self::Error> {
        self.as_slice().to_multilinestring()
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

    #[should_panic(expected = "DirectedEdgeInvalid")]
    #[test]
    fn checks_both_validity() {
        let edge = H3DirectedEdge::new(0x149283080ddbffff);
        assert!(edge.validate().is_ok());
        let edge = H3DirectedEdge::new(0x89283080ddbffff_u64);
        edge.validate().unwrap();
    }

    #[test]
    fn debug_hexadecimal() {
        let edge = H3DirectedEdge::new(0x149283080ddbffff);
        assert_eq!(
            format!("{:?}", edge),
            "H3DirectedEdge(149283080ddbffff)".to_string()
        );
    }

    #[test]
    fn reversed() {
        let edge = H3DirectedEdge::new(0x149283080ddbffff);
        let rev_edge = edge.reversed().unwrap();
        assert_ne!(edge, rev_edge);
        assert_eq!(
            edge.origin_cell().unwrap(),
            rev_edge.destination_cell().unwrap()
        );
        assert_eq!(
            edge.destination_cell().unwrap(),
            rev_edge.origin_cell().unwrap()
        );
    }

    #[test]
    fn boundary_linestring() {
        let edge = H3DirectedEdge::new(0x149283080ddbffff);
        let boundary_ls = edge.boundary_linestring().unwrap();
        assert_eq!(boundary_ls.0.len(), 2);
        dbg!(&boundary_ls);

        let ls = edge.to_linestring().unwrap();
        assert_eq!(ls.0.len(), 2);
        dbg!(&ls);
        assert_ne!(ls, boundary_ls);
    }

    #[test]
    fn test_cell_centroid_distance_m() {
        let edge = H3DirectedEdge::new(0x149283080ddbffff);
        assert!(edge.exact_length_m().unwrap() < edge.cell_centroid_distance_m().unwrap());
        assert!((2.0 * edge.exact_length_m().unwrap()) > edge.cell_centroid_distance_m().unwrap());
    }
}
