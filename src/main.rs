use std::{
    error::Error as StdError,
    sync::mpsc::channel,
    thread::{sleep, spawn},
    time::Duration,
};

use clap::Clap;
use git2::Oid;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rayon::prelude::*;

use crate::changelog::{ChangeLog, ChangeLogEntry, CommitField};
use crate::snapshots::{
    BranchName, RepositoryOrigin, RepositorySnapshot, Snapshot, SnapshotBuilder, SnapshotHistory,
};
use crate::{
    cli::{Command, SubCommand},
    config::Configuration,
    error::{
        Error::{InvalidSnapshotRef, SnapshotDoesntExist},
        Result,
    },
    project::{Project, Sentinels},
    report::OutputType,
};

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

    if command.verbose {
        simple_logger::init_with_level(log::Level::Info).unwrap();
    } else {
        simple_logger::init_with_level(log::Level::Warn).unwrap();
    }

    match &command.sub_command {
        SubCommand::Repository(subcmd) => {
            let change_log = process_repository(
                &subcmd.repository,
                subcmd.group_by.clone(),
                &subcmd.branches,
                subcmd.team.to_owned(),
            )?;

            if command.output == OutputType::Yaml {
                println!("{}", change_log.to_yaml()?);
            }
        }
        SubCommand::Projects(subcmd) => {
            let config = Configuration::from_file(&subcmd.config_file)?;

            let mut history = SnapshotHistory::from_file(&subcmd.state_file)
                .unwrap_or_else(|_| SnapshotHistory::new());

            let snapshot = if subcmd.no_state {
                None
            } else if let Some(snapshot_ref) = &subcmd.from_snapshot {
                let snapshot = if let Ok(index) = snapshot_ref.parse() {
                    history.get_by_index(index).cloned()
                } else if let Ok(hash) = snapshot_ref.parse().as_ref() {
                    history.get_by_hash(hash).cloned()
                } else {
                    return Err(InvalidSnapshotRef(snapshot_ref.to_owned()));
                };

                if snapshot.is_none() {
                    return Err(SnapshotDoesntExist(snapshot_ref.to_owned()));
                }

                snapshot
            } else {
                history.last().cloned()
            };

            let (change_log_entries, snapshot) = process_projects(config, snapshot)?;

            if subcmd.save_state {
                history.push(snapshot);
                history.to_file(&subcmd.state_file)?;
            }

            let mut change_log = ChangeLog::new(subcmd.group_by.to_owned());
            for change_log_entry in change_log_entries.into_iter() {
                change_log.insert(change_log_entry)?;
            }
            if command.output == OutputType::Yaml {
                println!("{}", change_log.to_yaml()?);
            }
        }
    }

    Ok(())
}

fn process_repository(
    repository: &str,
    order_by: Vec<CommitField>,
    branches_name: &[BranchName],
    team: Option<String>,
) -> Result<ChangeLog> {
    let mut project = Project::from_standalone_repository(repository, branches_name)?;
    project.team = team;
    let mut sentinels = Sentinels::new();
    let mut change_log = ChangeLog::new(order_by);
    for branch_name in &project.branches_name {
        let walker = project.build_walker(branch_name.as_str(), &sentinels)?;
        let (change_log_entries, new_sentinels) = project.extract_messages(walker);
        sentinels.extend(new_sentinels);
        for entry in change_log_entries {
            change_log.insert(ChangeLogEntry::new(
                "".to_string().into(),
                branch_name.to_owned(),
                entry,
            ))?;
        }
    }
    Ok(change_log)
}

fn process_projects(
    config: Configuration,
    snapshot: Option<Snapshot>,
) -> Result<(Vec<ChangeLogEntry>, Snapshot)> {
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
                |tx_bars,
                 cfg_project|
                 -> Result<(Vec<ChangeLogEntry>, RepositoryOrigin, RepositorySnapshot)> {
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
                    let mut change_sets = Vec::new();
                    for branch_name in &project.branches_name {
                        bar.set_message(format!("fetch branch: {}", &branch_name));
                        let hash = project.fetch_branch(branch_name)?;
                        repo_snapshot.insert(branch_name.clone(), hash);
                        bar.inc(1);
                    }

                    change_sets.extend(report_branches(&bar, &project)?);

                    bar.set_message("done");
                    bar.finish();
                    Ok((change_sets, cfg_project.origin.clone(), repo_snapshot))
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
    let mut all_change_sets = Vec::new();

    for result in results {
        let (change_sets, origin, repo_snapshot) = result?;
        builder.add_repository_snapshot(origin, repo_snapshot);
        all_change_sets.extend(change_sets);
    }

    Ok((all_change_sets, builder.build()))
}

fn report_branches(bar: &ProgressBar, project: &Project) -> Result<Vec<ChangeLogEntry>> {
    let mut sentinels = Sentinels::new();
    let mut entries = Vec::new();
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
        let (messages, new_sentinels) = project.extract_messages(walker);
        entries.extend(messages.into_iter().map(|message| {
            ChangeLogEntry::new(
                project.get_origin().unwrap(),
                branch_name.to_owned(),
                message,
            )
        }));
        sentinels.extend(&new_sentinels);
        bar.inc(1);
    }
    Ok(entries)
}
