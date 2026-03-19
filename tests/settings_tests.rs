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
    create_settings(
        tmp.path(),
        "settings.json",
        r#"{
        "permissions": {
            "allow": ["Bash(./gradlew:*)"],
            "deny": ["Bash(rm:*)"]
        }
    }"#,
    );

    let perms = load_permissions(tmp.path()).unwrap();
    assert_eq!(perms.allow, vec!["Bash(./gradlew:*)"]);
    assert_eq!(perms.deny, vec!["Bash(rm:*)"]);
    assert!(perms.ask.is_empty());
    assert_eq!(perms.settings_path, tmp.path().join(".claude"));
}

#[test]
fn load_walks_up() {
    let tmp = TempDir::new().unwrap();
    create_settings(
        tmp.path(),
        "settings.json",
        r#"{
        "permissions": { "allow": ["Bash(git *)"] }
    }"#,
    );

    let subdir = tmp.path().join("src").join("deep");
    fs::create_dir_all(&subdir).unwrap();

    let perms = load_permissions(&subdir).unwrap();
    assert_eq!(perms.allow, vec!["Bash(git *)"]);
}

#[test]
fn merge_local_and_project() {
    let tmp = TempDir::new().unwrap();
    create_settings(
        tmp.path(),
        "settings.json",
        r#"{
        "permissions": { "allow": ["Bash(git *)"] }
    }"#,
    );
    create_settings(
        tmp.path(),
        "settings.local.json",
        r#"{
        "permissions": { "allow": ["Bash(./gradlew:*)"], "deny": ["Bash(rm:*)"] }
    }"#,
    );

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
    create_settings(
        tmp.path(),
        "settings.json",
        r#"{
        "permissions": { "ask": ["Bash(npm publish)"] }
    }"#,
    );

    let perms = load_permissions(tmp.path()).unwrap();
    assert_eq!(perms.ask, vec!["Bash(npm publish)"]);
}
