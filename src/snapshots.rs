use std::{
    collections::BTreeMap,
    fmt::{self, Formatter},
    fs::File,
    io::BufReader,
    path::Path,
    str::FromStr,
};

use blake3::{Hash, Hasher};
use git2::Oid;
use serde::{Deserialize, Serialize};

use crate::error::Result;
use std::io::BufWriter;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub struct CommitHash(String);

impl CommitHash {
    pub fn as_str(&self) -> &str {
        &self.0.as_str()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0.as_bytes()
    }
}

impl From<Oid> for CommitHash {
    fn from(oid: Oid) -> Self {
        Self(oid.to_string())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub struct BranchName(String);

impl BranchName {
    pub fn as_str(&self) -> &str {
        &self.0.as_str()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0.as_bytes()
    }
}

impl From<String> for BranchName {
    fn from(string: String) -> Self {
        Self(string)
    }
}

impl FromStr for BranchName {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(s.to_owned().into())
    }
}

impl fmt::Display for BranchName {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub struct RepositoryOrigin(String);

impl RepositoryOrigin {
    pub fn as_str(&self) -> &str {
        &self.0.as_str()
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0.as_bytes()
    }
}

impl From<String> for RepositoryOrigin {
    fn from(string: String) -> Self {
        Self(string)
    }
}

impl FromStr for RepositoryOrigin {
    type Err = ();

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Ok(s.to_owned().into())
    }
}

impl fmt::Display for RepositoryOrigin {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Ord, PartialOrd)]
pub struct SnapshotHash(String);

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotHistory {
    snapshots: Vec<Snapshot>,
}

impl SnapshotHash {
    pub fn from_hash(hash: Hash) -> Self {
        Self(hash.to_hex().as_str().to_owned())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Snapshot {
    hash: SnapshotHash,
    repositories: BTreeMap<RepositoryOrigin, RepositorySnapshot>,
}

pub struct SnapshotBuilder {
    repositories: BTreeMap<RepositoryOrigin, RepositorySnapshot>,
}

pub type RepositorySnapshot = BTreeMap<BranchName, CommitHash>;

impl SnapshotHistory {
    pub fn new() -> Self {
        Self {
            snapshots: Vec::new(),
        }
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        Ok(serde_yaml::from_reader(reader)?)
    }

    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let file = File::create(path)?;
        let writer = BufWriter::new(file);
        Ok(serde_yaml::to_writer(writer, &self)?)
    }

    pub fn last(&self) -> Option<&Snapshot> {
        self.snapshots.last()
    }

    pub fn get_by_hash(&self, hash: &SnapshotHash) -> Option<&Snapshot> {
        for snapshot in self.snapshots.iter().rev() {
            if &snapshot.hash == hash {
                return Some(snapshot);
            }
        }
        None
    }

    pub fn get_by_index(&self, index: usize) -> Option<&Snapshot> {
        self.snapshots.get(self.snapshots.len() - index - 1)
    }

    pub fn push(&mut self, snapshot: Snapshot) {
        self.snapshots.push(snapshot);
    }
}

impl Snapshot {
    pub fn get(&self, origin: &RepositoryOrigin) -> Option<&RepositorySnapshot> {
        self.repositories.get(origin)
    }
}

impl SnapshotBuilder {
    pub fn new() -> Self {
        Self {
            repositories: BTreeMap::new(),
        }
    }

    pub fn add_repository_snapshot(
        &mut self,
        origin: RepositoryOrigin,
        snapshot: RepositorySnapshot,
    ) {
        self.repositories.insert(origin, snapshot);
    }

    pub fn build(self) -> Snapshot {
        let mut hasher = Hasher::new();
        for (origin, branches) in &self.repositories {
            hasher.update(origin.as_bytes());
            for (branch_name, head) in branches {
                hasher.update(branch_name.as_bytes());
                hasher.update(head.as_bytes());
            }
        }

        Snapshot {
            hash: SnapshotHash::from_hash(hasher.finalize()),
            repositories: self.repositories,
        }
    }
}
