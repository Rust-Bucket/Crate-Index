#![allow(clippy::clippy::missing_errors_doc)]

//! Abstractions over a git repository containing an index.

use std::path::Path;
use url::Url;

/// Representation of a git repository on the host filesystem
pub struct Repository {
    repo: git2::Repository,
}

pub(crate) struct Identity<'a> {
    pub username: &'a str,
    pub email: &'a str,
}

impl Repository {
    /// Initialise a new git repository at the given path.
    pub fn init(root: impl AsRef<Path>) -> Result<Self, git2::Error> {
        let repo = git2::Repository::init(root)?;

        Ok(Repository { repo })
    }

    /// Commit the current tree state as an "Initial commit"
    pub fn create_initial_commit(&self) -> Result<(), git2::Error> {
        let signature = self.repo.signature()?;
        let oid = self.repo.index()?.write_tree()?;
        let tree = self.repo.find_tree(oid)?;
        self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            "Initial commit",
            &tree,
            &[],
        )?;
        Ok(())
    }

    /// Open an existing repository
    pub fn open(root: impl AsRef<Path>) -> Result<Self, git2::Error> {
        let repo = git2::Repository::open(root)?;
        Ok(Repository { repo })
    }

    /// Add a remote to the repository
    pub(crate) fn add_origin(&self, remote: &Url) -> Result<(), git2::Error> {
        self.repo.remote("origin", remote.as_str())?;
        Ok(())
    }

    pub(crate) fn set_username(&self, username: impl AsRef<str>) -> Result<(), git2::Error> {
        self.repo.config()?.set_str("user.name", username.as_ref())
    }

    pub(crate) fn set_email(&self, email: impl AsRef<str>) -> Result<(), git2::Error> {
        self.repo.config()?.set_str("user.email", email.as_ref())
    }

    /// Add a file to the repository by relative path
    pub fn add_path(&self, path: impl AsRef<Path>) -> Result<(), git2::Error> {
        self.repo.index()?.add_path(path.as_ref())
    }

    /// Add every file in the tree to the repository.
    ///
    /// everything that matches '*', that is.
    pub fn add_all(&self) -> Result<(), git2::Error> {
        let mut index = self.repo.index()?;
        index.add_all(&["."], git2::IndexAddOption::DEFAULT, None)
    }

    /// Commit all staged changes
    pub fn commit(&self, message: impl AsRef<str>) -> Result<(), git2::Error> {
        let mut index = self.repo.index()?;
        let oid = index.write_tree()?;
        let signature = self.repo.signature()?;
        //let parent_commit = self.find_last_commit()?;
        let parent_commit = self.repo.head()?.peel_to_commit()?;
        let tree = self.repo.find_tree(oid)?;
        self.repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            message.as_ref(),
            &tree,
            &[&parent_commit],
        )?;
        Ok(())
    }

    fn fetch(&self) -> Result<git2::AnnotatedCommit, git2::Error> {
        let mut fetch_options = git2::FetchOptions::new();
        fetch_options.download_tags(git2::AutotagOption::All);

        self.repo
            .find_remote("origin")?
            .fetch(&["master"], Some(&mut fetch_options), None)?;

        let fetch_head = self.repo.find_reference("FETCH_HEAD")?;

        self.repo.reference_to_annotated_commit(&fetch_head)
    }

    fn merge(&self, commit: &git2::AnnotatedCommit) -> Result<(), git2::Error> {
        // 1. do a merge analysis
        let analysis = self.repo.merge_analysis(&[&commit])?;

        // 2. Do the appropriate merge
        if analysis.0.is_fast_forward() {
            // do a fast forward
            let refname = "refs/heads/master";
            if let Ok(mut r) = self.repo.find_reference(refname) {
                fast_forward(&self.repo, &mut r, &commit)?;
            } else {
                // The branch doesn't exist so just set the reference to the
                // commit directly. Usually this is because you are pulling
                // into an empty repository.
                self.repo.reference(
                    &refname,
                    commit.id(),
                    true,
                    &format!("Setting {} to {}", "master", commit.id()),
                )?;
                self.repo.set_head(&refname)?;
                self.repo.checkout_head(Some(
                    git2::build::CheckoutBuilder::default()
                        .allow_conflicts(true)
                        .conflict_style_merge(true)
                        .force(),
                ))?;
            }
        } else if analysis.0.is_normal() {
            // do a normal merge
            let head_commit = self
                .repo
                .reference_to_annotated_commit(&self.repo.head()?)?;
            normal_merge(&self.repo, &head_commit, &commit)?;
        } else {
        }
        Ok(())
    }

    /// Pull all commits from the configured remote
    pub fn pull(&self) -> Result<(), git2::Error> {
        let fetch_commit = self.fetch()?;
        self.merge(&fetch_commit)?;

        Ok(())
    }

    /// Push all commits to the configured remotes
    pub fn push(&self) -> Result<(), git2::Error> {
        self.repo
            .find_remote("origin")?
            .push(&["refs/heads/master:refs/heads/master"], None)?;

        Ok(())
    }
}

fn fast_forward(
    repo: &git2::Repository,
    lb: &mut git2::Reference,
    rc: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let name = match lb.name() {
        Some(s) => s.to_string(),
        None => String::from_utf8_lossy(lb.name_bytes()).to_string(),
    };
    let msg = format!("Fast-Forward: Setting {} to id: {}", name, rc.id());
    lb.set_target(rc.id(), &msg)?;
    repo.set_head(&name)?;
    repo.checkout_head(Some(git2::build::CheckoutBuilder::default()
            // For some reason the force is required to make the working directory actually get updated
            // I suspect we should be adding some logic to handle dirty working directory states
            // but this is just an example so maybe not.
            .force()))?;
    Ok(())
}

fn normal_merge(
    _repo: &git2::Repository,
    _local: &git2::AnnotatedCommit,
    _remote: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    unimplemented!()
}

#[cfg(test)]
mod tests {
    use super::Repository;
    use url::Url;

    fn create_bare_repo() -> (tempfile::TempDir, git2::Repository) {
        let temp_dir = tempfile::tempdir().expect("couldn't create temporary directory");
        let repository =
            git2::Repository::init_bare(temp_dir.path()).expect("couldn't create bare repository");
        (temp_dir, repository)
    }

    fn create_repository() -> (tempfile::TempDir, Repository) {
        let temp_dir = tempfile::tempdir().expect("couldn't create temporary directory");
        let repository = Repository::init(temp_dir.path()).expect("couldn't create Repository");
        repository.set_email("first.last@gmail.com").unwrap();
        repository.set_username("first last").unwrap();
        (temp_dir, repository)
    }

    #[test]
    fn push_to_origin() {
        let (remote_dir, _) = create_bare_repo();

        let (_temp_dir, local_repo) = create_repository();
        local_repo
            .add_origin(&Url::from_file_path(remote_dir.path()).unwrap())
            .expect("couldn't add origin");

        local_repo.create_initial_commit().unwrap();

        local_repo.push().expect("couldn't push to remote");
    }

    #[test]
    fn pull_from_origin_add_all() {
        // create a 'remote' git repo
        let (remote_dir, _remote_repo) = create_bare_repo();
        let remote_path = Url::from_file_path(remote_dir.path().canonicalize().unwrap()).unwrap();

        // Create some 'third-party' repo, create a file in it, and push it to the
        // remote
        let (foreign_dir, foreign_repo) = create_repository();
        foreign_repo.add_origin(&remote_path).unwrap();
        foreign_repo.create_initial_commit().unwrap();
        std::fs::File::create(foreign_dir.path().join("some-file")).unwrap();
        foreign_repo.add_all().unwrap();
        foreign_repo.commit("added some file").unwrap();
        foreign_repo.push().unwrap();

        // create a 'local' repo and pull from the remote repo. ensure the file is
        // present after pulling
        let (local_dir, local_repo) = create_repository();
        local_repo.add_origin(&remote_path).unwrap();
        local_repo.create_initial_commit().unwrap();
        local_repo.pull().unwrap();
        assert!(local_dir.path().join("some-file").exists())
    }

    #[test]
    fn pull_from_origin_add_path() {
        // create a 'remote' git repo
        let (remote_dir, _remote_repo) = create_bare_repo();
        let remote_path = Url::from_file_path(remote_dir.path().canonicalize().unwrap()).unwrap();

        // Create some 'third-party' repo, create a file in it, and push it to the
        // remote
        let (foreign_dir, foreign_repo) = create_repository();
        foreign_repo.add_origin(&remote_path).unwrap();
        foreign_repo.create_initial_commit().unwrap();
        std::fs::File::create(foreign_dir.path().join("some-file")).unwrap();
        foreign_repo.add_path("some-file").unwrap();
        foreign_repo.commit("added some file").unwrap();
        foreign_repo.push().unwrap();

        // create a 'local' repo and pull from the remote repo. ensure the file is
        // present after pulling
        let (local_dir, local_repo) = create_repository();
        local_repo.add_origin(&remote_path).unwrap();
        local_repo.create_initial_commit().unwrap();
        local_repo.pull().unwrap();
        assert!(local_dir.path().join("some-file").exists())
    }
}
