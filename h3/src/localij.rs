
use h3_sys::H3Index;
use std::result::Result;
use crate::error::Error;

#[derive(Debug)]
pub struct CoordIJ {
    pub i: i32,
    pub j: i32,
}

impl Default for CoordIJ {
    fn default() -> Self {
        Self { i: 0, j: 0 }
    }
}

pub fn h3_to_local_ij(origin_index: H3Index, h3_index: H3Index) -> Result<CoordIJ, Error> {
    unsafe {
        let mut cij = h3_sys::CoordIJ { i: 0, j: 0, };
        if h3_sys::experimentalH3ToLocalIj(origin_index, h3_index, &mut cij) == 0 {
            Ok(CoordIJ {
                i: cij.i,
                j: cij.j
            })
        }
        else { Err(Error::NoLocalIJCoordinates) }
    }
}

pub fn local_ij_to_h3(origin_index: H3Index, coordij: &CoordIJ) -> Result<H3Index, Error> {
    unsafe {
        let cij = h3_sys::CoordIJ {
            i: coordij.i,
            j: coordij.j
        };
        let mut h3_index_out: H3Index = 0;
        if h3_sys::experimentalLocalIjToH3(origin_index, &cij, &mut h3_index_out) == 0 {
            Ok(h3_index_out)
        }
        else { Err(Error::NoLocalIJCoordinates) }
    }
}

#[cfg(test)]
mod tests {
    use crate::k_ring;
    use crate::localij::{h3_to_local_ij, local_ij_to_h3};

    #[test]
    fn test_local_ij() {
        let h3index = 0x89283080ddbffff_u64;
        let ring = k_ring(h3index, 1);
        assert_ne!(ring.len(), 0);
        let other = ring.iter().find(|i| **i != h3index).unwrap().clone();
        let coordij = h3_to_local_ij(h3index, other).unwrap();
        let other2 = local_ij_to_h3(h3index, &coordij).unwrap();
        assert_eq!(other, other2);
    }

}