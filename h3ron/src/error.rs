use thiserror::Error as DeriveError;

use crate::{H3Cell, Index, H3_MAX_RESOLUTION};

/// Errors as defines in [RFC] 4.0.0, extended with some custom errors of this wrapper library.
///
/// [RFC]: https://github.com/uber/h3/blob/master/dev-docs/RFCs/v4.0.0/error-handling-rfc.md
#[derive(Debug, DeriveError)]
pub enum Error {
    /// The operation failed but a more specific error is not available
    #[error("operation failed")]
    Failed, // 1

    /// Argument was outside of acceptable range (when a more specific error code is not available)
    #[error("Argument was outside of acceptable range")]
    Domain, // 2

    /// Latitude or longitude arguments were outside of acceptable range
    #[error("Latitude or longitude arguments were outside of acceptable range")]
    LatLonDomain, // 3

    /// Resolution argument was outside of acceptable range
    #[error("Resolution argument was outside of acceptable range")]
    ResDomain, // 4

    /// H3Index cell argument was not valid
    #[error("H3Index cell argument was not valid")]
    CellInvalid, // 5

    /// H3Index directed edge argument was not valid
    #[error("H3Index directed edge argument was not valid")]
    DirectedEdgeInvalid, // 6

    /// H3Index undirected edge argument was not valid
    #[error("H3Index undirected edge argument was not valid")]
    UndirectedEdgeInvalid, // 7

    /// H3Index vertex argument was not valid
    #[error("H3Index vertex argument was not valid")]
    VertexInvalid, // 8

    /// Pentagon distortion was encountered which the algorithm could not handle it
    #[error("Pentagon distortion was encountered")]
    Pentagon, // 9

    /// Duplicate input was encountered in the arguments and the algorithm could not handle it
    #[error("Duplicate input was encountered in the arguments")]
    DuplicateInput, // 10

    /// H3Index cell arguments were not neighbors
    #[error("H3Index cell arguments were not neighbors")]
    NotNeighbors, // 11

    /// H3Index cell arguments had incompatible resolutions
    #[error("H3Index cell arguments had incompatible resolutions")]
    ResMismatch, // 12

    /// Necessary memory allocation failed
    #[error("Necessary memory allocation failed")]
    Memory, // 13

    /// Bounds of provided memory were not large enough
    #[error("Bounds of provided memory were not large enough")]
    MemoryBounds, // 14

    /// Unknown error code
    #[error("Unknown h3 error code")]
    UnknownError(u32),

    /// Invalid H3 direction
    #[error("Invalid H3 direction")]
    DirectionInvalid(u8),

    /// io error. The io error is always part of the enum
    /// regardless if the `io` feature is enabled to avoid having
    /// different variations of this enum depending on the selected
    /// feature flags.
    #[error("io error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("decompression error")]
    DecompressionError(String),
}

impl Error {
    /// returns the corresponding error for the given error code
    const fn get_error(value: u32) -> Self {
        match value {
            1 => Self::Failed,
            2 => Self::Domain,
            3 => Self::LatLonDomain,
            4 => Self::ResDomain,
            5 => Self::CellInvalid,
            6 => Self::DirectedEdgeInvalid,
            7 => Self::UndirectedEdgeInvalid,
            8 => Self::VertexInvalid,
            9 => Self::Pentagon,
            10 => Self::DuplicateInput,
            11 => Self::NotNeighbors,
            12 => Self::ResMismatch,
            13 => Self::Memory,
            14 => Self::MemoryBounds,
            v => Self::UnknownError(v),
        }
    }

    /// Checks if the H3 return value is an error
    #[inline(always)]
    pub const fn is_error(value: u32) -> bool {
        value != 0
    }

    /// checks the return code of h3ron-h3-sys functions
    #[inline(always)]
    pub const fn check_returncode(value: u32) -> Result<(), Self> {
        if Self::is_error(value) {
            Err(Self::get_error(value))
        } else {
            Ok(())
        }
    }
}

/// Ensure two cells have the same resolution
pub fn check_same_resolution(cell0: H3Cell, cell1: H3Cell) -> Result<(), Error> {
    let res0 = cell0.resolution();
    let res1 = cell1.resolution();
    if res0 == res1 {
        Ok(())
    } else {
        Err(Error::ResMismatch)
    }
}

/// Ensure the given resolution is valid
pub const fn check_valid_h3_resolution(h3_res: u8) -> Result<(), Error> {
    if h3_res > H3_MAX_RESOLUTION {
        Err(Error::ResDomain)
    } else {
        Ok(())
    }
}
