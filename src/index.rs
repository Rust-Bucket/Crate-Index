//! This module contains the constituent parts of the [`Index`](crate::Index).
//!
//! In normal usage, it would not be required to use these underlying types.
//! They are exposed here so that can be reused in other crates.

use crate::{validate::Error as ValidationError, Record, Url, WrappedResult};
use async_std::path::PathBuf;
use semver::Version;
use std::io::Error as IoError;

pub mod tree;
use tree::{Builder as TreeBuilder, NotFoundError, Tree};

pub mod git;

use git::{Identity, Repository};

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
#[must_use]
pub struct Builder<'a> {
    tree_builder: TreeBuilder,
    root: PathBuf,
    origin: Option<Url>,
    identity: Option<Identity<'a>>,
}

impl<'a> Builder<'a> {
    // Set the Url for the registry API.
    ///
    /// The API should implement the REST interface as defined in
    /// [the Cargo book](https://doc.rust-lang.org/cargo/reference/registries.html)
    pub fn api(mut self, api: Url) -> Self {
        self.tree_builder = self.tree_builder.api(api);
        self
    }

    /// Add a remote to the repository
    pub fn origin(mut self, remote: Url) -> Self {
        self.origin = Some(remote);
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

    /// Optionally set the username and email for the git repository
    pub fn identity(mut self, username: &'a str, email: &'a str) -> Self {
        self.identity = Some(Identity { username, email });
        self
    }

    /// Construct the [`Index`] with the given parameters.
    ///
    /// # Errors
    ///
    /// This method can fail if the root path doesn't exist, or the filesystem
    /// cannot be written to.
    pub async fn build(self) -> Result<Index, Error> {
        let tree = self.tree_builder.build().await?;
        let repo = Repository::init(self.root)?;

        if let Some(url) = self.origin {
            repo.add_origin(&url)?;
        }

        if let Some(identity) = self.identity {
            repo.set_username(identity.username)?;
            repo.set_email(identity.email)?;
        }

        repo.create_initial_commit()?;

        let index = Index { tree, repo };

        Ok(index)
    }
}

impl Index {
    /// Create a new index.
    ///
    /// The root path, and the URL for downloading .crate files is required.
    /// Additional options can be set using the builder API (see
    /// [`Builder`] for options).
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
    ///
    /// let index = Index::initialise(root, download).build().await?;
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
    /// let index = Index::initialise(root, download)
    ///     .api(Url::parse("https://my-crates-server.com/").unwrap())
    ///     .allowed_registry(Url::parse("https://my-intranet:8080/index").unwrap())
    ///     .allow_crates_io()
    ///     .origin(origin)
    ///     .build()
    ///     .await?;
    /// # Ok::<(), Error>(())
    /// # };
    /// ```
    pub fn initialise<'a>(root: impl Into<PathBuf>, download: impl Into<String>) -> Builder<'a> {
        let root = root.into();
        let tree_builder = Tree::initialise(&root, download);
        let origin = None;
        let identity = None;

        Builder {
            tree_builder,
            root,
            origin,
            identity,
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
    ///
    /// # Errors
    ///
    /// This method can return an error if the filepath doesn't exist, can't be
    /// read from, or if the index is malformed.
    pub async fn open(root: impl Into<PathBuf>) -> Result<Self, Error> {
        let root = root.into();
        let tree = Tree::open(&root).await?;
        let repo = Repository::open(&root)?;

        Ok(Self { tree, repo })
    }

    /// Insert a crate [`Record`] into the index.
    ///
    /// # Errors
    ///
    /// ## Outer Error
    ///
    /// A critical error is returned if the filesystem cannot be read, or a git
    /// error occurs
    ///
    /// ## Inner Error
    ///
    /// A [`ValidationError`] is returned if the crate record contains invalid
    /// data.
    pub async fn insert(&mut self, record: Record) -> WrappedResult<(), ValidationError, Error> {
        let commit_message = format!("updating crate `{}#{}`", record.name(), record.version());
        if let Err(e) = self.tree.insert(record).await? {
            return Ok(Err(e));
        }

        self.repo.add_all()?; //TODO: add just the required path
        self.repo.commit(commit_message)?;
        Ok(Ok(()))
    }

    /// 'Yank' a [`Record`] in the index.
    ///
    /// A 'yanked' crate version should *not* be used as a dependency.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crate_index::{Index, Error, tree::NotFoundError};
    /// #
    /// # #[async_std::main]
    /// # async fn main() -> Result<(), Error> {
    /// #    let mut index = Index::initialise("root", "download")
    /// #        .identity("dummy username", "dummy@email.com")
    /// #        .build()
    /// #        .await
    /// #        .unwrap();
    /// #
    ///     let crate_name = "some-crate";
    ///     let version = "0.1.0".parse().unwrap();
    ///
    ///     match index.yank(crate_name, &version).await? {
    ///         Ok(()) => println!("crate yanked!"),
    ///         Err(NotFoundError::Crate(e)) => println!("crate not found! ({})", e.crate_name()),
    ///         Err(NotFoundError::Version(e)) => println!("version not found! ({})", e.version()),
    ///     }
    /// #
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// ## Outer Error
    ///
    /// A critical error is returned if the filesystem cannot be read, or a git
    /// error occurs
    ///
    /// ## Inner Error
    ///
    /// A [`NotFoundError`] is returned if either the crate or the specified
    /// version can not be found in the index
    pub async fn yank(
        &mut self,
        crate_name: impl Into<String>,
        version: &Version,
    ) -> WrappedResult<(), NotFoundError, Error> {
        let crate_name = crate_name.into();
        let commit_message = format!("yanking crate `{}#{}`", &crate_name, &version);

        Ok(match self.tree.yank(crate_name, version).await? {
            Ok(()) => {
                self.repo.add_all()?; //TODO: add just the required path
                self.repo.commit(commit_message)?;
                Ok(())
            }
            Err(e) => Err(e),
        })
    }

    /// 'Unyank' a [`Record`] in the index.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crate_index::{Index, Error, tree::NotFoundError};
    /// #
    /// # #[async_std::main]
    /// # async fn main() -> Result<(), Error> {
    /// #    let mut index = Index::initialise("root", "download")
    /// #        .identity("dummy username", "dummy@email.com")
    /// #        .build()
    /// #        .await
    /// #        .unwrap();
    /// #
    ///     let crate_name = "some-crate";
    ///     let version = "0.1.0".parse().unwrap();
    ///
    ///     match index.unyank(crate_name, &version).await? {
    ///         Ok(()) => println!("crate unyanked!"),
    ///         Err(NotFoundError::Crate(e)) => println!("crate not found! ({})", e.crate_name()),
    ///         Err(NotFoundError::Version(e)) => println!("version not found! ({})", e.version()),
    ///     }
    /// #
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// ## Outer Error
    ///
    /// A critical error is returned if the filesystem cannot be read, or a git
    /// error occurs
    ///
    /// ## Inner Error
    ///
    /// A [`NotFoundError`] is returned if either the crate or the specified
    /// version can not be found in the index
    pub async fn unyank(
        &mut self,
        crate_name: impl Into<String>,
        version: &Version,
    ) -> WrappedResult<(), NotFoundError, Error> {
        let crate_name = crate_name.into();
        let commit_message = format!("unyanking crate `{}#{}`", &crate_name, &version);

        Ok(match self.tree.unyank(crate_name, version).await? {
            Ok(()) => {
                self.repo.add_all()?;
                self.repo.commit(commit_message)?;
                Ok(())
            }
            Err(e) => Err(e),
        })
    }

    /// The location on the filesystem of the root of the index
    #[must_use]
    pub fn root(&self) -> &PathBuf {
        self.tree.root()
    }

    /// The Url for downloading .crate files
    #[must_use]
    pub fn download(&self) -> &String {
        self.tree.download()
    }

    /// The Url of the API
    #[must_use]
    pub fn api(&self) -> Option<&Url> {
        self.tree.api()
    }

    /// The list of registries which crates in this index are allowed to have
    /// dependencies on
    #[must_use]
    pub fn allowed_registries(&self) -> &Vec<Url> {
        self.tree.allowed_registries()
    }

    /// Split this [`Index`] into its constituent parts
    #[must_use]
    pub fn into_parts(self) -> (Tree, Repository) {
        (self.tree, self.repo)
    }
}

#[derive(Debug, thiserror::Error)]
/// Critical errors for [`Index`] operations
pub enum Error {
    /// filesystem IO error
    #[error("IO Error")]
    Io(#[from] IoError),

    /// libgit2 error
    #[error("Git Error")]
    Git(#[from] git2::Error),
}

#[cfg(test)]
mod tests {
    use super::Index;
    use crate::{index::Record, Url};
    use async_std::path::PathBuf;
    use semver::Version;
    use test_case::test_case;

    #[async_std::test]
    async fn get_and_set() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root: PathBuf = temp_dir.path().into();
        let origin = Url::parse("https://my-git-server.com/").unwrap();

        let api = Url::parse("https://my-crates-server.com/").unwrap();

        let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

        let index = Index::initialise(root.clone(), download)
            .origin(origin)
            .api(api.clone())
            .allowed_registry(Url::parse("https://my-intranet:8080/index").unwrap())
            .allow_crates_io()
            .identity("dummy username", "dummy@email.com")
            .build()
            .await
            .unwrap();

        let expected_allowed_registries = vec![
            Url::parse("https://my-intranet:8080/index").unwrap(),
            Url::parse("https://github.com/rust-lang/crates.io-index").unwrap(),
        ];

        assert_eq!(index.root().as_path(), &root);
        assert_eq!(index.download(), download);
        assert_eq!(index.api(), Some(&api));
        assert_eq!(index.allowed_registries(), &expected_allowed_registries);
    }

    #[test_case("Some-Name", "0.1.1" ; "when used properly")]
    #[test_case("Some_Name", "0.1.1" => panics "invalid" ; "when crate names differ only by hypens and underscores")]
    #[test_case("some_name", "0.1.1" => panics "invalid" ; "when crate names differ only by capitalisation")]
    #[test_case("other-name", "0.1.1" ; "when inserting a different crate")]
    #[test_case("Some-Name", "0.1.0" => panics "invalid"; "when version is the same")]
    #[test_case("Some-Name", "0.0.1" => panics "invalid"; "when version is lower")]
    #[test_case("nul", "0.0.1" => panics "invalid"; "when name is reserved word")]
    #[test_case("-start-with-hyphen", "0.0.1" => panics "invalid"; "when name starts with non-alphabetical character")]
    fn insert(name: &str, version: &str) {
        async_std::task::block_on(async move {
            // create temporary directory
            let temp_dir = tempfile::tempdir().unwrap();
            let root = temp_dir.path();
            let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
            let origin = Url::parse("https://my-git-server.com/").unwrap();

            let initial_metadata = metadata("Some-Name", "0.1.0");

            // create index file and seed with initial metadata
            let mut index = Index::initialise(root, download)
                .origin(origin)
                .identity("dummy username", "dummy@email.com")
                .build()
                .await
                .expect("couldn't create index");

            index
                .insert(initial_metadata)
                .await
                .unwrap()
                .expect("couldn't insert initial metadata");

            // create and insert new metadata
            let new_metadata = metadata(name, version);
            index.insert(new_metadata).await.unwrap().expect("invalid");
        });
    }

    fn metadata(name: &str, version: &str) -> Record {
        Record::new(name, Version::parse(version).unwrap(), "checksum")
    }

    #[test_case("Some-Name", "0.1.0"; "when crate exists and version exists")]
    #[test_case("Some-Name", "0.2.0" => panics "not found"; "when crate exists but version doesn't exist")]
    #[test_case("Other-Name", "0.2.0" => panics "not found"; "when crate doesn't exist")]
    fn yank(crate_name: &str, version: &str) {
        let version = Version::parse(version).unwrap();
        async_std::task::block_on(async {
            let temp_dir = tempfile::tempdir().unwrap();
            let root = temp_dir.path();
            let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

            let initial_metadata = metadata("Some-Name", "0.1.0");

            // create index file and seed with initial metadata
            let mut index = Index::initialise(root, download)
                .identity("dummy username", "dummy@email.com")
                .build()
                .await
                .expect("couldn't create index");

            index
                .insert(initial_metadata)
                .await
                .unwrap()
                .expect("couldn't insert initial metadata");

            if let Err(_) = index.yank(crate_name, &version).await.unwrap() {
                panic!("not found")
            }

            index.unyank(crate_name, &version).await.unwrap().unwrap();
        })
    }
}
