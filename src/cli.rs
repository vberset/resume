use clap::Clap;

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
    #[clap(short, long, default_value = "master")]
    pub branch: String,
}

#[derive(Clap, Debug)]
pub struct Projects {
    #[clap(default_value = "resume.yaml")]
    pub config_file: String,
}
