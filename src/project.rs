use std::{collections::HashSet, path::PathBuf};

use git2::{
    build::RepoBuilder, Branch, BranchType, FetchOptions, Oid, RemoteCallbacks, Repository, Revwalk,
};
use git2_credentials::{ui4dialoguer::CredentialUI4Dialoguer, CredentialHandler};

use crate::{
    error::Result,
    message::ConventionalMessage,
    snapshots::{BranchName, CommitHash, RepositoryOrigin, RepositorySnapshot},
    utils::get_repo_cache_folder,
};

/// Set of commits to not travers
pub type Sentinels = HashSet<Oid>;

/// Project groups a repository and info to traverse its history.
pub struct Project {
    pub name: String,
    repository: Repository,
    pub branches_name: Vec<BranchName>,
    pub team: Option<String>,
    pub snapshot: Option<RepositorySnapshot>,
}

impl Project {
    /// Build a Project from a repository from the file system
    pub fn from_standalone_repository(path: &str, branches_name: &[BranchName]) -> Result<Self> {
        let path = PathBuf::from(path).canonicalize()?;
        let name = path.file_name().unwrap().to_str().unwrap().to_owned();
        let repository = Repository::open(path)?;
        Ok(Self {
            name,
            repository,
            branches_name: branches_name.to_vec(),
            team: None,
            snapshot: None,
        })
    }

    /// Build a Project from a cached clone
    pub fn from_cache(
        name: &str,
        origin: &RepositoryOrigin,
        branches_name: &[BranchName],
    ) -> Result<Self> {
        let path = get_repo_cache_folder(origin);
        let repo = Repository::open(path)?;
        Ok(Self {
            name: name.to_string(),
            repository: repo,
            branches_name: branches_name.to_vec(),
            team: None,
            snapshot: None,
        })
    }

    /// Clone the repository from the given origin then build a Project
    pub fn from_remote(
        name: &str,
        origin: &RepositoryOrigin,
        branches_name: &[BranchName],
    ) -> Result<Self> {
        let path = get_repo_cache_folder(origin);

        let repo = RepoBuilder::new()
            .fetch_options(Self::default_fetch_options())
            .bare(true)
            .clone(origin.as_str(), path.as_ref())?;

        Ok(Self {
            name: name.to_string(),
            repository: repo,
            branches_name: branches_name.to_vec(),
            team: None,
            snapshot: None,
        })
    }

    /// Build default `FetchOptions`, with credentials' callback, etc
    fn default_fetch_options() -> FetchOptions<'static> {
        let mut callbacks = RemoteCallbacks::new();
        let git_config = git2::Config::open_default().unwrap();
        let mut ch =
            CredentialHandler::new_with_ui(git_config, Box::new(CredentialUI4Dialoguer {}));
        callbacks.credentials(move |url, username_from_url, allowed_types| {
            ch.try_next_credential(url, username_from_url, allowed_types)
        });

        let mut fetch_option = FetchOptions::new();
        fetch_option.remote_callbacks(callbacks);
        fetch_option
    }

    /// Get the `Branch` object from the given branch name
    fn get_branch(&self, branch_name: &str) -> Result<Branch> {
        Ok(self
            .repository
            .find_branch(branch_name, BranchType::Local)?)
    }

    pub fn get_origin(&self) -> Result<RepositoryOrigin> {
        Ok(RepositoryOrigin::from(
            self.repository
                .find_remote("origin")
                .map(|ref remote| remote.url().unwrap_or("").to_string())?,
        ))
    }

    /// Get the `Branch` object from the given branch name. Create the branche if needed.
    fn get_or_create_branch(&self, branch_name: &BranchName) -> Result<Branch> {
        match self
            .repository
            .find_branch(branch_name.as_str(), BranchType::Local)
        {
            Ok(branch) => Ok(branch),
            Err(_) => Ok(self.repository.branch(
                branch_name.as_str(),
                &self.repository.head()?.peel_to_commit()?,
                false,
            )?),
        }
    }

    /// Fetch the branch from origin and return the pointed commit ID
    pub fn fetch_branch(&self, branch_name: &BranchName) -> Result<CommitHash> {
        let mut remote = self.repository.find_remote("origin")?;
        let branch = self.get_or_create_branch(branch_name)?;
        remote.fetch(
            &[&format!("refs/heads/{0}:refs/heads/{0}", branch_name)],
            Some(&mut Self::default_fetch_options()),
            None,
        )?;
        Ok(branch.get().target().unwrap().into())
    }

    /// Build a commits walker. Its path is bound by the `sentinels` set of commits.
    pub fn build_walker(&self, branch_name: &str, sentinels: &Sentinels) -> Result<Revwalk> {
        let branch = self.get_branch(branch_name)?;
        let mut walker = self.repository.revwalk()?;
        walker.push(branch.get().target().expect("Branch must point somewhere"))?;
        for oid in sentinels {
            walker.hide(*oid).unwrap();
        }
        Ok(walker)
    }

    pub fn extract_messages(&self, walker: Revwalk) -> (Vec<ConventionalMessage>, Sentinels) {
        let mut messages = Vec::new();
        let mut new_sentinels = Sentinels::new();

        for object in walker {
            let commit = self.repository.find_commit(object.unwrap()).unwrap();
            if commit.parent_count() > 1 {
                new_sentinels.insert(commit.id());
            }
            if let Some(raw_message) = commit.message() {
                if let Ok(message) = raw_message.parse::<ConventionalMessage>() {
                    if let Some(team) = self.team.as_ref() {
                        if message
                            .trailers
                            .iter()
                            .any(|(key, value)| key == "team" && value == team)
                        {
                            messages.push(message)
                        }
                    } else {
                        messages.push(message);
                    }
                }
            }
        }

        (messages, new_sentinels)
    }
}
