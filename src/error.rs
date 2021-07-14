use std::error::Error as StdError;
use std::fmt;

#[derive(Debug)]
pub enum Error {
    BranchDoesntExist(String, git2::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BranchDoesntExist(name, _) => write!(f, "Branch {} doesn't exist", name),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::BranchDoesntExist(_, source) => Some(source),
        }
    }
}
