use std::str::FromStr;

use crate::error::{Error, Result};

#[derive(Debug, Eq, PartialEq)]
pub enum OutputType {
    Yaml,
}

impl FromStr for OutputType {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "yaml" => Ok(OutputType::Yaml),
            _ => Err(Error::OutputType(s.to_string())),
        }
    }
}
