use std::convert::TryFrom;
use thiserror::Error as DeriveError;

use h3ron_h3_sys::H3Index;

use crate::{H3Cell, Index, H3_MAX_RESOLUTION};

#[derive(Debug, DeriveError)]
pub enum Error {
    #[error("Local IJ coordinates not found")]
    NoLocalIJCoordinates,
    #[error("Invalid input")]
    InvalidInput,
    #[error("Invalid H3 Hexagon index {0:x}")]
    InvalidH3Hexagon(H3Index),
    #[error("Invalid H3 Edge index {0:x}")]
    InvalidH3Edge(H3Index),
    #[error("Pentagonal distortion")]
    PentagonalDistortion,
    #[error("Line is not computable")]
    LineNotComputable,
    #[error("Mixed H3 resolutions: {0} and {1}")]
    MixedResolutions(u8, u8),
    #[error("Unsupported operation")]
    UnsupportedOperation,
    #[error("Invalid H3 resolution: {0}")]
    InvalidH3Resolution(u8),
    #[error("Invalid H3 direction bit {0}")]
    InvalidH3Direction(u8),
}

/// ensure two indexes have the same resolution
pub fn check_same_resolution(index0: H3Index, index1: H3Index) -> Result<(), Error> {
    let res0 = H3Cell::try_from(index0)?.resolution();
    let res1 = H3Cell::try_from(index1)?.resolution();
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
