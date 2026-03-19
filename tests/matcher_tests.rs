use claude_perm_router::matcher::{match_rule, evaluate_segment};
use claude_perm_router::types::{Permissions, SegmentResult};
use std::path::PathBuf;

#[test]
fn match_colon_star_prefix() {
    // Spec matcher test 1
    assert!(match_rule("./gradlew:*", "./gradlew test"));
    assert!(match_rule("./gradlew:*", "./gradlew build"));
    assert!(!match_rule("./gradlew:*", "npm test"));
}

#[test]
fn match_exact() {
    // Spec matcher test 2
    assert!(match_rule("./gradlew test", "./gradlew test"));
    assert!(!match_rule("./gradlew test", "./gradlew build"));
}

#[test]
fn match_space_star_prefix() {
    // Spec matcher test 3
    assert!(match_rule("git *", "git status"));
    assert!(match_rule("git *", "git push --force"));
    assert!(!match_rule("git *", "npm test"));
}

#[test]
fn match_literal_star_in_middle() {
    // Spec matcher test 9
    assert!(!match_rule("foo*bar", "fooXbar"));
    assert!(match_rule("foo*bar", "foo*bar")); // literal match
}

#[test]
fn evaluate_segment_deny() {
    // Spec matcher test 4
    let perms = Permissions {
        deny: vec!["Bash(git push:*)".to_string()],
        allow: vec![],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    let result = evaluate_segment("git push --force", &perms);
    assert!(matches!(result, SegmentResult::Denied { .. }));
}

#[test]
fn evaluate_segment_deny_over_allow() {
    // Spec matcher test 5
    let perms = Permissions {
        deny: vec!["Bash(git push:*)".to_string()],
        allow: vec!["Bash(git *)".to_string()],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    let result = evaluate_segment("git push", &perms);
    assert!(matches!(result, SegmentResult::Denied { .. }));
}

#[test]
fn evaluate_segment_allow() {
    let perms = Permissions {
        deny: vec![],
        allow: vec!["Bash(./gradlew:*)".to_string()],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    let result = evaluate_segment("./gradlew test", &perms);
    assert!(matches!(result, SegmentResult::Allowed { .. }));
}

#[test]
fn evaluate_segment_ask() {
    // Spec matcher test 7
    let perms = Permissions {
        deny: vec![],
        allow: vec![],
        ask: vec!["Bash(npm publish)".to_string()],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    let result = evaluate_segment("npm publish", &perms);
    assert!(matches!(result, SegmentResult::Ask { .. }));
}

#[test]
fn evaluate_segment_unresolved() {
    // Spec matcher test 6
    let perms = Permissions {
        deny: vec![],
        allow: vec!["Bash(git *)".to_string()],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    let result = evaluate_segment("npm test", &perms);
    assert!(matches!(result, SegmentResult::Unresolved));
}

#[test]
fn both_wildcard_styles_work() {
    // Spec matcher test 8
    assert!(match_rule("./gradlew:*", "./gradlew test"));
    assert!(match_rule("git *", "git status"));
}

#[test]
fn non_bash_rules_ignored() {
    let perms = Permissions {
        deny: vec![],
        allow: vec!["Read(/foo/*)".to_string(), "Bash(git *)".to_string()],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    // Read rule is silently ignored, Bash rule matches
    let result = evaluate_segment("git status", &perms);
    assert!(matches!(result, SegmentResult::Allowed { .. }));
}

#[test]
fn only_non_bash_rules_unresolved() {
    let perms = Permissions {
        deny: vec![],
        allow: vec!["Read(/foo/*)".to_string()],
        ask: vec![],
        settings_path: PathBuf::from("/repo/.claude"),
    };
    // Only non-Bash rules present — nothing matches
    let result = evaluate_segment("git status", &perms);
    assert!(matches!(result, SegmentResult::Unresolved));
}
