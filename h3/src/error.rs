use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoLocalIJCoordinates,
    InvalidInput,
    InvalidH3Index,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput => write!(f, "invalid input"),
            Self::InvalidH3Index => write!(f, "invalid h3 index"),
            Self::NoLocalIJCoordinates => write!(f, "no local IJ coordinates found"),
        }
    }
}

impl std::error::Error for Error {}