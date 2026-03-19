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
- **Absolute executable** (e.g., `/foo/bar/build/dist/hawk scan`) — Target directory found by walking up from the executable's parent to find the nearest `.claude/` directory. If no `.claude/` is found, the segment is unresolved (falls through). Effective command is the basename of the executable plus all trailing arguments (e.g., `/foo/bar/dist/hawk scan` → effective command is `hawk scan`).
- **Plain command** — Uses the current directory accumulator. If accumulator is empty (no `cd` seen), this segment has no target directory.

### Step 3: Directory accumulator rules

- `cd /absolute/path` — Sets accumulator to that path.
- `cd relative/path` — Appends to current accumulator. If accumulator is empty (no prior `cd`, CWD unknown), this has no effect — segment treated as having no target directory. This includes `cd ..` and `cd -`, which are unresolvable without CWD.
- Accumulator carries across `&&`, `||`, `;` boundaries.
- All segments in a pipe chain inherit the directory from the last chain-operator segment preceding the pipe. After the pipe group ends, the accumulator resumes from the pre-pipe state.
- `git -C` does NOT affect the accumulator — it's self-contained. If the `-C` path is relative, it is resolved against the current directory accumulator. If the accumulator is empty, the segment is treated as having no target directory.

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

Each rule is a string like `Bash(pattern)`. Non-`Bash(...)` rules are ignored. If `tool_name` is not `Bash`, output nothing and exit 0 (fall-through).

The inner pattern is matched against the effective command. There are two wildcard forms — both exist because Claude Code's own permission syntax uses both conventions:

- `./gradlew:*` — `:*` suffix means prefix match. The `:` is a separator; matches any command starting with `./gradlew` (e.g., `./gradlew test`, `./gradlew build`).
- `git *` — Trailing ` *` (space-star) means prefix match. Matches `git status`, `git push`, etc.
- `./gradlew test` — No wildcard. Exact match only.
- `*` appearing in any other position is treated as a literal character.

### Per-segment evaluation order

1. Check deny list — if matched, segment is **denied**
2. Check allow list — if matched, segment is **allowed**
3. Check ask list — if matched, segment is **ask**
4. No match — segment is **unresolved**

### Cross-segment aggregation

Each segment's result is determined by per-segment evaluation before aggregation begins.

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

- Must complete in <50ms for typical cases (steady-state). The 5-second hook timeout provides headroom for cold starts and slow filesystem traversal.
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
7. `/foo/bar/build/dist/hawk scan` → walk up from /foo/bar/build/dist for .claude/, effective cmd=`hawk scan`
8. `/foo/bar/build/dist/hawk scan` with no .claude/ found → segment is unresolved
9. `./gradlew test` (no directory context) → no target dir, falls through
10. `cd /foo && cd bar && ls` → accumulates to /foo/bar, cmd=`ls`
11. `echo "hello && world"` → not split on quoted `&&`
12. `cd /foo && cmd1 | cmd2 | cmd3 && cmd4` → cmd1/cmd2/cmd3 all use /foo (pipe inheritance), cmd4 also uses /foo
13. `cd /foo && git -C ../bar status` → git -C resolved to /bar (relative to accumulator /foo)
14. `git -C ../bar status` (no accumulator) → no target dir, unresolved
15. `cd .. && ls` (no prior absolute cd) → cd .. unresolvable, no target dir

### Matcher tests
1. `./gradlew test` matches `Bash(./gradlew:*)` → allow
2. `./gradlew test` matches `Bash(./gradlew test)` → allow (exact)
3. `git status` matches `Bash(git *)` → allow
4. `git push --force` matches deny `Bash(git push:*)` → deny
5. `git push` in both deny `Bash(git push:*)` and allow `Bash(git *)` → deny (deny takes precedence per per-segment evaluation order)
6. No match → unresolved
7. `npm publish` matches ask rule `Bash(npm publish)` → ask
8. `:*` and ` *` rules in same list both work correctly
9. `*` in middle of pattern (e.g., `Bash(foo*bar)`) is treated as literal

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
10. `cd /repo && npm publish` where repo has `npm publish` in ask list → ask
11. Non-Bash tool_name in input → no output, fall through

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
