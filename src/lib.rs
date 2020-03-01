#![deny(clippy::all)]
#![warn(clippy::pedantic, missing_docs)]

mod error;
pub use error::{Error, Result};

mod metadata;
pub use metadata::Metadata;

pub mod index;
pub use index::Index;

mod validate;

mod git;

mod utils;

pub use url::Url;
