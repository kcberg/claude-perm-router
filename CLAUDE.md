# CLAUDE.md

## Project

claude-perm-router is a Rust CLI that runs as a Claude Code PreToolUse hook. It intercepts Bash commands, determines which directory each segment targets, loads that directory's `.claude/settings*.json` permissions, and returns allow/deny/ask decisions.

## Commands

```bash
cargo build              # Debug build
cargo build --release    # Release build
cargo test               # Run all 59 tests
cargo test --test parser_tests       # Parser tests only (24)
cargo test --test settings_tests     # Settings tests only (6)
cargo test --test matcher_tests      # Matcher tests only (18)
cargo test --test integration_tests  # Integration tests only (11)
```

Install: `./install.sh` or manually `cp target/release/claude-perm-router ~/.claude/bin/`

## Architecture

```
src/lib.rs       → shared find_project_root(), re-exports modules
src/types.rs     → all shared types (HookInput, HookOutput, SegmentResult, etc.)
src/parser.rs    → split_command() + parse_command() — quote-aware splitting, cd/git-C/absolute-path detection
src/settings.rs  → load_permissions() — walk up for .claude/, load+merge JSON
src/matcher.rs   → match_rule() + evaluate_segment() + aggregate() — rule matching and decision logic
src/main.rs      → stdin→parse→settings→match→stdout pipeline (imports from lib crate)
```

`main.rs` imports from the library crate (`use claude_perm_router::{...}`), not via `mod` declarations. Tests use the library crate too.

## Conventions

- Rust edition 2024. Only deps: serde, serde_json. No async, no CLI framework.
- All paths must go through `try_canonicalize()` (resolves `..`, symlinks, validates existence). Never store raw `PathBuf` from user input in the accumulator.
- `find_project_root()` lives in `lib.rs` — do NOT duplicate it in other modules.
- Integration tests spawn the actual binary via `env!("CARGO_BIN_EXE_claude-perm-router")` and pipe JSON to stdin.
- Parser tests that need directories must use real `tempfile::TempDir`, not fake paths like `/foo` — `try_canonicalize` returns None for nonexistent paths.
- Fall-through (no output, exit 0) is the safe default. Only emit JSON when we can fully resolve the path AND find matching permission rules.

## Gotchas

- macOS: `/var` is a symlink to `/private/var`. `canonicalize()` resolves this. Tests must compare canonicalized paths, not raw ones.
- `accumulator.take().or_else(|| std::env::current_dir().ok())` — `take()` is needed to avoid borrow-after-move in loops. Don't use `accumulator.or_else(...)` directly.
- The `Bash(pattern)` rule syntax has TWO wildcard forms: `:*` suffix (no word boundary) and ` *` trailing (space boundary). Both are intentional — they match Claude Code's own syntax.
- `settings_path` in `Permissions` stores the `.claude/` directory path, not the project root.
