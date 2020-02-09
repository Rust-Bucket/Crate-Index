use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt};
use url::Url;

/// Rust crate metadata, as stored in the crate index.
/// 
/// *[See the documentation for details](https://doc.rust-lang.org/cargo/reference/registries.html)*
#[derive(Serialize, Deserialize)]
pub struct Metadata {
    name: String,
    vers: Version,
    deps: Vec<Dependency>,
    cksum: String,
    features: HashMap<String, Vec<String>>,
    yanked: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    links: Option<String>,
}

impl Metadata {
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn version(&self) -> &Version {
        &self.vers
    }

    pub fn dependencies(&self) -> &Vec<Dependency> {
        &self.deps
    }

    pub fn check_sum(&self) -> &String {
        &self.cksum
    }

    pub fn features(&self) -> &HashMap<String, Vec<String>> {
        &self.features
    }

    pub fn yanked(&self) -> bool {
        self.yanked
    }

    pub fn links(&self) -> Option<&String> {
        self.links.as_ref()
    }
}

impl fmt::Display for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", &serde_json::to_string(self).unwrap())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Dependency {
    /// Name of the dependency.
    /// If the dependency is renamed from the original package name,
    /// this is the new name. The original package name is stored in
    /// the `package` field.
    name: String,

    /// The semver requirement for this dependency.
    req: VersionReq,

    /// Array of features (as strings) enabled for this dependency.
    features: Vec<String>,

    /// Boolean of whether or not this is an optional dependency.
    optional: bool,

    /// Boolean of whether or not default features are enabled.
    default_features: bool,

    /// The target platform for the dependency.
    /// null if not a target dependency.
    /// Otherwise, a string such as "cfg(windows)".
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

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum DependencyKind {
    Dev,
    Build,
    Normal,
}
