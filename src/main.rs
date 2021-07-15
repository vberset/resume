use std::collections::HashMap;
use std::error::Error as StdError;

use clap::Clap;
use git2::{AutotagOption, BranchType, Cred, FetchOptions, RemoteCallbacks, Repository, Revwalk};
use git2::build::RepoBuilder;

use crate::cli::{Command, SubCommand};
use crate::config::Configuration;
use crate::error::{Error, Result};
use crate::message::{CommitType, ConventionalMessage};
use crate::report::build_report;
use crate::utils::get_cache_folder;

mod cli;
mod config;
mod error;
mod message;
mod report;
mod utils;

type ChangeLog = HashMap<CommitType, Vec<ConventionalMessage>>;

fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {}", error);
        let mut error = error.source();
        while let Some(cause) = error {
            eprintln!("⤷ caused by: {}", &cause);
            error = cause.source();
        }
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let command = Command::parse();

    match command.sub_command {
        SubCommand::Repository(subcmd) => {
            let repo = Repository::open(subcmd.repository).unwrap();
            println!("{}", resume_repo(repo, &subcmd.branch)?);
        }
        SubCommand::Projects(subcmd) => {
            let config = Configuration::from_file(subcmd.config_file)?;
            println!();
            for project in &config.projects {
                let branch_name = project.branch.as_ref().unwrap_or(&config.default_branch);
                let repo = open_or_clone_repo(&project.origin, branch_name)?;
                fetch_branch(&repo, branch_name)?;
                println!("================================================================================\n");
                println!("# Project: {}\n", project.name);
                let resume = resume_repo(
                    repo,
                    &project.branch.as_ref().unwrap_or(&config.default_branch),
                )?;

                println!("{}", resume);
            }
            println!("================================================================================\n");
        }
    }

    Ok(())
}

fn open_or_clone_repo(origin: &str, branch_name: &str) -> Result<Repository> {
    let name = origin.split('/').last().unwrap();
    let mut path = get_cache_folder();
    path.push(name);

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key_from_agent(username_from_url.unwrap())
    });

    let mut fetch_option = FetchOptions::new();
    fetch_option
        .remote_callbacks(callbacks)
        .download_tags(AutotagOption::All);

    let repo = match Repository::open(&path) {
        Ok(repo) => {
            repo
        }
        Err(_) => {
            RepoBuilder::new()
                .fetch_options(fetch_option)
                .branch(branch_name)
                .bare(true)
                .clone(origin, path.as_ref())?
        }
    };

    Ok(repo)
}

fn fetch_branch(repo: &Repository, branch_name: &str) -> Result<()> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key_from_agent(username_from_url.unwrap())
    });

    let mut fetch_option = FetchOptions::new();
    fetch_option
        .remote_callbacks(callbacks)
        .download_tags(AutotagOption::All);

    let mut remote = repo.find_remote("origin")?;
    remote.fetch(&[branch_name], Some(&mut fetch_option), None)?;
    Ok(())
}

fn resume_repo(repo: Repository, branch: &str) -> Result<String> {
    let branch = match repo.find_branch(branch, BranchType::Local) {
        Ok(branch) => branch,
        Err(_) => {
            match repo.find_branch(branch, BranchType::Remote) {
                Ok(branch) => branch,
                Err(error) => {
                    return Err(Error::BranchDoesntExist(branch.to_owned(), error));
                }
            }
        }
    };

    let mut walker = repo.revwalk().unwrap();
    walker.push(branch.get().target().unwrap()).unwrap();

    let changelog = build_changelog(&repo, walker);
    let mut report = String::new();
    build_report(&mut report, &changelog)?;
    Ok(report)
}

fn build_changelog(repo: &Repository, walker: Revwalk) -> ChangeLog {
    let mut changelog = HashMap::new();

    for object in walker {
        let commit = repo.find_commit(object.unwrap()).unwrap();
        if let Some(raw_message) = commit.message() {
            if let Ok(message) = raw_message.parse::<ConventionalMessage>() {
                let list = changelog
                    .entry(message.ctype.clone())
                    .or_insert_with(Vec::new);
                list.push(message);
            }
        }
    }

    changelog
}
