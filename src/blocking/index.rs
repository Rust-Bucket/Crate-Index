//! This module contains the constituent parts of the [`Index`](crate::Index).
//!
//! In normal usage, it would not be required to use these underlying types.
//! They are exposed here so that can be reused in other crates.

mod tree;
pub use tree::{Builder as TreeBuilder, Tree};
