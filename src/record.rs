//! Representations of crate metadata in an index

use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
use url::Url;

/// Rust crate metadata, as stored in the crate index.
///
/// *[See the documentation for details](https://doc.rust-lang.org/cargo/reference/registries.html)*
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    name: String,

    vers: Version,

    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    deps: Vec<Dependency>,

    cksum: String,

    #[serde(skip_serializing_if = "HashMap::is_empty", default)]
    features: HashMap<String, Vec<String>>,

    #[serde(skip_serializing_if = "std::ops::Not::not", default)]
    yanked: bool,

    #[serde(skip_serializing_if = "Option::is_none", default)]
    links: Option<String>,
}

impl Record {
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
    #[must_use]
    pub fn name(&self) -> &String {
        &self.name
    }

    /// The version of the crate
    #[must_use]
    pub fn version(&self) -> &Version {
        &self.vers
    }

    /// A vector of crate [`Dependency`]
    #[must_use]
    pub fn dependencies(&self) -> &Vec<Dependency> {
        &self.deps
    }

    /// A SHA256 checksum of the `.crate` file.
    #[must_use]
    pub fn check_sum(&self) -> &String {
        &self.cksum
    }

    /// Set of features defined for the package.
    ///
    /// Each feature maps to an array of features or dependencies it enables.
    #[must_use]
    pub fn features(&self) -> &HashMap<String, Vec<String>> {
        &self.features
    }

    /// Whether or not this version has been yanked
    #[must_use]
    pub fn yanked(&self) -> bool {
        self.yanked
    }

    /// The `links` string value from the package's manifest
    #[must_use]
    pub fn links(&self) -> Option<&String> {
        self.links.as_ref()
    }

    /// Set the 'yanked' status of the crate version to 'true'
    pub fn yank(&mut self) {
        self.yanked = true;
    }

    /// Set the 'yanked' status of the crate version to 'false'
    pub fn unyank(&mut self) {
        self.yanked = false;
    }
}

impl PartialOrd for Record {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.version().partial_cmp(other.version())
    }
}

impl PartialOrd<Version> for Record {
    fn partial_cmp(&self, other: &Version) -> Option<std::cmp::Ordering> {
        self.version().partial_cmp(other)
    }
}

impl PartialEq<Version> for Record {
    fn eq(&self, other: &Version) -> bool {
        self.version().eq(other)
    }
}

impl fmt::Display for Record {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &serde_json::to_string(self).unwrap())
    }
}

/// A dependency on another crate
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dependency {
    /// Name of the dependency.
    /// If the dependency is renamed from the original package name,
    /// this is the new name. The original package name is stored in
    /// the `package` field.
    name: String,

    /// The semver requirement for this dependency.
    req: VersionReq,

    /// Array of features (as strings) enabled for this dependency.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    features: Vec<String>,

    /// Boolean of whether or not this is an optional dependency.
    optional: bool,

    /// Boolean of whether or not default features are enabled.
    default_features: bool,

    /// The target platform for the dependency.
    /// null if not a target dependency.
    /// Otherwise, a string such as "cfg(windows)".
    #[serde(skip_serializing_if = "Option::is_none", default)]
    target: Option<String>,

    /// The dependency kind.
    /// "dev", "build", or "normal".
    kind: DependencyKind,

    /// The URL of the index of the registry where this dependency is
    /// from as a string. If not specified or null, it is assumed the
    /// dependency is in the current registry.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    registry: Option<Url>,

    /// If the dependency is renamed, this is a string of the actual
    /// package name. If not specified or null, this dependency is not
    /// renamed.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    package: Option<String>,
}

/// Type of crate dependency
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    /// A dependency used only during testing
    Dev,

    /// A dependency only used during building
    Build,

    /// A normal dependency of the crate
    Normal,
}

#[cfg(test)]
mod tests {
    use super::Record;
    use semver::Version;

    #[test]
    fn serialize() {
        let name = "foo";
        let version = Version::parse("0.1.0").unwrap();
        let check_sum = "d867001db0e2b6e0496f9fac96930e2d42233ecd3ca0413e0753d4c7695d289c";

        let metadata = Record::new(name, version, check_sum);

        let expected = r#"{"name":"foo","vers":"0.1.0","cksum":"d867001db0e2b6e0496f9fac96930e2d42233ecd3ca0413e0753d4c7695d289c"}"#.to_string();
        let actual = metadata.to_string();

        assert_eq!(expected, actual);
    }

    #[test]
    fn deserialize() {
        let example1 = r#"
        {
            "name": "foo",
            "vers": "0.1.0",
            "deps": [
                {
                    "name": "rand",
                    "req": "^0.6",
                    "features": ["i128_support"],
                    "optional": false,
                    "default_features": true,
                    "target": null,
                    "kind": "normal",
                    "registry": null,
                    "package": null
                }
            ],
            "cksum": "d867001db0e2b6e0496f9fac96930e2d42233ecd3ca0413e0753d4c7695d289c",
            "features": {
                "extras": ["rand/simd_support"]
            },
            "yanked": false,
            "links": null
        }
        "#;

        serde_json::from_str::<Record>(example1).unwrap();

        let example2 = r#"
        {
            "name": "my_serde",
            "vers": "1.0.11",
            "deps": [
                {
                    "name": "serde",
                    "req": "^1.0",
                    "registry": "https://github.com/rust-lang/crates.io-index",
                    "features": [],
                    "optional": true,
                    "default_features": true,
                    "target": null,
                    "kind": "normal"
                }
            ],
            "cksum": "f7726f29ddf9731b17ff113c461e362c381d9d69433f79de4f3dd572488823e9",
            "features": {
                "default": [
                    "std"
                ],
                "derive": [
                    "serde_derive"
                ],
                "std": [
        
                ]
            },
            "yanked": false
        }
        "#;

        serde_json::from_str::<Record>(example2).unwrap();
    }

    #[test]
    fn yank() {
        let name = "foo";
        let version = Version::parse("0.1.0").unwrap();
        let check_sum = "CHECK_SUM";

        let mut metadata = Record::new(name, version, check_sum);

        assert!(!metadata.yanked());

        metadata.yank();
        assert!(metadata.yanked());

        metadata.unyank();
        assert!(!metadata.yanked());
    }
}
