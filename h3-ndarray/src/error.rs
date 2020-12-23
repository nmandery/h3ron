use std::fmt;

#[derive(Debug)]
pub enum Error {
    TransformNotInvertible,
    EmptyArray,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TransformNotInvertible => write!(f, "transform is not invertible"),
            Self::EmptyArray => write!(f, "empty array"),
        }
    }
}

impl std::error::Error for Error {}
