use git2::Error;
use std::path::Path;
use url::Url;

pub struct Repository {}

impl Repository {
    pub fn init(root: impl AsRef<Path>, origin: Url) -> Result<Self, Error> {
        unimplemented!()
    }

    pub fn open(root: impl AsRef<Path>) -> Result<Self, Error> {
        unimplemented!()
    }

    pub fn add_path(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        unimplemented!()
    }

    pub fn add_all(&self) -> Result<(), Error> {
        unimplemented!()
    }

    pub fn commit(&self, message: impl AsRef<str>) -> Result<(), Error> {
        unimplemented!()
    }

    pub fn push(&self) -> Result<(), Error> {
        unimplemented!()
    }
}
