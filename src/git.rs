use std::path::Path;
use url::Url;

pub struct Repository {

}

impl Repository {
    pub fn init(root: impl AsRef<Path>, origin: Url) -> Result<Self, ()> {
        unimplemented!()
    }

    pub fn open(root: impl AsRef<Path>) -> Result<Self, ()> {
        unimplemented!()
    }

    pub fn add_path(&self, path: impl AsRef<Path>) -> Result<(),()> {
        unimplemented!()
    }

    pub fn add_all(&self) -> Result<(),()> {
        unimplemented!()
    }

    pub fn commit(&self, message: impl AsRef<str>) -> Result<(),()> {
        unimplemented!()
    }

    pub fn push(&self) -> Result<(),()> {
        unimplemented!()
    }
}