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
}

impl Error {
    pub fn version(current: &Version, given: Version) -> Self {
        let required = VersionReq::parse(&format!("> {}", current)).unwrap();

        debug_assert!(!required.matches(&given));

        Self::Version { required, given }
    }

    pub fn name_mismatch(expected: impl Into<String>, given: impl Into<String>) -> Self {
        Self::NameMismatch {
            given: given.into(),
            expected: expected.into(),
        }
    }
}

pub fn version(current: &Version, given: &Version) -> Result<(), Error> {
    match given.cmp(current) {
        std::cmp::Ordering::Greater => Ok(()),
        _ => Err(Error::version(current, given.clone())),
    }
}
