use thiserror::Error as ThisError;

#[derive(ThisError, Debug)]
pub enum Error {
    #[error("h3ron error: {0}")]
    H3ron(#[from] h3ron::Error),

    #[error("io error: {0}")]
    IOError(#[from] std::io::Error),

    #[error("mixed h3 resolutions: {0} <> {1}")]
    MixedH3Resolutions(u8, u8),

    #[error("too high h3 resolution: {0}")]
    TooHighH3Resolution(u8),

    #[error("empty path")]
    EmptyPath,

    #[error("none of the routing destinations is part of the routing graph")]
    DestinationsNotInGraph,

    #[error("other error: {0}")]
    Other(String),

    #[error("insufficient number of edges")]
    InsufficientNumberOfEdges,

    #[error("path is segmented into multiple parts")]
    SegmentedPath,
}
