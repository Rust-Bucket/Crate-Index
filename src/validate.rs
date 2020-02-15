use semver::{Version, VersionReq};

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Invalid version (required: {required}, given: {given})")]
    Version {
        required: VersionReq,
        given: Version,
    },
}

impl Error {
    pub fn version(current: &Version, given: Version) -> Self {
        let required = VersionReq::parse(&format!("> {}", current)).unwrap();

        debug_assert!(!required.matches(&given));

        Self::Version { required, given }
    }
}

pub fn version(current: &Version, given: &Version) -> Result<(), Error> {
    match given.cmp(current) {
        std::cmp::Ordering::Greater => Ok(()),
        _ => Err(Error::version(current, given.clone())),
    }
}
