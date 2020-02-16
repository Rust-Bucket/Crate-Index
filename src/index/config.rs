use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url;
use async_std::fs::File;
use async_std::path::Path;
use async_std::io::prelude::{WriteExt, ReadExt};
use async_std::io::BufReader;

/// The index config. this lives at the root of a valid index.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    dl: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    api: Option<Url>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    allowed_registries: Vec<Url>,
}

impl Config {
    /// Create a new [`Config`]
    ///
    /// only the download Url for crates is required. optional values can be set
    /// using the builder methods.
    ///
    /// # Example
    /// ```
    /// use crate_index::{Url, index::Config};
    ///
    /// let download = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
    ///
    /// // Create a new Config struct, setting the url for downloading .crate files
    /// let config = Config::new(download)
    ///
    ///     // Optionally set the URL that cargo should use to publish, yank, etc.
    ///     .with_api(Url::parse("https://my-crates-server.com/").unwrap())
    ///
    ///     // Set registries that crates within this registry are allowed to depend on
    ///     .with_allowed_registry(Url::parse("https://github.com/rust-lang/crates.io-index").unwrap());
    pub fn new(crate_download: impl Into<String>) -> Self {
        let crate_download = crate_download.into();

        debug_assert!(Url::parse(&crate_download).is_ok());

        Self {
            dl: crate_download,
            api: None,
            allowed_registries: Vec::default(),
        }
    }

    /// Set the url of the API.
    pub fn with_api(mut self, api: Url) -> Self {
        self.api = Some(api);
        self
    }

    /// Set crates.io as an allowed registry (you'll almost always want this).
    ///
    /// This is just a handy shortcut.
    pub fn with_crates_io_registry(self) -> Self {
        self.with_allowed_registry(
            Url::parse("https://github.com/rust-lang/crates.io-index").unwrap(),
        )
    }

    /// Set an allowed registry
    pub fn with_allowed_registry(mut self, registry: Url) -> Self {
        self.allowed_registries.push(registry);
        self
    }

    /// The Url for downloading .crate files
    pub fn download(&self) -> &String {
        &self.dl
    }

    /// The Url of the API
    pub fn api(&self) -> &Option<Url> {
        &self.api
    }

    /// The list of registries which crates in this index are allowed to have
    /// dependencies on
    pub fn allowed_registries(&self) -> &Vec<Url> {
        &self.allowed_registries
    }

    pub(crate) async fn to_file(&self, path: impl AsRef<Path>) -> std::io::Result<()> {
        let mut file = File::create(path).await?;
        file.write_all(self.to_string().as_bytes()).await?;

        Ok(())
    }

    pub(crate) async fn from_file(path: impl AsRef<Path>) -> std::io::Result<Self> {
        let mut file = File::open(path).await?;
        let mut reader = BufReader::new(file);
        
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        
        let metadata = serde_json::from_slice(&bytes).expect("malformed json");

        Ok(metadata)
    }
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &serde_json::to_string_pretty(self).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::Config;
    use url::Url;

    #[test]
    fn new() {
        let url = "https://crates.io/api/v1/crates/{crate}/{version}/download";

        let _ = Config::new(url);
    }

    #[test]
    fn allow_crates_io() {
        let config1 =
            Config::new("https://my-crates-server.com/api/v1/crates/{crate}/{version}/download")
                .with_allowed_registry(
                    Url::parse("https://github.com/rust-lang/crates.io-index").unwrap(),
                );

        let config2 =
            Config::new("https://my-crates-server.com/api/v1/crates/{crate}/{version}/download")
                .with_crates_io_registry();

        assert_eq!(config1, config2)
    }

    #[test]
    fn set_and_get() {
        let url = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
        let api = Url::parse("https://my-crates-server.com/").unwrap();
        let registries = vec![
            Url::parse("https://github.com/rust-lang/crates.io-index").unwrap(),
            Url::parse("https://my-intranet:8080/index").unwrap(),
        ];

        let config = Config::new(url)
            .with_api(api.clone())
            .with_allowed_registry(registries[0].clone())
            .with_allowed_registry(registries[1].clone());

        assert_eq!(config.download(), &url);
        assert_eq!(config.api(), &Some(api));
        assert_eq!(config.allowed_registries(), &registries);
    }

    #[test]
    fn format_simple() {
        let url = "https://crates.io/api/v1/crates/{crate}/{version}/download";

        let config = Config::new(url);

        let expected = r#"{
  "dl": "https://crates.io/api/v1/crates/{crate}/{version}/download"
}"#
        .to_string();

        let actual = config.to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn format_full() {
        let url = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
        let api = Url::parse("https://my-crates-server.com/").unwrap();

        let config = Config::new(url)
            .with_api(api)
            .with_allowed_registry(
                Url::parse("https://github.com/rust-lang/crates.io-index").unwrap(),
            )
            .with_allowed_registry(Url::parse("https://my-intranet:8080/index").unwrap());

        let expected: serde_json::Value = serde_json::from_str(
            r#"
            {
                "dl": "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download",
                "api": "https://my-crates-server.com/",
                "allowed-registries": [
                    "https://github.com/rust-lang/crates.io-index",
                    "https://my-intranet:8080/index"
                ]
            }"#,
        )
        .unwrap();

        let actual: serde_json::Value = serde_json::from_str(&config.to_string()).unwrap();

        assert_eq!(expected, actual);
    }
}
