use std::{
    error::Error as StdError,
    fmt::Write,
    sync::mpsc::channel,
    thread::{sleep, spawn},
    time::Duration,
};

use clap::Clap;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;

use crate::snapshots::{
    BranchName, RepositoryOrigin, RepositorySnapshot, Snapshot, SnapshotBuilder, SnapshotHistory,
};
use crate::{
    changelog::ChangeLog,
    cli::{Command, SubCommand},
    config::Configuration,
    error::Result,
    project::{Project, Sentinels},
    report::build_report,
};
use git2::Oid;

mod changelog;
mod cli;
mod config;
mod error;
mod message;
mod project;
mod report;
mod snapshots;
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

    match &command.sub_command {
        SubCommand::Repository(subcmd) => {
            process_repository(&subcmd.repository, &subcmd.branches, subcmd.team.to_owned())?;
        }
        SubCommand::Projects(subcmd) => {
            let config = Configuration::from_file(&subcmd.config_file)?;

            let mut history = SnapshotHistory::from_file(&subcmd.state_file)
                .unwrap_or_else(|_| SnapshotHistory::new());

            let snapshot = if subcmd.load_state {
                history.last().cloned()
            } else {
                None
            };

            let snapshot = process_projects(config, snapshot)?;

            if subcmd.save_state {
                history.push(snapshot);
                history.to_file(&subcmd.state_file)?;
            }
        }
    }

    Ok(())
}

fn process_repository(
    repository: &str,
    branches_name: &[BranchName],
    team: Option<String>,
) -> Result<()> {
    let mut project = Project::from_standalone_repository(repository, branches_name)?;
    project.team = team;
    let mut report = format!("# Project: {}\n\n", project.name);
    let mut sentinels = Sentinels::new();
    for branch_name in &project.branches_name {
        let walker = project.build_walker(branch_name.as_str(), &sentinels)?;
        let (changelog, new_sentinels) = project.build_changelog(walker);
        sentinels.extend(new_sentinels);
        build_report(&mut report, &changelog)?;
    }
    print!("{}", report);
    Ok(())
}

fn process_projects(config: Configuration, snapshot: Option<Snapshot>) -> Result<Snapshot> {
    let bars = MultiProgress::new();

    let name_max_len = config.get_branch_name_max_len();
    let bar_style = ProgressStyle::default_spinner()
        .tick_chars("⠈⠐⠠⢀⡀⠄⠂⠁ ")
        .template(&format!(
            "{{prefix:>{}.bold}} [{{pos}}/{{len}}] {{spinner}} {{wide_msg}} [{{elapsed}}]",
            name_max_len
        ));

    let (tx_bars, rx_bars) = channel();
    let projects_count = config.projects.len();
    // Spawn the parallel iterator in a dedicated thread, because of the call
    // of `MultiProcess.join_and_clear()` blocking method is required to draws bars.
    let handle = spawn(move || {
        let default_branches_name = vec![config.default_branch.clone()];
        config
            .projects
            .par_iter()
            .map_with(
                tx_bars.clone(),
                |tx_bars, cfg_project| -> Result<(String, RepositoryOrigin, RepositorySnapshot)> {
                    let branches_name = cfg_project.get_branches_name(&default_branches_name);

                    let steps = 1 + (branches_name.len() as u64) * 2;
                    let bar = ProgressBar::new(steps);
                    tx_bars.send(bar.clone()).unwrap();
                    // wait a little to let the MultiProgress processes the message
                    // otherwise display non-styled,  non-managed, bars
                    sleep(Duration::from_millis(10));
                    bar.set_style(bar_style.clone());
                    bar.set_prefix(cfg_project.name.to_owned());
                    bar.set_message("pending");
                    bar.enable_steady_tick(100);
                    bar.set_message(format!(
                        "try to open cached repository: {}",
                        cfg_project.origin
                    ));

                    let team = cfg_project.team.clone();

                    let mut project = if let Ok(project) =
                        Project::from_cache(&cfg_project.name, &cfg_project.origin, &branches_name)
                    {
                        project
                    } else {
                        bar.set_message(format!("clone repository: {}", cfg_project.origin));
                        Project::from_remote(
                            &cfg_project.name,
                            &cfg_project.origin,
                            &branches_name,
                        )?
                    };
                    project.team = team;
                    if let Some(snapshot) = &snapshot {
                        project.snapshot = snapshot.get(&cfg_project.origin).cloned();
                    }
                    bar.inc(1);

                    let mut repo_snapshot = RepositorySnapshot::new();
                    for branch_name in &project.branches_name {
                        bar.set_message(format!("fetch branch: {}", &branch_name));
                        let hash = project.fetch_branch(&branch_name)?;
                        repo_snapshot.insert(branch_name.clone(), hash);
                        bar.inc(1);
                    }

                    let mut report = format!("# Project: {}\n\n", project.name);

                    report_branches(&bar, &project, &mut report)?;

                    bar.set_message("done");
                    bar.finish();
                    Ok((report, cfg_project.origin.clone(), repo_snapshot))
                },
            )
            .collect::<Vec<_>>()
    });
    rx_bars.iter().take(projects_count).for_each(|bar| {
        bars.add(bar);
    });
    bars.join_and_clear().unwrap();
    let results = handle.join().unwrap();

    let mut builder = SnapshotBuilder::new();

    for result in results {
        let (report, origin, repo_snapshot) = result?;
        builder.add_repository_snapshot(origin, repo_snapshot);
        println!("{}", report);
    }

    Ok(builder.build())
}

fn report_branches<W: Write>(bar: &ProgressBar, project: &Project, report: &mut W) -> Result<()> {
    let mut sentinels = Sentinels::new();
    for branch_name in &project.branches_name {
        bar.set_message(format!("traverse branch {}", branch_name));
        if let Some(Some(head)) = project
            .snapshot
            .as_ref()
            .map(|snapshot| snapshot.get(branch_name))
        {
            sentinels.insert(Oid::from_str(head.as_str())?);
        }
        let walker = project.build_walker(branch_name.as_str(), &sentinels)?;
        let (changelog, new_sentinels) = project.build_changelog(walker);
        sentinels.extend(&new_sentinels);
        if !changelog.is_empty() {
            writeln!(report, "  ## branch: {}\n", branch_name).unwrap();
            build_report(report, &changelog)?;
        }
        bar.inc(1);
    }
    Ok(())
}
