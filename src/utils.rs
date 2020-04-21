use async_std::{
    fs::{read_dir, DirEntry},
    path::PathBuf,
};
use futures_util::{
    future, stream,
    stream::{Stream, StreamExt, TryStreamExt},
};
use std::{collections::HashSet, io::Error as IoError};

pub async fn crate_names(path: impl Into<PathBuf>) -> Result<HashSet<String>, IoError> {
    fn is_hidden(entry: &DirEntry) -> bool {
        entry
            .file_name()
            .to_str()
            .map(|s| s.starts_with("."))
            .unwrap_or(false)
    }

    fn is_special_file(entry: &DirEntry) -> bool {
        let special_files = vec!["config.json"];

        entry
            .file_name()
            .to_str()
            .map(|s| special_files.contains(&s))
            .unwrap_or(false)
    }

    walk_dir(path)
        .try_filter(|entry| future::ready(!is_hidden(entry)))
        .try_filter(|entry| future::ready(!is_special_file(entry)))
        .map_ok(|entry| entry.file_name().to_string_lossy().into_owned())
        .try_collect()
        .await
}

fn walk_dir(
    path: impl Into<PathBuf>,
) -> impl Stream<Item = Result<DirEntry, IoError>> + Send + 'static {
    async fn one_level(
        path: PathBuf,
        to_visit: &mut Vec<PathBuf>,
    ) -> Result<Vec<DirEntry>, IoError> {
        let mut dir = read_dir(path).await?;
        let mut files = Vec::new();

        while let Some(child) = dir.next().await {
            let child = child?;
            if child.metadata().await?.is_dir() {
                to_visit.push(child.path());
            } else {
                files.push(child)
            }
        }

        Ok(files)
    }

    stream::unfold(vec![path.into()], |mut to_visit| async {
        let path = to_visit.pop()?;
        let file_stream = match one_level(path, &mut to_visit).await {
            Ok(files) => stream::iter(files).map(Ok).left_stream(),
            Err(e) => stream::once(async { Err(e) }).right_stream(),
        };

        Some((file_stream, to_visit))
    })
    .flatten()
}

#[cfg(test)]
mod tests {
    use super::crate_names;
    use std::{collections::HashSet, fs::File};

    #[test]
    fn with_flat_directory() {
        let temp_dir = tempfile::tempdir().unwrap();

        let mut file_names = HashSet::new();
        file_names.insert("alpha".to_string());
        file_names.insert("beta".to_string());
        file_names.insert("gamma".to_string());

        for name in &file_names {
            let path = temp_dir.path().join(name);
            File::create(path).unwrap();
        }

        let result = async_std::task::block_on(crate_names(temp_dir.path())).unwrap();

        assert_eq!(file_names, result)
    }

    #[test]
    fn with_nested_directory() {
        let temp_dir = tempfile::tempdir().unwrap();

        let mut file_names = HashSet::new();
        file_names.insert("alpha".to_string());
        file_names.insert("beta".to_string());
        file_names.insert("gamma".to_string());

        let mut file_paths = HashSet::new();
        file_paths.insert("folder/alpha".to_string());
        file_paths.insert("a/deep/folder/beta".to_string());
        file_paths.insert("gamma".to_string());

        for name in &file_paths {
            let path = temp_dir.path().join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            File::create(path).unwrap();
        }

        let result = async_std::task::block_on(crate_names(temp_dir.path())).unwrap();

        assert_eq!(file_names, result)
    }

    #[test]
    fn with_config_file() {
        let temp_dir = tempfile::tempdir().unwrap();

        let mut file_names = HashSet::new();
        file_names.insert("alpha".to_string());
        file_names.insert("beta".to_string());
        file_names.insert("gamma".to_string());

        for name in &file_names {
            let path = temp_dir.path().join(name);
            File::create(path).unwrap();
        }

        let path = temp_dir.path().join("config.json");
        File::create(path).unwrap();

        let result = async_std::task::block_on(crate_names(temp_dir.path())).unwrap();

        assert_eq!(file_names, result)
    }
}
