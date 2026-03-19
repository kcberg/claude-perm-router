# Developer Guide

## Development

```bash
cargo build              # Debug build
cargo test               # Run all tests
cargo fmt                # Format code
cargo clippy             # Lint
```

## Cutting a Release

Releases are triggered by pushing a version tag. The GitHub Actions workflow builds binaries for all platforms and creates a GitHub Release automatically.

### Steps

1. **Bump the version in `Cargo.toml`:**

   ```bash
   # Install cargo-edit if you don't have it
   cargo install cargo-edit

   # Bump patch (0.1.5 → 0.1.6)
   cargo set-version --bump patch

   # Or bump minor (0.1.5 → 0.2.0)
   cargo set-version --bump minor

   # Or set an explicit version
   cargo set-version 1.0.0
   ```

2. **Update `Cargo.lock`:**

   ```bash
   cargo generate-lockfile
   ```

3. **Commit, push via PR, and merge:**

   ```bash
   git checkout -b release/v0.1.6
   git add Cargo.toml Cargo.lock
   git commit -m "chore: bump version to 0.1.6"
   git push -u origin release/v0.1.6
   # Create PR, get CI green, merge
   ```

4. **Tag and push the tag on main:**

   ```bash
   git checkout main
   git pull
   git tag v0.1.6
   git push origin v0.1.6
   ```

5. **Done.** The release workflow will:
   - Build binaries for macOS (ARM), Linux (x86_64), Linux (ARM64)
   - Create a GitHub Release at the tag with auto-generated release notes
   - Attach all binaries to the release

### Release artifacts

| Platform | Binary name |
|---|---|
| macOS (Apple Silicon) | `claude-perm-router-macos-aarch64` |
| Linux (x86_64) | `claude-perm-router-linux-x86_64` |
| Linux (ARM64) | `claude-perm-router-linux-aarch64` |

## CI

- **On PRs and pushes to main:** Build, format check, clippy lint, tests
- **On version tags (`v*`):** Build release binaries, create GitHub Release
