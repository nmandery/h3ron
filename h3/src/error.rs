use std::fmt;

#[derive(Debug)]
pub enum Error {
    NoLocalIJCoordinates
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "no local IJ coordinates found")
    }
}

impl std::error::Error for Error {}