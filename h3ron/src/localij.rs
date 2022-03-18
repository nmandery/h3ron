use std::ops::{Add, Sub};
use std::result::Result;

use h3ron_h3_sys::H3Index;

use crate::error::Error;
use crate::H3Cell;
use crate::Index;

/// [`CoordIj`] coordinates for [`H3Cell`] anchored by an origin [`H3Cell`].
///
/// Thus coordinate space may have deleted regions or warping due
/// to pentagonal distortion.
///
/// This values are not guaranteed
/// to be compatible across different versions of H3.
#[derive(Debug, PartialEq, Copy, Clone, Default)]
pub struct CoordIj {
    pub i: i32,
    pub j: i32,
}

impl Sub for CoordIj {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            i: self.i - rhs.i,
            j: self.j - rhs.j,
        }
    }
}

impl Add for CoordIj {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            i: self.i + rhs.i,
            j: self.j + rhs.j,
        }
    }
}

impl From<h3ron_h3_sys::CoordIJ> for CoordIj {
    fn from(sys_cij: h3ron_h3_sys::CoordIJ) -> Self {
        Self {
            i: sys_cij.i,
            j: sys_cij.j,
        }
    }
}

impl Into<h3ron_h3_sys::CoordIJ> for CoordIj {
    fn into(self) -> h3ron_h3_sys::CoordIJ {
        h3ron_h3_sys::CoordIJ {
            i: self.i,
            j: self.j,
        }
    }
}

impl H3Cell {
    /// Produces an [`H3Cell`] for the given `coordij` coordinates anchored by `origin_cell`.
    ///
    /// The coordinate space used by this function may have deleted
    /// regions or warping due to pentagonal distortion.
    ///
    /// Failure may occur if the index is too far away from the origin
    /// or if the index is on the other side of a pentagon.
    ///
    /// This function's output is not guaranteed
    /// to be compatible across different versions of H3.
    pub fn from_localij(origin_cell: Self, coordij: CoordIj) -> Result<Self, Error> {
        let cij: h3ron_h3_sys::CoordIJ = coordij.into();
        let mut h3_index_out: H3Index = 0;
        Error::check_returncode(unsafe {
            h3ron_h3_sys::localIjToCell(origin_cell.h3index(), &cij, 0, &mut h3_index_out)
        })?;
        Ok(Self::new(h3_index_out))
    }

    /// Produces [`CoordIj`] coordinates for this [`H3Cell`] instance anchored by an origin [`H3Cell`] `origin_cell`.
    ///
    /// The coordinate space used by this function may have deleted
    /// regions or warping due to pentagonal distortion.
    ///
    /// Failure may occur if the index is too far away from the origin
    /// or if the index is on the other side of a pentagon.
    ///
    /// This function's output is not guaranteed
    /// to be compatible across different versions of H3.
    pub fn to_localij(&self, origin_cell: Self) -> Result<CoordIj, Error> {
        let mut cij = h3ron_h3_sys::CoordIJ { i: 0, j: 0 };
        Error::check_returncode(unsafe {
            h3ron_h3_sys::cellToLocalIj(origin_cell.h3index(), self.h3index(), 0, &mut cij)
        })?;
        Ok(cij.into())
    }
}

#[cfg(test)]
mod tests {
    use crate::H3Cell;

    #[test]
    fn test_local_ij() {
        let origin_cell = H3Cell::try_from(0x89283080ddbffff_u64).unwrap();
        let ring = origin_cell.grid_disk(1).unwrap();
        assert_ne!(ring.iter().count(), 0);
        let other_cell = ring.iter().find(|i| *i != origin_cell).unwrap();

        // the coordij of the origin index. This is not necessarily at (0, 0)
        let coordij_origin = origin_cell.to_localij(origin_cell).unwrap();

        // the coordij of the other index in the coordinate system of the origin index
        let coordij_other = other_cell.to_localij(origin_cell).unwrap();

        // As the other_index was taken from k_ring 1, the difference of the i and j coordinates
        // must be -1, 0 or 1
        let coordij_diff = coordij_origin - coordij_other;
        assert!(coordij_diff.i.abs() <= 1);
        assert!(coordij_diff.j.abs() <= 1);

        // convert the coordij back to an index
        let other_cell_2 = H3Cell::from_localij(origin_cell, coordij_other).unwrap();
        assert_eq!(other_cell, other_cell_2);
    }
}
