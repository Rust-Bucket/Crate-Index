use super::{IndexError, Metadata};
use crate::validate::{validate_version, ValidationError};
use async_std::{
    fs::{File, OpenOptions},
    io::{
        prelude::{BufReadExt, WriteExt},
        BufReader,
    },
    path::Path,
    stream::StreamExt,
};
use semver::Version;
use std::io;

pub struct IndexFile {
    file: File,
    entries: Vec<Metadata>,
}

impl IndexFile {
    pub async fn open(path: &Path) -> io::Result<IndexFile> {
        let file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(path)
            .await?;

        let mut lines = BufReader::new(&file).lines();

        let mut entries = Vec::new();

        while let Some(line) = lines.next().await {
            let metadata: Metadata = serde_json::from_str(&line?).expect("JSON encoding error");
            entries.push(metadata);
        }

        Ok(Self { file, entries })
    }

    pub async fn insert(&mut self, metadata: Metadata) -> std::result::Result<(), IndexError> {
        self.validate(&metadata)?;

        let mut string = metadata.to_string();
        string.push('\r');

        self.file.write_all(string.as_bytes()).await?;
        self.entries.push(metadata);
        Ok(())
    }

    pub fn current_version(&self) -> Option<&Version> {
        self.entries.last().map(Metadata::version)
    }

    fn validate(&self, metadata: &Metadata) -> std::result::Result<(), ValidationError> {
        self.validate_version(metadata.version())?;
        Ok(())
    }

    fn validate_version(
        &self,
        given_version: &Version,
    ) -> std::result::Result<(), ValidationError> {
        if let Some(current_version) = self.current_version() {
            validate_version(current_version, given_version)
        } else {
            Ok(())
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
