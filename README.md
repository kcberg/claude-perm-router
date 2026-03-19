# claude-perm-router

Per-directory permission routing for Claude Code Bash commands.

When you use `--add-dir` to work across multiple repos, Claude Code's built-in tools (Read, Grep, etc.) respect each directory's permissions. But Bash commands like `cd /other/repo && ./gradlew test` don't — they all use the main project's permissions. This hook fixes that.

## Install

### One-liner (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/kcberg/claude-perm-router/main/install-release.sh | bash
```

This detects your platform (macOS ARM, Linux x86_64/ARM), downloads the latest release binary to `~/.claude/bin/`, and offers to configure the hook.

### From source

If you prefer to build it yourself:

```bash
git clone https://github.com/kcberg/claude-perm-router.git
cd claude-perm-router
./install.sh
```

### Manual setup

```bash
# Download a specific release binary (example: macOS ARM)
curl -fsSL https://github.com/kcberg/claude-perm-router/releases/latest/download/claude-perm-router-macos-aarch64 \
  -o ~/.claude/bin/claude-perm-router
chmod +x ~/.claude/bin/claude-perm-router
```

Then add this hook to `~/.claude/settings.json`:

```json
{
  "hooks": {
    "PreToolUse": [
      {
        "matcher": "Bash",
        "hooks": [
          {
            "type": "command",
            "command": "~/.claude/bin/claude-perm-router",
            "timeout": 5
          }
        ]
      }
    ]
  }
}
```

If you already have hooks configured, merge the `PreToolUse` entry into your existing hooks object.

### Available binaries

| Platform | Binary |
|---|---|
| macOS (Apple Silicon) | `claude-perm-router-macos-aarch64` |
| Linux (x86_64) | `claude-perm-router-linux-x86_64` |
| Linux (ARM64) | `claude-perm-router-linux-aarch64` |

## How It Works

The hook intercepts every Bash command before execution. It:

1. **Parses the command** into segments, splitting on `&&`, `||`, `;`, and `|`
2. **Tracks which directory** each segment targets (via `cd`, `git -C`, or absolute paths)
3. **Loads permissions** from that directory's `.claude/settings.json` and `.claude/settings.local.json`
4. **Matches the command** against the allow/deny/ask rules
5. **Returns a decision** — or falls through to Claude Code's normal permission system

### Example

Your main project allows `./gradlew` commands. You also have a second repo added with `--add-dir` that denies `git push`:

```
# Main project: .claude/settings.json
{ "permissions": { "allow": ["Bash(./gradlew:*)"] } }

# Second repo: .claude/settings.json
{ "permissions": { "deny": ["Bash(git push:*)"] } }
```

When Claude runs `cd /path/to/second-repo && git push`, the hook:
- Detects the target directory is the second repo
- Loads the second repo's permissions
- Matches `git push` against `Bash(git push:*)` in the deny list
- Returns **deny** — the command is blocked

Without this hook, the command would be evaluated against the main project's permissions, which might allow it.

## What It Catches

| Pattern | Example | How It's Detected |
|---|---|---|
| `cd /path && command` | `cd /repo && ./gradlew test` | Directory accumulator tracks `cd` |
| `cd /path ; command` | `cd /repo ; ls` | Same as above, semicolon variant |
| Chained commands | `cd /repo && cmd1 && cmd2` | Both commands evaluated against `/repo` |
| Pipes | `cd /repo && cmd1 \| cmd2` | Pipe segments inherit the directory |
| `git -C /path` | `git -C /repo status` | Git's directory flag is parsed |
| Relative paths | `cd ../other-repo && cmd` | Resolved against CWD |
| `cd ~/path` | `cd ~/projects/repo && cmd` | Tilde expanded to `$HOME` |
| Absolute executables | `/repo/build/dist/hawk scan` | Walks up to find `.claude/` |
| Cross-repo chains | `cd /repo1 && cmd && cd /repo2 && cmd` | Each segment evaluated against its own repo's permissions |

## Decision Logic

When a command has multiple segments, the strictest rule wins:

1. **Any segment denied** — entire command is denied
2. **Any segment unresolved** (no `.claude/` found, no matching rule) — falls through to Claude Code's normal permissions
3. **Any segment is "ask"** — entire command prompts for confirmation
4. **All segments allowed** — entire command is allowed

## Permission Rule Syntax

Rules use Claude Code's standard `Bash(pattern)` syntax:

- `Bash(./gradlew:*)` — prefix match: allows any command starting with `./gradlew`
- `Bash(git *)` — space-star: allows any `git` subcommand (`git status`, `git push`, etc.)
- `Bash(npm test)` — exact match: allows only `npm test`

Evaluation order: **deny > allow > ask**. If a command matches both a deny and an allow rule, deny wins.

## Limitations

- **Only detects `cd`, `git -C`, and absolute executable paths.** Other tools with directory flags (e.g., `make -C`, `npm --prefix`) are not detected — those commands fall through to Claude Code's normal permissions.
- **Does not parse subshells or command substitution.** `echo $(cat /other/repo/secret)` is matched as a literal string, not as two separate commands.
- **Requires `.claude/settings.json` in target directories.** If a directory doesn't have permission rules, the hook falls through — it can't make a decision without rules to match against.
- **Nonexistent directories fall through.** If `cd /bad/path && rm -rf /` targets a path that doesn't exist, the hook can't resolve it and falls through to Claude Code's normal prompt.
- **Quoted operators are handled, but not all shell syntax.** Operators inside single or double quotes are correctly ignored. Heredocs, process substitution, and other advanced shell syntax are not parsed.

## Performance

The hook targets <50ms for typical commands. It reads settings files on every invocation (no caching) since they may change mid-session. The 5-second hook timeout is headroom for cold starts and slow filesystems.

## Development

```bash
# Run all tests (59 total)
cargo test

# Build debug binary
cargo build

# Build release binary
cargo build --release
```

Tests cover the parser (24 tests), settings loading (6 tests), permission matching (18 tests), and end-to-end integration (11 tests).
