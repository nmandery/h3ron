use std::convert::TryFrom;
use std::ffi::CString;
use std::mem::MaybeUninit;
use std::os::raw::c_int;
use std::str::FromStr;

use geo_types::{Coordinate, LineString, Point, Polygon};

#[cfg(feature = "use-serde")]
use serde::{Deserialize, Serialize};

use h3ron_h3_sys::{GeoCoord, H3Index};

use crate::collections::indexvec::IndexVec;
use crate::error::Error;
use crate::index::{HasH3Resolution, Index};
use crate::util::{coordinate_to_geocoord, geoboundary_to_coordinates, point_to_geocoord};
use crate::{max_k_ring_size, ExactArea, FromH3Index, H3Edge, ToCoordinate, ToPolygon};
use std::fmt::{self, Debug, Formatter};
use std::ops::Deref;

/// H3 Index representing a H3 Cell (hexagon)
#[derive(PartialOrd, PartialEq, Clone, Hash, Eq, Ord, Copy)]
#[cfg_attr(feature = "use-serde", derive(Serialize, Deserialize))]
#[repr(transparent)]
pub struct H3Cell(H3Index);

impl Debug for H3Cell {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "H3Cell({})", self.to_string())
    }
}

/// convert to index including validation
impl TryFrom<u64> for H3Cell {
    type Error = Error;

    fn try_from(h3index: H3Index) -> Result<Self, Self::Error> {
        let index = Self::new(h3index);
        index.validate()?;
        Ok(index)
    }
}

impl FromH3Index for H3Cell {
    fn from_h3index(h3index: H3Index) -> Self {
        Self::new(h3index)
    }
}

impl Index for H3Cell {
    fn h3index(&self) -> H3Index {
        self.0
    }

    fn new(h3index: H3Index) -> Self {
        Self(h3index)
    }

    fn validate(&self) -> Result<(), Error> {
        if !unsafe { h3ron_h3_sys::h3IsValid(self.h3index()) != 0 } {
            Err(Error::InvalidH3Cell(self.h3index()))
        } else {
            Ok(())
        }
    }
}

impl HasH3Resolution for H3Cell {
    fn h3_resolution(&self) -> u8 {
        self.resolution()
    }
}

impl H3Cell {
    /// Build a new `Index` from a `Point`.
    ///
    /// # Returns
    /// The built index may be invalid.
    /// Use the `from_point` method for validity check.
    pub fn from_point_unchecked(pt: &Point<f64>, h3_resolution: u8) -> Self {
        let h3index = unsafe {
            let gc = point_to_geocoord(pt);
            h3ron_h3_sys::geoToH3(&gc, h3_resolution as c_int)
        };
        Self::new(h3index)
    }

    /// Build a new `Index` from a `Point`.
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    /// Use the `from_point_unchecked` to avoid error.
    pub fn from_point(pt: &Point<f64>, h3_resolution: u8) -> Result<Self, Error> {
        let res = Self::from_point_unchecked(pt, h3_resolution);
        res.validate()?;
        Ok(res)
    }

    /// Build a new `Index` from coordinates.
    ///
    /// # Returns
    /// The built index may be invalid.
    /// Use the `from_coordinate` method for validity check.
    pub fn from_coordinate_unchecked(c: &Coordinate<f64>, h3_resolution: u8) -> Self {
        let h3index = unsafe {
            let gc = coordinate_to_geocoord(c);
            h3ron_h3_sys::geoToH3(&gc, h3_resolution as c_int)
        };
        Self::new(h3index)
    }

    /// Build a new `Index` from coordinates.
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    /// Use the `from_coordinate_unchecked` to avoid error.
    pub fn from_coordinate(c: &Coordinate<f64>, h3_resolution: u8) -> Result<Self, Error> {
        let res = Self::from_coordinate_unchecked(c, h3_resolution);
        res.validate()?;
        Ok(res)
    }

    /// Checks if the current index and `other` are neighbors.
    pub fn is_neighbor_to(&self, other: &Self) -> bool {
        let res: i32 = unsafe { h3ron_h3_sys::h3IndexesAreNeighbors(self.0, other.0) };
        res == 1
    }

    /// `k_ring` produces all cells within k distance of the origin cell.
    ///
    /// k-ring 0 is defined as the origin cell, k-ring 1 is defined as k-ring 0 and all
    /// neighboring cells, and so on.
    ///
    /// # Note
    ///
    /// For repeated building of k-rings, there is also [`super::iter::KRingBuilder`].
    pub fn k_ring(&self, k: u32) -> IndexVec<H3Cell> {
        let mut index_vec =
            IndexVec::with_length(unsafe { h3ron_h3_sys::maxKringSize(k as i32) as usize });
        unsafe {
            h3ron_h3_sys::kRing(self.0, k as c_int, index_vec.as_mut_ptr());
        }
        index_vec
    }

    pub fn hex_ring(&self, k: u32) -> Result<IndexVec<H3Cell>, Error> {
        // calculation of max_size taken from
        // https://github.com/uber/h3-py/blob/dd08189b378429291c342d0af3d3cc1e38a659d5/src/h3/_cy/cells.pyx#L111
        let mut index_vec = IndexVec::with_length(if k > 0 { 6 * k as usize } else { 1 });

        let res =
            unsafe { h3ron_h3_sys::hexRing(self.0, k as c_int, index_vec.as_mut_ptr()) as c_int };
        if res == 0 {
            Ok(index_vec)
        } else {
            Err(Error::PentagonalDistortion)
        }
    }

    /// Retrieves indexes around `self` through K Rings.
    ///
    /// # Arguments
    ///
    /// * `k_min` - the minimum k ring distance
    /// * `k_max` - the maximum k ring distance
    ///
    /// # Returns
    ///
    /// A `Vec` of `(u32, Index)` tuple is returned. The `u32` value is the K Ring distance
    /// of the `Index` value.
    ///
    /// # Note
    ///
    /// For repeated building of k-rings, there is also [`super::iter::KRingBuilder`].
    ///
    pub fn k_ring_distances(&self, k_min: u32, k_max: u32) -> Vec<(u32, H3Cell)> {
        let max_size = max_k_ring_size(k_max);
        let mut h3_indexes_out: Vec<H3Index> = vec![0; max_size];
        let mut distances_out: Vec<c_int> = vec![0; max_size];
        unsafe {
            h3ron_h3_sys::kRingDistances(
                self.0,
                k_max as c_int,
                h3_indexes_out.as_mut_ptr(),
                distances_out.as_mut_ptr(),
            )
        };
        self.associate_index_distances(h3_indexes_out, distances_out, k_min)
    }

    pub fn hex_range_distances(&self, k_min: u32, k_max: u32) -> Result<Vec<(u32, H3Cell)>, Error> {
        let max_size = max_k_ring_size(k_max);
        let mut h3_indexes_out: Vec<H3Index> = vec![0; max_size];
        let mut distances_out: Vec<c_int> = vec![0; max_size];
        let res = unsafe {
            h3ron_h3_sys::hexRangeDistances(
                self.0,
                k_max as c_int,
                h3_indexes_out.as_mut_ptr(),
                distances_out.as_mut_ptr(),
            ) as c_int
        };
        if res == 0 {
            Ok(self.associate_index_distances(h3_indexes_out, distances_out, k_min))
        } else {
            Err(Error::PentagonalDistortion) // may also be PentagonEncountered
        }
    }

    /// Retrieves the number of K Rings between `self` and `other`.
    ///
    /// For distance in miles or kilometers use haversine algorithms.
    pub fn distance_to(&self, other: &Self) -> i32 {
        unsafe { h3ron_h3_sys::h3Distance(self.0, other.0) }
    }

    fn associate_index_distances(
        &self,
        mut h3_indexes_out: Vec<H3Index>,
        distances_out: Vec<c_int>,
        k_min: u32,
    ) -> Vec<(u32, H3Cell)> {
        h3_indexes_out
            .drain(..)
            .enumerate()
            .filter(|(idx, h3index)| *h3index != 0 && distances_out[*idx] >= k_min as i32)
            .map(|(idx, h3index)| (distances_out[idx] as u32, H3Cell::new(h3index)))
            .collect()
    }

    /// determines if an H3 cell is a pentagon
    pub fn is_pentagon(&self) -> bool {
        unsafe { h3ron_h3_sys::h3IsPentagon(self.0) == 1 }
    }

    /// returns the base cell "number" (0 to 121) of the provided H3 cell
    pub fn get_base_cell(&self) -> u8 {
        unsafe { h3ron_h3_sys::h3GetBaseCell(self.0) as u8 }
    }

    /// Gets the unidirectional edge from `self` to `destination`
    ///
    /// # Returns
    /// The built index may be invalid.
    /// Use the `unidirectional_edge_to_unchecked` method for validity check.
    pub fn unidirectional_edge_to_unchecked(&self, destination: &Self) -> H3Edge {
        H3Edge::new(unsafe {
            h3ron_h3_sys::getH3UnidirectionalEdge(self.h3index(), destination.h3index())
        })
    }

    /// Gets the unidirectional edge from `self` to `destination`
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    /// Use the `unidirectional_edge_to_unchecked` to avoid error.
    pub fn unidirectional_edge_to(&self, destination: &Self) -> Result<H3Edge, Error> {
        let res = self.unidirectional_edge_to_unchecked(destination);
        res.validate()?;
        Ok(res)
    }

    /// Retrieves all unidirectional H3 edges around `self`
    ///
    /// For repeated creation of [`H3Edge`] around a [`H3Cell`] also
    /// see [`crate::iter::H3EdgesBuilder`], which is more efficient.
    pub fn unidirectional_edges(&self) -> IndexVec<H3Edge> {
        let mut index_vec = IndexVec::with_length(6);
        unsafe {
            h3ron_h3_sys::getH3UnidirectionalEdgesFromHexagon(
                self.h3index(),
                index_vec.as_mut_ptr(),
            )
        };
        index_vec
    }

    /// get the average cell area at `resolution` in square meters.
    ///
    /// ```
    /// use h3ron::H3Cell;
    ///
    /// assert_eq!(15047.5, H3Cell::area_m2(10));
    /// ```
    pub fn area_m2(resolution: u8) -> f64 {
        unsafe { h3ron_h3_sys::hexAreaM2(resolution as i32) }
    }

    /// get the average cell area at `resolution` in square kilometers.
    pub fn area_km2(resolution: u8) -> f64 {
        unsafe { h3ron_h3_sys::hexAreaKm2(resolution as i32) }
    }
}

impl ExactArea for H3Cell {
    fn exact_area_m2(&self) -> f64 {
        unsafe { h3ron_h3_sys::cellAreaM2(self.0) }
    }

    fn exact_area_km2(&self) -> f64 {
        unsafe { h3ron_h3_sys::cellAreaKm2(self.0) }
    }

    fn exact_area_rads2(&self) -> f64 {
        unsafe { h3ron_h3_sys::cellAreaRads2(self.0) }
    }
}

impl ToString for H3Cell {
    fn to_string(&self) -> String {
        format!("{:x}", self.0)
    }
}

impl FromStr for H3Cell {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let h3index: H3Index = CString::new(s)
            .map(|cs| unsafe { h3ron_h3_sys::stringToH3(cs.as_ptr()) })
            .map_err(|_| Error::InvalidInput)?;
        H3Cell::try_from(h3index)
    }
}

impl ToPolygon for H3Cell {
    /// the polygon spanning the area of the index
    fn to_polygon(&self) -> Polygon<f64> {
        let gb = unsafe {
            let mut mu = MaybeUninit::<h3ron_h3_sys::GeoBoundary>::uninit();
            h3ron_h3_sys::h3ToGeoBoundary(self.0, mu.as_mut_ptr());
            mu.assume_init()
        };

        let mut coordinates = geoboundary_to_coordinates(&gb);
        coordinates.push(coordinates.first().copied().unwrap());
        Polygon::new(LineString::from(coordinates), vec![])
    }
}

impl ToCoordinate for H3Cell {
    /// the centroid coordinate of the h3 index
    fn to_coordinate(&self) -> Coordinate<f64> {
        unsafe {
            let mut gc = GeoCoord { lat: 0.0, lon: 0.0 };
            h3ron_h3_sys::h3ToGeo(self.0, &mut gc);

            Coordinate {
                x: h3ron_h3_sys::radsToDegs(gc.lon),
                y: h3ron_h3_sys::radsToDegs(gc.lat),
            }
        }
    }
}

impl Deref for H3Cell {
    type Target = H3Index;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::convert::{TryFrom, TryInto};
    use std::str::FromStr;

    #[cfg(feature = "use-serde")]
    use bincode::{deserialize, serialize};

    use h3ron_h3_sys::H3Index;

    use crate::h3_cell::H3Cell;
    use crate::Index;

    #[test]
    fn test_h3_to_string() {
        let h3index = 0x89283080ddbffff_u64;
        assert_eq!(
            H3Cell::try_from(h3index).unwrap().to_string(),
            "89283080ddbffff".to_string()
        );
    }

    #[test]
    fn test_debug_hexadecimal() {
        let cell = H3Cell::new(0x89283080ddbffff_u64);
        assert_eq!(format!("{:?}", cell), "H3Cell(89283080ddbffff)".to_string())
    }

    #[test]
    fn test_string_to_h3() {
        let index = H3Cell::from_str("89283080ddbffff").expect("parsing failed");
        assert_eq!(H3Cell::try_from(0x89283080ddbffff_u64).unwrap(), index);
    }

    #[test]
    fn test_is_valid() {
        assert!(H3Cell::try_from(0x89283080ddbffff_u64).unwrap().is_valid());
        assert!(!H3Cell::new(0_u64).is_valid());
        assert!(H3Cell::try_from(0_u64).is_err());
    }

    #[test]
    fn test_eq() {
        assert_eq!(
            H3Cell::try_from(0x89283080ddbffff_u64).unwrap(),
            H3Cell::try_from(0x89283080ddbffff_u64).unwrap()
        );
    }

    #[test]
    fn test_hex_ring_1() {
        let idx = H3Cell::try_from(0x89283080ddbffff_u64).unwrap();
        let ring = idx.hex_ring(1).unwrap();
        assert_eq!(ring.iter().count(), 6);
        assert!(ring.iter().all(|index| index.is_valid()));
    }

    #[test]
    fn test_hex_ring_0() {
        let idx = H3Cell::new(0x89283080ddbffff_u64);
        let ring = idx.hex_ring(0).unwrap();
        assert_eq!(ring.iter().count(), 1);
        assert!(ring.iter().all(|index| index.is_valid()));
    }

    #[test]
    fn test_k_ring_distances() {
        let idx = H3Cell::new(0x89283080ddbffff_u64);
        let k_min = 2;
        let k_max = 2;
        let indexes = idx.k_ring_distances(k_min, k_max);
        assert!(indexes.len() > 10);
        for (k, index) in indexes.iter() {
            assert!(index.is_valid());
            assert!(*k >= k_min);
            assert!(*k <= k_max);
        }
    }

    #[test]
    fn test_hex_range_distances() {
        let idx = H3Cell::new(0x89283080ddbffff_u64);
        let k_min = 2;
        let k_max = 2;
        let indexes = idx.hex_range_distances(k_min, k_max).unwrap();
        assert!(indexes.len() > 10);
        for (k, index) in indexes.iter() {
            assert!(index.is_valid());
            assert!(*k >= k_min);
            assert!(*k <= k_max);
        }
    }

    #[test]
    fn test_hex_range_distances_2() {
        let idx = H3Cell::new(0x89283080ddbffff_u64);
        let k_min = 0;
        let k_max = 10;
        let indexes = idx.hex_range_distances(k_min, k_max).unwrap();

        let mut indexes_resolutions: HashMap<H3Index, Vec<u32>> = HashMap::new();
        for (dist, idx) in indexes.iter() {
            indexes_resolutions
                .entry(idx.h3index())
                .and_modify(|v| v.push(*dist))
                .or_insert_with(|| vec![*dist]);
        }

        assert!(indexes.len() > 10);
        for (k, index) in indexes.iter() {
            assert!(index.is_valid());
            assert!(*k >= k_min);
            assert!(*k <= k_max);
        }
    }

    #[cfg(feature = "use-serde")]
    #[test]
    fn serde_index_roundtrip() {
        let idx = H3Cell::new(0x89283080ddbffff_u64);
        let serialized_data = serialize(&idx).unwrap();
        let idx_2: H3Cell = deserialize(&serialized_data).unwrap();
        assert_eq!(idx, idx_2);
        assert_eq!(idx.h3index(), idx_2.h3index());
    }

    /// this test is not really a hard requirement, but it is nice to know
    /// Index is handled just like an u64
    #[cfg(feature = "use-serde")]
    #[test]
    fn serde_index_from_h3index() {
        let idx: H3Index = 0x89283080ddbffff_u64;
        let serialized_data = serialize(&idx).unwrap();
        let idx_2: H3Cell = deserialize(&serialized_data).unwrap();
        assert_eq!(idx, idx_2.h3index());
    }

    #[test]
    fn test_is_neighbor() {
        let idx: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
        let ring = idx.hex_ring(1).unwrap();
        let neighbor = ring.first().unwrap();
        assert!(idx.is_neighbor_to(&neighbor));
        let wrong_neighbor = 0x8a2a1072b59ffff_u64.try_into().unwrap();
        assert!(!idx.is_neighbor_to(&wrong_neighbor));
        // Self
        assert!(!idx.is_neighbor_to(&idx));
    }

    #[test]
    fn test_distance_to() {
        let idx: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
        assert_eq!(idx.distance_to(&idx), 0);
        let ring = idx.hex_ring(1).unwrap();
        let neighbor = ring.first().unwrap();
        assert_eq!(idx.distance_to(&neighbor), 1);
        let ring = idx.hex_ring(3).unwrap();
        let neighbor = ring.first().unwrap();
        assert_eq!(idx.distance_to(&neighbor), 3);
    }

    mod edges {
        use super::*;

        #[test]
        fn can_retrieve_edges() {
            let index: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
            assert_eq!(index.resolution(), 9);
            let edges = index.unidirectional_edges();
            let indexes: Vec<(String, u8)> = edges
                .into_iter()
                .map(|e| (e.to_string(), e.resolution()))
                .collect();
            assert_eq!(
                indexes,
                vec![
                    ("119283080ddbffff".to_string(), 9),
                    ("129283080ddbffff".to_string(), 9),
                    ("139283080ddbffff".to_string(), 9),
                    ("149283080ddbffff".to_string(), 9),
                    ("159283080ddbffff".to_string(), 9),
                    ("169283080ddbffff".to_string(), 9)
                ]
            );
        }

        #[test]
        fn retrieved_edges_are_valid() {
            let index: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
            let edges = index.unidirectional_edges();
            for edge in edges.into_iter() {
                edge.validate().unwrap();
            }
        }

        #[test]
        fn can_find_edge_to() {
            let index: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
            let ring = index.hex_ring(1).unwrap();
            let neighbor = ring.first().unwrap();
            let edge_to = index.unidirectional_edge_to(&neighbor).unwrap();
            let edge_from = neighbor.unidirectional_edge_to(&index).unwrap();
            assert_ne!(edge_to.h3index(), 0);
            assert_ne!(edge_from.h3index(), 0);
            assert_ne!(edge_from, edge_to);
            assert_eq!(edge_to.destination_index().unwrap(), neighbor);
            assert_eq!(edge_to.origin_index().unwrap(), index);
            assert_eq!(edge_from.destination_index().unwrap(), index);
            assert_eq!(edge_from.origin_index().unwrap(), neighbor);
        }

        #[should_panic(expected = "InvalidH3Edge")]
        #[test]
        fn can_fail_to_find_edge_to() {
            let index: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
            let wrong_neighbor: H3Cell = 0x8a2a1072b59ffff_u64.try_into().unwrap();
            index.unidirectional_edge_to(&wrong_neighbor).unwrap();
        }
    }
}
