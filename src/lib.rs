#![feature(type_alias_impl_trait)]
#![deny(clippy::all)]
#![warn(clippy::pedantic, missing_docs)]

mod error;
pub use error::{Error, Result};

mod config;
use config::Config;

mod metadata;
pub use metadata::Metadata;

mod index;
pub use index::Index;

mod validate;

pub use url::Url;
