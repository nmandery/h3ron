use thiserror::Error as DeriveError;

#[derive(Debug, DeriveError)]
pub enum Error {
    #[error(transparent)]
    Polars(#[from] polars::error::PolarsError),
    #[error(transparent)]
    Arrow(#[from] polars::error::ArrowError),
    #[error(transparent)]
    H3ron(#[from] h3ron::Error),
}
