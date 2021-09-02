use thiserror::Error as DeriveError;

use h3ron_h3_sys::H3Index;

use crate::{H3Cell, Index, H3_MAX_RESOLUTION};

#[derive(Debug, DeriveError)]
pub enum Error {
    #[error("Local IJ coordinates not found")]
    NoLocalIjCoordinates,
    #[error("Invalid input")]
    InvalidInput,
    #[error("Invalid H3 Cell index {0:x}")]
    InvalidH3Cell(H3Index),
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

    /// io error. The io error is always part of the enum
    /// regardless if the `io` feature is enabled to avoid having
    /// different variations of this enum depending on the selected
    /// feature flags.
    #[error("io error: {0}")]
    IOError(#[from] std::io::Error),
}

/// Ensure two cells have the same resolution
pub fn check_same_resolution(cell0: H3Cell, cell1: H3Cell) -> Result<(), Error> {
    let res0 = cell0.resolution();
    let res1 = cell1.resolution();
    if res0 != res1 {
        Err(Error::MixedResolutions(res0, res1))
    } else {
        Ok(())
    }
}

/// Ensure the given resolution is valid
pub fn check_valid_h3_resolution(h3_res: u8) -> Result<(), Error> {
    if h3_res > H3_MAX_RESOLUTION {
        Err(Error::InvalidH3Resolution(h3_res))
    } else {
        Ok(())
    }
}
