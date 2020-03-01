use lazy_static::lazy_static;
use regex::Regex;
use semver::{Version, VersionReq};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid version (required: {required}, given: {given})")]
    Version {
        required: VersionReq,
        given: Version,
    },

    #[error("Crate name mismatch (expected: {expected}, given: {given}")]
    NameMismatch { expected: String, given: String },

    #[error("Crate name '{name}' is invalid: {reason}")]
    InvalidName { name: String, reason: String },
}

impl Error {
    pub(crate) fn version(current: &Version, given: Version) -> Self {
        let required = VersionReq::parse(&format!("> {}", current)).unwrap();

        debug_assert!(!required.matches(&given));

        Self::Version { required, given }
    }

    pub(crate) fn name_mismatch(expected: impl Into<String>, given: impl Into<String>) -> Self {
        Self::NameMismatch {
            given: given.into(),
            expected: expected.into(),
        }
    }

    pub(crate) fn invalid_name(name: impl Into<String>, reason: impl Into<String>) -> Self {
        Self::InvalidName {
            name: name.into(),
            reason: reason.into(),
        }
    }
}

pub(crate) fn version(current: &Version, given: &Version) -> Result<(), Error> {
    match given.cmp(current) {
        std::cmp::Ordering::Greater => Ok(()),
        _ => Err(Error::version(current, given.clone())),
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
            "crate name must be ASCII, be alphanumeric + '-' and '_', and begin with a letter ([a-zA-Z][a-zA-Z0-9-_]*).",
        ))
    } else {
        Ok(())
    }
}
