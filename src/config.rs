use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    dl: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    api: Option<Url>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    allowed_registries: Vec<Url>,
}

impl Config {
    pub fn new(crate_download: impl Into<String>) -> Self {
        let crate_download = crate_download.into();

        debug_assert!(Url::parse(&crate_download).is_ok());

        Self {
            dl: crate_download,
            api: None,
            allowed_registries: Vec::default(),
        }
    }

    pub fn with_api(mut self, api: Url) -> Self {
        self.api = Some(api);
        self
    }

    pub fn with_allowed_registry(mut self, registry: Url) -> Self {
        self.allowed_registries.push(registry);
        self
    }

    pub fn download(&self) -> &String {
        &self.dl
    }

    pub fn api(&self) -> &Option<Url> {
        &self.api
    }

    pub fn allowed_registries(&self) -> impl Iterator<Item = &Url> {
        self.allowed_registries.iter()
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
    fn set_and_get() {
        let url = "https://my-crates-server.com/api/v1/crates/{crate}/{version}/download";
        let api = Url::parse("https://my-crates-server.com/").unwrap();
        let registries = vec![
            Url::parse("https://github.com/rust-lang/crates.io-index").unwrap(),
            Url::parse("https://my-intranet:8080/index").unwrap(),
        ];

        let config = Config::new(url).with_api(api.clone()).with_allowed_registry(registries[0].clone()).with_allowed_registry(registries[1].clone());

        assert_eq!(config.download(), &url);
        assert_eq!(config.api(), &Some(api));
        assert!(config.allowed_registries().eq(registries.iter()));
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
