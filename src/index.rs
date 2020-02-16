use super::Metadata;
use crate::validate;
use async_std::{fs::File, io::prelude::WriteExt, path::PathBuf};
use std::io;

mod index_file;
use index_file::IndexFile;

mod config;
pub use config::Config;

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
    pub fn new(root: impl Into<PathBuf>, download: impl Into<String>) -> Self {
        let root = root.into();
        let config = Config::new(download);
        Self { root, config }
    }

    /// Initialise an index at the root path.
    ///
    /// # Example
    /// ```no_run
    /// use crate_index::Index;
    /// # async {
    /// let root = "/index";
    /// let download_url = "https://crates.io/api/v1/crates/";
    ///
    /// let index = Index::new(root, download_url);
    /// index.init().await?;
    /// # Ok::<(), std::io::Error>(())
    /// # };
    /// ```
    pub async fn init(&self) -> io::Result<()> {
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
        // open the index file for editing
        let mut index_file = IndexFile::open(self.root(), crate_metadata.name()).await?;

        // insert the new metadata
        index_file.insert(crate_metadata).await?;

        Ok(())
    }

    /// The location on the filesystem of the root of the index
    pub fn root(&self) -> &PathBuf {
        &self.root
    }
}

#[derive(Debug, thiserror::Error)]
pub enum IndexError {
    #[error("Validation Error")]
    Validation(#[from] validate::Error),

    #[error("IO Error")]
    Io(#[from] io::Error),
}

#[cfg(test)]
mod tests {}
