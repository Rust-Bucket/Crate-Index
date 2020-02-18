use git2::Error;
use std::path::Path;
use url::Url;

/// Representation of a git repository on the host filesystem
pub struct Repository {
    repo: git2::Repository,
}

impl Repository {
    /// Initialise a new git repository at the given path.
    pub fn init(root: impl AsRef<Path>) -> Result<Self, Error> {
        let repo = git2::Repository::init(root)?;
        Ok(Repository { repo })
    }

    /// Open an existing repository
    pub fn open(root: impl AsRef<Path>) -> Result<Self, Error> {
        let repo = git2::Repository::open(root)?;
        Ok(Repository { repo })
    }

    /// Add a remote to the repository
    pub fn add_remote(&self, name: impl AsRef<str>, remote: Url) -> Result<(), Error> {
        self.repo.remote(name.as_ref(), remote.as_str())?;
        Ok(())
    }

    /// Add a file to the repository by relative path
    pub fn add_path(&self, path: impl AsRef<Path>) -> Result<(), Error> {
        self.repo.index()?.add_path(path.as_ref())
    }

    /// Add every file in the tree to the repository.
    ///
    /// everything that matches '*', that is.
    pub fn add_all(&self) -> Result<(), Error> {
        self.repo
            .index()?
            .add_all(&["*"], git2::IndexAddOption::DEFAULT, None)
    }

    fn find_last_commit(&self) -> Result<git2::Commit, Error> {
        self.repo.head()?.peel_to_commit()
    }

    /// Commit all staged changes
    pub fn commit(&self, message: impl AsRef<str>) -> Result<(), Error> {
        let mut index = self.repo.index()?;
        let oid = index.write_tree()?;
        let signature = self.repo.signature()?;
        let parent_commit = self.find_last_commit()?;
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

    /// Push all commits to the configured remotes
    pub fn push(&self) -> Result<(), Error> {
        let refspecs: Vec<String> = Vec::new();

        for remote_name in self.repo.remotes()?.into_iter().filter_map(|x| x) {
            self.repo.find_remote(&remote_name)?.push(&refspecs, None)?;
        }

        Ok(())
    }
}
