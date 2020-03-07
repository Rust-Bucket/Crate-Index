use super::Config;
use crate::{
    index::{IndexFile, Metadata},
    utils,
    validate::Error as ValidationError,
    Result,
};
use async_std::path::PathBuf;
use std::{collections::HashSet, io};
use url::Url;

/// An interface to a crate index directory on the filesystem
pub struct Tree {
    root: PathBuf,
    config: Config,
    crates: HashSet<String>,
}

/// Builder for creating a new [`Tree`]
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
    /// [`Builder`] for options).
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
    pub fn init(root: impl Into<PathBuf>, download: impl Into<String>) -> Builder {
        let root = root.into();
        let config = Config::new(download);
        Builder { root, config }
    }

    async fn new(root: PathBuf, config: Config) -> io::Result<Self> {
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
    pub async fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        let config = Config::from_file(root.join("config.json")).await?;
        let crates = utils::filenames(&root).await?;

        let tree = Self {
            root,
            config,
            crates,
        };

        Ok(tree)
    }

    /// Insert crate ['Metadata'] into the index.
    ///
    /// # Errors
    ///
    /// This method can fail if the metadata is deemed to be invalid, or if the
    /// filesystem cannot be written to.
    pub async fn insert(&mut self, crate_metadata: Metadata) -> Result<()> {
        self.validate_name(crate_metadata.name())?;

        let crate_name = crate_metadata.name().clone();

        // open the index file for editing
        let mut index_file = IndexFile::open(self.root(), &crate_name).await?;

        // insert the new metadata
        index_file.insert(crate_metadata).await?;

        self.crates.insert(crate_name);

        Ok(())
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

    fn validate_name(&self, name: impl AsRef<str>) -> std::result::Result<(), ValidationError> {
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

#[cfg(test)]
mod tests {

    use super::{Metadata, Tree};
    use crate::Url;
    use async_std::path::PathBuf;
    use semver::Version;
    use test_case::test_case;

    #[async_std::test]
    async fn get_and_set() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root: PathBuf = temp_dir.path().into();
        let api = Url::parse("https://my-crates-server.com/").unwrap();

        let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";

        let index_tree = Tree::init(root.clone(), download)
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
        assert_eq!(index_tree.api(), &Some(api));
        assert_eq!(
            index_tree.allowed_registries(),
            &expected_allowed_registries
        );
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

            let initial_metadata = metadata("Some-Name", "0.1.0");

            // create index file and seed with initial metadata
            let mut tree = Tree::init(root, download)
                .build()
                .await
                .expect("couldn't create index tree");

            tree.insert(initial_metadata)
                .await
                .expect("couldn't insert initial metadata");

            // create and insert new metadata
            let new_metadata = metadata(name, version);
            tree.insert(new_metadata).await.expect("invalid");
        });
    }

    fn metadata(name: &str, version: &str) -> Metadata {
        Metadata::new(name, Version::parse(version).unwrap(), "checksum")
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
            let mut tree = Tree::init(root.clone(), download)
                .build()
                .await
                .expect("couldn't create index tree");

            tree.insert(initial_metadata)
                .await
                .expect("couldn't insert initial metadata");
        }

        // reopen the same tree and check crate is there
        let tree = Tree::open(root).await.expect("couldn't open index tree");
        assert!(tree.contains_crate("Some-Name"))
    }
}
