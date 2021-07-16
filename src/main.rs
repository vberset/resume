use std::{
    error::Error as StdError,
    sync::mpsc::channel,
    thread::{sleep, spawn},
    time::Duration,
};

use clap::Clap;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;

use crate::{
    changelog::ChangeLog,
    cli::{Command, SubCommand},
    config::Configuration,
    error::Result,
    project::Project,
    report::build_report,
};

mod changelog;
mod cli;
mod config;
mod error;
mod message;
mod project;
mod report;
mod utils;

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
            let project = Project::from_standalone_repository(
                &subcmd.repository,
                &[subcmd.branch.to_owned()],
            )?;
            print!("{}", resume_repo(&project)?);
        }
        SubCommand::Projects(subcmd) => {
            let config = Configuration::from_file(subcmd.config_file)?;

            let bars = MultiProgress::new();

            let name_max_len = config
                .projects
                .iter()
                .map(|projet| projet.name.len())
                .max()
                .unwrap();
            let spinner_style = ProgressStyle::default_spinner()
                .tick_chars("⠈⠐⠠⢀⡀⠄⠂⠁ ")
                .template(&format!(
                    "{{prefix:>{}.bold}} [{{pos}}/{{len}}] {{spinner}} {{wide_msg}} [{{elapsed}}]",
                    name_max_len
                ));

            let (tx, rx) = channel();

            let projects_count = config.projects.len();
            // Spawn the parallel iterator in a dedicated thread, because of the call
            // of `MultiProcess.join_and_clear()` blocking method is required to draws bars.
            let handle = spawn(move || {
                config
                    .projects
                    .par_iter()
                    .map_with(tx.clone(), |tx, project| -> Result<String> {
                        let steps = 3;
                        let bar = ProgressBar::new(steps);
                        tx.send(bar.clone()).unwrap();
                        // wait a little to let the MultiProgress processes the message
                        // otherwise display non-styled,  non-managed, bars
                        sleep(Duration::from_millis(10));
                        bar.set_style(spinner_style.clone());
                        bar.set_prefix(project.name.to_owned());
                        bar.set_message("pending");

                        let branch_name = project
                            .branch
                            .as_ref()
                            .unwrap_or(&config.default_branch)
                            .to_owned();
                        bar.enable_steady_tick(100);
                        bar.set_message(format!(
                            "try to open cached repository: {}",
                            project.origin
                        ));
                        let project = if let Ok(project) = Project::from_cache(
                            &project.name,
                            &project.origin,
                            &[branch_name.clone()],
                        ) {
                            project
                        } else {
                            bar.set_message(format!("clone repository: {}", project.origin));
                            Project::from_remote(
                                &project.name,
                                &project.origin,
                                &[branch_name.clone()],
                            )?
                        };
                        bar.inc(1);

                        bar.set_message(format!("fetch branch: {}", branch_name));
                        project.fetch_branch(&branch_name)?;
                        bar.inc(1);

                        bar.set_message("generate résumé");
                        let report = resume_repo(&project)?;
                        bar.inc(1);
                        bar.set_message("done");
                        bar.finish();
                        Ok(report)
                    })
                    .collect::<Vec<_>>()
            });
            rx.iter().take(projects_count).for_each(|bar| {
                bars.add(bar);
            });
            bars.join_and_clear().unwrap();
            let reports = handle.join().unwrap();
            println!("================================================================================\n");
            for report in reports {
                println!("{}", report?);
            }
        }
    }

    Ok(())
}

fn resume_repo(project: &Project) -> Result<String> {
    let branch_name = project.branches_name.first().unwrap();
    let walker = project.build_walker(branch_name)?;
    let changelog = project.build_changelog(walker);
    let mut report = format!("# Project: {}\n\n", project.name);
    build_report(&mut report, &changelog)?;
    Ok(report)
}
