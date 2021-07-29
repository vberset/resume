use std::env;
use std::path::PathBuf;

use blake3::hash;

use crate::snapshots::RepositoryOrigin;

#[cfg(target_os = "macos")]
fn get_cache_folder() -> PathBuf {
    PathBuf::from(format!(
        "{}/Library/Caches/resume",
        env::var("HOME").unwrap(),
    ))
}

#[cfg(target_os = "linux")]
pub fn get_cache_folder() -> PathBuf {
    let mut path = PathBuf::from(
        env::var("XDG_CACHE_HOME").unwrap_or(format!("{}/.cache", env::var("HOME").unwrap())),
    );
    path.push("resume");
    path
}

/// Get the user's cache folder where store the given repository origin
pub fn get_repo_cache_folder(origin: &RepositoryOrigin) -> PathBuf {
    let mut path = get_cache_folder();
    path.push(hash(origin.as_bytes()).to_string());
    path
}
