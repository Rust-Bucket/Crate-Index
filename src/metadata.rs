use async_std::path::PathBuf;
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
use url::Url;

/// Rust crate metadata, as stored in the crate index.
///
/// *[See the documentation for details](https://doc.rust-lang.org/cargo/reference/registries.html)*
#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    name: String,
    vers: Version,

    #[serde(skip_serializing_if = "Vec::is_empty")]
    deps: Vec<Dependency>,
    cksum: String,

    #[serde(skip_serializing_if = "HashMap::is_empty")]
    features: HashMap<String, Vec<String>>,
    yanked: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    links: Option<String>,
}

impl Metadata {
    /// Create a new metadata object.
    ///
    /// The method parameters are all required, optional parameters can be set
    /// using the builder API.
    pub fn new(name: impl Into<String>, version: Version, check_sum: impl Into<String>) -> Self {
        let name = name.into();
        let vers = version;
        let deps = Vec::new();
        let cksum = check_sum.into();
        let features = HashMap::new();
        let yanked = false;
        let links = None;

        Self {
            name,
            vers,
            deps,
            cksum,
            features,
            yanked,
            links,
        }
    }

    /// The name of the crate
    pub fn name(&self) -> &String {
        &self.name
    }

    /// The version of the crate
    pub fn version(&self) -> &Version {
        &self.vers
    }

    /// A vector of crate [`Dependency`]
    pub fn dependencies(&self) -> &Vec<Dependency> {
        &self.deps
    }

    /// A SHA256 checksum of the `.crate` file.
    pub fn check_sum(&self) -> &String {
        &self.cksum
    }

    /// Set of features defined for the package.
    ///
    /// Each feature maps to an array of features or dependencies it enables.
    pub fn features(&self) -> &HashMap<String, Vec<String>> {
        &self.features
    }

    /// Whether or not this version has been yanked
    pub fn yanked(&self) -> bool {
        self.yanked
    }

    /// The `links` string value from the package's manifest
    pub fn links(&self) -> Option<&String> {
        self.links.as_ref()
    }

    pub(crate) fn path(&self) -> PathBuf {
        get_path(self.name())
    }
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &serde_json::to_string(self).unwrap())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Dependency {
    /// Name of the dependency.
    /// If the dependency is renamed from the original package name,
    /// this is the new name. The original package name is stored in
    /// the `package` field.
    name: String,

    /// The semver requirement for this dependency.
    req: VersionReq,

    /// Array of features (as strings) enabled for this dependency.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    features: Vec<String>,

    /// Boolean of whether or not this is an optional dependency.
    optional: bool,

    /// Boolean of whether or not default features are enabled.
    default_features: bool,

    /// The target platform for the dependency.
    /// null if not a target dependency.
    /// Otherwise, a string such as "cfg(windows)".
    #[serde(skip_serializing_if = "Option::is_none")]
    target: Option<String>,

    /// The dependency kind.
    /// "dev", "build", or "normal".
    kind: DependencyKind,

    /// The URL of the index of the registry where this dependency is
    /// from as a string. If not specified or null, it is assumed the
    /// dependency is in the current registry.
    #[serde(skip_serializing_if = "Option::is_none")]
    registry: Option<Url>,

    /// If the dependency is renamed, this is a string of the actual
    /// package name. If not specified or null, this dependency is not
    /// renamed.
    #[serde(skip_serializing_if = "Option::is_none")]
    package: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DependencyKind {
    /// A dependency used only during testing
    Dev,

    /// A dependency only used during building
    Build,

    /// A normal dependency of the crate
    Normal,
}

fn get_path(name: &str) -> PathBuf {
    let mut path = PathBuf::new();

    let name_lowercase = name.to_ascii_lowercase();

    match name.len() {
        1 => {
            path.push("1");
            path.push(name);
            path
        }
        2 => {
            path.push("2");
            path.push(name);
            path
        }
        3 => {
            path.push("3");
            path.push(&name_lowercase[0..1]);
            path.push(name);
            path
        }
        _ => {
            path.push(&name_lowercase[0..2]);
            path.push(&name_lowercase[2..4]);
            path.push(name);
            path
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Metadata;
    use semver::Version;
    use test_case::test_case;

    #[test]
    fn serialize() {
        let name = "foo";
        let version = Version::parse("0.1.0").unwrap();
        let check_sum = "d867001db0e2b6e0496f9fac96930e2d42233ecd3ca0413e0753d4c7695d289c";

        let metadata = Metadata::new(name, version, check_sum);

        let expected = r#"{"name":"foo","vers":"0.1.0","cksum":"d867001db0e2b6e0496f9fac96930e2d42233ecd3ca0413e0753d4c7695d289c","yanked":false}"#.to_string();
        let actual = metadata.to_string();

        assert_eq!(expected, actual)
    }

    #[test_case("x" => "1/x" ; "one-letter crate name")]
    #[test_case("xx" => "2/xx" ; "two-letter crate name")]
    #[test_case("xxx" =>"3/x/xxx" ; "three-letter crate name")]
    #[test_case("abcd" => "ab/cd/abcd" ; "four-letter crate name")]
    #[test_case("abcde" => "ab/cd/abcde" ; "five-letter crate name")]
    #[test_case("aBcD" => "ab/cd/aBcD" ; "mixed-case crate name")]
    fn get_path(name: &str) -> String {
        super::super::get_path(name).to_str().unwrap().to_string()
    }
}
