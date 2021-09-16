//! Abstractions over a filesystem directory containing an index.

use crate::{index::Record, utils, validate::Error as ValidationError, WrappedResult};
use async_std::path::PathBuf;
use semver::Version;
use std::{collections::HashSet, io::Error as IoError};
use url::Url;

mod file;
use file::IndexFile;
pub use file::VersionNotFoundError;

mod config;
use config::Config;

/// An interface to a crate index directory on the filesystem
#[derive(Debug)]
pub struct Tree {
    root: PathBuf,
    config: Config,
    crates: HashSet<String>,
}

/// Builder for creating a new [`Tree`]
#[derive(Debug)]
#[must_use]
pub struct Builder {
    root: PathBuf,
    config: Config,
}

impl Builder {
    /// Set the Url for the registry API.
    ///
    /// The API should implement the REST interface as defined in
    /// [the Cargo book](https://doc.rust-lang.org/cargo/reference/registries.html)
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

    /// Construct the [`Tree`] with the given parameters.
    ///
    /// # Errors
    ///
    /// This method can fail if the root path doesn't exist, or the filesystem
    /// cannot be written to.
    pub async fn build(self) -> Result<Tree, IoError> {
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
    /// [`Builder`] for options).
    ///
    /// # Example
    ///
    /// ## Basic Config
    ///
    /// ```no_run
    /// use crate_index::tree::Tree;
    /// # use crate_index::Error;
    /// # async {
    /// let root = "/index";
    /// let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
    ///
    /// let index_tree = Tree::initialise(root, download).build().await?;
    /// # Ok::<(), Error>(())
    /// # };
    /// ```
    ///
    /// ## More Options
    ///
    /// ```no_run
    /// use crate_index::{tree::Tree, Url};
    /// # use crate_index::Error;
    /// # async {
    /// let root = "/index";
    /// let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
    ///
    /// let index_tree = Tree::initialise(root, download)
    ///     .api(Url::parse("https://my-crates-server.com/").unwrap())
    ///     .allowed_registry(Url::parse("https://my-intranet:8080/index").unwrap())
    ///     .allow_crates_io()
    ///     .build()
    ///     .await?;
    /// # Ok::<(), Error>(())
    /// # };
    /// ```
    pub fn initialise(root: impl Into<PathBuf>, download: impl Into<String>) -> Builder {
        let root = root.into();
        let config = Config::new(download);
        Builder { root, config }
    }

    pub(crate) async fn new(root: PathBuf, config: Config) -> Result<Self, IoError> {
        config.to_file(root.join("config.json")).await?;

        let crates = HashSet::default();

        let tree = Self {
            root,
            config,
            crates,
        };

        Ok(tree)
    }

    /// Open an existing index tree at the given root path.
    ///
    /// # Errors
    ///
    /// This method can fail if the given path does not exist, or the config
    /// file cannot be read.
    pub async fn open(root: impl Into<PathBuf>) -> Result<Self, IoError> {
        let root = root.into();
        let config = Config::from_file(root.join("config.json")).await?;
        let crates = utils::crate_names(&root).await?;

        let tree = Self {
            root,
            config,
            crates,
        };

        Ok(tree)
    }

    async fn file(&self, crate_name: impl Into<String>) -> Result<IndexFile, IoError> {
        IndexFile::open(self.root(), crate_name).await
    }

    /// Insert a crate [`Record`] into the index.
    ///
    /// # Errors
    ///
    /// ## Outer Error
    ///
    /// an [`IoError`] is returned if the filesystem cannot be read or written
    /// to.
    ///
    /// ## Inner Error
    ///
    /// a [`ValidationError`] is returned if the inserted metadata is not valid.
    ///
    /// This can occur if the name contains invalid characters, or if the crate
    /// name is too similar to an existing crate.
    pub async fn insert(
        &mut self,
        crate_metadata: Record,
    ) -> WrappedResult<(), ValidationError, IoError> {
        if let Err(e) = self.validate_name(crate_metadata.name()) {
            return Ok(Err(e));
        }

        let crate_name = crate_metadata.name().clone();

        // open the index file for editing
        let mut index_file = self.file(&crate_name).await?;

        // insert the new metadata
        if let Err(e) = index_file.insert(crate_metadata).await? {
            return Ok(Err(e));
        }

        self.crates.insert(crate_name);

        Ok(Ok(()))
    }

    /// Mark a selected version of a crate as 'yanked'.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crate_index::{tree::{Tree, NotFoundError}, Error};
    /// #
    /// # #[async_std::main]
    /// # async fn main() -> Result<(), Error> {
    /// #    let mut tree = Tree::initialise("root", "download")
    /// #        .build()
    /// #        .await
    /// #        .expect("couldn't create tree");
    /// #
    /// let crate_name = "some-crate";
    /// let version = "0.1.0".parse().unwrap();
    ///
    /// match tree.yank(crate_name, &version).await? {
    ///     Ok(()) => println!("crate yanked!"),
    ///     Err(NotFoundError::Crate(e)) => println!("crate not found! ({})", e.crate_name()),
    ///     Err(NotFoundError::Version(e)) => println!("version not found! ({})", e.version()),
    /// }
    /// #
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// ## Outer Error
    ///
    /// an [`IoError`] is returned if the filesystem cannot be read or written
    /// to.
    ///
    /// ## Inner Error
    ///
    /// This function will return [`NotFoundError`] if the crate or the
    /// selected version does not exist in the index.
    pub async fn yank(
        &mut self,
        crate_name: impl Into<String>,
        version: &Version,
    ) -> WrappedResult<(), NotFoundError, IoError> {
        let crate_name = crate_name.into();
        if self.crates.contains(&crate_name) {
            Ok(self
                .file(crate_name)
                .await?
                .yank(version)
                .await?
                .map_err(NotFoundError::from))
        } else {
            Ok(Err(NotFoundError::no_crate(crate_name)))
        }
    }

    /// Mark a selected version of a crate as 'unyanked'.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crate_index::{tree::{Tree, NotFoundError}, Error};
    /// #
    /// # #[async_std::main]
    /// # async fn main() -> Result<(), Error> {
    /// #    let mut tree = Tree::initialise("root", "download")
    /// #        .build()
    /// #        .await
    /// #        .expect("couldn't create tree");
    /// #
    /// let crate_name = "some-crate";
    /// let version = "0.1.0".parse().unwrap();
    ///
    /// match tree.unyank(crate_name, &version).await? {
    ///     Ok(()) => println!("crate unyanked!"),
    ///     Err(NotFoundError::Crate(e)) => println!("crate not found! ({})", e.crate_name()),
    ///     Err(NotFoundError::Version(e)) => println!("version not found! ({})", e.version()),
    /// }
    /// #
    /// #     Ok(())
    /// # }
    /// ```
    ///
    /// # Errors
    ///
    /// ## Outer Error
    ///
    /// an [`IoError`] is returned if the filesystem cannot be read or written
    /// to.
    ///
    /// ## Inner Error
    ///
    /// This function will return [`NotFoundError`] if the crate or the
    /// selected version does not exist in the index.
    pub async fn unyank(
        &mut self,
        crate_name: impl Into<String>,
        version: &Version,
    ) -> WrappedResult<(), NotFoundError, IoError> {
        let crate_name = crate_name.into();
        if self.crates.contains(&crate_name) {
            Ok(self
                .file(crate_name)
                .await?
                .unyank(version)
                .await?
                .map_err(NotFoundError::from))
        } else {
            Ok(Err(NotFoundError::no_crate(crate_name)))
        }
    }

    /// The location on the filesystem of the root of the index
    #[must_use]
    pub fn root(&self) -> &PathBuf {
        &self.root
    }

    /// The Url for downloading .crate files
    #[must_use]
    pub fn download(&self) -> &String {
        self.config.download()
    }

    /// The Url of the API
    #[must_use]
    pub fn api(&self) -> Option<&Url> {
        self.config.api()
    }

    /// The list of registries which crates in this index are allowed to have
    /// dependencies on
    #[must_use]
    pub fn allowed_registries(&self) -> &Vec<Url> {
        self.config.allowed_registries()
    }

    /// Test whether the index contains a particular crate name.
    ///
    /// This method is fast, since the crate names are stored in memory.
    pub fn contains_crate(&self, name: impl AsRef<str>) -> bool {
        self.crates.contains(name.as_ref())
    }

    fn contains_crate_canonical(&self, name: impl AsRef<str>) -> bool {
        let name = canonicalise(name);
        self.crates.iter().map(canonicalise).any(|x| x == name)
    }

    fn validate_name(&self, name: impl AsRef<str>) -> Result<(), ValidationError> {
        let name = name.as_ref();
        if self.contains_crate_canonical(name) && !self.contains_crate(name) {
            Err(ValidationError::invalid_name(
                name,
                "name is too similar to existing crate",
            ))
        } else {
            Ok(())
        }
    }
}

fn canonicalise(name: impl AsRef<str>) -> String {
    name.as_ref().to_lowercase().replace('-', "_")
}

/// The error raised when a given crate does not exist in the index
#[derive(Debug, Clone, thiserror::Error)]
#[error("crate not found (no data in index for {crate_name})")]
pub struct CrateNotFoundError {
    crate_name: String,
}

impl CrateNotFoundError {
    /// The name of the crate
    #[must_use]
    pub fn crate_name(&self) -> &String {
        &self.crate_name
    }
}
/// Recoverable [`Tree`] errors.
///
/// # Example
/// ```
/// # use crate_index::tree::NotFoundError;
/// # let error = NotFoundError::no_crate("some-crate");
/// #
/// match error {
///     NotFoundError::Crate(e) => println!("couldn't find crate {}", e.crate_name()),
///     NotFoundError::Version(e) => println!(
///         "found crate {} but no version matching {}",
///         e.crate_name(),
///         e.version()
///     ),
/// }
/// ```
#[derive(Debug, Clone, thiserror::Error)]
pub enum NotFoundError {
    /// The error type thrown when the requested crate cannot be found in the
    /// index
    #[error(transparent)]
    Crate(#[from] CrateNotFoundError),

    /// The error type thrown when the requested crate version can not be found
    /// in the index
    #[error(transparent)]
    Version(#[from] VersionNotFoundError),
}

impl NotFoundError {
    /// No crate found with this name
    ///
    /// # Example
    /// ```
    /// use crate_index::tree::NotFoundError;
    ///
    /// let error = NotFoundError::no_crate("some-crate");
    /// ```
    pub fn no_crate(crate_name: impl Into<String>) -> Self {
        Self::Crate(CrateNotFoundError {
            crate_name: crate_name.into(),
        })
    }

    /// Crate was found, but no matching version
    ///
    /// # Example
    /// ```
    /// use crate_index::tree::NotFoundError;
    ///
    /// let version = "0.1.0".parse().unwrap();
    ///
    /// let error = NotFoundError::no_version("some-crate", version);
    /// ```
    pub fn no_version(crate_name: impl Into<String>, version: Version) -> Self {
        Self::Version(VersionNotFoundError {
            crate_name: crate_name.into(),
            version,
        })
    }
}

#[cfg(test)]
mod tests {

    use super::{Record, Tree};
    use crate::Url;
    use async_std::path::PathBuf;
    use semver::Version;
    use std::collections::HashSet;
    use test_case::test_case;

    #[async_std::test]
    async fn get_and_set() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root: PathBuf = temp_dir.path().into();
        let api = Url::parse("https://my-crates-server.com/").unwrap();

        let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

        let index_tree = Tree::initialise(root.clone(), download)
            .api(api.clone())
            .allowed_registry(Url::parse("https://my-intranet:8080/index").unwrap())
            .allow_crates_io()
            .build()
            .await
            .unwrap();

        let expected_allowed_registries = vec![
            Url::parse("https://my-intranet:8080/index").unwrap(),
            Url::parse("https://github.com/rust-lang/crates.io-index").unwrap(),
        ];

        assert_eq!(index_tree.root().as_path(), &root);
        assert_eq!(index_tree.download(), download);
        assert_eq!(index_tree.api(), Some(&api));
        assert_eq!(
            index_tree.allowed_registries(),
            &expected_allowed_registries
        );
    }

    #[test_case("Some-Name", "0.1.1" ; "when used properly")]
    #[test_case("Some_Name", "0.1.1" => panics "invalid" ; "when crate names differ only by hyphens and underscores")]
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

            let initial_metadata = metadata("Some-Name", "0.1.0");

            // create index file and seed with initial metadata
            let mut tree = Tree::initialise(root, download)
                .build()
                .await
                .expect("couldn't create index tree");

            tree.insert(initial_metadata)
                .await
                .unwrap()
                .expect("couldn't insert initial metadata");

            // create and insert new metadata
            let new_metadata = metadata(name, version);
            tree.insert(new_metadata).await.unwrap().expect("invalid");
        });
    }

    fn metadata(name: &str, version: &str) -> Record {
        Record::new(name, Version::parse(version).unwrap(), "checksum")
    }

    #[async_std::test]
    async fn open() {
        // create temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

        let initial_metadata = metadata("Some-Name", "0.1.0");

        {
            // create index file and seed with initial metadata
            let mut tree = Tree::initialise(root, download)
                .build()
                .await
                .expect("couldn't create index tree");

            tree.insert(initial_metadata)
                .await
                .unwrap()
                .expect("couldn't insert initial metadata");
        }

        // reopen the same tree and check crate is there
        let tree = Tree::open(root).await.expect("couldn't open index tree");
        assert!(tree.contains_crate("Some-Name"));

        // check there aren't any extra files in there
        let mut before_names = HashSet::new();
        before_names.insert("Some-Name".to_string());
        assert_eq!(before_names, tree.crates);
    }

    #[test_case("Some-Name", "0.1.0"; "when crate exists and version exists")]
    #[test_case("Some-Name", "0.2.0" => panics "not found"; "when crate exists but version doesn't exist")]
    #[test_case("Other-Name", "0.2.0" => panics "not found"; "when crate doesn't exist")]
    fn yank(crate_name: &str, version: &str) {
        let version = Version::parse(version).unwrap();
        async_std::task::block_on(async {
            // create temporary directory
            let temp_dir = tempfile::tempdir().unwrap();
            let root = temp_dir.path();
            let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

            let initial_metadata = metadata("Some-Name", "0.1.0");

            // create index file and seed with initial metadata
            let mut tree = Tree::initialise(root, download)
                .build()
                .await
                .expect("couldn't create tree");

            tree.insert(initial_metadata)
                .await
                .unwrap()
                .expect("couldn't insert initial metadata");

            if tree.yank(crate_name, &version).await.unwrap().is_err() {
                panic!("not found");
            }

            tree.unyank(crate_name, &version).await.unwrap().unwrap();
        });
    }
}
