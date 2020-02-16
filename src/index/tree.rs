use super::Config;
use crate::{
    index::{IndexFile, Metadata},
    Result,
};
use async_std::path::PathBuf;
use std::io;
use url::Url;

/// An interface to a crate index directory on the filesystem
pub struct Tree {
    root: PathBuf,
    config: Config,
}

/// Builder for creating a new [`Tree`]
pub struct TreeBuilder {
    root: PathBuf,
    config: Config,
}

impl TreeBuilder {
    /// Set the Url for the registry API.
    ///
    /// The API should implement the REST interface as defined in [the Cargo book](https://doc.rust-lang.org/cargo/reference/registries.html)
    pub fn api(mut self, api: Url) -> Self {
        self.config = self.config.with_api(api);
        self
    }

    /// Add an allowed registry.
    ///
    /// Crates in this registry are only allowed to have dependencies which are
    /// also in this registry, or in one of the allowed registries.
    ///
    /// Add multiple registries my calling this method multiple times.
    pub fn allowed_registry(mut self, registry: Url) -> Self {
        self.config = self.config.with_allowed_registry(registry);
        self
    }

    /// Add crates.io as an allowed registry.
    ///
    /// You will almost always want this, so this exists as a handy shortcut.
    pub fn allow_crates_io(mut self) -> Self {
        self.config = self.config.with_crates_io_registry();
        self
    }

    pub async fn build(self) -> io::Result<Tree> {
        // once 'IntoFuture' is stabilised, this 'build' method should be replaced with
        // an 'IntoFuture' implementation so that the builder can be awaited directly
        Tree::new(self.root, self.config).await
    }
}

impl Tree {
    /// Create a new index `Tree`.
    ///
    /// The root path, and the URL for downloading .crate files is required.
    /// Additional options can be set using the builder API (see
    /// [`TreeBuilder`] for options).
    ///
    /// # Example
    ///
    /// ## Basic Config
    ///
    /// ```no_run
    /// use crate_index::index::Tree;
    /// # use crate_index::Error;
    /// # async {
    /// let root = "/index";
    /// let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
    ///
    /// let index_tree = Tree::init(root, download).build().await?;
    /// # Ok::<(), Error>(())
    /// # };
    /// ```
    ///
    /// ## More Options
    ///
    /// ```no_run
    /// use crate_index::{index::Tree, Url};
    /// # use crate_index::Error;
    /// # async {
    /// let root = "/index";
    /// let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
    ///
    ///
    /// let index_tree = Tree::init(root, download)
    ///     .api(Url::parse("https://my-crates-server.com/").unwrap())
    ///     .allowed_registry(Url::parse("https://my-intranet:8080/index").unwrap())
    ///     .allow_crates_io()
    ///     .build()
    ///     .await?;
    /// # Ok::<(), Error>(())
    /// # };
    /// ```
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

    /// Open an existing index tree at the given root path.
    ///
    /// # Errors
    ///
    /// This method can fail if the given path does not exist, or the config
    /// file cannot be read.
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

    /// The Url for downloading .crate files
    pub fn download(&self) -> &String {
        self.config.download()
    }

    /// The Url of the API
    pub fn api(&self) -> &Option<Url> {
        self.config.api()
    }

    /// The list of registries which crates in this index are allowed to have
    /// dependencies on
    pub fn allowed_registries(&self) -> &Vec<Url> {
        self.config.allowed_registries()
    }
}
