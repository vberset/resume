use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use serde::Deserialize;

use crate::error::Result;

#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct Configuration {
    #[serde(default = "default_branch")]
    pub default_branch: String,
    pub projects: Vec<Project>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
pub struct Project {
    pub name: String,
    pub origin: String,
    pub branches: Option<Vec<String>>,
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
    pub fn get_branches_name(&self, default: &[String]) -> Vec<String> {
        self.branches.as_deref().unwrap_or(default).to_owned()
    }
}

fn default_branch() -> String {
    "master".to_string()
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
            default_branch: "master".to_string(),
            projects: vec![Project {
                name: "repo".to_string(),
                origin: "git@example.com:user/repository.git".to_string(),
                branches: None,
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
"#;
        let expected = Configuration {
            default_branch: "master".to_string(),
            projects: vec![Project {
                name: "repo".to_string(),
                origin: "git@example.com:user/repository.git".to_string(),
                branches: Some(vec!["foo".to_string(), "bar".to_string()]),
            }],
        };
        let ouput = serde_yaml::from_str(input).unwrap();
        assert_eq!(expected, ouput);
    }
}
