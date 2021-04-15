use crate::{HexagonIndex, Index, H3_MAX_RESOLUTION};
use h3ron_h3_sys::H3Index;
use std::convert::TryFrom;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoLocalIJCoordinates,
    InvalidInput,
    InvalidH3Hexagon(H3Index),
    InvalidH3Edge(H3Index),
    PentagonalDistortion,
    LineNotComputable,
    MixedResolutions(u8, u8),
    UnsupportedOperation,
    InvalidH3Resolution(u8),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => write!(f, "invalid input"),
            Self::InvalidH3Hexagon(i) => write!(f, "invalid h3ron hexagon index {:x}", i),
            Self::InvalidH3Edge(i) => write!(f, "invalid h3ron edge index {:x}", i),
            Self::NoLocalIJCoordinates => write!(f, "no local IJ coordinates found"),
            Self::PentagonalDistortion => write!(f, "pentagonal distortion"),
            Self::LineNotComputable => write!(f, "line is not computable"),
            Self::MixedResolutions(r1, r2) => write!(f, "mixed h3 resolutions: {} and {}", r1, r2),
            Self::UnsupportedOperation => write!(f, "unsupported operation"),
            Self::InvalidH3Resolution(r) => write!(f, "invalid h3 resolution: {}", r),
        }
    }
}

impl std::error::Error for Error {}

/// ensure two indexes have the same resolution
pub fn check_same_resolution(index0: H3Index, index1: H3Index) -> Result<(), Error> {
    let res0 = HexagonIndex::try_from(index0)?.resolution();
    let res1 = HexagonIndex::try_from(index1)?.resolution();
    if res0 != res1 {
        Err(Error::MixedResolutions(res0, res1))
    } else {
        Ok(())
    }
}

/// ensure the given resolution is valid
pub fn check_valid_h3_resolution(h3_res: u8) -> Result<(), Error> {
    if h3_res > H3_MAX_RESOLUTION {
        Err(Error::InvalidH3Resolution(h3_res))
    } else {
        Ok(())
    }
}
