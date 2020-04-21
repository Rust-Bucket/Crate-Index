use async_std::{
    fs::{read_dir, DirEntry},
    path::PathBuf,
};
use futures_util::{
    stream,
    stream::{Stream, StreamExt, TryStreamExt},
};
use std::{collections::HashSet, io::Error as IoError};

pub async fn filenames(path: impl Into<PathBuf>) -> Result<HashSet<String>, IoError> {
    walk_dir(path)
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
