use super::Config;
use crate::{
    index::{IndexFile, Metadata},
    validate, Error, Result,
};
use async_std::path::PathBuf;
use std::io;
use url::Url;

/// An interface to a crate index directory on the filesystem
pub struct Tree {
    root: PathBuf,
    config: Config,
}

pub struct TreeBuilder {
    root: PathBuf,
    config: Config,
}

impl TreeBuilder {
    pub fn api(mut self, api: Url) -> Self {
        self.config = self.config.with_api(api);
        self
    }

    pub fn allowed_registry(mut self, registry: Url) -> Self {
        self.config = self.config.with_allowed_registry(registry);
        self
    }

    pub fn allow_crates_io(mut self) -> Self {
        self.config = self.config.with_crates_io_registry();
        self
    }

    pub async fn build(self) -> io::Result<Tree> {
        Tree::new(self.root, self.config).await
    }
}

impl Tree {
    pub fn init(root: impl Into<PathBuf>, download: impl Into<String>) -> TreeBuilder {
        let root = root.into();
        let config = Config::new(download);
        TreeBuilder { root, config }
    }

    async fn new(root: PathBuf, config: Config) -> io::Result<Self> {
        config.to_file(root.join("config.json")).await?;

        let tree = Self { root, config };

        Ok(tree)
    }

    pub async fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        let config = Config::from_file(&root).await?;

        let tree = Self { root, config };

        Ok(tree)
    }

    /// Insert crate ['Metadata'] into the index.
    ///
    /// # Errors
    ///
    /// This method can fail if the metadata is deemed to be invalid, or if the
    /// filesystem cannot be written to.
    pub async fn insert(&self, crate_metadata: Metadata) -> Result<()> {
        // open the index file for editing
        let mut index_file = IndexFile::open(self.root(), crate_metadata.name()).await?;

        // insert the new metadata
        index_file.insert(crate_metadata).await
    }

    /// The location on the filesystem of the root of the index
    pub fn root(&self) -> &PathBuf {
        &self.root
    }
}
