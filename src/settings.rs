use crate::types::Permissions;
use std::path::{Path, PathBuf};

/// Walk up from `start_dir` to find the nearest `.claude/` directory,
/// then load and merge `settings.json` and `settings.local.json`.
/// Returns None if no `.claude/` directory is found.
pub fn load_permissions(start_dir: &Path) -> Option<Permissions> {
    let claude_dir = find_claude_dir(start_dir)?;
    let settings_path = claude_dir.clone();

    let project = load_file(&claude_dir.join("settings.json"));
    let local = load_file(&claude_dir.join("settings.local.json"));

    if project.is_none() && local.is_none() {
        return Some(Permissions {
            settings_path,
            ..Default::default()
        });
    }

    let mut allow = Vec::new();
    let mut deny = Vec::new();
    let mut ask = Vec::new();

    for perms in [&project, &local].into_iter().flatten() {
        allow.extend(perms.allow.iter().cloned());
        deny.extend(perms.deny.iter().cloned());
        ask.extend(perms.ask.iter().cloned());
    }

    Some(Permissions {
        allow,
        deny,
        ask,
        settings_path,
    })
}

/// Walk up from a directory to find the nearest ancestor containing .claude/
fn find_claude_dir(start: &Path) -> Option<PathBuf> {
    let mut dir = start.to_path_buf();
    loop {
        let candidate = dir.join(".claude");
        if candidate.is_dir() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Parsed permission arrays from a single settings file
#[derive(Default)]
struct FilePermissions {
    allow: Vec<String>,
    deny: Vec<String>,
    ask: Vec<String>,
}

/// Load a single settings file and extract permissions arrays.
fn load_file(path: &Path) -> Option<FilePermissions> {
    let content = std::fs::read_to_string(path).ok()?;
    let value: serde_json::Value = serde_json::from_str(&content).ok()?;

    let perms = value.get("permissions")?;

    Some(FilePermissions {
        allow: extract_string_array(perms, "allow"),
        deny: extract_string_array(perms, "deny"),
        ask: extract_string_array(perms, "ask"),
    })
}

fn extract_string_array(value: &serde_json::Value, key: &str) -> Vec<String> {
    value
        .get(key)
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}
