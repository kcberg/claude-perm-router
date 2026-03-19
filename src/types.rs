use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Raw input from Claude Code hook on stdin
#[derive(Debug, Deserialize)]
pub struct HookInput {
    #[allow(dead_code)] // Part of Claude Code hook protocol, deserialized but not used
    pub session_id: String,
    pub tool_name: String,
    pub tool_input: ToolInput,
}

#[derive(Debug, Deserialize)]
pub struct ToolInput {
    pub command: Option<String>,
}

/// Output JSON for Claude Code hook on stdout
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    pub hook_specific_output: HookSpecificOutput,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HookSpecificOutput {
    pub hook_event_name: String,
    pub permission_decision: String,
    pub permission_decision_reason: String,
}

impl HookOutput {
    pub fn new(decision: PermissionDecision, reason: String) -> Self {
        Self {
            hook_specific_output: HookSpecificOutput {
                hook_event_name: "PreToolUse".to_string(),
                permission_decision: decision.as_str().to_string(),
                permission_decision_reason: reason,
            },
        }
    }
}

/// The three possible permission decisions
#[derive(Debug, Clone, PartialEq)]
pub enum PermissionDecision {
    Allow,
    Deny,
    Ask,
}

impl PermissionDecision {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Allow => "allow",
            Self::Deny => "deny",
            Self::Ask => "ask",
        }
    }
}

/// Result of evaluating a single segment against permissions
#[derive(Debug, Clone, PartialEq)]
pub enum SegmentResult {
    Allowed { rule: String, settings_path: PathBuf },
    Denied { rule: String, settings_path: PathBuf },
    Ask { rule: String, settings_path: PathBuf },
    Unresolved,
}

/// A parsed command segment with its resolved directory and effective command
#[derive(Debug, Clone, PartialEq)]
pub struct EvaluatedSegment {
    pub target_dir: Option<PathBuf>,
    pub effective_cmd: String,
    pub raw_segment: String,
}

/// Loaded and merged permission rules from .claude/settings*.json
#[derive(Debug, Clone, Default)]
pub struct Permissions {
    pub allow: Vec<String>,
    pub deny: Vec<String>,
    pub ask: Vec<String>,
    pub settings_path: PathBuf,
}
