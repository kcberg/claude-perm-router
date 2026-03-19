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
