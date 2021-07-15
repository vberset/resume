use std::collections::HashMap;
use std::error::Error as StdError;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::thread::{sleep, spawn};
use std::time::Duration;

use clap::Clap;
use git2::{AutotagOption, Branch, BranchType, Cred, FetchOptions, RemoteCallbacks, Repository, Revwalk};
use git2::build::RepoBuilder;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;

use crate::cli::{Command, SubCommand};
use crate::config::{Configuration, Project};
use crate::error::{Error, Result};
use crate::message::{CommitType, ConventionalMessage};
use crate::report::build_report;
use crate::utils::get_repo_cache_folder;

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
            let project = Project {
                name: PathBuf::from(&subcmd.repository).canonicalize().unwrap().file_name().unwrap().to_str().unwrap().to_owned(),
                origin: subcmd.repository.to_owned(),
                branch: Some(subcmd.branch),
            };
            let repo = Repository::open(subcmd.repository).unwrap();
            print!("{}", resume_repo(&project, repo)?);
        }
        SubCommand::Projects(subcmd) => {
            let config = Configuration::from_file(subcmd.config_file)?;

            let bars = MultiProgress::new();

            let name_max_len = config.projects.iter().map(|projet| projet.name.len()).max().unwrap();
            let spinner_style = ProgressStyle::default_spinner()
                .tick_chars("⠈⠐⠠⢀⡀⠄⠂⠁ ")
                .template(&format!("{{prefix:>{}.bold}} [{{pos}}/{{len}}] {{spinner}} {{wide_msg}} [{{elapsed}}]", name_max_len));

            let (tx, rx) = channel();

            let projects_count = config.projects.len();
            let handle = spawn(move || {
                config.projects.par_iter()
                    .map_with(tx.clone(), |tx, project| -> Result<String> {
                        let steps = 3;
                        let bar = ProgressBar::new(steps);
                        tx.send(bar.clone()).unwrap();
                        sleep(Duration::from_millis(10));
                        bar.set_style(spinner_style.clone());
                        bar.set_prefix(project.name.to_owned());
                        bar.set_message("pending");

                        let branch_name = project.branch.as_ref().unwrap_or(&config.default_branch);
                        bar.enable_steady_tick(100);
                        let repo = if let Ok(repo) = open_repo(&project.origin) {
                            bar.set_message(format!("open cached repository: {}", project.origin));
                            repo
                        } else {
                            bar.set_message(format!("clone repository: {}", project.origin));
                            clone_repo(&project.origin)?
                        };
                        bar.inc(1);

                        bar.set_message(format!("fetch branch: {}", branch_name));
                        fetch_branch(&repo, branch_name)?;
                        bar.inc(1);

                        bar.set_message("generate résumé");
                        let report = resume_repo(project, repo)?;
                        bar.inc(1);
                        bar.set_message("done");
                        bar.finish();
                        Ok(report)
                    })
                    .collect::<Vec<_>>()
            });
            rx.iter().take(projects_count).for_each(|bar| { bars.add(bar); });
            bars.join_and_clear().unwrap();
            let reports = handle.join().unwrap();
            println!("================================================================================\n");
            for report in reports.into_iter().flatten() {
                println!("{}", report);
            }
        }
    }

    Ok(())
}

fn open_repo(origin: &str) -> Result<Repository> {
    let path = get_repo_cache_folder(origin);
    Ok(Repository::open(&path)?)
}

fn clone_repo(origin: &str) -> Result<Repository> {
    let path = get_repo_cache_folder(origin);

    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key_from_agent(username_from_url.unwrap())
    });

    let mut fetch_option = FetchOptions::new();
    fetch_option
        .remote_callbacks(callbacks)
        .download_tags(AutotagOption::All);

    let repo = RepoBuilder::new()
        .fetch_options(fetch_option)
        .bare(true)
        .clone(origin, path.as_ref())?;

    Ok(repo)
}

fn get_or_create_branch<'a>(repo: &'a Repository, branch_name: &str) -> Result<Branch<'a>> {
    match repo.find_branch(branch_name, BranchType::Local) {
        Ok(branch) => Ok(branch),
        Err(_) => {
            Ok(repo.branch(branch_name, &repo.head()?.peel_to_commit()?, false)?)
        }
    }
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
    get_or_create_branch(repo, branch_name)?;
    remote.fetch(&[&format!("refs/heads/{0}:refs/heads/{0}", branch_name)], Some(&mut fetch_option), None)?;
    Ok(())
}

fn resume_repo(project: &Project, repo: Repository) -> Result<String> {
    let default = "master".to_string();
    let branch_name = project.branch.as_ref().unwrap_or(&default);
    let branch = match repo.find_branch(&branch_name, BranchType::Local) {
        Ok(branch) => branch,
        Err(_) => {
            match repo.find_branch(&branch_name, BranchType::Remote) {
                Ok(branch) => branch,
                Err(error) => {
                    return Err(Error::BranchDoesntExist(branch_name.to_owned(), error));
                }
            }
        }
    };

    let mut walker = repo.revwalk().unwrap();
    walker.push(branch.get().target().unwrap()).unwrap();

    let changelog = build_changelog(&repo, walker);
    let mut report = format!("# Project: {}\n\n", project.name);
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
