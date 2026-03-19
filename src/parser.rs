use crate::types::EvaluatedSegment;
use std::path::PathBuf;

/// A raw split segment before classification
#[derive(Debug, Clone, PartialEq)]
pub struct RawSegment {
    pub text: String,
    pub is_pipe: bool,
}

/// Split a command string on unquoted &&, ||, ;, and |.
/// Tracks whether each segment was preceded by a pipe operator.
pub fn split_command(cmd: &str) -> Vec<RawSegment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut is_pipe = false;
    let chars: Vec<char> = cmd.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        // Track quote state
        if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            current.push(c);
            i += 1;
            continue;
        }
        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            current.push(c);
            i += 1;
            continue;
        }

        // Only split when not inside quotes
        if !in_single_quote && !in_double_quote {
            // Check for && or ||
            if i + 1 < len
                && ((c == '&' && chars[i + 1] == '&')
                    || (c == '|' && chars[i + 1] == '|'))
            {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    segments.push(RawSegment {
                        text: trimmed,
                        is_pipe,
                    });
                }
                current.clear();
                is_pipe = false; // && and || are chain operators
                i += 2;
                continue;
            }

            // Check for ; (chain) or | (pipe)
            if c == ';' {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    segments.push(RawSegment {
                        text: trimmed,
                        is_pipe,
                    });
                }
                current.clear();
                is_pipe = false;
                i += 1;
                continue;
            }

            if c == '|' {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    segments.push(RawSegment {
                        text: trimmed,
                        is_pipe,
                    });
                }
                current.clear();
                is_pipe = true;
                i += 1;
                continue;
            }
        }

        current.push(c);
        i += 1;
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        segments.push(RawSegment {
            text: trimmed,
            is_pipe,
        });
    }

    segments
}

/// Parse a full command string into evaluated segments with resolved directories.
/// `cd` segments are consumed by the accumulator and not returned.
pub fn parse_command(cmd: &str) -> Vec<EvaluatedSegment> {
    let raw_segments = split_command(cmd);
    let mut result = Vec::new();
    let mut accumulator: Option<PathBuf> = None;
    // Save accumulator state before entering a pipe group so we can restore it after
    let mut pre_pipe_accumulator: Option<PathBuf> = None;
    let mut in_pipe_group = false;

    for raw in &raw_segments {
        let text = raw.text.trim();

        // Track pipe group transitions
        if raw.is_pipe && !in_pipe_group {
            // Entering a pipe group — save accumulator
            pre_pipe_accumulator = accumulator.clone();
            in_pipe_group = true;
        } else if !raw.is_pipe && in_pipe_group {
            // Leaving a pipe group — restore accumulator
            accumulator = pre_pipe_accumulator.take();
            in_pipe_group = false;
        }

        // Classify the segment
        if let Some(path) = parse_cd(text) {
            // cd updates accumulator, not returned as a segment
            if path.starts_with('/') {
                accumulator = try_canonicalize(&PathBuf::from(&path));
            } else if path == ".." || path == "-" || path.starts_with("..") {
                // Relative paths that need CWD to resolve
                if let Some(acc) = accumulator {
                    accumulator = try_canonicalize(&acc.join(&path));
                }
                // If no accumulator, leave it as None (unresolvable)
            } else if let Some(acc) = accumulator {
                accumulator = try_canonicalize(&acc.join(&path));
            }
            // If no accumulator and relative path, do nothing
            continue;
        }

        if let Some((dir, cmd)) = parse_git_c(text, &accumulator) {
            result.push(EvaluatedSegment {
                target_dir: dir,
                effective_cmd: cmd,
                raw_segment: text.to_string(),
            });
            continue;
        }

        if let Some((dir, cmd)) = parse_absolute_executable(text) {
            result.push(EvaluatedSegment {
                target_dir: dir,
                effective_cmd: cmd,
                raw_segment: text.to_string(),
            });
            continue;
        }

        // Plain command — uses accumulator
        result.push(EvaluatedSegment {
            target_dir: accumulator.clone(),
            effective_cmd: text.to_string(),
            raw_segment: text.to_string(),
        });
    }

    result
}

/// Extract path from a `cd <path>` command. Returns None if not a cd command.
/// Handles tilde expansion and quoted paths.
fn parse_cd(segment: &str) -> Option<String> {
    let trimmed = segment.trim();
    if trimmed == "cd" {
        return Some(String::new()); // bare cd, no path
    }
    if let Some(rest) = trimmed.strip_prefix("cd ") {
        let path = unquote(rest.trim());
        // Expand ~ to home directory
        if path == "~" {
            if let Some(home) = std::env::var_os("HOME") {
                return Some(home.to_string_lossy().into_owned());
            }
        } else if let Some(rest) = path.strip_prefix("~/") {
            if let Some(home) = std::env::var_os("HOME") {
                return Some(format!("{}/{rest}", home.to_string_lossy()));
            }
        }
        Some(path)
    } else {
        None
    }
}

/// Parse `git -C <path> <subcmd>` segments.
/// Returns (target_dir, effective_cmd) or None if not a git -C command.
/// Handles quoted paths (e.g., `git -C "/path with spaces" status`).
fn parse_git_c(segment: &str, accumulator: &Option<PathBuf>) -> Option<(Option<PathBuf>, String)> {
    let words = split_words(segment);
    // Need at least: git -C <path> <subcmd>
    if words.len() >= 4 && words[0] == "git" && words[1] == "-C" {
        let path = unquote(&words[2]);
        let subcmd = words[3..].join(" ");
        let effective_cmd = format!("git {subcmd}");

        let target_dir = if path.starts_with('/') {
            try_canonicalize(&PathBuf::from(&path))
        } else if let Some(acc) = accumulator {
            try_canonicalize(&acc.join(&path))
        } else {
            None // relative path with no accumulator
        };

        return Some((target_dir, effective_cmd));
    }
    None
}

/// Split a segment into words, respecting single and double quotes.
/// Quoted strings are kept as single tokens (with quotes preserved).
fn split_words(s: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for c in s.chars() {
        match c {
            '\'' if !in_double => {
                in_single = !in_single;
                current.push(c);
            }
            '"' if !in_single => {
                in_double = !in_double;
                current.push(c);
            }
            ' ' if !in_single && !in_double => {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    words.push(trimmed);
                }
                current.clear();
            }
            _ => current.push(c),
        }
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        words.push(trimmed);
    }

    words
}

/// Remove surrounding quotes from a string if present.
fn unquote(s: &str) -> String {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix('"').and_then(|s| s.strip_suffix('"')) {
        inner.to_string()
    } else if let Some(inner) = s.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        inner.to_string()
    } else {
        s.to_string()
    }
}

/// Parse absolute executable paths (e.g., /foo/bar/dist/hawk scan).
/// Returns (target_dir, effective_cmd) or None if not an absolute path command.
fn parse_absolute_executable(segment: &str) -> Option<(Option<PathBuf>, String)> {
    if !segment.starts_with('/') {
        return None;
    }

    let parts: Vec<&str> = segment.splitn(2, ' ').collect();
    let executable_path = PathBuf::from(parts[0]);

    // Walk up from executable's parent to find .claude/
    let parent = executable_path.parent()?;
    let target_dir = find_claude_dir(parent).and_then(|p| try_canonicalize(&p));

    let basename = executable_path.file_name()?.to_str()?;
    let effective_cmd = if parts.len() > 1 {
        format!("{basename} {}", parts[1])
    } else {
        basename.to_string()
    };

    Some((target_dir, effective_cmd))
}

/// Try to canonicalize a path, resolving `..`, symlinks, etc.
/// Returns None if the path doesn't exist on disk.
fn try_canonicalize(path: &std::path::Path) -> Option<PathBuf> {
    std::fs::canonicalize(path).ok()
}

/// Walk up from a directory to find the nearest ancestor containing .claude/
fn find_claude_dir(start: &std::path::Path) -> Option<PathBuf> {
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
