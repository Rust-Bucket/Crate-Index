use super::{validate::ValidationError, Config, Metadata};
use async_std::{
    fs::File,
    io::prelude::WriteExt,
    path::{Path, PathBuf},
};
use std::io;
use thiserror::Error;
use url::Url;

mod index_file;
use index_file::IndexFile;

pub struct Index {
    root: PathBuf,
    config: Config,
}

impl Index {
    /// Create a new `Index`.
    ///
    /// # Parameters
    ///
    /// - *root*: The path on the filesystem at which the root of the index is
    ///   located
    /// - *download*- This is the URL for downloading crates listed in the
    ///   index. The value may have the markers {crate} and {version} which are
    ///   replaced with the name and version of the crate to download. If the
    ///   markers are not present, then the value /{crate}/{version}/download is
    ///   appended to the end.
    ///
    /// This method does not touch the filesystem. use [`init()`](Index::init)
    /// to initialise the index in the filesystem.
    pub fn new(root: impl Into<PathBuf>, download: Url) -> Self {
        let root = root.into();
        let config = Config::new(download);
        Self { root, config }
    }

    /// Initialise an index at the root path.
    ///
    /// # Example
    /// ```no_run
    /// use cargo_registry::{Index, Url};
    /// # async {
    /// let root = "/index";
    /// let download_url = Url::parse("https://crates.io/api/v1/crates/").unwrap();
    ///
    /// let index = Index::new(root, download_url);
    /// index.init().await?;
    /// # Ok::<(), std::io::Error>(())
    /// # };
    /// ```
    pub async fn init(&self) -> io::Result<()> {
        async_std::fs::DirBuilder::new()
            .recursive(true)
            .create(&self.root)
            .await?;
        let mut file = File::create(&self.root.join("config.json")).await?;
        file.write_all(self.config.to_string().as_bytes()).await?;

        Ok(())
    }

    /// Insert crate ['Metadata'] into the index.
    ///
    /// # Errors
    ///
    /// This method can fail if the metadata is deemed to be invalid, or if the
    /// filesystem cannot be written to.
    pub async fn insert(&self, crate_metadata: Metadata) -> Result<(), IndexError> {
        // get the full path to the index file
        let path = self.get_path(crate_metadata.name());

        // create the parent directories to the file
        create_parents(&path).await?;

        // open the index file for editing
        let mut file = IndexFile::open(&path).await?;

        // insert the new metadata
        file.insert(crate_metadata).await?;

        Ok(())
    }

    fn get_path(&self, name: &str) -> PathBuf {
        let stem = get_path(name);
        self.root.join(stem)
    }
}

#[derive(Debug, Error)]
pub enum IndexError {
    #[error("Validation Error")]
    Validation(#[from] ValidationError),

    #[error("IO Error")]
    Io(#[from] io::Error),
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

async fn create_parents(path: &Path) -> io::Result<()> {
    async_std::fs::DirBuilder::new()
        .recursive(true)
        .create(path.parent().unwrap())
        .await
}

#[cfg(test)]
mod tests {

    use super::Metadata;
    use test_case::test_case;

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

        let _: Metadata = serde_json::from_str(example1).unwrap();

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

        let _: Metadata = serde_json::from_str(example2).unwrap();
    }

    #[test_case("x" => "1/x" ; "one-letter crate name")]
    #[test_case("xx" => "2/xx" ; "two-letter crate name")]
    #[test_case("xxx" =>"3/x/xxx" ; "three-letter crate name")]
    #[test_case("abcd" => "ab/cd/abcd" ; "four-letter crate name")]
    #[test_case("abcde" => "ab/cd/abcde" ; "five-letter crate name")]
    #[test_case("aBcD" => "ab/cd/aBcD" ; "mixed-case crate name")]
    fn get_path(name: &str) -> String {
        crate::index::get_path(name).to_str().unwrap().to_string()
    }
}
