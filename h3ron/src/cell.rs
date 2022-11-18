use std::fmt::{self, Debug, Formatter};
use std::ops::Deref;
use std::os::raw::c_int;
use std::str::FromStr;

use geo_types::{Coord, Point, Polygon};
#[cfg(feature = "use-serde")]
use serde::{Deserialize, Serialize};

use h3ron_h3_sys::H3Index;

use crate::collections::indexvec::IndexVec;
use crate::error::Error;
use crate::index::{index_from_str, Index};
use crate::iter::CellBoundaryBuilder;
use crate::{max_grid_disk_size, FromH3Index, H3DirectedEdge, ToCoordinate, ToPolygon};

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
        if unsafe { h3ron_h3_sys::isValidCell(self.h3index()) == 0 } {
            Err(Error::CellInvalid)
        } else {
            Ok(())
        }
    }
}

impl H3Cell {
    /// Build a new `Index` from a `Point`.
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    pub fn from_point(pt: Point<f64>, h3_resolution: u8) -> Result<Self, Error> {
        Self::from_coordinate(pt.0, h3_resolution)
    }

    /// Build a new `Index` from coordinates.
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    pub fn from_coordinate(c: Coord<f64>, h3_resolution: u8) -> Result<Self, Error> {
        let lat_lng = h3ron_h3_sys::LatLng::from(c);
        let mut cell_h3index: H3Index = 0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::latLngToCell(&lat_lng, c_int::from(h3_resolution), &mut cell_h3index)
        })
        .map(|_| Self::new(cell_h3index))
    }

    /// Checks if `self` is a parent of `other`
    pub fn is_parent_of(&self, other: &Self) -> Result<bool, Error> {
        Ok(*self == other.get_parent(self.resolution())?)
    }

    /// Checks if `other` is a parent of `self`
    pub fn is_child_of(&self, other: &Self) -> Result<bool, Error> {
        other.is_parent_of(self)
    }

    /// Checks if `self` is a parent of `other`
    pub fn contains(&self, other: &Self) -> Result<bool, Error> {
        self.is_parent_of(other)
    }

    /// Retrieves the parent (or grandparent, etc) cell of the given cell
    pub fn get_parent(&self, parent_resolution: u8) -> Result<Self, Error> {
        let mut cell_index: H3Index = 0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::cellToParent(
                self.h3index(),
                c_int::from(parent_resolution),
                &mut cell_index,
            )
        })
        .map(|_| Self::new(cell_index))
    }

    /// Retrieves all children of `self` at resolution `child_resolution`
    pub fn get_children(&self, child_resolution: u8) -> Result<IndexVec<Self>, Error> {
        let child_resolution = c_int::from(child_resolution);

        let mut children_size: i64 = 0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::cellToChildrenSize(self.h3index(), child_resolution, &mut children_size)
        })?;

        let mut index_vec = IndexVec::with_length(children_size as usize);

        Error::check_returncode(unsafe {
            h3ron_h3_sys::cellToChildren(self.h3index(), child_resolution, index_vec.as_mut_ptr())
        })?;
        Ok(index_vec)
    }

    /// Checks if the current index and `other` are neighbors.
    pub fn are_neighbor_cells(&self, other: Self) -> Result<bool, Error> {
        let mut res: i32 = 0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::areNeighborCells(self.0, other.0, &mut res)
        })
        .map(|_| res == 1)
    }

    /// `grid_disk` produces all cells within k distance of the origin cell.
    ///
    /// k=0 is defined as the origin cell, k=1 is defined as k=0 + all
    /// neighboring cells, and so on.
    ///
    /// # Note
    ///
    /// For repeated building of grid disks, there is also [`super::iter::GridDiskBuilder`].
    pub fn grid_disk(&self, k: u32) -> Result<IndexVec<Self>, Error> {
        let mut index_vec = IndexVec::with_length(max_grid_disk_size(k)?);
        Error::check_returncode(unsafe {
            h3ron_h3_sys::gridDisk(self.0, k as c_int, index_vec.as_mut_ptr())
        })
        .map(|_| index_vec)
    }

    /// hollow hexagon ring at `self`
    pub fn grid_ring_unsafe(&self, k: u32) -> Result<IndexVec<Self>, Error> {
        // calculation of max_size taken from
        // https://github.com/uber/h3-py/blob/dd08189b378429291c342d0af3d3cc1e38a659d5/src/h3/_cy/cells.pyx#L111
        //let mut index_vec = IndexVec::with_length(if k > 0 { 6 * k as usize } else { 1 });
        let mut index_vec = IndexVec::with_length(max_grid_disk_size(k)?);

        Error::check_returncode(unsafe {
            h3ron_h3_sys::gridRingUnsafe(self.0, k as c_int, index_vec.as_mut_ptr())
        })
        .map(|_| index_vec)
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
    /// For repeated building of k-rings, there is also [`super::iter::GridDiskBuilder`].
    ///
    pub fn grid_disk_distances(&self, k_min: u32, k_max: u32) -> Result<Vec<(u32, Self)>, Error> {
        let max_size = max_grid_disk_size(k_max)?;
        let mut h3_indexes_out: Vec<H3Index> = vec![0; max_size];
        let mut distances_out: Vec<c_int> = vec![0; max_size];
        Error::check_returncode(unsafe {
            h3ron_h3_sys::gridDiskDistances(
                self.0,
                k_max as c_int,
                h3_indexes_out.as_mut_ptr(),
                distances_out.as_mut_ptr(),
            )
        })
        .map(|_| Self::associate_index_distances(h3_indexes_out, &distances_out, k_min))
    }

    pub fn grid_disk_distances_unsafe(
        &self,
        k_min: u32,
        k_max: u32,
    ) -> Result<Vec<(u32, Self)>, Error> {
        let max_size = max_grid_disk_size(k_max)?;
        let mut h3_indexes_out: Vec<H3Index> = vec![0; max_size];
        let mut distances_out: Vec<c_int> = vec![0; max_size];
        Error::check_returncode(unsafe {
            h3ron_h3_sys::gridDiskDistancesUnsafe(
                self.0,
                k_max as c_int,
                h3_indexes_out.as_mut_ptr(),
                distances_out.as_mut_ptr(),
            )
        })
        .map(|_| Self::associate_index_distances(h3_indexes_out, &distances_out, k_min))
    }

    /// Retrieves the number of K Rings between `self` and `other`.
    ///
    /// For distance in miles or kilometers use haversine algorithms.
    pub fn grid_distance_to(&self, other: Self) -> Result<usize, Error> {
        let mut grid_distance: i64 = 0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::gridDistance(self.0, other.0, &mut grid_distance)
        })
        .map(|_| grid_distance as usize)
    }

    fn associate_index_distances(
        h3_indexes_out: Vec<H3Index>,
        distances_out: &[c_int],
        k_min: u32,
    ) -> Vec<(u32, Self)> {
        h3_indexes_out
            .into_iter()
            .enumerate()
            .filter(|(idx, h3index)| *h3index != 0 && distances_out[*idx] >= k_min as i32)
            .map(|(idx, h3index)| (distances_out[idx] as u32, Self::new(h3index)))
            .collect()
    }

    /// determines if an H3 cell is a pentagon
    pub fn is_pentagon(&self) -> bool {
        unsafe { h3ron_h3_sys::isPentagon(self.0) == 1 }
    }

    /// returns the base cell "number" (0 to 121) of the provided H3 cell
    pub fn get_base_cell_number(&self) -> u8 {
        unsafe { h3ron_h3_sys::getBaseCellNumber(self.0) as u8 }
    }

    /// Gets the directed edge from `self` to `destination`
    ///
    /// # Returns
    /// If the built index is invalid, returns an Error.
    /// Use the `unidirectional_edge_to_unchecked` to avoid error.
    pub fn directed_edge_to(&self, destination: Self) -> Result<H3DirectedEdge, Error> {
        let mut edge_h3index: H3Index = 0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::cellsToDirectedEdge(
                self.h3index(),
                destination.h3index(),
                &mut edge_h3index,
            )
        })
        .map(|_| H3DirectedEdge::new(edge_h3index))
    }

    /// Retrieves all directed H3 edges around `self` where `self` is the origin
    ///
    /// For repeated creation of [`H3DirectedEdge`] around a [`H3Cell`] also
    /// see [`crate::iter::H3DirectedEdgesBuilder`], which is more efficient.
    pub fn directed_edges(&self) -> Result<IndexVec<H3DirectedEdge>, Error> {
        let mut index_vec = IndexVec::with_length(6);
        Error::check_returncode(unsafe {
            h3ron_h3_sys::originToDirectedEdges(self.h3index(), index_vec.as_mut_ptr())
        })
        .map(|_| index_vec)
    }

    /// get the average cell area at `resolution` in square meters.
    ///
    /// ```
    /// use h3ron::H3Cell;
    ///
    /// assert_eq!(15047, H3Cell::area_avg_m2(10).unwrap() as i32);
    /// ```
    pub fn area_avg_m2(resolution: u8) -> Result<f64, Error> {
        let mut area: f64 = 0.0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::getHexagonAreaAvgM2(i32::from(resolution), &mut area)
        })
        .map(|_| area)
    }

    /// get the average cell area at `resolution` in square kilometers.
    pub fn area_avg_km2(resolution: u8) -> Result<f64, Error> {
        let mut area: f64 = 0.0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::getHexagonAreaAvgKm2(i32::from(resolution), &mut area)
        })
        .map(|_| area)
    }

    /// Retrieves the exact area of `self` in square meters
    pub fn area_m2(&self) -> Result<f64, Error> {
        let mut area: f64 = 0.0;
        Error::check_returncode(unsafe { h3ron_h3_sys::cellAreaM2(self.0, &mut area) })
            .map(|_| area)
    }

    /// Retrieves the exact area of `self` in square kilometers
    pub fn area_km2(&self) -> Result<f64, Error> {
        let mut area: f64 = 0.0;
        Error::check_returncode(unsafe { h3ron_h3_sys::cellAreaKm2(self.0, &mut area) })
            .map(|_| area)
    }

    /// Retrieves the exact area of `self` in square radians
    pub fn area_rads2(&self) -> Result<f64, Error> {
        let mut area: f64 = 0.0;
        Error::check_returncode(unsafe { h3ron_h3_sys::cellAreaRads2(self.0, &mut area) })
            .map(|_| area)
    }

    /// returns the center child of `self` at the specified resolution.
    pub fn center_child(&self, resolution: u8) -> Result<Self, Error> {
        let mut cell_index: H3Index = 0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::cellToCenterChild(
                self.h3index(),
                c_int::from(resolution),
                &mut cell_index,
            )
        })
        .map(|_| Self::new(cell_index))
    }
}

impl ToString for H3Cell {
    fn to_string(&self) -> String {
        format!("{:x}", self.0)
    }
}

impl FromStr for H3Cell {
    type Err = Error;

    /// Parse a hex-representation of a H3Cell from a string.
    ///
    /// With the `parse` feature enabled this function is also able
    /// to parse strings containing integers and a custom coordinate-based format
    /// in the form of `"x,y,resolution"`.
    ///
    /// Examples:
    ///
    /// ```rust
    /// use h3ron::{H3Cell, Index};
    /// use std::str::FromStr;
    ///
    /// let index = H3Cell::from_str("89283080ddbffff").unwrap();
    ///
    /// #[cfg(feature = "parse")]
    /// {
    ///     // parse from a string containing an integer
    ///     let index = H3Cell::from_str("617700169518678015").unwrap();
    ///
    ///     // parse from coordinates and resolution
    ///     let index = H3Cell::from_str("23.3,12.3,6").unwrap();
    /// }
    /// ```
    ///
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[cfg(not(feature = "parse"))]
        {
            index_from_str(s)
        }

        #[cfg(feature = "parse")]
        {
            if let Ok(cell) = index_from_str(s) {
                return Ok(cell);
            }

            if let Ok(h3index) = u64::from_str(s) {
                return H3Cell::try_from(h3index);
            }

            // attempt to parse as coordinate pair and resolution
            if let Ok((_, (coord, res))) = parse::parse_coordinate_and_resolution(s) {
                return H3Cell::from_coordinate(coord, res);
            }

            Err(Self::Err::Failed)
        }
    }
}

#[cfg(feature = "parse")]
mod parse {
    use geo_types::Coord;
    use nom::branch::alt;
    use nom::bytes::complete::{tag, take_while, take_while_m_n};
    use nom::combinator::map_res;
    use nom::number::complete::double;
    use nom::IResult;
    use std::str::FromStr;

    fn is_whitespace(c: char) -> bool {
        c.is_ascii_whitespace()
    }

    fn seperator(s: &str) -> IResult<&str, &str> {
        alt((tag(","), (tag(";"))))(s)
    }

    fn u8_str(s: &str) -> IResult<&str, u8> {
        map_res(take_while_m_n(1, 2, |c: char| c.is_ascii_digit()), |u8s| {
            u8::from_str(u8s)
        })(s)
    }

    pub(crate) fn parse_coordinate_and_resolution(s: &str) -> IResult<&str, (Coord, u8)> {
        let (s, _) = take_while(is_whitespace)(s)?;
        let (s, x) = double(s)?;
        let (s, _) = take_while(is_whitespace)(s)?;
        let (s, _) = seperator(s)?;
        let (s, _) = take_while(is_whitespace)(s)?;
        let (s, y) = double(s)?;
        let (s, _) = take_while(is_whitespace)(s)?;
        let (s, _) = seperator(s)?;
        let (s, _) = take_while(is_whitespace)(s)?;
        let (s, r) = u8_str(s)?;
        Ok((s, (Coord::from((x, y)), r as u8)))
    }
}

impl ToPolygon for H3Cell {
    type Error = Error;

    /// the polygon spanning the area of the index
    fn to_polygon(&self) -> Result<Polygon<f64>, Self::Error> {
        CellBoundaryBuilder::new()
            .iter_cell_boundary_vertices(self, true)
            .map(Into::into)
    }
}

impl ToCoordinate for H3Cell {
    type Error = Error;

    /// the centroid coordinate of the h3 index
    fn to_coordinate(&self) -> Result<Coord<f64>, Self::Error> {
        let mut ll = h3ron_h3_sys::LatLng { lat: 0.0, lng: 0.0 };
        Error::check_returncode(unsafe { h3ron_h3_sys::cellToLatLng(self.0, &mut ll) })
            .map(|_| ll.into())
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
    use std::str::FromStr;

    #[cfg(feature = "use-serde")]
    use bincode::{deserialize, serialize};

    use h3ron_h3_sys::H3Index;

    use crate::cell::H3Cell;
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
        assert_eq!(format!("{:?}", cell), "H3Cell(89283080ddbffff)".to_string());
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
    fn test_grid_ring_unsafe_1() {
        let idx = H3Cell::try_from(0x89283080ddbffff_u64).unwrap();
        let ring = idx.grid_ring_unsafe(1).unwrap();
        assert_eq!(ring.iter().count(), 6);
        assert!(ring.iter().all(|index| index.is_valid()));
    }

    #[test]
    fn test_grid_ring_unsafe_0() {
        let idx = H3Cell::new(0x89283080ddbffff_u64);
        let ring = idx.grid_ring_unsafe(0).unwrap();
        assert_eq!(ring.iter().count(), 1);
        assert!(ring.iter().all(|index| index.is_valid()));
    }

    #[test]
    fn test_k_ring_distances() {
        let idx = H3Cell::new(0x89283080ddbffff_u64);
        let k_min = 2;
        let k_max = 2;
        let indexes = idx.grid_disk_distances(k_min, k_max).unwrap();
        assert!(indexes.len() > 10);
        for (k, index) in &indexes {
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
        let indexes = idx.grid_disk_distances_unsafe(k_min, k_max).unwrap();
        assert!(indexes.len() > 10);
        for (k, index) in &indexes {
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
        let indexes = idx.grid_disk_distances_unsafe(k_min, k_max).unwrap();

        let mut indexes_resolutions: HashMap<H3Index, Vec<u32>> = HashMap::new();
        for (dist, idx) in &indexes {
            indexes_resolutions
                .entry(idx.h3index())
                .and_modify(|v| v.push(*dist))
                .or_insert_with(|| vec![*dist]);
        }

        assert!(indexes.len() > 10);
        for (k, index) in &indexes {
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
        let ring = idx.grid_ring_unsafe(1).unwrap();
        let neighbor = ring.first().unwrap();
        assert!(idx.are_neighbor_cells(neighbor).unwrap());
        let wrong_neighbor = 0x8a2a1072b59ffff_u64.try_into().unwrap();
        assert!(idx.are_neighbor_cells(wrong_neighbor).is_err());
        // Self
        assert!(idx.are_neighbor_cells(idx).is_ok()); // fix in H3?
    }

    #[test]
    fn test_distance_to() {
        let idx: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
        assert_eq!(idx.grid_distance_to(idx).unwrap(), 0);
        let ring = idx.grid_ring_unsafe(1).unwrap();
        let neighbor = ring.first().unwrap();
        assert_eq!(idx.grid_distance_to(neighbor).unwrap(), 1);
        let ring = idx.grid_ring_unsafe(3).unwrap();
        let neighbor = ring.first().unwrap();
        assert_eq!(idx.grid_distance_to(neighbor).unwrap(), 3);
    }

    mod edges {
        use super::*;

        #[test]
        fn can_retrieve_edges() {
            let index: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
            assert_eq!(index.resolution(), 9);
            let edges = index.directed_edges().unwrap();
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
            let edges = index.directed_edges().unwrap();
            for edge in &edges {
                edge.validate().unwrap();
            }
        }

        #[test]
        fn can_find_edge_to() {
            let index: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
            let ring = index.grid_ring_unsafe(1).unwrap();
            let neighbor = ring.first().unwrap();
            let edge_to = index.directed_edge_to(neighbor).unwrap();
            let edge_from = neighbor.directed_edge_to(index).unwrap();
            assert_ne!(edge_to.h3index(), 0);
            assert_ne!(edge_from.h3index(), 0);
            assert_ne!(edge_from, edge_to);
            assert_eq!(edge_to.destination_cell().unwrap(), neighbor);
            assert_eq!(edge_to.origin_cell().unwrap(), index);
            assert_eq!(edge_from.destination_cell().unwrap(), index);
            assert_eq!(edge_from.origin_cell().unwrap(), neighbor);
        }

        #[should_panic(expected = "NotNeighbors")]
        #[test]
        fn can_fail_to_find_edge_to() {
            let index: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
            let wrong_neighbor: H3Cell = 0x8a2a1072b59ffff_u64.try_into().unwrap();
            index.directed_edge_to(wrong_neighbor).unwrap();
        }
    }

    #[cfg(feature = "parse")]
    mod parse {
        use crate::{H3Cell, Index, ToCoordinate};
        use std::str::FromStr;

        #[test]
        fn parse_cell_from_numeric() {
            let cell: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
            let s = format!("{}", cell.h3index());

            let cell2 = H3Cell::from_str(&s).unwrap();
            assert_eq!(cell, cell2);
        }

        #[test]
        fn parse_cell_from_coordinate_and_resolution() {
            let cell: H3Cell = 0x89283080ddbffff_u64.try_into().unwrap();
            let coord = cell.to_coordinate().unwrap();
            let s = format!("{},{},{}", coord.x, coord.y, cell.resolution());

            let cell2 = H3Cell::from_str(&s).unwrap();
            assert_eq!(cell, cell2);
        }
    }
}
