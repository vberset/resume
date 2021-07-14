use std::collections::HashMap;
use std::error::Error as StdError;

use clap::Clap;
use git2::Repository;

use crate::cli::{Command, SubCommand};
use crate::config::Configuration;
use crate::error::{Error, Result};
use crate::message::{CommitType, ConventionalMessage};
use crate::report::report;
use std::path::Path;

mod cli;
mod config;
mod error;
mod message;
mod report;

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
            resume_repo(subcmd.repository, &subcmd.branch)?;
        }
        SubCommand::Projects(subcmd) => {
            let config = Configuration::from_file(subcmd.config_file)?;
            for project in &config.projects {
                println!("# Project: {}\n", project.name);
                resume_repo(
                    &project.source,
                    &project.branch.as_ref().unwrap_or(&config.default_branch),
                )?;
            }
        }
    }

    Ok(())
}

fn resume_repo<P: AsRef<Path>>(path: P, branch: &str) -> Result<()> {
    let repo = Repository::open(path).expect("unable to open repository");

    let reference = match repo.find_reference(&("refs/heads/".to_owned() + branch)) {
        Ok(reference) => reference,
        Err(error) => {
            return Err(Error::BranchDoesntExist(branch.to_owned(), error));
        }
    };

    let mut walker = repo.revwalk().unwrap();
    walker.push(reference.target().unwrap()).unwrap();

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

    report(&changelog);
    Ok(())
}
