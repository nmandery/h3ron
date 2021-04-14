use std::result::Result;

use h3ron_h3_sys::H3Index;

use crate::error::Error;
use crate::hexagon_index::HexagonIndex;
use crate::Index;

#[derive(Debug, PartialEq, Clone)]
pub struct CoordIJ {
    pub i: i32,
    pub j: i32,
}

impl Default for CoordIJ {
    fn default() -> Self {
        Self { i: 0, j: 0 }
    }
}

pub fn h3_to_local_ij(origin_index: &HexagonIndex, index: &HexagonIndex) -> Result<CoordIJ, Error> {
    unsafe {
        let mut cij = h3ron_h3_sys::CoordIJ { i: 0, j: 0 };
        if h3ron_h3_sys::experimentalH3ToLocalIj(origin_index.h3index(), index.h3index(), &mut cij)
            == 0
        {
            Ok(CoordIJ { i: cij.i, j: cij.j })
        } else {
            Err(Error::NoLocalIJCoordinates)
        }
    }
}

pub fn local_ij_to_h3(
    origin_index: &HexagonIndex,
    coordij: &CoordIJ,
) -> Result<HexagonIndex, Error> {
    unsafe {
        let cij = h3ron_h3_sys::CoordIJ {
            i: coordij.i,
            j: coordij.j,
        };
        let mut h3_index_out: H3Index = 0;
        if h3ron_h3_sys::experimentalLocalIjToH3(origin_index.h3index(), &cij, &mut h3_index_out)
            == 0
        {
            Ok(HexagonIndex::new(h3_index_out))
        } else {
            Err(Error::NoLocalIJCoordinates)
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::experimental::{h3_to_local_ij, local_ij_to_h3};
    use crate::hexagon_index::HexagonIndex;
    use std::convert::TryFrom;

    #[test]
    fn test_local_ij() {
        let origin_index = HexagonIndex::try_from(0x89283080ddbffff_u64).unwrap();
        let ring = origin_index.k_ring(1);
        assert_ne!(ring.len(), 0);
        let other_index = ring.iter().find(|i| **i != origin_index).unwrap().clone();

        // the coordij of the origin index. This is not necessarily at (0, 0)
        let coordij_origin = h3_to_local_ij(&origin_index, &origin_index).unwrap();

        // the coordij of the other index in the coordinate system of the origin index
        let coordij_other = h3_to_local_ij(&origin_index, &other_index).unwrap();

        // As the other_index was taken from k_ring 1, the difference of the i and j coordinates
        // must be -1, 0 or 1
        assert!(
            ((coordij_origin.i - coordij_other.i) >= -1)
                && ((coordij_origin.i - coordij_other.i) <= 1)
        );
        assert!(
            ((coordij_origin.j - coordij_other.j) >= -1)
                && ((coordij_origin.j - coordij_other.j) <= 1)
        );

        // convert the coordij back to an index
        let other_index_2 = local_ij_to_h3(&origin_index, &coordij_other).unwrap();
        assert_eq!(other_index, other_index_2);
    }
}
