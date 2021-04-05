use std::result::Result;

use h3ron_h3_sys::H3Index;

use crate::error::Error;
use crate::index::Index;
use crate::HasH3Index;

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

pub fn h3_to_local_ij(origin_index: &Index, index: &Index) -> Result<CoordIJ, Error> {
    unsafe {
        let mut cij = h3ron_h3_sys::CoordIJ { i: 0, j: 0 };
        if h3ron_h3_sys::experimentalH3ToLocalIj(origin_index.h3index(), index.h3index(), &mut cij) == 0 {
            Ok(CoordIJ {
                i: cij.i,
                j: cij.j,
            })
        } else { Err(Error::NoLocalIJCoordinates) }
    }
}

pub fn local_ij_to_h3(origin_index: &Index, coordij: &CoordIJ) -> Result<Index, Error> {
    unsafe {
        let cij = h3ron_h3_sys::CoordIJ {
            i: coordij.i,
            j: coordij.j,
        };
        let mut h3_index_out: H3Index = 0;
        if h3ron_h3_sys::experimentalLocalIjToH3(origin_index.h3index(), &cij, &mut h3_index_out) == 0 {
            Ok(Index::new(h3_index_out))
        } else { Err(Error::NoLocalIJCoordinates) }
    }
}

#[cfg(test)]
mod tests {
    use crate::experimental::{h3_to_local_ij, local_ij_to_h3};
    use crate::index::Index;
    use std::convert::TryFrom;

    #[test]
    fn test_local_ij() {
        let index = Index::try_from(0x89283080ddbffff_u64).unwrap();
        let ring = index.k_ring(1);
        assert_ne!(ring.len(), 0);
        let other = ring.iter().find(|i| **i != index).unwrap().clone();
        let coordij = h3_to_local_ij(&index, &other).unwrap();
        let other2 = local_ij_to_h3(&index, &coordij).unwrap();
        assert_eq!(other, other2);
    }
}
