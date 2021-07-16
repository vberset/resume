use std::collections::HashMap;

use crate::message::{CommitType, ConventionalMessage};

pub type ChangeLog = HashMap<CommitType, Vec<ConventionalMessage>>;
