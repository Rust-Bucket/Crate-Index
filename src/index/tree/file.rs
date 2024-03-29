use super::Record;
use crate::{validate, validate::Error as ValidationError, WrappedResult};
use async_std::{
    fs::{File, OpenOptions},
    io::{
        prelude::{BufReadExt, SeekExt, WriteExt},
        BufReader, SeekFrom,
    },
    path::{Path, PathBuf},
    stream::StreamExt,
};
use semver::Version;
use std::{collections::BTreeMap, fmt, io::Error as IoError};

/// A file in an index.
///
/// The `IndexFile` will cache the entries in memory after opening the file,
/// hence the file is only read once when the `IndexFile` is created.
/// Inserting a [`Record`] into the `IndexFile` is performed by updating the
/// cache, and writing to the underlying file.
///
/// # Warning
///
/// This object makes no attempt to *lock* the underlying file. It is the
/// caller's responsibility to perform any locking or access pooling required.
#[derive(Debug)]
#[allow(clippy::module_name_repetitions)]
pub struct IndexFile {
    crate_name: String,
    file: File,
    entries: BTreeMap<Version, Record>,
}

impl IndexFile {
    /// Open an existing file, or create a new one if it doesn't exist.
    ///
    /// For convenience, this method will also create the parent folders in the
    /// index if they don't yet exist.
    pub async fn open(
        root: impl AsRef<Path>,
        crate_name: impl Into<String>,
    ) -> Result<Self, IoError> {
        let crate_name = crate_name.into();
        let path = root.as_ref().join(get_path(&crate_name));

        create_parents(&path).await?;

        let file = open_file(&path).await?;

        let mut lines = BufReader::new(&file).lines();

        let mut entries = BTreeMap::new();

        while let Some(line) = lines.next().await {
            let line = line?;
            let metadata: Record = serde_json::from_str(&line).expect("JSON encoding error");
            entries.insert(metadata.version().clone(), metadata);
        }

        Ok(Self {
            crate_name,
            file,
            entries,
        })
    }

    /// Insert a [`Record`] into the `IndexFile`.
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
    ///
    /// # Panics
    ///
    /// This function will panic in debug builds if the inserted [`Record`] name
    /// doesn't match the data in the index file. This assertion is not present
    /// in release builds
    pub async fn insert(
        &mut self,
        metadata: Record,
    ) -> WrappedResult<(), ValidationError, IoError> {
        if let Err(e) = self.validate(&metadata) {
            return Ok(Err(e));
        }

        self.entries.insert(metadata.version().clone(), metadata);

        self.save().await?;

        Ok(Ok(()))
    }

    fn get_mut(&mut self, version: &Version) -> Option<&mut Record> {
        self.entries.get_mut(version)
    }

    /// Mark a selected version of the crate as 'yanked'.
    ///
    /// # Errors
    ///
    /// This function will return [`VersionNotFoundError`] if the selected
    /// version does not exist in the index.
    pub async fn yank(
        &mut self,
        version: &Version,
    ) -> WrappedResult<(), VersionNotFoundError, IoError> {
        match self.get_mut(version) {
            Some(record) => {
                record.yank();
                self.save().await?;
                Ok(Ok(()))
            }
            None => Ok(Err(VersionNotFoundError {
                crate_name: self.crate_name.clone(),
                version: version.clone(),
            })),
        }
    }

    /// Mark a selected version of the crate as 'unyanked'.
    ///
    /// # Errors
    ///
    /// This function will return [`VersionNotFoundError`] if the selected
    /// version does not exist in the index.
    pub async fn unyank(
        &mut self,
        version: &Version,
    ) -> WrappedResult<(), VersionNotFoundError, IoError> {
        match self.get_mut(version) {
            Some(record) => {
                record.unyank();
                self.save().await?;
                Ok(Ok(()))
            }
            None => Ok(Err(VersionNotFoundError {
                crate_name: self.crate_name.clone(),
                version: version.clone(),
            })),
        }
    }

    /// The latest version of crate metadata in the file
    pub fn latest_version(&self) -> Option<(&Version, &Record)> {
        self.entries.iter().next_back()
    }

    fn validate(&self, metadata: &Record) -> Result<(), ValidationError> {
        self.validate_name(metadata.name())?;
        self.validate_version(metadata.version())?;

        Ok(())
    }

    /// Check that the incoming crate name is correct
    fn validate_name(&self, given: impl AsRef<str>) -> Result<(), ValidationError> {
        validate::name(given.as_ref())?;

        debug_assert_eq!(
            self.crate_name,
            given.as_ref(),
            "the given crate Record name doesn't match!"
        );

        Ok(())
    }

    /// Check that the incoming crate version is greater than any in the index
    fn validate_version(&self, version: &Version) -> Result<(), ValidationError> {
        match self.greatest_minor_version(version.major) {
            Some(current) => {
                if current.0 < version {
                    Ok(())
                } else {
                    Err(ValidationError::version(current.0, version.clone()))
                }
            }
            None => Ok(()),
        }
    }

    fn greatest_minor_version(&self, major_version: u64) -> Option<(&Version, &Record)> {
        let min = Version::new(major_version, 0, 0);
        let max = Version::new(major_version + 1, 0, 0);

        self.entries.range(min..max).next_back()
    }

    async fn save(&mut self) -> Result<(), IoError> {
        self.file.seek(SeekFrom::Start(0)).await?;
        self.file.set_len(0).await?;
        self.file.write_all(self.to_string().as_bytes()).await
    }
}

impl fmt::Display for IndexFile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let entries: Vec<String> = self
            .entries
            .values()
            .map(std::string::ToString::to_string)
            .collect();
        let output = entries.join("\n");
        write!(f, "{}", output)
    }
}

/// Create all parent directories for the given filepath
async fn create_parents(path: &Path) -> Result<(), IoError> {
    async_std::fs::DirBuilder::new()
        .recursive(true)
        .create(path.parent().unwrap())
        .await
}

async fn open_file(path: &Path) -> Result<File, IoError> {
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
    type IntoIter = std::collections::btree_map::Values<'a, Version, Record>;
    type Item = &'a Record;

    fn into_iter(self) -> Self::IntoIter {
        self.entries.values()
    }
}

impl IntoIterator for IndexFile {
    type IntoIter = std::collections::btree_map::IntoIter<Version, Record>;
    type Item = (Version, Record);

    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}

/// The error type thrown when the specified crate version cannot be found
#[derive(Debug, Clone, thiserror::Error)]
#[error("version not found (no data in index for {crate_name} - {version})")]
pub struct VersionNotFoundError {
    pub(crate) crate_name: String,
    pub(crate) version: Version,
}

impl VersionNotFoundError {
    /// The name of the crate
    #[must_use]
    pub fn crate_name(&self) -> &String {
        &self.crate_name
    }

    /// The specified crate version
    #[must_use]
    pub fn version(&self) -> &Version {
        &self.version
    }
}

#[cfg(test)]
mod tests {
    use super::IndexFile;
    use crate::Record;
    use semver::Version;
    use test_case::test_case;

    #[async_std::test]
    async fn open() {
        let temp_dir = tempfile::tempdir().unwrap();
        let root = temp_dir.path();

        IndexFile::open(root, "other-name").await.unwrap();
    }

    #[test_case("Some-Name", "2.1.1" ; "when used properly")]
    #[test_case("Some-Name", "2.1.0" => panics "invalid"; "when version is the same")]
    #[test_case("Some-Name", "2.0.0" => panics "invalid"; "when version is lower and major version is the same")]
    #[test_case("Some-Name", "1.0.0" ; "when version is lower but major version is different")]
    #[test_case("nul", "2.1.1" => panics "invalid"; "when name is reserved word")]
    #[test_case("-start-with-hyphen", "2.1.1" => panics "invalid"; "when name starts with non-alphabetical character")]
    fn insert(name: &str, version: &str) {
        async_std::task::block_on(async move {
            // create temporary directory
            let temp_dir = tempfile::tempdir().unwrap();
            let root = temp_dir.path();

            // create index file and seed with initial metadata
            let initial_metadata = Record::new("Some-Name", Version::new(2, 1, 0), "checksum");
            let mut index_file = IndexFile::open(root, initial_metadata.name())
                .await
                .unwrap();
            index_file.insert(initial_metadata).await.unwrap().unwrap();

            // create and insert new metadata
            let new_metadata = Record::new(name, Version::parse(version).unwrap(), "checksum");
            index_file
                .insert(new_metadata)
                .await
                .unwrap()
                .expect("invalid");
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
            .insert(Record::new("some-name", Version::new(0, 1, 0), "checksum"))
            .await
            .unwrap()
            .unwrap();

        index_file
            .insert(Record::new("some-name", Version::new(0, 1, 1), "checksum"))
            .await
            .unwrap()
            .unwrap();

        index_file
            .insert(Record::new("some-name", Version::new(0, 2, 0), "checksum"))
            .await
            .unwrap()
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

    fn metadata(version: &str) -> Record {
        Record::new("Some-Name", Version::parse(version).unwrap(), "checksum")
    }

    #[test_case("0.1.0"; "when version exists")]
    #[test_case("0.2.0" => panics "version doesn't exist"; "when version doesn't exist")]
    fn yank(version: &str) {
        let version = Version::parse(version).unwrap();
        async_std::task::block_on(async {
            // create temporary directory
            let temp_dir = tempfile::tempdir().unwrap();
            let root = temp_dir.path();

            let initial_metadata = metadata("0.1.0");

            // create index file and seed with initial metadata
            let mut index_file = IndexFile::open(root, initial_metadata.name())
                .await
                .expect("couldn't open index file");

            index_file
                .insert(initial_metadata)
                .await
                .unwrap()
                .expect("couldn't insert initial metadata");

            if index_file.yank(&version).await.unwrap().is_err() {
                panic!("version doesn't exist");
            }

            index_file.unyank(&version).await.unwrap().unwrap();
        });
    }
}
