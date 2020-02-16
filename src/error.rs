use crate::validate;
use std::io;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Validation Error")]
    Validation(#[from] validate::Error),

    #[error("IO Error")]
    Io(#[from] io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;
