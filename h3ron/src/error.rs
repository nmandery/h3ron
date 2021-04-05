use std::fmt;
use crate::{Index, H3_MAX_RESOLUTION};
use std::convert::TryFrom;
use h3ron_h3_sys::H3Index;

#[derive(Debug)]
pub enum Error {
    NoLocalIJCoordinates,
    InvalidInput,
    InvalidH3Index,
    PentagonalDistortion,
    LineNotComputable,
    MixedResolutions,
    UnsupportedOperation,
    InvalidH3Resolution,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => write!(f, "invalid input"),
            Self::InvalidH3Index => write!(f, "invalid h3ron index"),
            Self::NoLocalIJCoordinates => write!(f, "no local IJ coordinates found"),
            Self::PentagonalDistortion => write!(f, "pentagonal distortion"),
            Self::LineNotComputable => write!(f, "line is not computable"),
            Self::MixedResolutions => write!(f, "mixed h3 resolutions"),
            Self::UnsupportedOperation => write!(f, "unsupported operation"),
            Self::InvalidH3Resolution => write!(f, "invalid h3 resolution"),
        }
    }
}

impl std::error::Error for Error {}

/// ensure two indexes have the same resolution
pub fn check_same_resolution(index0: H3Index, index1: H3Index) -> Result<(), Error> {
    if Index::try_from(index0)?.resolution() != Index::try_from(index1)?.resolution() {
        Err(Error::MixedResolutions)
    } else {
        Ok(())
    }
}

/// ensure the given resolution is valid
pub fn check_valid_h3_resolution(h3_res: u8) -> Result<(), Error> {
    if h3_res > H3_MAX_RESOLUTION {
        Err(Error::InvalidH3Resolution)
    } else {
        Ok(())
    }
}

