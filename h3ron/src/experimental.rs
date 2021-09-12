use std::ops::{Add, Sub};
use std::result::Result;

use h3ron_h3_sys::H3Index;

use crate::error::Error;
use crate::H3Cell;
use crate::Index;

#[derive(Debug, PartialEq, Copy, Clone)]
pub struct CoordIj {
    pub i: i32,
    pub j: i32,
}

impl Default for CoordIj {
    fn default() -> Self {
        Self { i: 0, j: 0 }
    }
}

impl Sub for CoordIj {
    type Output = CoordIj;

    fn sub(self, rhs: Self) -> Self::Output {
        CoordIj {
            i: self.i - rhs.i,
            j: self.j - rhs.j,
        }
    }
}

impl Add for CoordIj {
    type Output = CoordIj;

    fn add(self, rhs: Self) -> Self::Output {
        CoordIj {
            i: self.i + rhs.i,
            j: self.j + rhs.j,
        }
    }
}

pub fn h3_to_local_ij(origin_index: &H3Cell, index: &H3Cell) -> Result<CoordIj, Error> {
    unsafe {
        let mut cij = h3ron_h3_sys::CoordIJ { i: 0, j: 0 };
        if h3ron_h3_sys::experimentalH3ToLocalIj(origin_index.h3index(), index.h3index(), &mut cij)
            == 0
        {
            Ok(CoordIj { i: cij.i, j: cij.j })
        } else {
            Err(Error::NoLocalIjCoordinates)
        }
    }
}

pub fn local_ij_to_h3(origin_index: &H3Cell, coordij: &CoordIj) -> Result<H3Cell, Error> {
    unsafe {
        let cij = h3ron_h3_sys::CoordIJ {
            i: coordij.i,
            j: coordij.j,
        };
        let mut h3_index_out: H3Index = 0;
        if h3ron_h3_sys::experimentalLocalIjToH3(origin_index.h3index(), &cij, &mut h3_index_out)
            == 0
        {
            Ok(H3Cell::new(h3_index_out))
        } else {
            Err(Error::NoLocalIjCoordinates)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::convert::TryFrom;

    use crate::experimental::{h3_to_local_ij, local_ij_to_h3};
    use crate::h3_cell::H3Cell;

    #[test]
    fn test_local_ij() {
        let origin_cell = H3Cell::try_from(0x89283080ddbffff_u64).unwrap();
        let ring = origin_cell.k_ring(1);
        assert_ne!(ring.iter().count(), 0);
        let other_cell = ring.iter().find(|i| *i != origin_cell).unwrap();

        // the coordij of the origin index. This is not necessarily at (0, 0)
        let coordij_origin = h3_to_local_ij(&origin_cell, &origin_cell).unwrap();

        // the coordij of the other index in the coordinate system of the origin index
        let coordij_other = h3_to_local_ij(&origin_cell, &other_cell).unwrap();

        // As the other_index was taken from k_ring 1, the difference of the i and j coordinates
        // must be -1, 0 or 1
        let coordij_diff = coordij_origin - coordij_other;
        assert!(coordij_diff.i.abs() <= 1);
        assert!(coordij_diff.j.abs() <= 1);

        // convert the coordij back to an index
        let other_index_2 = local_ij_to_h3(&origin_cell, &coordij_other).unwrap();
        assert_eq!(other_cell, other_index_2);
    }
}
