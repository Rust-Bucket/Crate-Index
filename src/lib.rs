#![deny(clippy::all, missing_docs, missing_debug_implementations)]
#![warn(clippy::pedantic)]

//! Crate-Index is a library for managing and manipulating a Cargo crate
//! registry.
//!
//! *see the [cargo docs](https://doc.rust-lang.org/cargo/reference/registries.html#running-a-registry) for details*
//!
//! # Basic Usage
//! ```no_run
//! use crate_index::{Index, Record, Url, Version};
//! # use crate_index::Error;
//!
//! # async {
//! // Create a new index, backed by the filesystem and a git repository
//! let root = "/index";
//! let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
//!
//! let mut index = Index::initialise(root, download).build().await?;
//!
//! // Create a new crate 'Record' object
//! let name = "foo";
//! let version = Version::parse("0.1.0").unwrap();
//! let check_sum = "d867001db0e2b6e0496f9fac96930e2d42233ecd3ca0413e0753d4c7695d289c";
//!
//! let record = Record::new(name, version, check_sum);
//!
//! // Insert the Record into the index
//! index.insert(record).await?;
//!
//! # Ok::<(), Error>(())
//! # };
//! ```
//!
//! # Requirements
//!
//! - Minimum compiler version: **1.46.0**

pub mod record;
#[doc(inline)]
pub use record::Record;

mod index;
pub use index::{git, tree, Builder, Error, Index};

mod utils;
pub mod validate;

pub use semver::Version;
pub use url::Url;

#[cfg(feature = "blocking")]
pub mod blocking;

/// A 'double-wrapped' result type
///
/// This pattern is inspired by [this blog post](http://sled.rs/errors).
///
/// - Outer result type encodes critical application errors that should be
///   propagated upwards. This can be done ergonomically using the `?` operator.
/// - Inner result type encodes 'local' errors which can occur during normal
///   operation and should be explicitly handled (ie not *usually* propagated).
///
/// *This error handling pattern ensures that critical errors, and 'normal'
/// errors are not conflated. This means that errors are more likely to be
/// correctly handled in calling code. The downside is ugly function
/// signatures.*
pub type WrappedResult<T, LocalError, GlobalError> = Result<Result<T, LocalError>, GlobalError>;
