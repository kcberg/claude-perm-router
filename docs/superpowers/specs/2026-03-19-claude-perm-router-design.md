# claude-perm-router Design Spec

## Overview

A Rust CLI that runs as a Claude Code PreToolUse hook. It intercepts Bash commands, parses them into individual segments, determines which directory each segment targets, loads that directory's `.claude/settings.json` and `.claude/settings.local.json`, and returns an allow/deny/ask decision based on the per-directory permission rules.

## Why

Claude Code's `--add-dir` feature lets built-in tools (Read, Grep, etc.) work across multiple repos, but Bash commands that target other directories (via `cd`, `git -C`, absolute paths) don't respect per-directory permissions. This hook bridges that gap.

## Interface

### Input (stdin)

```json
{
  "session_id": "abc123",
  "tool_name": "Bash",
  "tool_input": {
    "command": "cd /Users/kberg/StackHawkProjects/Nest && ./gradlew test"
  }
}
```

### Output (stdout)

- **Allow:** JSON with `permissionDecision: "allow"` and a reason listing all matched segments
- **Deny:** JSON with `permissionDecision: "deny"` and a reason identifying the denied segment and rule
- **Ask:** JSON with `permissionDecision: "ask"` and a reason identifying the ask-matched segment
- **Fall-through:** No output, exit 0 — delegates to Claude Code's normal permission system
- **Error:** Exit non-zero, human-readable message on stderr — Claude Code treats as fall-through

### Output Format

```json
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "allow|deny|ask",
    "permissionDecisionReason": "Human-readable explanation"
  }
}
```

## Command Parsing Pipeline

The parser takes a raw command string and produces a list of evaluated segments.

### Step 1: Quote-aware splitting

Walk the string character by character, tracking whether we're inside single or double quotes. Split on unquoted `&&`, `||`, `;`, and `|`. Each split records which operator preceded it:

- `&&`, `||`, `;` — chain operators. The directory accumulator carries forward.
- `|` — pipe operator. Directory context is inherited from the parent chain segment.

### Step 2: Segment classification

After trimming whitespace, each segment is classified as:

- **`cd <path>`** — Updates the directory accumulator. Not itself evaluated against permissions.
- **`git -C <path> <subcmd>`** — Has its own target directory (independent of the accumulator). Effective command is `git <subcmd>`.
- **Absolute executable** (e.g., `/foo/bar/build/dist/hawk scan`) — Target directory found by walking up from the executable's parent to find the nearest `.claude/` directory. Effective command is the basename portion.
- **Plain command** — Uses the current directory accumulator. If accumulator is empty (no `cd` seen), this segment has no target directory.

### Step 3: Directory accumulator rules

- `cd /absolute/path` — Sets accumulator to that path.
- `cd relative/path` — Appends to current accumulator. If accumulator is empty (no prior `cd`, CWD unknown), this has no effect — segment treated as having no target directory.
- Accumulator carries across `&&`, `||`, `;` boundaries.
- Pipes inherit the accumulator of their parent chain segment.
- `git -C` does NOT affect the accumulator — it's self-contained.

### Output

`Vec<EvaluatedSegment>` where each segment has:
- `target_dir: Option<PathBuf>` — resolved directory, or None
- `effective_cmd: String` — the command to match against permissions
- `raw_segment: String` — original text for error reporting

### Subshells and command substitution

Not parsed. `echo $(cat /bar/secret.txt)` is matched as the literal string `echo $(cat /bar/secret.txt)` against the current directory's permissions. This is an ergonomic layer, not a security sandbox — Claude Code's own permission system is the backstop.

## Permission Loading

### Settings discovery

Given a target directory, walk up the directory tree looking for a `.claude/` directory containing `settings.json` and/or `settings.local.json`.

### Merging

Extract `permissions.allow`, `permissions.deny`, and `permissions.ask` arrays from both files. Concatenate arrays — local settings add to project settings (local allow + project allow = combined allow list).

## Permission Matching

### Rule syntax

Each rule is a string like `Bash(pattern)`. Non-`Bash(...)` rules are ignored.

The inner pattern is matched against the effective command:
- `./gradlew:*` — `:*` suffix means prefix match. Matches any command starting with `./gradlew`.
- `./gradlew test` — No wildcard. Exact match only.
- `git *` — Trailing ` *` (space-star) means prefix match. Matches `git status`, `git push`, etc.

### Per-segment evaluation order

1. Check deny list — if matched, segment is **denied**
2. Check allow list — if matched, segment is **allowed**
3. Check ask list — if matched, segment is **ask**
4. No match — segment is **unresolved**

### Cross-segment aggregation

1. If ANY segment is **denied** → entire command is **denied**. Reason identifies which segment and rule.
2. If ANY segment is **unresolved** (no target dir, no `.claude/` found, or no rule matched) → entire command **falls through** (no output, exit 0).
3. If ANY segment is **ask** (and none denied/unresolved) → entire command is **ask**.
4. Only if ALL segments are **allowed** → entire command is **allowed**.

## Module Structure

```
claude-perm-router/
├── Cargo.toml
├── src/
│   ├── main.rs          # stdin read, orchestration, stdout write, error handling
│   ├── parser.rs        # quote-aware splitting, segment classification, directory accumulator
│   ├── settings.rs      # walk up to find .claude/, load & merge settings JSON
│   ├── matcher.rs       # parse Bash() rule syntax, match commands, per-segment & aggregate evaluation
│   └── types.rs         # HookInput, HookOutput, EvaluatedSegment, SegmentResult, PermissionDecision
└── tests/
    ├── parser_tests.rs
    ├── matcher_tests.rs
    ├── settings_tests.rs
    └── integration_tests.rs
```

## Dependencies

- `serde` + `serde_json` — JSON parsing
- No async runtime, no CLI argument parser

## Performance

- Must complete in <50ms for typical cases
- Read settings files on every invocation (no caching) — they may change mid-session
- Parse only what's needed from JSON

## Test Cases

### Parser tests
1. `cd /foo && ./gradlew test` → segment: dir=/foo, cmd=`./gradlew test`
2. `cd /foo ; ls` → segment: dir=/foo, cmd=`ls`
3. `cd /foo && cmd1 && cmd2` → two segments both with dir=/foo
4. `cd /foo && cmd1 | cmd2` → pipe inherits /foo for both
5. `git -C /foo status` → segment: dir=/foo, cmd=`git status`
6. `git -C /foo status && git -C /bar push` → two segments with independent dirs
7. `/foo/bar/build/dist/hawk scan` → walk up from /foo/bar/build/dist for .claude/
8. `./gradlew test` (no directory context) → no target dir, falls through
9. `cd /foo && cd bar && ls` → accumulates to /foo/bar, cmd=`ls`
10. `echo "hello && world"` → not split on quoted `&&`

### Matcher tests
1. `./gradlew test` matches `Bash(./gradlew:*)` → allow
2. `./gradlew test` matches `Bash(./gradlew test)` → allow (exact)
3. `git status` matches `Bash(git *)` → allow
4. `git push --force` matches deny `Bash(git push:*)` → deny
5. Deny takes precedence over allow when both match
6. No match → unresolved

### Integration tests
1. `cd /repo && ./gradlew test` where repo allows `Bash(./gradlew:*)` → allow
2. `cd /repo && rm -rf /` where repo has no rm permission → fall through
3. `cd /repo && git push` where repo denies `Bash(git push:*)` → deny
4. `./gradlew test` (no directory) → no output, fall through
5. `cd /nonexistent && ls` → no `.claude/` found, fall through
6. `cd /repo && cmd1 && cmd2` → both evaluated against /repo
7. `git -C /repo status` → evaluated against /repo
8. `cd /repo1 && ./gradlew test && cd /repo2 && npm publish` where repo1 allows gradlew and repo2 denies npm publish → deny
9. Mixed scope: one segment has `.claude/`, another doesn't → fall through

## Installation

After `cargo build --release`, binary goes at `~/.claude/bin/claude-perm-router`.

## Hook Configuration

```json
{
  "hooks": {
    "PreToolUse": [{
      "matcher": "Bash",
      "hooks": [{
        "type": "command",
        "command": "~/.claude/bin/claude-perm-router",
        "timeout": 5
      }]
    }]
  }
}
```
