use crate::validate;
use std::io;

/// This [`Error`] represents anything that can go wrong with this library
#[derive(Debug, thiserror::Error)]
pub enum Error {

    /// Metadata validation error
    #[error("Validation Error")]
    Validation(#[from] validate::Error),

    /// filesystem IO error
    #[error("IO Error")]
    Io(#[from] io::Error),
}

/// The result type for fallible functions in this library
pub type Result<T> = std::result::Result<T, Error>;
