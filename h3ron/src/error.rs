use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoLocalIJCoordinates,
    InvalidInput,
    InvalidH3Index,
    PentagonalDistortion,
    LineNotComputable,
    MixedResolutions,
    UnsupportedOperation,
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
        }
    }
}

impl std::error::Error for Error {}
