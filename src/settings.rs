use crate::find_project_root;
use crate::types::Permissions;
use std::path::Path;

/// Walk up from `start_dir` to find the nearest `.claude/` directory,
/// then load and merge `settings.json` and `settings.local.json`.
/// Returns None if no `.claude/` directory is found.
pub fn load_permissions(start_dir: &Path) -> Option<Permissions> {
    let project_root = find_project_root(start_dir)?;
    let claude_dir = project_root.join(".claude");

    let project = load_file(&claude_dir.join("settings.json"));
    let local = load_file(&claude_dir.join("settings.local.json"));

    if project.is_none() && local.is_none() {
        return Some(Permissions {
            settings_path: claude_dir,
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
        settings_path: claude_dir,
    })
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
