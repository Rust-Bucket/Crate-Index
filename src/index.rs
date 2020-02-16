use super::Metadata;
use crate::{Result, Url};
use async_std::{fs::File, io::prelude::WriteExt, path::PathBuf};
use std::io;

mod index_file;
use index_file::IndexFile;

mod config;
pub use config::Config;

mod tree;
use tree::{Tree, TreeBuilder};

pub struct Index {
    tree: Tree,
}

pub struct IndexBuilder {
    tree_builder: TreeBuilder,
}

impl IndexBuilder {
    pub fn api(mut self, api: Url) -> Self {
        self.tree_builder = self.tree_builder.api(api);
        self
    }

    pub fn allowed_registry(mut self, registry: Url) -> Self {
        self.tree_builder = self.tree_builder.allowed_registry(registry);
        self
    }

    pub fn allow_crates_io(mut self) -> Self {
        self.tree_builder = self.tree_builder.allow_crates_io();
        self
    }

    pub async fn build(self) -> Result<Index> {
        let tree = self.tree_builder.build().await?;

        let index = Index { tree };

        Ok(index)
    }
}

impl Index {
    /// Create a new index.
    ///
    /// The root path, and the URL for downloading .crate files is required.
    /// Additional options can be set using the builder API (see
    /// [`IndexBuilder`] for options).
    ///
    /// # Example
    ///
    /// ## Basic Config
    /// ```no_run
    /// use crate_index::Index;
    /// # use crate_index::Error;
    /// # async {
    /// let root = "/index";
    /// let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
    ///
    /// let index = Index::init(root, download).build().await?;
    /// # Ok::<(), Error>(())
    /// # };
    /// ```
    /// ## More Options
    ///
    /// ```no_run
    /// use crate_index::{Index, Url};
    /// # use crate_index::Error;
    /// # async {
    /// let root = "/index";
    /// let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
    ///
    ///
    /// let index = Index::init(root, download)
    ///     .api(Url::parse("https://my-crates-server.com/").unwrap())
    ///     .allowed_registry(Url::parse("https://my-intranet:8080/index").unwrap())
    ///     .allow_crates_io()
    ///     .build()
    ///     .await?;
    /// # Ok::<(), Error>(())
    /// # };
    /// ```
    pub fn init(root: impl Into<PathBuf>, download: impl Into<String>) -> IndexBuilder {
        let tree_builder = Tree::init(root, download);

        IndexBuilder { tree_builder }
    }

    /// Open an existing index at the given root path.
    ///
    /// # Example
    /// ```no_run
    /// use crate_index::Index;
    /// # use crate_index::Error;
    /// # async {
    /// let root = "/index";
    ///
    /// let index = Index::open("/index").await?;
    /// # Ok::<(), Error>(())
    /// # };
    /// ```
    pub async fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let tree = Tree::open(root).await?;

        Ok(Self { tree })
    }

    /// Insert crate ['Metadata'] into the index.
    ///
    /// # Errors
    ///
    /// This method can fail if the metadata is deemed to be invalid, or if the
    /// filesystem cannot be written to.
    pub async fn insert(&self, crate_metadata: Metadata) -> Result<()> {
        self.tree.insert(crate_metadata).await
    }

    /// The location on the filesystem of the root of the index
    pub fn root(&self) -> &PathBuf {
        self.tree.root()
    }
}

#[cfg(test)]
mod tests {}
