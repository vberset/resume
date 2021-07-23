use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde::Deserialize;

use crate::error::Result;
use crate::snapshots::{BranchName, RepositoryOrigin};

#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct Configuration {
    #[serde(default = "default_branch")]
    pub default_branch: BranchName,
    pub projects: Vec<Project>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct Project {
    pub name: String,
    pub origin: RepositoryOrigin,
    pub branches: Option<Vec<BranchName>>,
    pub team: Option<String>,
}

impl Configuration {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(serde_yaml::from_reader(reader)?)
    }

    pub fn get_branch_name_max_len(&self) -> usize {
        self.projects
            .iter()
            .map(|projet| projet.name.len())
            .max()
            .unwrap_or(0)
    }
}

impl Project {
    pub fn get_branches_name(&self, default: &[BranchName]) -> Vec<BranchName> {
        self.branches.as_deref().unwrap_or(default).to_owned()
    }
}

fn default_branch() -> BranchName {
    "master".to_string().into()
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_parse_bare_project() {
        let input = r#"
projects:
  - name: repo
    origin: git@example.com:user/repository.git
"#;
        let expected = Configuration {
            default_branch: "master".to_string().into(),
            projects: vec![Project {
                name: "repo".to_string(),
                origin: "git@example.com:user/repository.git".to_string().into(),
                branches: None,
                team: None,
            }],
        };
        let output = serde_yaml::from_str(input).unwrap();
        assert_eq!(expected, output);
    }

    #[test]
    fn test_parse_project_with_branches() {
        let input = r#"
projects:
  - name: repo
    origin: git@example.com:user/repository.git
    branches:
      - foo
      - bar
    team: X functional
"#;
        let expected = Configuration {
            default_branch: "master".to_string().into(),
            projects: vec![Project {
                name: "repo".to_string(),
                origin: "git@example.com:user/repository.git".to_string().into(),
                branches: Some(vec!["foo".to_string().into(), "bar".to_string().into()]),
                team: Some("X functional".to_string()),
            }],
        };
        let ouput = serde_yaml::from_str(input).unwrap();
        assert_eq!(expected, ouput);
    }
}
