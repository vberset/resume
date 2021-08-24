use clap::Clap;

use crate::changelog::CommitField;
use crate::report::OutputType;
use crate::snapshots::BranchName;

#[derive(Clap, Debug)]
#[clap(name = "resume")]
pub struct Command {
    #[clap(subcommand)]
    pub sub_command: SubCommand,
    #[clap(short, long, global(true), multiple_occurrences(true))]
    pub verbose: bool,
    #[clap(short, long, global(true), default_value = "yaml", possible_values = & ["yaml"])]
    pub output: OutputType,
}

#[derive(Clap, Debug)]
pub enum SubCommand {
    #[clap(alias = "r")]
    Repository(Repository),
    #[clap(alias = "p")]
    Projects(Projects),
}

#[derive(Clap, Debug)]
pub struct Repository {
    pub repository: String,
    #[clap(
        short,
        long("branch"),
        max_values(1),
        multiple_values(true),
        default_value = "master"
    )]
    pub branches: Vec<BranchName>,
    #[clap(short, long)]
    pub team: Option<String>,
    #[clap(
        short,
        long,
        default_values = &["branch", "commit-type"],
        possible_values = &["branch", "commit-type", "scope"],
        multiple_values(true),
        require_delimiter(true),
        value_delimiter(','),
    )]
    pub group_by: Vec<CommitField>,
}

#[derive(Clap, Debug)]
pub struct Projects {
    #[clap(default_value = "resume.yaml")]
    pub config_file: String,
    #[clap(long, default_value = "resume.state")]
    pub state_file: String,
    #[clap(long)]
    pub no_state: bool,
    #[clap(short, long)]
    pub save_state: bool,
    #[clap(short, long)]
    pub from_snapshot: Option<String>,
    #[clap(
        short,
        long,
        default_values = &["origin", "branch", "commit-type"],
        possible_values = &["branch", "commit-type", "origin", "scope"],
        multiple_values(true),
        require_delimiter(true),
        value_delimiter(','),
    )]
    pub group_by: Vec<CommitField>,
}
