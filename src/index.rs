use super::Metadata;
use crate::{Error, Result, Url};
use async_std::path::PathBuf;
use futures::future::TryFutureExt;

mod index_file;
use index_file::IndexFile;

mod config;
pub use config::Config;

mod tree;
pub use tree::Tree;
use tree::TreeBuilder;

pub use crate::git::Repository;

/// A representation of a crates registry, backed by both a directory and a git
/// repository on the filesystem.
///
/// This struct is essentially a thin wrapper around both an index [`Tree`] and
/// a git [`Repository`].
///
/// It functions exactly the same way as a [`Tree`], except that all changes to
/// the crates index are also committed to the git repository, which allows this
/// to be synced to a remote.
pub struct Index {
    tree: Tree,
    repo: Repository,
}

/// A builder for initialising a new [`Index`]
pub struct IndexBuilder {
    tree_builder: TreeBuilder,
    root: PathBuf,
    origin: Url,
}

impl IndexBuilder {
    // Set the Url for the registry API.
    ///
    /// The API should implement the REST interface as defined in
    /// [the Cargo book](https://doc.rust-lang.org/cargo/reference/registries.html)
    pub fn api(mut self, api: Url) -> Self {
        self.tree_builder = self.tree_builder.api(api);
        self
    }

    /// Add an allowed registry.
    ///
    /// Crates in this registry are only allowed to have dependencies which are
    /// also in this registry, or in one of the allowed registries.
    ///
    /// Add multiple registries my calling this method multiple times.
    pub fn allowed_registry(mut self, registry: Url) -> Self {
        self.tree_builder = self.tree_builder.allowed_registry(registry);
        self
    }

    /// Add crates.io as an allowed registry.
    ///
    /// You will almost always want this, so this exists as a handy shortcut.
    pub fn allow_crates_io(mut self) -> Self {
        self.tree_builder = self.tree_builder.allow_crates_io();
        self
    }

    /// Construct the [`Index`] with the given parameters.
    ///
    /// # Errors
    ///
    /// This method can fail if the root path doesn't exist, or the filesystem
    /// cannot be written to.
    pub async fn build(self) -> Result<Index> {
        let tree = self.tree_builder.build().await?;
        let repo = Repository::init(self.root, self.origin)?;

        let index = Index { tree, repo };

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
    /// # use crate_index::{Error, Url};
    /// # async {
    /// let root = "/index";
    /// let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
    /// let origin = Url::parse("https://github.com/crates/index.git").unwrap();
    ///
    /// let index = Index::init(root, download, origin).build().await?;
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
    /// let origin = Url::parse("https://github.com/crates/index.git").unwrap();
    ///
    ///
    /// let index = Index::init(root, download, origin)
    ///     .api(Url::parse("https://my-crates-server.com/").unwrap())
    ///     .allowed_registry(Url::parse("https://my-intranet:8080/index").unwrap())
    ///     .allow_crates_io()
    ///     .build()
    ///     .await?;
    /// # Ok::<(), Error>(())
    /// # };
    /// ```
    pub fn init(
        root: impl Into<PathBuf>,
        download: impl Into<String>,
        origin: Url,
    ) -> IndexBuilder {
        let root = root.into();
        let tree_builder = Tree::init(&root, download);

        IndexBuilder {
            tree_builder,
            root,
            origin,
        }
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
        let root = root.into();
        let tree_fut = Tree::open(&root).map_err(Error::from);
        let repo_fut = async { Repository::open(&root).map_err(Error::from) };

        let (tree, repo) = futures::try_join!(tree_fut, repo_fut)?;

        Ok(Self { tree, repo })
    }

    /// Insert crate ['Metadata'] into the index.
    ///
    /// # Errors
    ///
    /// This method can fail if the metadata is deemed to be invalid, or if the
    /// filesystem cannot be written to.
    pub async fn insert(&self, crate_metadata: Metadata) -> Result<()> {
        let commit_message = format!(
            "updating crate `{}#{}`",
            crate_metadata.name(),
            crate_metadata.version()
        );
        self.tree.insert(crate_metadata).await?;
        self.repo.add_all()?; //TODO: add just the required path
        self.repo.commit(commit_message)?;
        Ok(())
    }

    /// The location on the filesystem of the root of the index
    pub fn root(&self) -> &PathBuf {
        self.tree.root()
    }
}

#[cfg(test)]
mod tests {}
