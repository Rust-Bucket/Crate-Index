use super::Metadata;
use crate::{validate, Result};
use async_std::{
    fs::{File, OpenOptions},
    io::{
        prelude::{BufReadExt, WriteExt},
        BufReader,
    },
    path::{Path, PathBuf},
    stream::StreamExt,
};
use itertools::Itertools;
use semver::Version;
use std::{collections::BTreeMap, fmt, io};

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
    crate_name: String,
    file: File,
    entries: BTreeMap<Version, Metadata>,
}

impl IndexFile {
    /// Open an existing file, or create a new one if it does't exist.
    ///
    /// For convenience, this method will also create the parent folders in the
    /// index if they don't yet exist.
    pub async fn open(root: impl AsRef<Path>, crate_name: impl Into<String>) -> Result<Self> {
        let crate_name = crate_name.into();
        let path = root.as_ref().join(get_path(&crate_name));

        create_parents(&path).await?;

        let file = open_file(&path).await?;

        let mut lines = BufReader::new(&file).lines();

        let mut entries = BTreeMap::new();

        while let Some(line) = lines.next().await {
            let line = line?;
            println!("{}", &line);
            let metadata: Metadata = serde_json::from_str(&line).expect("JSON encoding error");
            entries.insert(metadata.version().clone(), metadata);
        }

        Ok(Self {
            crate_name,
            file,
            entries,
        })
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
    pub async fn insert(&mut self, metadata: Metadata) -> Result<()> {
        self.validate(&metadata)?;

        self.entries.insert(metadata.version().clone(), metadata);

        self.save().await?;

        Ok(())
    }

    /// The latest version of crate metadata in the file
    pub fn latest_version(&self) -> Option<(&Version, &Metadata)> {
        self.entries.last_key_value()
    }

    fn validate(&self, metadata: &Metadata) -> std::result::Result<(), validate::Error> {
        self.validate_name(metadata.name())?;
        self.validate_version(metadata.version())?;

        Ok(())
    }

    /// Check that the incoming crate name is correct
    fn validate_name(&self, given: impl AsRef<str>) -> std::result::Result<(), validate::Error> {
        validate::name(given.as_ref())?;

        if self.crate_name == given.as_ref() {
            Ok(())
        } else {
            Err(validate::Error::name_mismatch(
                self.crate_name.clone(),
                given.as_ref().to_string(),
            ))
        }
    }

    /// Check that the incoming crate version is greater than any in the index
    fn validate_version(&self, version: &Version) -> std::result::Result<(), validate::Error> {
        match self.greatest_minor_version(version.major) {
            Some(current) => {
                if current.0 < version {
                    Ok(())
                } else {
                    Err(validate::Error::version(current.0, version.clone()))
                }
            }
            None => Ok(()),
        }
    }

    fn greatest_minor_version(&self, major_version: u64) -> Option<(&Version, &Metadata)> {
        let min = Version::new(major_version, 0, 0);
        let max = Version::new(major_version + 1, 0, 0);

        self.entries.range(min..max).next_back()
    }

    async fn save(&mut self) -> io::Result<()> {
        self.file.write_all(self.to_string().as_bytes()).await
    }
}

impl fmt::Display for IndexFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}",
            self.entries.values().map(|x| x.to_string()).join("\n")
        )
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
        .write(true)
        .read(true)
        .create(true)
        .open(path)
        .await
}

fn get_path(name: impl AsRef<str>) -> PathBuf {
    let name = name.as_ref();
    let canonical_name = name.to_ascii_lowercase().replace('_', "-");
    let mut path = PathBuf::new();

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
            path.push(&canonical_name[0..1]);
            path.push(name);
            path
        }
        _ => {
            path.push(&canonical_name[0..2]);
            path.push(&canonical_name[2..4]);
            path.push(name);
            path
        }
    }
}

impl<'a> IntoIterator for &'a IndexFile {
    type Item = &'a Metadata;
    type IntoIter = std::collections::btree_map::Values<'a, Version, Metadata>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.values()
    }
}

impl IntoIterator for IndexFile {
    type Item = (Version, Metadata);
    type IntoIter = std::collections::btree_map::IntoIter<Version, Metadata>;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::IndexFile;
    use crate::Metadata;
    use semver::Version;
    use test_case::test_case;

    #[async_std::test]
    async fn open() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        IndexFile::open(root, "other-name").await.unwrap();
    }

    #[test_case("some-name", "0.1.1" ; "when used properly")]
    #[test_case("other-name", "0.1.1" => panics "invalid"; "when name doesnt match")]
    #[test_case("some-name", "0.1.0" => panics "invalid"; "when version is the same")]
    #[test_case("some-name", "0.0.1" => panics "invalid"; "when version is lower")]
    fn insert(name: &str, version: &str) {
        async_std::task::block_on(async move {
            // create temporary directory
            let temp_dir = tempfile::tempdir().unwrap();
            let root = temp_dir.path();

            // create index file and seed with initial metadata
            let initial_metadata = Metadata::new("some-name", Version::new(0, 1, 0), "checksum");
            let mut index_file = IndexFile::open(root, initial_metadata.name())
                .await
                .unwrap();
            index_file.insert(initial_metadata).await.unwrap();

            // create and insert new metadata
            let new_metadata = Metadata::new(name, Version::parse(version).unwrap(), "checksum");
            index_file.insert(new_metadata).await.expect("invalid");
        });
    }

    #[async_std::test]
    async fn latest() {
        // create temporary directory
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        let mut index_file = IndexFile::open(root, "some-name").await.unwrap();

        // create index file and seed with initial metadata
        index_file
            .insert(Metadata::new(
                "some-name",
                Version::new(0, 1, 0),
                "checksum",
            ))
            .await
            .unwrap();

        index_file
            .insert(Metadata::new(
                "some-name",
                Version::new(0, 1, 1),
                "checksum",
            ))
            .await
            .unwrap();

        index_file
            .insert(Metadata::new(
                "some-name",
                Version::new(0, 2, 0),
                "checksum",
            ))
            .await
            .unwrap();

        assert_eq!(
            index_file.latest_version().unwrap().0,
            &Version::new(0, 2, 0)
        );
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
