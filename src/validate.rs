use semver::{Version, VersionReq};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ValidationError {
    #[error("Invalid version (required: {required}, given: {given})")]
    Version {
        required: VersionReq,
        given: Version,
    },
}

impl ValidationError {
    pub fn version(current: &Version, given: Version) -> Self {
        let required = VersionReq::parse(&format!("> {}", current)).unwrap();

        debug_assert!(!required.matches(&given));

        Self::Version { required, given }
    }
}

pub fn validate_version(current: &Version, given: &Version) -> Result<(), ValidationError> {
    match given.cmp(current) {
        std::cmp::Ordering::Greater => Ok(()),
        _ => Err(ValidationError::version(current, given.clone())),
    }
}
