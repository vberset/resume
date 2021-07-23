use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use git2::{
    build::RepoBuilder, Branch, BranchType, Cred, FetchOptions, Oid, RemoteCallbacks, Repository,
    Revwalk,
};

use crate::snapshots::{BranchName, CommitHash, RepositoryOrigin, RepositorySnapshot};
use crate::{
    changelog::ChangeLog, error::Result, message::ConventionalMessage, utils::get_repo_cache_folder,
};

pub type Sentinels = HashSet<Oid>;

pub struct Project {
    pub name: String,
    repository: Repository,
    pub branches_name: Vec<BranchName>,
    pub team: Option<String>,
    pub snapshot: Option<RepositorySnapshot>,
}

impl Project {
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

    fn default_fetch_options() -> FetchOptions<'static> {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username_from_url, _allowed_types| {
            Cred::ssh_key_from_agent(username_from_url.unwrap())
        });

        let mut fetch_option = FetchOptions::new();
        fetch_option.remote_callbacks(callbacks);
        fetch_option
    }

    fn get_branch(&self, branch_name: &str) -> Result<Branch> {
        Ok(self
            .repository
            .find_branch(branch_name, BranchType::Local)?)
    }

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

    pub fn build_walker(&self, branch_name: &str, sentinels: &Sentinels) -> Result<Revwalk> {
        let branch = self.get_branch(branch_name)?;
        let mut walker = self.repository.revwalk()?;
        walker.push(branch.get().target().expect("Branch must point somewhere"))?;
        for oid in sentinels {
            walker.hide(*oid).unwrap();
        }
        Ok(walker)
    }

    pub fn build_changelog(&self, walker: Revwalk) -> (ChangeLog, Sentinels) {
        let mut changelog = HashMap::new();
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
                            let list = changelog
                                .entry(message.ctype.clone())
                                .or_insert_with(Vec::new);
                            list.push(message);
                        }
                    } else {
                        let list = changelog
                            .entry(message.ctype.clone())
                            .or_insert_with(Vec::new);
                        list.push(message);
                    }
                }
            }
        }

        (changelog, new_sentinels)
    }
}
