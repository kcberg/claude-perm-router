use crate::types::{Permissions, SegmentResult};

/// Match an effective command against a permission rule pattern.
///
/// Pattern forms:
/// - `cmd:*` — prefix match (`:` separator, matches anything starting with `cmd`).
///   Note: this intentionally has no word-boundary requirement — `./gradlew:*` matches
///   both `./gradlew test` and `./gradlew`. This matches Claude Code's behavior.
/// - `cmd *` — prefix match (space-star, matches anything starting with `cmd `)
/// - `cmd` — exact match
/// - `*` elsewhere is literal
pub fn match_rule(pattern: &str, command: &str) -> bool {
    // Check for :* suffix (prefix match with : separator)
    if let Some(prefix) = pattern.strip_suffix(":*") {
        return command.starts_with(prefix);
    }

    // Check for trailing " *" (space-star prefix match)
    if let Some(prefix) = pattern.strip_suffix(" *") {
        let prefix_with_space = format!("{prefix} ");
        return command.starts_with(&prefix_with_space) || command == prefix;
    }

    // Exact match (any remaining * is literal)
    command == pattern
}

/// Extract the inner pattern from a `Bash(pattern)` rule.
/// Returns None if the rule is not a Bash rule.
fn extract_bash_pattern(rule: &str) -> Option<&str> {
    let trimmed = rule.trim();
    if let Some(inner) = trimmed.strip_prefix("Bash(") {
        if let Some(pattern) = inner.strip_suffix(')') {
            return Some(pattern);
        }
    }
    None
}

/// Evaluate a single command against a set of permissions.
/// Order: deny → allow → ask → unresolved
pub fn evaluate_segment(command: &str, permissions: &Permissions) -> SegmentResult {
    // 1. Check deny
    for rule in &permissions.deny {
        if let Some(pattern) = extract_bash_pattern(rule) {
            if match_rule(pattern, command) {
                return SegmentResult::Denied {
                    rule: rule.clone(),
                    settings_path: permissions.settings_path.clone(),
                };
            }
        }
    }

    // 2. Check allow
    for rule in &permissions.allow {
        if let Some(pattern) = extract_bash_pattern(rule) {
            if match_rule(pattern, command) {
                return SegmentResult::Allowed {
                    rule: rule.clone(),
                    settings_path: permissions.settings_path.clone(),
                };
            }
        }
    }

    // 3. Check ask
    for rule in &permissions.ask {
        if let Some(pattern) = extract_bash_pattern(rule) {
            if match_rule(pattern, command) {
                return SegmentResult::Ask {
                    rule: rule.clone(),
                    settings_path: permissions.settings_path.clone(),
                };
            }
        }
    }

    // 4. No match
    SegmentResult::Unresolved
}
