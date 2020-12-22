use std::fmt;

#[derive(Debug)]
pub enum Error {
    TransformNotInvertible,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransformNotInvertible => write!(f, "transform is not invertible"),
        }
    }
}

impl std::error::Error for Error {}
