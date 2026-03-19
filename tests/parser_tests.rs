use claude_perm_router::parser::split_command;
use claude_perm_router::parser::parse_command;
use std::path::PathBuf;

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
