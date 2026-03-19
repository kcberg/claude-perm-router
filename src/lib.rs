pub mod matcher;
pub mod parser;
pub mod settings;
pub mod types;

use std::path::{Path, PathBuf};

/// Walk up from a directory to find the nearest ancestor containing a `.claude/` directory.
/// Returns the project root (the directory that contains `.claude/`), not `.claude/` itself.
pub fn find_project_root(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join(".claude").is_dir() {
            return Some(dir);
        }
        if !dir.pop() {
            return None;
        }
    }
}
