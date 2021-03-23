//! Abstractions over a filesystem directory containing an index.

use crate::{
    tree::{Builder as AsyncBuilder, NotFoundError, Tree as AsyncTree},
    validate::Error as ValidationError,
    Record, WrappedResult,
};
use semver::Version;
use std::{
    future::Future,
    io::Error as IoError,
    path::{Path, PathBuf},
};
use url::Url;

fn block_on<F, T>(future: F) -> T
where
    F: Future<Output = T>,
{
    async_std::task::block_on(future)
}

/// An interface to a crate index directory on the filesystem
#[derive(Debug)]
pub struct Tree {
    async_tree: AsyncTree,
}

/// Builder for creating a new [`Tree`]
#[derive(Debug)]
#[must_use]
pub struct Builder {
    async_builder: AsyncBuilder,
}

impl Builder {
    /// Set the Url for the registry API.
    ///
    /// The API should implement the REST interface as defined in
    /// [the Cargo book](https://doc.rust-lang.org/cargo/reference/registries.html)
    pub fn api(mut self, api: Url) -> Self {
        self.async_builder = self.async_builder.api(api);
        self
    }

    /// Add an allowed registry.
    ///
    /// Crates in this registry are only allowed to have dependencies which are
    /// also in this registry, or in one of the allowed registries.
    ///
    /// Add multiple registries my calling this method multiple times.
    pub fn allowed_registry(mut self, registry: Url) -> Self {
        self.async_builder = self.async_builder.allowed_registry(registry);
        self
    }

    /// Add crates.io as an allowed registry.
    ///
    /// You will almost always want this, so this exists as a handy shortcut.
    pub fn allow_crates_io(mut self) -> Self {
        self.async_builder = self.async_builder.allow_crates_io();
        self
    }

    /// Construct the [`Tree`] with the given parameters.
    ///
    /// # Errors
    ///
    /// This method can fail if the root path doesn't exist, or the filesystem
    /// cannot be written to.
    pub fn build(self) -> Result<Tree, IoError> {
        let async_tree = block_on(self.async_builder.build())?;
        Ok(Tree { async_tree })
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
        let async_builder = AsyncTree::initialise(root.into(), download);
        Builder { async_builder }
    }

    /// Open an existing index tree at the given root path.
    ///
    /// # Errors
    ///
    /// This method can fail if the given path does not exist, or the config
    /// file cannot be read.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self, IoError> {
        let async_tree = block_on(AsyncTree::open(root.into()))?;
        let tree = Self { async_tree };

        Ok(tree)
    }

    /// Insert crate [`Record`] into the index.
    ///
    /// # Errors
    ///
    /// This method can fail if the metadata is deemed to be invalid, or if the
    /// filesystem cannot be written to.
    pub fn insert(
        &mut self,
        crate_metadata: Record,
    ) -> WrappedResult<(), ValidationError, IoError> {
        block_on(self.async_tree.insert(crate_metadata))
    }

    /// Mark a selected version of a crate as 'yanked'.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crate_index::{blocking::tree::Tree, Error, tree::NotFoundError};
    /// #
    /// # fn main() -> Result<(), Error> {
    /// #    let mut tree = Tree::initialise("root", "download")
    /// #        .build()
    /// #        .expect("couldn't create tree");
    /// #
    /// let crate_name = "some-crate";
    /// let version = "0.1.0".parse().unwrap();
    ///
    /// match tree.yank(crate_name, &version)? {
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
    pub fn yank(
        &mut self,
        crate_name: impl Into<String>,
        version: &Version,
    ) -> WrappedResult<(), NotFoundError, IoError> {
        block_on(self.async_tree.yank(crate_name, version))
    }

    /// Mark a selected version of a crate as 'unyanked'.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use crate_index::{blocking::tree::Tree, Error, tree::NotFoundError};
    /// #
    /// # fn main() -> Result<(), Error> {
    /// #    let mut tree = Tree::initialise("root", "download")
    /// #        .build()
    /// #        .expect("couldn't create tree");
    /// #
    /// let crate_name = "some-crate";
    /// let version = "0.1.0".parse().unwrap();
    ///
    /// match tree.unyank(crate_name, &version)? {
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
    pub fn unyank(
        &mut self,
        crate_name: impl Into<String>,
        version: &Version,
    ) -> WrappedResult<(), NotFoundError, IoError> {
        block_on(self.async_tree.unyank(crate_name, version))
    }

    /// The location on the filesystem of the root of the index
    #[must_use]
    pub fn root(&self) -> &Path {
        self.async_tree.root().as_ref()
    }

    /// The Url for downloading .crate files
    #[must_use]
    pub fn download(&self) -> &String {
        self.async_tree.download()
    }

    /// The Url of the API
    #[must_use]
    pub fn api(&self) -> Option<&Url> {
        self.async_tree.api()
    }

    /// The list of registries which crates in this index are allowed to have
    /// dependencies on
    #[must_use]
    pub fn allowed_registries(&self) -> &Vec<Url> {
        self.async_tree.allowed_registries()
    }

    /// Test whether the index contains a particular crate name.
    ///
    /// This method is fast, since the crate names are stored in memory.
    #[must_use]
    pub fn contains_crate(&self, name: impl AsRef<str>) -> bool {
        self.async_tree.contains_crate(name)
    }
}

#[cfg(test)]
mod tests {

    use super::{Record, Tree};
    use crate::Url;
    use semver::Version;
    use std::path::PathBuf;
    use test_case::test_case;

    #[test]
    fn get_and_set() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root: PathBuf = temp_dir.path().into();
        let api = Url::parse("https://my-crates-server.com/").unwrap();

        let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

        let index_tree = Tree::initialise(root.clone(), download)
            .api(api.clone())
            .allowed_registry(Url::parse("https://my-intranet:8080/index").unwrap())
            .allow_crates_io()
            .build()
            .unwrap();

        let expected_allowed_registries = vec![
            Url::parse("https://my-intranet:8080/index").unwrap(),
            Url::parse("https://github.com/rust-lang/crates.io-index").unwrap(),
        ];

        assert_eq!(index_tree.root(), &root);
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
        // create temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

        let initial_metadata = metadata("Some-Name", "0.1.0");

        // create index file and seed with initial metadata
        let mut tree = Tree::initialise(root, download)
            .build()
            .expect("couldn't create index tree");

        tree.insert(initial_metadata)
            .expect("io error")
            .expect("couldn't insert initial metadata");

        // create and insert new metadata
        let new_metadata = metadata(name, version);
        tree.insert(new_metadata)
            .expect("io error")
            .expect("invalid");
    }

    fn metadata(name: &str, version: &str) -> Record {
        Record::new(name, Version::parse(version).unwrap(), "checksum")
    }

    #[test]
    fn open() {
        // create temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

        let initial_metadata = metadata("Some-Name", "0.1.0");

        {
            // create index file and seed with initial metadata
            let mut tree = Tree::initialise(root, download)
                .build()
                .expect("couldn't create index tree");

            tree.insert(initial_metadata)
                .expect("critical error")
                .expect("validation error");
        }

        // reopen the same tree and check crate is there
        let tree = Tree::open(root).expect("couldn't open index tree");
        assert!(tree.contains_crate("Some-Name"))
    }

    #[test_case("Some-Name", "0.1.0"; "when crate exists and version exists")]
    #[test_case("Some-Name", "0.2.0" => panics "not found"; "when crate exists but version doesn't exist")]
    #[test_case("Other-Name", "0.2.0" => panics "not found"; "when crate doesn't exist")]
    fn yank(crate_name: &str, version: &str) {
        let version = Version::parse(version).unwrap();
        // create temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();
        let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

        let initial_metadata = metadata("Some-Name", "0.1.0");

        // create index file and seed with initial metadata
        let mut tree = Tree::initialise(root, download)
            .build()
            .expect("couldn't create tree");

        tree.insert(initial_metadata)
            .unwrap()
            .expect("couldn't insert initial metadata");

        if tree.yank(crate_name, &version).unwrap().is_err() {
            panic!("not found")
        }

        tree.unyank(crate_name, &version).unwrap().unwrap();
    }
}
