//! crate record validation

use lazy_static::lazy_static;
use regex::Regex;
use semver::{Version, VersionReq};

/// The error returned when a crate record is invalid
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// The [`Record`](crate::Record) version is not valid
    #[error("Invalid version (required: {required}, given: {given})")]
    Version {
        /// The version requirement
        required: VersionReq,
        /// The given version
        given: Version,
    },

    /// The name of the crate is not valid
    #[error("Crate name '{name}' is invalid: {reason}")]
    InvalidName {
        /// the given crate name
        name: String,
        /// the reason the crate name is invalid
        reason: String,
    },
}

impl Error {
    pub(crate) fn version(current: &Version, given: Version) -> Self {
        let required = VersionReq::parse(&format!("> {}", current)).unwrap();

        debug_assert!(!required.matches(&given));

        Self::Version { required, given }
    }

    pub(crate) fn invalid_name(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidName {
            name: name.into(),
            reason: reason.into(),
        }
    }
}

fn is_allowed_name(name: &str) -> bool {
    let disallowed_names = vec!["nul"];

    !disallowed_names.contains(&name)
}

pub(crate) fn name(name: &str) -> Result<(), Error> {
    lazy_static! {
        static ref REGEX: Regex = Regex::new("^[a-zA-Z][a-zA-Z0-9-_]*$").unwrap();
    }

    if name.is_empty() {
        Err(Error::invalid_name(name, "crate name cannot be empty"))
    } else if !is_allowed_name(name) {
        Err(Error::invalid_name(name, "crate name is blacklisted"))
    } else if !REGEX.is_match(name) || !name.is_ascii() {
        Err(Error::invalid_name(
            name,
            "crate name must be ASCII, be alphanumeric + '-' and '_', and begin with a letter \
             ([a-zA-Z][a-zA-Z0-9-_]*).",
        ))
    } else {
        Ok(())
    }
}
