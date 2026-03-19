# claude-perm-router Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust CLI that intercepts Claude Code Bash commands, parses them into segments with directory context, loads per-directory `.claude/settings*.json` permissions, and returns allow/deny/ask decisions.

**Architecture:** Synchronous stdin→stdout pipe with four modules: `types.rs` (shared types), `parser.rs` (quote-aware command splitting and directory tracking), `settings.rs` (filesystem settings discovery and merging), `matcher.rs` (permission rule matching and aggregation). `main.rs` orchestrates the pipeline.

**Tech Stack:** Rust (edition 2024), serde + serde_json for JSON I/O. No async, no CLI framework.

**Spec:** `docs/superpowers/specs/2026-03-19-claude-perm-router-design.md`

---

## File Structure

| File | Responsibility |
|---|---|
| `Cargo.toml` | Add serde, serde_json dependencies |
| `src/types.rs` | All shared types: `HookInput`, `HookOutput`, `EvaluatedSegment`, `SegmentResult`, `PermissionDecision`, `Operator` |
| `src/parser.rs` | `parse_command(cmd: &str) -> Vec<EvaluatedSegment>` — quote-aware split, segment classification, directory accumulator |
| `src/settings.rs` | `load_permissions(dir: &Path) -> Option<Permissions>` — walk up for `.claude/`, load+merge JSON, extract allow/deny/ask arrays |
| `src/matcher.rs` | `match_rule(pattern: &str, cmd: &str) -> bool` and `evaluate(segments: Vec<EvaluatedSegment>) -> Option<HookOutput>` — rule parsing, per-segment evaluation, cross-segment aggregation |
| `src/main.rs` | Read stdin, deserialize, gate on `tool_name == "Bash"`, call parser→settings→matcher, serialize output |
| `tests/parser_tests.rs` | Unit tests for all 15 parser test cases from spec |
| `tests/matcher_tests.rs` | Unit tests for all 9 matcher test cases from spec |
| `tests/settings_tests.rs` | Unit tests with temp dirs for settings discovery and merging |
| `tests/integration_tests.rs` | End-to-end binary tests for all 11 integration test cases from spec |

---

## Task 1: Project Setup and Types

**Files:**
- Modify: `Cargo.toml`
- Create: `src/types.rs`
- Modify: `src/main.rs`

- [ ] **Step 1: Add dependencies to Cargo.toml**

```toml
[package]
name = "claude-perm-router"
version = "0.1.0"
edition = "2024"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

- [ ] **Step 2: Create src/types.rs with all shared types**

```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Raw input from Claude Code hook on stdin
#[derive(Debug, Deserialize)]
pub struct HookInput {
    pub session_id: String,
    pub tool_name: String,
    pub tool_input: ToolInput,
}

#[derive(Debug, Deserialize)]
pub struct ToolInput {
    pub command: Option<String>,
}

/// Output JSON for Claude Code hook on stdout
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    pub hook_specific_output: HookSpecificOutput,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookSpecificOutput {
    pub hook_event_name: String,
    pub permission_decision: String,
    pub permission_decision_reason: String,
}

impl HookOutput {
    pub fn new(decision: PermissionDecision, reason: String) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PreToolUse".to_string(),
                permission_decision: decision.as_str().to_string(),
                permission_decision_reason: reason,
            },
        }
    }
}

/// The three possible permission decisions
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionDecision {
    Allow,
    Deny,
    Ask,
}

impl PermissionDecision {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Ask => "ask",
        }
    }
}

/// Result of evaluating a single segment against permissions
#[derive(Debug, Clone, PartialEq)]
pub enum SegmentResult {
    Allowed { rule: String, settings_path: PathBuf },
    Denied { rule: String, settings_path: PathBuf },
    Ask { rule: String, settings_path: PathBuf },
    Unresolved,
}

/// Which shell operator preceded a segment
#[derive(Debug, Clone, PartialEq)]
pub enum Operator {
    /// First segment or after &&, ||, ;
    Chain,
    /// After |
    Pipe,
}

/// A parsed command segment with its resolved directory and effective command
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedSegment {
    pub target_dir: Option<PathBuf>,
    pub effective_cmd: String,
    pub raw_segment: String,
}

/// Loaded and merged permission rules from .claude/settings*.json
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub ask: Vec<String>,
    pub settings_path: PathBuf,
}
```

- [ ] **Step 3: Update src/main.rs to declare modules**

```rust
mod matcher;
mod parser;
mod settings;
mod types;

fn main() {
    // TODO: will be implemented in Task 6
}
```

- [ ] **Step 4: Create empty module files so it compiles**

Create `src/parser.rs`, `src/settings.rs`, `src/matcher.rs` as empty files.

- [ ] **Step 5: Verify it compiles**

Run: `cargo build`
Expected: Compiles with warnings about unused imports/dead code (that's fine)

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock src/
git commit -m "feat: add project dependencies and shared types"
```

---

## Task 2: Parser — Quote-Aware Splitting

**Files:**
- Modify: `src/parser.rs`
- Create: `tests/parser_tests.rs`

- [ ] **Step 1: Write failing tests for quote-aware splitting**

In `tests/parser_tests.rs`:

```rust
use claude_perm_router::parser::split_command;

#[test]
fn split_simple_and() {
    let parts = split_command("cd /foo && ./gradlew test");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].text, "cd /foo");
    assert_eq!(parts[1].text, "./gradlew test");
    assert!(!parts[1].is_pipe);
}

#[test]
fn split_semicolon() {
    let parts = split_command("cd /foo ; ls");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].text, "cd /foo");
    assert_eq!(parts[1].text, "ls");
}

#[test]
fn split_pipe() {
    let parts = split_command("cd /foo && cmd1 | cmd2");
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[2].text, "cmd2");
    assert!(parts[2].is_pipe);
}

#[test]
fn split_multi_pipe() {
    let parts = split_command("cd /foo && cmd1 | cmd2 | cmd3 && cmd4");
    assert_eq!(parts.len(), 5);
    assert!(parts[2].is_pipe);  // cmd2
    assert!(parts[3].is_pipe);  // cmd3
    assert!(!parts[4].is_pipe); // cmd4
}

#[test]
fn split_quoted_and() {
    let parts = split_command(r#"echo "hello && world""#);
    assert_eq!(parts.len(), 1);
    assert_eq!(parts[0].text, r#"echo "hello && world""#);
}

#[test]
fn split_single_quoted() {
    let parts = split_command("echo 'a && b' && ls");
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].text, "echo 'a && b'");
    assert_eq!(parts[1].text, "ls");
}

#[test]
fn split_or_operator() {
    let parts = split_command("cmd1 || cmd2");
    assert_eq!(parts.len(), 2);
    assert!(!parts[1].is_pipe);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test parser_tests`
Expected: FAIL — `split_command` doesn't exist

- [ ] **Step 3: Implement quote-aware splitting in src/parser.rs**

```rust
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
```

Also add `pub mod parser;` to `src/lib.rs` (create `src/lib.rs` to expose modules for tests):

```rust
pub mod matcher;
pub mod parser;
pub mod settings;
pub mod types;
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test parser_tests`
Expected: All 7 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/parser.rs src/lib.rs tests/parser_tests.rs
git commit -m "feat: implement quote-aware command splitting"
```

---

## Task 3: Parser — Segment Classification and Directory Accumulator

**Files:**
- Modify: `src/parser.rs`
- Modify: `tests/parser_tests.rs`

- [ ] **Step 1: Write failing tests for parse_command (full pipeline)**

Append to `tests/parser_tests.rs`:

```rust
use claude_perm_router::parser::parse_command;
use std::path::PathBuf;

#[test]
fn parse_cd_and_command() {
    // Spec parser test 1
    let segs = parse_command("cd /foo && ./gradlew test");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(PathBuf::from("/foo")));
    assert_eq!(segs[0].effective_cmd, "./gradlew test");
}

#[test]
fn parse_cd_semicolon() {
    // Spec parser test 2
    let segs = parse_command("cd /foo ; ls");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(PathBuf::from("/foo")));
    assert_eq!(segs[0].effective_cmd, "ls");
}

#[test]
fn parse_cd_chain_multiple() {
    // Spec parser test 3
    let segs = parse_command("cd /foo && cmd1 && cmd2");
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0].target_dir, Some(PathBuf::from("/foo")));
    assert_eq!(segs[0].effective_cmd, "cmd1");
    assert_eq!(segs[1].target_dir, Some(PathBuf::from("/foo")));
    assert_eq!(segs[1].effective_cmd, "cmd2");
}

#[test]
fn parse_cd_pipe_inherits() {
    // Spec parser test 4
    let segs = parse_command("cd /foo && cmd1 | cmd2");
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0].target_dir, Some(PathBuf::from("/foo")));
    assert_eq!(segs[1].target_dir, Some(PathBuf::from("/foo")));
}

#[test]
fn parse_git_c() {
    // Spec parser test 5
    let segs = parse_command("git -C /foo status");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(PathBuf::from("/foo")));
    assert_eq!(segs[0].effective_cmd, "git status");
}

#[test]
fn parse_git_c_independent_dirs() {
    // Spec parser test 6
    let segs = parse_command("git -C /foo status && git -C /bar push");
    assert_eq!(segs.len(), 2);
    assert_eq!(segs[0].target_dir, Some(PathBuf::from("/foo")));
    assert_eq!(segs[1].target_dir, Some(PathBuf::from("/bar")));
}

#[test]
fn parse_no_directory_context() {
    // Spec parser test 9
    let segs = parse_command("./gradlew test");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, None);
    assert_eq!(segs[0].effective_cmd, "./gradlew test");
}

#[test]
fn parse_cd_relative_accumulation() {
    // Spec parser test 10
    let segs = parse_command("cd /foo && cd bar && ls");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(PathBuf::from("/foo/bar")));
    assert_eq!(segs[0].effective_cmd, "ls");
}

#[test]
fn parse_quoted_not_split() {
    // Spec parser test 11
    let segs = parse_command(r#"echo "hello && world""#);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, None);
}

#[test]
fn parse_multi_pipe_chain() {
    // Spec parser test 12
    let segs = parse_command("cd /foo && cmd1 | cmd2 | cmd3 && cmd4");
    assert_eq!(segs.len(), 4);
    for seg in &segs {
        assert_eq!(seg.target_dir, Some(PathBuf::from("/foo")));
    }
}

#[test]
fn parse_git_c_relative_with_accumulator() {
    // Spec parser test 13
    let segs = parse_command("cd /foo && git -C ../bar status");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(PathBuf::from("/foo/../bar")));
    assert_eq!(segs[0].effective_cmd, "git status");
}

#[test]
fn parse_git_c_relative_no_accumulator() {
    // Spec parser test 14
    let segs = parse_command("git -C ../bar status");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, None);
}

#[test]
fn parse_cd_dotdot_no_accumulator() {
    // Spec parser test 15
    let segs = parse_command("cd .. && ls");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, None);
}

#[test]
fn parse_absolute_executable_with_claude_dir() {
    // Spec parser test 7 — requires a temp dir with .claude/
    // This test is filesystem-dependent; uses tempfile
    use tempfile::TempDir;
    use std::fs;

    let tmp = TempDir::new().unwrap();
    let claude_dir = tmp.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();

    let bin_dir = tmp.path().join("build").join("dist");
    fs::create_dir_all(&bin_dir).unwrap();

    let executable = bin_dir.join("hawk");
    let cmd = format!("{} scan", executable.display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(tmp.path().to_path_buf()));
    assert_eq!(segs[0].effective_cmd, "hawk scan");
}

#[test]
fn parse_absolute_executable_no_claude_dir() {
    // Spec parser test 8 — no .claude/ found, segment is unresolved
    use tempfile::TempDir;
    use std::fs;

    let tmp = TempDir::new().unwrap();
    let bin_dir = tmp.path().join("build").join("dist");
    fs::create_dir_all(&bin_dir).unwrap();

    let executable = bin_dir.join("hawk");
    let cmd = format!("{} scan", executable.display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, None);
    assert_eq!(segs[0].effective_cmd, "hawk scan");
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test parser_tests`
Expected: FAIL — `parse_command` doesn't exist

- [ ] **Step 3: Implement segment classification and directory accumulator**

Add to `src/parser.rs`:

```rust
use crate::types::EvaluatedSegment;
use std::path::PathBuf;

/// Parse a full command string into evaluated segments with resolved directories.
/// `cd` segments are consumed by the accumulator and not returned.
pub fn parse_command(cmd: &str) -> Vec<EvaluatedSegment> {
    let raw_segments = split_command(cmd);
    let mut result = Vec::new();
    let mut accumulator: Option<PathBuf> = None;

    for raw in &raw_segments {
        let text = raw.text.trim();

        // Classify the segment
        if let Some(path) = parse_cd(text) {
            // cd updates accumulator, not returned as a segment
            if path.starts_with('/') {
                accumulator = Some(PathBuf::from(path));
            } else if path == ".." || path == "-" || path.starts_with("..") {
                // Relative paths that need CWD to resolve
                if let Some(ref acc) = accumulator {
                    accumulator = Some(acc.join(path));
                }
                // If no accumulator, leave it as None (unresolvable)
            } else if let Some(ref acc) = accumulator {
                accumulator = Some(acc.join(path));
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
fn parse_cd(segment: &str) -> Option<&str> {
    let trimmed = segment.trim();
    if trimmed == "cd" {
        return Some(""); // bare cd, no path
    }
    if let Some(rest) = trimmed.strip_prefix("cd ") {
        Some(rest.trim())
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
            Some(PathBuf::from(&path))
        } else if let Some(ref acc) = accumulator {
            Some(acc.join(&path))
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
    if (s.starts_with('"') && s.ends_with('"'))
        || (s.starts_with('\'') && s.ends_with('\''))
    {
        s[1..s.len() - 1].to_string()
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
    let target_dir = find_claude_dir(parent);

    let basename = executable_path.file_name()?.to_str()?;
    let effective_cmd = if parts.len() > 1 {
        format!("{basename} {}", parts[1])
    } else {
        basename.to_string()
    };

    Some((target_dir, effective_cmd))
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
```

Note: `splitn(4, ' ')` for `git -C` gives us `["git", "-C", "/path", "rest of subcmd"]` which is correct because the 4th element captures everything remaining.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test parser_tests`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/parser.rs tests/parser_tests.rs
git commit -m "feat: implement segment classification and directory accumulator"
```

---

## Task 4: Settings — Discovery and Loading

**Files:**
- Modify: `src/settings.rs`
- Create: `tests/settings_tests.rs`

- [ ] **Step 1: Write failing tests for settings loading**

In `tests/settings_tests.rs`:

```rust
use claude_perm_router::settings::load_permissions;
use std::fs;
use tempfile::TempDir;

fn create_settings(dir: &std::path::Path, filename: &str, content: &str) {
    let claude_dir = dir.join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(claude_dir.join(filename), content).unwrap();
}

#[test]
fn load_from_direct_dir() {
    let tmp = TempDir::new().unwrap();
    create_settings(tmp.path(), "settings.json", r#"{
        "permissions": {
            "allow": ["Bash(./gradlew:*)"],
            "deny": ["Bash(rm:*)"]
        }
    }"#);

    let perms = load_permissions(tmp.path()).unwrap();
    assert_eq!(perms.allow, vec!["Bash(./gradlew:*)"]);
    assert_eq!(perms.deny, vec!["Bash(rm:*)"]);
    assert!(perms.ask.is_empty());
    assert_eq!(perms.settings_path, tmp.path().join(".claude"));
}

#[test]
fn load_walks_up() {
    let tmp = TempDir::new().unwrap();
    create_settings(tmp.path(), "settings.json", r#"{
        "permissions": { "allow": ["Bash(git *)"] }
    }"#);

    let subdir = tmp.path().join("src").join("deep");
    fs::create_dir_all(&subdir).unwrap();

    let perms = load_permissions(&subdir).unwrap();
    assert_eq!(perms.allow, vec!["Bash(git *)"]);
}

#[test]
fn merge_local_and_project() {
    let tmp = TempDir::new().unwrap();
    create_settings(tmp.path(), "settings.json", r#"{
        "permissions": { "allow": ["Bash(git *)"] }
    }"#);
    create_settings(tmp.path(), "settings.local.json", r#"{
        "permissions": { "allow": ["Bash(./gradlew:*)"], "deny": ["Bash(rm:*)"] }
    }"#);

    let perms = load_permissions(tmp.path()).unwrap();
    // Both allow lists merged
    assert!(perms.allow.contains(&"Bash(git *)".to_string()));
    assert!(perms.allow.contains(&"Bash(./gradlew:*)".to_string()));
    assert_eq!(perms.deny, vec!["Bash(rm:*)"]);
}

#[test]
fn no_claude_dir_returns_none() {
    let tmp = TempDir::new().unwrap();
    let result = load_permissions(tmp.path());
    assert!(result.is_none());
}

#[test]
fn missing_permissions_key() {
    let tmp = TempDir::new().unwrap();
    create_settings(tmp.path(), "settings.json", r#"{ "someOtherKey": true }"#);

    let perms = load_permissions(tmp.path()).unwrap();
    assert!(perms.allow.is_empty());
    assert!(perms.deny.is_empty());
    assert!(perms.ask.is_empty());
}

#[test]
fn load_ask_rules() {
    let tmp = TempDir::new().unwrap();
    create_settings(tmp.path(), "settings.json", r#"{
        "permissions": { "ask": ["Bash(npm publish)"] }
    }"#);

    let perms = load_permissions(tmp.path()).unwrap();
    assert_eq!(perms.ask, vec!["Bash(npm publish)"]);
}
```

- [ ] **Step 2: Add tempfile dev-dependency to Cargo.toml**

Add under `[dependencies]`:
```toml
[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo test --test settings_tests`
Expected: FAIL — `load_permissions` doesn't exist

- [ ] **Step 4: Implement settings discovery and loading**

In `src/settings.rs`:

```rust
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
```

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --test settings_tests`
Expected: All 6 tests PASS

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml src/settings.rs tests/settings_tests.rs
git commit -m "feat: implement settings discovery, loading, and merging"
```

---

## Task 5: Matcher — Rule Parsing and Permission Evaluation

**Files:**
- Modify: `src/matcher.rs`
- Create: `tests/matcher_tests.rs`

- [ ] **Step 1: Write failing tests for rule matching**

In `tests/matcher_tests.rs`:

```rust
use claude_perm_router::matcher::{match_rule, evaluate_segment};
use claude_perm_router::types::{Permissions, SegmentResult};
use std::path::PathBuf;

#[test]
fn match_colon_star_prefix() {
    // Spec matcher test 1
    assert!(match_rule("./gradlew:*", "./gradlew test"));
    assert!(match_rule("./gradlew:*", "./gradlew build"));
    assert!(!match_rule("./gradlew:*", "npm test"));
}

#[test]
fn match_exact() {
    // Spec matcher test 2
    assert!(match_rule("./gradlew test", "./gradlew test"));
    assert!(!match_rule("./gradlew test", "./gradlew build"));
}

#[test]
fn match_space_star_prefix() {
    // Spec matcher test 3
    assert!(match_rule("git *", "git status"));
    assert!(match_rule("git *", "git push --force"));
    assert!(!match_rule("git *", "npm test"));
}

#[test]
fn match_literal_star_in_middle() {
    // Spec matcher test 9
    assert!(!match_rule("foo*bar", "fooXbar"));
    assert!(match_rule("foo*bar", "foo*bar")); // literal match
}

#[test]
fn evaluate_segment_deny() {
    // Spec matcher test 4
    let perms = Permissions {
        deny: vec!["Bash(git push:*)".to_string()],
        allow: vec![],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    let result = evaluate_segment("git push --force", &perms);
    assert!(matches!(result, SegmentResult::Denied { .. }));
}

#[test]
fn evaluate_segment_deny_over_allow() {
    // Spec matcher test 5
    let perms = Permissions {
        deny: vec!["Bash(git push:*)".to_string()],
        allow: vec!["Bash(git *)".to_string()],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    let result = evaluate_segment("git push", &perms);
    assert!(matches!(result, SegmentResult::Denied { .. }));
}

#[test]
fn evaluate_segment_allow() {
    let perms = Permissions {
        deny: vec![],
        allow: vec!["Bash(./gradlew:*)".to_string()],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    let result = evaluate_segment("./gradlew test", &perms);
    assert!(matches!(result, SegmentResult::Allowed { .. }));
}

#[test]
fn evaluate_segment_ask() {
    // Spec matcher test 7
    let perms = Permissions {
        deny: vec![],
        allow: vec![],
        ask: vec!["Bash(npm publish)".to_string()],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    let result = evaluate_segment("npm publish", &perms);
    assert!(matches!(result, SegmentResult::Ask { .. }));
}

#[test]
fn evaluate_segment_unresolved() {
    // Spec matcher test 6
    let perms = Permissions {
        deny: vec![],
        allow: vec!["Bash(git *)".to_string()],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    let result = evaluate_segment("npm test", &perms);
    assert!(matches!(result, SegmentResult::Unresolved));
}

#[test]
fn both_wildcard_styles_work() {
    // Spec matcher test 8
    assert!(match_rule("./gradlew:*", "./gradlew test"));
    assert!(match_rule("git *", "git status"));
}

#[test]
fn non_bash_rules_ignored() {
    let perms = Permissions {
        deny: vec![],
        allow: vec!["Read(/foo/*)".to_string(), "Bash(git *)".to_string()],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    // Read rule is silently ignored, Bash rule matches
    let result = evaluate_segment("git status", &perms);
    assert!(matches!(result, SegmentResult::Allowed { .. }));
}

#[test]
fn only_non_bash_rules_unresolved() {
    let perms = Permissions {
        deny: vec![],
        allow: vec!["Read(/foo/*)".to_string()],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    // Only non-Bash rules present — nothing matches
    let result = evaluate_segment("git status", &perms);
    assert!(matches!(result, SegmentResult::Unresolved));
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test matcher_tests`
Expected: FAIL — `match_rule` and `evaluate_segment` don't exist

- [ ] **Step 3: Implement rule matching and segment evaluation**

In `src/matcher.rs`:

```rust
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
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test matcher_tests`
Expected: All 10 tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/matcher.rs tests/matcher_tests.rs
git commit -m "feat: implement permission rule matching and segment evaluation"
```

---

## Task 6: Matcher — Cross-Segment Aggregation

**Files:**
- Modify: `src/matcher.rs`
- Modify: `tests/matcher_tests.rs`

- [ ] **Step 1: Write failing tests for aggregation**

Append to `tests/matcher_tests.rs`:

```rust
use claude_perm_router::matcher::aggregate;
use claude_perm_router::types::PermissionDecision;

#[test]
fn aggregate_all_allowed() {
    let results = vec![
        SegmentResult::Allowed {
            rule: "Bash(git *)".into(),
            settings_path: "/r/.claude".into(),
        },
        SegmentResult::Allowed {
            rule: "Bash(./gradlew:*)".into(),
            settings_path: "/r/.claude".into(),
        },
    ];
    let (decision, _reason) = aggregate(&results).unwrap();
    assert_eq!(decision, PermissionDecision::Allow);
}

#[test]
fn aggregate_one_denied() {
    let results = vec![
        SegmentResult::Allowed {
            rule: "Bash(git *)".into(),
            settings_path: "/r/.claude".into(),
        },
        SegmentResult::Denied {
            rule: "Bash(rm:*)".into(),
            settings_path: "/r/.claude".into(),
        },
    ];
    let (decision, _reason) = aggregate(&results).unwrap();
    assert_eq!(decision, PermissionDecision::Deny);
}

#[test]
fn aggregate_one_unresolved_falls_through() {
    let results = vec![
        SegmentResult::Allowed {
            rule: "Bash(git *)".into(),
            settings_path: "/r/.claude".into(),
        },
        SegmentResult::Unresolved,
    ];
    assert!(aggregate(&results).is_none());
}

#[test]
fn aggregate_ask_with_allowed() {
    let results = vec![
        SegmentResult::Allowed {
            rule: "Bash(git *)".into(),
            settings_path: "/r/.claude".into(),
        },
        SegmentResult::Ask {
            rule: "Bash(npm publish)".into(),
            settings_path: "/r/.claude".into(),
        },
    ];
    let (decision, _reason) = aggregate(&results).unwrap();
    assert_eq!(decision, PermissionDecision::Ask);
}

#[test]
fn aggregate_deny_beats_ask() {
    let results = vec![
        SegmentResult::Ask {
            rule: "Bash(npm publish)".into(),
            settings_path: "/r/.claude".into(),
        },
        SegmentResult::Denied {
            rule: "Bash(rm:*)".into(),
            settings_path: "/r/.claude".into(),
        },
    ];
    let (decision, _reason) = aggregate(&results).unwrap();
    assert_eq!(decision, PermissionDecision::Deny);
}

#[test]
fn aggregate_empty_falls_through() {
    assert!(aggregate(&[]).is_none());
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --test matcher_tests`
Expected: FAIL — `aggregate` doesn't exist

- [ ] **Step 3: Implement cross-segment aggregation**

Add to `src/matcher.rs`:

```rust
use crate::types::PermissionDecision;

/// Aggregate per-segment results into a final decision.
/// Returns None for fall-through (no output).
pub fn aggregate(results: &[SegmentResult]) -> Option<(PermissionDecision, String)> {
    if results.is_empty() {
        return None;
    }

    // 1. Any denied → deny
    for r in results {
        if let SegmentResult::Denied { rule, settings_path } = r {
            let reason = format!(
                "Denied: matched '{}' in {}",
                rule,
                settings_path.display()
            );
            return Some((PermissionDecision::Deny, reason));
        }
    }

    // 2. Any unresolved → fall through
    if results.iter().any(|r| matches!(r, SegmentResult::Unresolved)) {
        return None;
    }

    // 3. Any ask → ask
    for r in results {
        if let SegmentResult::Ask { rule, settings_path } = r {
            let reason = format!(
                "Ask: matched '{}' in {}",
                rule,
                settings_path.display()
            );
            return Some((PermissionDecision::Ask, reason));
        }
    }

    // 4. All allowed
    let reasons: Vec<String> = results
        .iter()
        .filter_map(|r| {
            if let SegmentResult::Allowed { rule, settings_path } = r {
                Some(format!("'{}' in {}", rule, settings_path.display()))
            } else {
                None
            }
        })
        .collect();

    let reason = format!(
        "All {} segment(s) allowed: {}",
        results.len(),
        reasons.join("; ")
    );

    Some((PermissionDecision::Allow, reason))
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --test matcher_tests`
Expected: All tests PASS

- [ ] **Step 5: Commit**

```bash
git add src/matcher.rs tests/matcher_tests.rs
git commit -m "feat: implement cross-segment permission aggregation"
```

---

## Task 7: Main — Orchestration Pipeline

**Files:**
- Modify: `src/main.rs`

- [ ] **Step 1: Implement the main orchestration**

Replace `src/main.rs`:

```rust
mod matcher;
mod parser;
mod settings;
mod types;

use std::io::Read;
use types::{HookInput, HookOutput};

fn main() {
    let result = run();
    match result {
        Ok(Some(output)) => {
            let json = serde_json::to_string(&output).expect("failed to serialize output");
            println!("{json}");
        }
        Ok(None) => {
            // Fall through — no output, exit 0
        }
        Err(e) => {
            eprintln!("claude-perm-router: {e}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<Option<HookOutput>, Box<dyn std::error::Error>> {
    // Read stdin
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    // Parse input
    let hook_input: HookInput = serde_json::from_str(&input)?;

    // Gate on Bash tool
    if hook_input.tool_name != "Bash" {
        return Ok(None);
    }

    // Extract command
    let command = match hook_input.tool_input.command {
        Some(cmd) if !cmd.is_empty() => cmd,
        _ => return Ok(None),
    };

    // Parse command into segments
    let segments = parser::parse_command(&command);

    if segments.is_empty() {
        return Ok(None);
    }

    // Evaluate each segment against its directory's permissions
    let mut results = Vec::new();

    for segment in &segments {
        match &segment.target_dir {
            None => {
                // No target directory — unresolved
                results.push(types::SegmentResult::Unresolved);
            }
            Some(dir) => {
                match settings::load_permissions(dir) {
                    None => {
                        // No .claude/ found — unresolved
                        results.push(types::SegmentResult::Unresolved);
                    }
                    Some(perms) => {
                        let result =
                            matcher::evaluate_segment(&segment.effective_cmd, &perms);
                        results.push(result);
                    }
                }
            }
        }
    }

    // Aggregate results
    match matcher::aggregate(&results) {
        Some((decision, reason)) => {
            Ok(Some(HookOutput::new(decision, reason)))
        }
        None => Ok(None),
    }
}
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build`
Expected: Compiles successfully

- [ ] **Step 3: Commit**

```bash
git add src/main.rs
git commit -m "feat: implement main orchestration pipeline"
```

---

## Task 8: Integration Tests

**Files:**
- Create: `tests/integration_tests.rs`

- [ ] **Step 1: Write integration tests**

In `tests/integration_tests.rs`:

```rust
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

fn run_hook(tool_name: &str, command: &str) -> (String, i32) {
    let input = serde_json::json!({
        "session_id": "test",
        "tool_name": tool_name,
        "tool_input": {
            "command": command
        }
    });

    let mut child = Command::new(env!("CARGO_BIN_EXE_claude-perm-router"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn");

    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.to_string().as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let code = output.status.code().unwrap_or(-1);
    (stdout, code)
}

fn setup_repo(allow: &[&str], deny: &[&str], ask: &[&str]) -> TempDir {
    let tmp = TempDir::new().unwrap();
    let claude_dir = tmp.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();

    let settings = serde_json::json!({
        "permissions": {
            "allow": allow,
            "deny": deny,
            "ask": ask
        }
    });

    fs::write(
        claude_dir.join("settings.json"),
        serde_json::to_string_pretty(&settings).unwrap(),
    )
    .unwrap();

    tmp
}

#[test]
fn integration_allow() {
    // Spec integration test 1
    let repo = setup_repo(&["Bash(./gradlew:*)"], &[], &[]);
    let cmd = format!("cd {} && ./gradlew test", repo.path().display());
    let (stdout, code) = run_hook("Bash", &cmd);
    assert_eq!(code, 0);
    let output: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        output["hookSpecificOutput"]["permissionDecision"],
        "allow"
    );
}

#[test]
fn integration_no_matching_rule_falls_through() {
    // Spec integration test 2
    let repo = setup_repo(&["Bash(./gradlew:*)"], &[], &[]);
    let cmd = format!("cd {} && rm -rf /", repo.path().display());
    let (stdout, code) = run_hook("Bash", &cmd);
    assert_eq!(code, 0);
    assert!(stdout.is_empty(), "Expected fall-through (no output)");
}

#[test]
fn integration_deny() {
    // Spec integration test 3
    let repo = setup_repo(&[], &["Bash(git push:*)"], &[]);
    let cmd = format!("cd {} && git push", repo.path().display());
    let (stdout, code) = run_hook("Bash", &cmd);
    assert_eq!(code, 0);
    let output: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[test]
fn integration_no_directory_falls_through() {
    // Spec integration test 4
    let (stdout, code) = run_hook("Bash", "./gradlew test");
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
}

#[test]
fn integration_nonexistent_dir_falls_through() {
    // Spec integration test 5
    let (stdout, code) = run_hook("Bash", "cd /nonexistent_path_xyz && ls");
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
}

#[test]
fn integration_chained_same_repo() {
    // Spec integration test 6
    let repo = setup_repo(&["Bash(cmd1)", "Bash(cmd2)"], &[], &[]);
    let cmd = format!("cd {} && cmd1 && cmd2", repo.path().display());
    let (stdout, code) = run_hook("Bash", &cmd);
    assert_eq!(code, 0);
    let output: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        output["hookSpecificOutput"]["permissionDecision"],
        "allow"
    );
}

#[test]
fn integration_git_c() {
    // Spec integration test 7
    let repo = setup_repo(&["Bash(git *)"], &[], &[]);
    let cmd = format!("git -C {} status", repo.path().display());
    let (stdout, code) = run_hook("Bash", &cmd);
    assert_eq!(code, 0);
    let output: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(
        output["hookSpecificOutput"]["permissionDecision"],
        "allow"
    );
}

#[test]
fn integration_cross_repo_deny() {
    // Spec integration test 8
    let repo1 = setup_repo(&["Bash(./gradlew:*)"], &[], &[]);
    let repo2 = setup_repo(&[], &["Bash(npm publish)"], &[]);
    let cmd = format!(
        "cd {} && ./gradlew test && cd {} && npm publish",
        repo1.path().display(),
        repo2.path().display()
    );
    let (stdout, code) = run_hook("Bash", &cmd);
    assert_eq!(code, 0);
    let output: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "deny");
}

#[test]
fn integration_mixed_scope_falls_through() {
    // Spec integration test 9
    let repo = setup_repo(&["Bash(./gradlew:*)"], &[], &[]);
    let cmd = format!(
        "cd {} && ./gradlew test && cd /nonexistent_xyz && ls",
        repo.path().display()
    );
    let (stdout, code) = run_hook("Bash", &cmd);
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
}

#[test]
fn integration_ask() {
    // Spec integration test 10
    let repo = setup_repo(&[], &[], &["Bash(npm publish)"]);
    let cmd = format!("cd {} && npm publish", repo.path().display());
    let (stdout, code) = run_hook("Bash", &cmd);
    assert_eq!(code, 0);
    let output: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(output["hookSpecificOutput"]["permissionDecision"], "ask");
}

#[test]
fn integration_non_bash_tool() {
    // Spec integration test 11
    let (stdout, code) = run_hook("Read", "/some/file");
    assert_eq!(code, 0);
    assert!(stdout.is_empty());
}
```

- [ ] **Step 2: Run integration tests**

Run: `cargo test --test integration_tests`
Expected: All 11 tests PASS

- [ ] **Step 3: Run all tests together**

Run: `cargo test`
Expected: All tests across all test files PASS

- [ ] **Step 4: Commit**

```bash
git add tests/integration_tests.rs
git commit -m "feat: add integration tests for end-to-end hook behavior"
```

---

## Task 9: Build and Install

**Files:**
- No new files

- [ ] **Step 1: Build release binary**

Run: `cargo build --release`
Expected: Binary at `target/release/claude-perm-router`

- [ ] **Step 2: Install to ~/.claude/bin/**

Run: `mkdir -p ~/.claude/bin && cp target/release/claude-perm-router ~/.claude/bin/`

- [ ] **Step 3: Verify binary runs**

Run: `echo '{"session_id":"test","tool_name":"Read","tool_input":{"command":"x"}}' | ~/.claude/bin/claude-perm-router`
Expected: No output, exit 0 (non-Bash tool falls through)

- [ ] **Step 4: Commit any final changes**

Run: `cargo test` one final time to confirm everything passes.
