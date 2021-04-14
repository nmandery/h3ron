use crate::index::Index;
use crate::{Error, FromH3Index, HexagonIndex};
use h3ron_h3_sys::H3Index;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::ffi::CString;
use std::os::raw::c_int;
use std::str::FromStr;

/// H3 Index representing an Unidirectional H3 edge
#[derive(PartialOrd, PartialEq, Clone, Debug, Serialize, Deserialize, Hash, Eq, Ord, Copy)]
pub struct EdgeIndex(H3Index);

/// convert to index including validation
impl TryFrom<u64> for EdgeIndex {
    type Error = Error;

    fn try_from(h3index: H3Index) -> Result<Self, Self::Error> {
        let index = Self::new(h3index);
        index.validate()?;
        Ok(index)
    }
}

impl EdgeIndex {
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

    /// Retrieves the exact length of `self` in kilometers
    pub fn exact_length_km(&self) -> f64 {
        unsafe { h3ron_h3_sys::exactEdgeLengthKm(self.h3index()) }
    }

    /// Retrieves the exact length of `self` in meters
    pub fn exact_length_m(&self) -> f64 {
        unsafe { h3ron_h3_sys::exactEdgeLengthM(self.h3index()) }
    }

    /// Retrieves the exact length of `self` in radians
    pub fn exact_length_rads(&self) -> f64 {
        unsafe { h3ron_h3_sys::exactEdgeLengthRads(self.h3index()) }
    }

    /// Retrieves the destination hexagon H3 Index of `self`
    ///
    /// # Returns
    /// The built index may be invalid.
    /// Use the `destination_index` method for validity check.
    pub fn destination_index_unchecked(&self) -> HexagonIndex {
        let index =
            unsafe { h3ron_h3_sys::getDestinationH3IndexFromUnidirectionalEdge(self.h3index()) };
        HexagonIndex::new(index)
    }

    /// Retrieves the destination hexagon H3 Index of `self`
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    /// Use the `destination_index_unchecked` to avoid error.
    pub fn destination_index(&self) -> Result<HexagonIndex, Error> {
        let res = self.destination_index_unchecked();
        res.validate()?;
        Ok(res)
    }

    /// Retrieves the origin hexagon H3 Index of `self`
    ///
    /// # Returns
    /// The built index may be invalid.
    /// Use the `origin_index` method for validity check.
    pub fn origin_index_unchecked(&self) -> HexagonIndex {
        let index = unsafe { h3ron_h3_sys::getOriginH3IndexFromUnidirectionalEdge(self.h3index()) };
        HexagonIndex::new(index)
    }

    /// Retrieves the origin hexagon H3 Index of `self`
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    /// Use the `origin_index_unchecked` to avoid error.
    pub fn origin_index(&self) -> Result<HexagonIndex, Error> {
        let res = self.origin_index_unchecked();
        res.validate()?;
        Ok(res)
    }
}

impl FromH3Index for EdgeIndex {
    fn from_h3index(h3index: H3Index) -> Self {
        EdgeIndex::new(h3index)
    }
}

impl Index for EdgeIndex {
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

impl ToString for EdgeIndex {
    fn to_string(&self) -> String {
        format!("{:x}", self.0)
    }
}

impl FromStr for EdgeIndex {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let h3index: H3Index = CString::new(s)
            .map(|cs| unsafe { h3ron_h3_sys::stringToH3(cs.as_ptr()) })
            .map_err(|_| Error::InvalidInput)?;
        Self::try_from(h3index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[should_panic(expected = "InvalidH3Edge")]
    #[test]
    fn checks_both_validity() {
        let edge = EdgeIndex::new(0x149283080ddbffff);
        assert!(edge.validate().is_ok());
        let edge = EdgeIndex::new(0x89283080ddbffff_u64);
        edge.validate().unwrap();
    }

    #[test]
    fn can_find_parent() {
        let edge = EdgeIndex::new(0x149283080ddbffff);
        assert_eq!(edge.resolution(), 9);
        let parent_8 = edge.get_parent(8).unwrap();
        assert_eq!(parent_8.resolution(), 8);
        assert!(edge.is_child_of(&parent_8));
        let parent_1 = edge.get_parent(1).unwrap();
        assert_eq!(parent_1.resolution(), 1);
        assert!(edge.is_child_of(&parent_1));
    }
}
