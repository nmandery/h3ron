use std::collections::HashMap;
use std::ffi::CString;
use std::mem::MaybeUninit;
use std::os::raw::c_int;
use std::str::FromStr;

#[cfg(feature = "with-geo-types-0_4")]
use geo_types_04::{Coordinate, LineString, Point, Polygon};
#[cfg(feature = "with-geo-types-0_6")]
use geo_types_06::{Coordinate, LineString, Point, Polygon};

use h3_sys::{GeoCoord, H3Index};

use crate::error::Error;
use crate::geo::{coordinate_to_geocoord, point_to_geocoord};
use crate::util::h3indexes_to_indexes;

#[derive(PartialOrd, PartialEq, Clone, Debug)]
pub struct Index(H3Index);

impl From<H3Index> for Index {
    fn from(h3index: H3Index) -> Self {
        Index(h3index)
    }
}

/*
impl TryFrom<H3Index> for Index {
    type Error = Error;

    fn try_from(h3index: H3Index) -> Result<Self, Self::Error> {
        let index = Index::from(h3index);
        if !index.is_valid() {
            Err(Error::InvalidH3Index)
        } else {
            Ok(index)
        }
    }
}
 */

impl Index {
    pub fn new(h3index: H3Index) -> Self {
        Self(h3index)
    }

    pub fn h3index(&self) -> H3Index {
        self.0
    }

    pub fn resolution(&self) -> u8 {
        (unsafe { h3_sys::h3GetResolution(self.0) }) as u8
    }

    pub fn is_valid(&self) -> bool {
        unsafe { h3_sys::h3IsValid(self.0) != 0 }
    }

    pub fn is_parent_of(&self, other: &Index) -> bool {
        *self == other.get_parent(self.resolution())
    }

    pub fn is_child_of(&self, other: &Index) -> bool {
        other.is_parent_of(self)
    }

    pub fn get_parent(&self, parent_resolution: u8) -> Index {
        Index::from(unsafe { h3_sys::h3ToParent(self.0, parent_resolution as c_int) })
    }

    pub fn get_children(&self, child_resolution: u8) -> Vec<Index> {
        let max_size = unsafe { h3_sys::maxH3ToChildrenSize(self.0, child_resolution as c_int) };
        let mut h3_indexes_out: Vec<h3_sys::H3Index> = vec![0; max_size as usize];
        unsafe {
            h3_sys::h3ToChildren(self.0, child_resolution as c_int, h3_indexes_out.as_mut_ptr());
        }
        remove_zero_indexes_from_vec!(h3_indexes_out);
        h3indexes_to_indexes(h3_indexes_out)
    }

    pub fn polygon(&self) -> Polygon<f64> {
        let gb = unsafe {
            let mut mu = MaybeUninit::<h3_sys::GeoBoundary>::uninit();
            h3_sys::h3ToGeoBoundary(self.0, mu.as_mut_ptr());
            mu.assume_init()
        };

        let mut nodes = vec![];
        for i in 0..gb.numVerts {
            nodes.push((
                unsafe { h3_sys::radsToDegs(gb.verts[i as usize].lon) },
                unsafe { h3_sys::radsToDegs(gb.verts[i as usize].lat) },
            ));
        }
        nodes.push(*nodes.first().unwrap());
        Polygon::new(LineString::from(nodes), vec![])
    }

    pub fn coordinate(&self) -> Coordinate<f64> {
        unsafe {
            let mut gc = GeoCoord {
                lat: 0.0,
                lon: 0.0,
            };
            h3_sys::h3ToGeo(self.0, &mut gc);

            Coordinate {
                x: h3_sys::radsToDegs(gc.lon),
                y: h3_sys::radsToDegs(gc.lat),
            }
        }
    }

    pub fn from_point(pt: &Point<f64>, h3_resolution: u8) -> Self {
        let h3index = unsafe {
            let gc = point_to_geocoord(pt);
            h3_sys::geoToH3(&gc, h3_resolution as c_int)
        };
        Index::from(h3index)
    }


    pub fn from_coordinate(c: &Coordinate<f64>, h3_resolution: u8) -> Self {
        let h3index = unsafe {
            let gc = coordinate_to_geocoord(c);
            h3_sys::geoToH3(&gc, h3_resolution as c_int)
        };
        Index::from(h3index)
    }

    pub fn k_ring(&self, k: i32) -> Vec<Index> {
        let max_size = unsafe { h3_sys::maxKringSize(k) as usize };
        let mut h3_indexes_out: Vec<H3Index> = vec![0; max_size];

        unsafe {
            h3_sys::kRing(self.0, k as c_int, h3_indexes_out.as_mut_ptr());
        }
        remove_zero_indexes_from_vec!(h3_indexes_out);
        h3indexes_to_indexes(h3_indexes_out)
    }
}

impl ToString for Index {
    fn to_string(&self) -> String {
        format!("{:x}", self.0)
    }
}

impl FromStr for Index {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let h3index: H3Index = CString::new(s).map(|cs| unsafe {
            h3_sys::stringToH3(cs.as_ptr())
        }).map_err(|_| Error::InvalidInput)?;
        Ok(Index::from(h3index))
    }
}

/// group indexes by their resolution
pub fn group_indexes_by_resolution(mut indexes: Vec<Index>) -> HashMap<u8, Vec<Index>> {
    let mut m = HashMap::new();
    indexes.drain(..).for_each(|idx: Index| {
        m.entry(idx.resolution())
            .or_insert_with(Vec::new)
            .push(idx);
    });
    m
}


#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::index::Index;

    #[test]
    fn test_h3_to_string() {
        let h3index = 0x89283080ddbffff_u64;
        assert_eq!(Index::from(h3index).to_string(), "89283080ddbffff".to_string());
    }

    #[test]
    fn test_string_to_h3() {
        let h3index = Index::from_str("89283080ddbffff")
            .expect("parsing failed");
        assert_eq!(Index::from(0x89283080ddbffff_u64), h3index);
    }

    #[test]
    fn test_is_valid() {
        assert_eq!(Index::from(0x89283080ddbffff_u64).is_valid(), true);
        assert_eq!(Index::from(0_u64).is_valid(), false);
    }
}
