use std::collections::HashMap;
use std::error::Error as StdError;

use clap::Clap;
use git2::Repository;

use crate::error::Error;
use crate::message::{CommitType, ConventionalMessage};

mod error;
mod message;

#[derive(Clap, Debug)]
#[clap(name = "resume")]
struct Command {
    repository: String,
    #[clap(short, long, default_value = "master")]
    branch: String,
}

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

fn run() -> Result<(), Error> {
    let command = Command::parse();
    let repo = Repository::open(command.repository).expect("unable to open repository");

    let reference = match repo.find_reference(&("refs/heads/".to_owned() + command.branch.as_str()))
    {
        Ok(reference) => reference,
        Err(error) => {
            return Err(Error::BranchDoesntExist(command.branch, error));
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

    if let Some(features) = changelog.get(&CommitType::Feature) {
        println!("✨ New Features\n");
        for message in features {
            println!(
                " - {} {}",
                if message.is_breaking { "💥 " } else { "" },
                message.summary
            );
        }
        println!();
    }

    if let Some(features) = changelog.get(&CommitType::BugFix) {
        println!("🐛 Bug Fixes\n");
        for message in features {
            println!(
                " - {} {}",
                if message.is_breaking { "💥 " } else { "" },
                message.summary
            );
        }
        println!();
    }

    Ok(())
}
