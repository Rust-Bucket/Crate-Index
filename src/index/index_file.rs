use super::{IndexError, Metadata};
use crate::validate;
use async_std::{
    fs::{File, OpenOptions},
    io::{
        prelude::{BufReadExt, WriteExt},
        BufReader,
    },
    path::{Path, PathBuf},
    stream::StreamExt,
};
use semver::Version;
use std::io;

/// A file in an index.
///
/// The `IndexFile` will cache the entries in memory after opening the file,
/// hence the file is only read once when the `IndexFile` is created.
/// Inserting [`Metadata`] into the `IndexFile` is performed by updating the
/// cache, and appending to the underlying file.
///
/// # Warning
///
/// This object makes no attempt to *lock* the underlying file. It is the
/// caller's responsibility to perform any locking or access pooling required.
#[derive(Debug)]
pub struct IndexFile {
    file: File,
    entries: Vec<Metadata>,
}

impl IndexFile {
    /// Open an existing file, or create a new one if it does't exist.
    ///
    /// For convenience, this method will also create the parent folders in the
    /// index if they don't yet exist.
    pub async fn open(
        root: impl AsRef<Path>,
        crate_name: impl AsRef<str>,
    ) -> std::result::Result<Self, IndexError> {
        let path = root.as_ref().join(get_path(crate_name));

        create_parents(&path).await?;

        let file = open_file(&path).await?;

        let mut lines = BufReader::new(&file).lines();

        let mut entries = Vec::new();

        while let Some(line) = lines.next().await {
            let metadata: Metadata = serde_json::from_str(&line?).expect("JSON encoding error");
            entries.push(metadata);
        }

        Ok(Self { file, entries })
    }

    /// Insert [`Metadata`] into the `IndexFile`.
    ///
    /// This will-
    /// - cache the metadata
    /// - append the metadata to the file
    ///
    /// # Errors
    ///
    /// This function will return an error if the version of the incoming
    /// metadata is not later than the all existing entries, or if the the file
    /// cannot be written to.
    pub async fn insert(&mut self, metadata: Metadata) -> std::result::Result<(), IndexError> {
        self.validate(&metadata)?;

        let mut string = metadata.to_string();
        string.push('\r');

        self.file.write_all(string.as_bytes()).await?;
        self.entries.push(metadata);
        Ok(())
    }

    /// The latest version of crate metadata in the file
    pub fn current_version(&self) -> Option<&Version> {
        self.entries.last().map(Metadata::version)
    }

    fn validate(&self, metadata: &Metadata) -> std::result::Result<(), validate::Error> {
        if let Some(current_version) = self.current_version() {
            let given_version = metadata.version();
            validate::version(current_version, given_version)
        } else {
            Ok(())
        }
    }
}

/// Create all parent directories for the given filepath
async fn create_parents(path: &Path) -> io::Result<()> {
    async_std::fs::DirBuilder::new()
        .recursive(true)
        .create(path.parent().unwrap())
        .await
}

async fn open_file(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .append(true)
        .read(true)
        .create(true)
        .open(path)
        .await
}

fn get_path(name: impl AsRef<str>) -> PathBuf {
    let name = name.as_ref();
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

impl<'a> IntoIterator for &'a IndexFile {
    type Item = &'a Metadata;
    type IntoIter = impl Iterator<Item = Self::Item> + 'a;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter()
    }
}

impl IntoIterator for IndexFile {
    type Item = Metadata;
    type IntoIter = impl Iterator<Item = Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::IndexFile;
    use test_case::test_case;

    #[async_std::test]
    async fn open() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        IndexFile::open(root, "other-name").await.unwrap();
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
