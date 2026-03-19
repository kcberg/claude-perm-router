use claude_perm_router::parser::parse_command;
use claude_perm_router::parser::split_command;
use std::fs;
use tempfile::TempDir;

// --- Split tests (don't need real dirs) ---

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
    assert!(parts[2].is_pipe); // cmd2
    assert!(parts[3].is_pipe); // cmd3
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

// --- Parse tests (use real temp dirs) ---

/// Helper to create a temp dir and return its canonicalized path
fn make_dir() -> TempDir {
    TempDir::new().unwrap()
}

#[test]
fn parse_cd_and_command() {
    let tmp = make_dir();
    let cmd = format!("cd {} && ./gradlew test", tmp.path().display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(tmp.path().canonicalize().unwrap()));
    assert_eq!(segs[0].effective_cmd, "./gradlew test");
}

#[test]
fn parse_cd_semicolon() {
    let tmp = make_dir();
    let cmd = format!("cd {} ; ls", tmp.path().display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(tmp.path().canonicalize().unwrap()));
    assert_eq!(segs[0].effective_cmd, "ls");
}

#[test]
fn parse_cd_chain_multiple() {
    let tmp = make_dir();
    let cmd = format!("cd {} && cmd1 && cmd2", tmp.path().display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 2);
    let expected = Some(tmp.path().canonicalize().unwrap());
    assert_eq!(segs[0].target_dir, expected);
    assert_eq!(segs[0].effective_cmd, "cmd1");
    assert_eq!(segs[1].target_dir, expected);
    assert_eq!(segs[1].effective_cmd, "cmd2");
}

#[test]
fn parse_cd_pipe_inherits() {
    let tmp = make_dir();
    let cmd = format!("cd {} && cmd1 | cmd2", tmp.path().display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 2);
    let expected = Some(tmp.path().canonicalize().unwrap());
    assert_eq!(segs[0].target_dir, expected);
    assert_eq!(segs[1].target_dir, expected);
}

#[test]
fn parse_git_c() {
    let tmp = make_dir();
    let cmd = format!("git -C {} status", tmp.path().display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(tmp.path().canonicalize().unwrap()));
    assert_eq!(segs[0].effective_cmd, "git status");
}

#[test]
fn parse_git_c_independent_dirs() {
    let tmp1 = make_dir();
    let tmp2 = make_dir();
    let cmd = format!(
        "git -C {} status && git -C {} push",
        tmp1.path().display(),
        tmp2.path().display()
    );
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 2);
    assert_eq!(
        segs[0].target_dir,
        Some(tmp1.path().canonicalize().unwrap())
    );
    assert_eq!(
        segs[1].target_dir,
        Some(tmp2.path().canonicalize().unwrap())
    );
}

#[test]
fn parse_no_directory_context() {
    let segs = parse_command("./gradlew test");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, None);
    assert_eq!(segs[0].effective_cmd, "./gradlew test");
}

#[test]
fn parse_cd_relative_accumulation() {
    let tmp = make_dir();
    let subdir = tmp.path().join("bar");
    fs::create_dir_all(&subdir).unwrap();

    let cmd = format!("cd {} && cd bar && ls", tmp.path().display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(subdir.canonicalize().unwrap()));
    assert_eq!(segs[0].effective_cmd, "ls");
}

#[test]
fn parse_quoted_not_split() {
    let segs = parse_command(r#"echo "hello && world""#);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, None);
}

#[test]
fn parse_multi_pipe_chain() {
    let tmp = make_dir();
    let cmd = format!("cd {} && cmd1 | cmd2 | cmd3 && cmd4", tmp.path().display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 4);
    let expected = Some(tmp.path().canonicalize().unwrap());
    for seg in &segs {
        assert_eq!(seg.target_dir, expected);
    }
}

#[test]
fn parse_git_c_relative_with_accumulator() {
    // Create /tmp/xxx/foo and /tmp/xxx/bar, cd to foo, git -C ../bar
    let tmp = make_dir();
    let foo = tmp.path().join("foo");
    let bar = tmp.path().join("bar");
    fs::create_dir_all(&foo).unwrap();
    fs::create_dir_all(&bar).unwrap();

    let cmd = format!("cd {} && git -C ../bar status", foo.display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(bar.canonicalize().unwrap()));
    assert_eq!(segs[0].effective_cmd, "git status");
}

#[test]
fn parse_git_c_relative_no_accumulator() {
    let segs = parse_command("git -C ../bar status");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, None);
}

#[test]
fn parse_cd_dotdot_no_accumulator() {
    // cd .. with no prior absolute cd resolves against CWD
    let segs = parse_command("cd .. && ls");
    assert_eq!(segs.len(), 1);
    // Should resolve to parent of CWD
    let expected = std::env::current_dir()
        .ok()
        .and_then(|cwd| cwd.parent().map(|p| p.to_path_buf()))
        .and_then(|p| p.canonicalize().ok());
    assert_eq!(segs[0].target_dir, expected);
}

#[test]
fn parse_absolute_executable_with_claude_dir() {
    let tmp = TempDir::new().unwrap();
    let claude_dir = tmp.path().join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();

    let bin_dir = tmp.path().join("build").join("dist");
    fs::create_dir_all(&bin_dir).unwrap();

    let executable = bin_dir.join("hawk");
    let cmd = format!("{} scan", executable.display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(tmp.path().canonicalize().unwrap()));
    assert_eq!(segs[0].effective_cmd, "hawk scan");
}

#[test]
fn parse_absolute_executable_no_claude_dir() {
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

#[test]
fn parse_cd_nonexistent_dir_falls_through() {
    // cd to a path that doesn't exist → target_dir is None
    let segs = parse_command("cd /nonexistent_xyz_123 && ls");
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, None);
}

#[test]
fn parse_cd_with_dotdot_in_path() {
    // The original bug: cd /real/path/../other should canonicalize
    let tmp = make_dir();
    let sub1 = tmp.path().join("repo1");
    let sub2 = tmp.path().join("repo2");
    fs::create_dir_all(&sub1).unwrap();
    fs::create_dir_all(&sub2).unwrap();

    let cmd = format!("cd {}/repo1/../repo2 && git status", tmp.path().display());
    let segs = parse_command(&cmd);
    assert_eq!(segs.len(), 1);
    assert_eq!(segs[0].target_dir, Some(sub2.canonicalize().unwrap()));
    assert_eq!(segs[0].effective_cmd, "git status");
}
