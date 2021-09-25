use std::convert::TryFrom;
use std::ffi::CString;
use std::fmt::{self, Debug, Formatter};
use std::os::raw::c_int;
use std::str::FromStr;

use geo::LineString;
use serde::{Deserialize, Serialize};

use h3ron_h3_sys::H3Index;

use crate::index::{HasH3Resolution, Index};
use crate::to_geo::ToLineString;
use crate::{Error, ExactLength, FromH3Index, H3Cell, ToCoordinate};

/// H3 Index representing an Unidirectional H3 edge
#[derive(PartialOrd, PartialEq, Clone, Serialize, Deserialize, Hash, Eq, Ord, Copy)]
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

    /// Gets the average length of an edge in kilometers at `resolution`
    pub fn edge_length_km(resolution: u8) -> f64 {
        unsafe { h3ron_h3_sys::edgeLengthKm(resolution as c_int) }
    }

    /// Gets the average length of an edge in meters at `resolution`
    pub fn edge_length_m(resolution: u8) -> f64 {
        unsafe { h3ron_h3_sys::edgeLengthM(resolution as c_int) }
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
}

impl ExactLength for H3Edge {
    /// Retrieves the exact length of `self` in meters
    fn exact_length_m(&self) -> f64 {
        unsafe { h3ron_h3_sys::exactEdgeLengthM(self.h3index()) }
    }

    /// Retrieves the exact length of `self` in kilometers
    fn exact_length_km(&self) -> f64 {
        unsafe { h3ron_h3_sys::exactEdgeLengthKm(self.h3index()) }
    }

    /// Retrieves the exact length of `self` in radians
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
    /// create a linestring from the origin index to the destination index
    fn to_linestring(&self) -> Result<LineString<f64>, Error> {
        Ok(LineString::from(vec![
            self.origin_index()?.to_coordinate(),
            self.destination_index()?.to_coordinate(),
        ]))
    }

    /// create a linestring from the origin index to the destination index
    fn to_linestring_unchecked(&self) -> LineString<f64> {
        LineString::from(vec![
            self.origin_index_unchecked().to_coordinate(),
            self.destination_index_unchecked().to_coordinate(),
        ])
    }
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
    fn to_linestring() {
        let edge = H3Edge::new(0x149283080ddbffff);
        let ls = edge.to_linestring().unwrap();
        assert_eq!(ls.0.len(), 2);
        assert_eq!(ls.0[0], edge.origin_index_unchecked().to_coordinate());
        assert_eq!(ls.0[1], edge.destination_index_unchecked().to_coordinate());
    }
}
