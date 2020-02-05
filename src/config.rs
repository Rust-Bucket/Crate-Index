use serde::{Deserialize, Serialize};
use std::fmt;
use url::Url;

#[derive(Serialize, Deserialize)]
pub struct Config {
    dl: Url,

    #[serde(skip_serializing_if = "Option::is_none")]
    api: Option<Url>,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    allowed_registries: Vec<Url>,
}

impl Config {
    pub fn new(crate_download: Url) -> Self {
        Self {
            dl: crate_download,
            api: None,
            allowed_registries: Vec::default(),
        }
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
        let url = Url::parse("https://crates.io/api/v1/crates/{crate}/{version}/download")
            .expect("URL is invalid!");

        let _ = Config::new(url);
    }
}
