use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde::Deserialize;

use crate::error::Result;

#[derive(Debug, Deserialize)]
pub struct Configuration {
    #[serde(default = "default_branch")]
    pub default_branch: String,
    pub projects: Vec<Project>,
}

#[derive(Debug, Deserialize)]
pub struct Project {
    pub name: String,
    pub origin: String,
    pub branch: Option<String>,
}

impl Configuration {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(serde_yaml::from_reader(reader)?)
    }
}

fn default_branch() -> String {
    "master".to_string()
}
