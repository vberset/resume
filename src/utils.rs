use std::env;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
pub fn get_cache_folder() -> PathBuf {
    PathBuf::from(format!(
        "{}/Library/Caches/resume",
        env::var("HOME").unwrap(),
    ))
}

#[cfg(target_os = "linux")]
pub fn get_cache_folder() -> PathBuf {
    PathBuf::from(
        env::var("XDG_CACHE_HOME")
            .unwrap_or_else(|| format!("{}/.cache/resume", env::var("HOME").unwrap(),)),
    )
}
