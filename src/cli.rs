use clap::Clap;

use crate::snapshots::BranchName;

#[derive(Clap, Debug)]
#[clap(name = "resume")]
pub struct Command {
    #[clap(subcommand)]
    pub sub_command: SubCommand,
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
        multiple(true),
        default_value = "master"
    )]
    pub branches: Vec<BranchName>,
    #[clap(short, long)]
    pub team: Option<String>,
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
}
