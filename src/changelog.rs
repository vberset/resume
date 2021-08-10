use std::{fmt, hash::Hash, str::FromStr};

use indexmap::map::IndexMap;
use serde::Serialize;

use crate::{
    error::{Error, Result},
    message::ConventionalMessage,
    snapshots::{BranchName, RepositoryOrigin},
};
use std::fmt::Debug;

#[derive(Clone, Serialize)]
pub struct ChangeLogEntry {
    origin: RepositoryOrigin,
    branch: BranchName,
    message: ConventionalMessage,
}

impl ChangeLogEntry {
    pub fn new(origin: RepositoryOrigin, branch: BranchName, message: ConventionalMessage) -> Self {
        Self {
            origin,
            branch,
            message,
        }
    }

    pub fn get(&self, field: &CommitField) -> &str {
        use CommitField::*;
        match field {
            Scope => self
                .message
                .scope
                .as_ref()
                .map(|scope| scope.as_str())
                .unwrap_or(""),
            Branch => self.branch.as_str(),
            Origin => self.origin.as_str(),
            CommitType => self.message.ctype.as_str(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum HierarchicalBuckets<K, V>
where
    K: Eq + Hash + Serialize,
    V: Serialize,
{
    Index(IndexMap<K, HierarchicalBuckets<K, V>>),
    Bucket(Vec<V>),
}

impl<'de, K, V> HierarchicalBuckets<K, V>
where
    K: Debug + Eq + Hash + Serialize,
    V: Serialize,
{
    pub fn insert(&mut self, mut keys: Vec<K>, value: V) -> Result<()> {
        keys.reverse();
        self.insert_helper(keys, value)
    }

    fn insert_helper(&mut self, mut keys: Vec<K>, value: V) -> Result<()> {
        match (keys.pop(), self) {
            (Some(key), HierarchicalBuckets::Index(index)) => {
                match index.get_mut(&key) {
                    Some(child) => child.insert_helper(keys, value)?,
                    None => {
                        let mut child = if keys.is_empty() {
                            HierarchicalBuckets::Bucket(Vec::new())
                        } else {
                            HierarchicalBuckets::Index(IndexMap::new())
                        };
                        child.insert_helper(keys, value)?;
                        index.insert(key, child);
                    }
                }
                Ok(())
            }
            (None, HierarchicalBuckets::Bucket(bucket)) => {
                bucket.push(value);
                Ok(())
            }
            (Some(key), HierarchicalBuckets::Bucket(_)) => Err(Error::InvalidIndex(format!(
                "expected index on key {:?}, found bucket",
                &key
            ))),
            (None, HierarchicalBuckets::Index(_)) => Err(Error::InvalidIndex(
                "expected bucket, found index".to_string(),
            )),
        }
    }
}

pub struct ChangeLog {
    group_by: Vec<CommitField>,
    index: HierarchicalBuckets<String, ChangeLogEntry>,
}

impl ChangeLog {
    pub fn new(group_by: Vec<CommitField>) -> Self {
        let index = if group_by.is_empty() {
            HierarchicalBuckets::Bucket(Vec::new())
        } else {
            HierarchicalBuckets::Index(IndexMap::new())
        };

        Self { group_by, index }
    }

    pub fn insert(&mut self, entry: ChangeLogEntry) -> Result<()> {
        let keys = self
            .group_by
            .iter()
            .map(|field| entry.get(field).to_owned())
            .collect();
        self.index.insert(keys, entry)?;
        Ok(())
    }

    pub fn to_yaml(&self) -> Result<String> {
        Ok(serde_yaml::to_string(&self.index)?)
    }
}

#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum CommitField {
    Scope,
    Branch,
    Origin,
    CommitType,
}

impl fmt::Display for CommitField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use CommitField::*;

        let scope = match self {
            Scope => "scope",
            Branch => "branch",
            Origin => "origin",
            CommitType => "commit-type",
        };
        writeln!(f, "{}", scope)
    }
}

impl FromStr for CommitField {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "scope" => Ok(Self::Scope),
            "branch" => Ok(Self::Branch),
            "origin" => Ok(Self::Origin),
            "commit-type" => Ok(Self::CommitType),
            _ => Err(Error::InvalidSelector(s.to_owned())),
        }
    }
}
