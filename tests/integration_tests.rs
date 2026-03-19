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
