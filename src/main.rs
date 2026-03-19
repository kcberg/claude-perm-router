mod matcher;
mod parser;
mod settings;
mod types;

use std::io::Read;
use types::{HookInput, HookOutput};

fn main() {
    let result = run();
    match result {
        Ok(Some(output)) => {
            let json = serde_json::to_string(&output).expect("failed to serialize output");
            println!("{json}");
        }
        Ok(None) => {
            // Fall through — no output, exit 0
        }
        Err(e) => {
            eprintln!("claude-perm-router: {e}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<Option<HookOutput>, Box<dyn std::error::Error>> {
    // Read stdin
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;

    // Parse input
    let hook_input: HookInput = serde_json::from_str(&input)?;

    // Gate on Bash tool
    if hook_input.tool_name != "Bash" {
        return Ok(None);
    }

    // Extract command
    let command = match hook_input.tool_input.command {
        Some(cmd) if !cmd.is_empty() => cmd,
        _ => return Ok(None),
    };

    // Parse command into segments
    let segments = parser::parse_command(&command);

    if segments.is_empty() {
        return Ok(None);
    }

    // Evaluate each segment against its directory's permissions
    let mut results = Vec::new();

    for segment in &segments {
        match &segment.target_dir {
            None => {
                // No target directory — unresolved
                results.push(types::SegmentResult::Unresolved);
            }
            Some(dir) => {
                match settings::load_permissions(dir) {
                    None => {
                        // No .claude/ found — unresolved
                        results.push(types::SegmentResult::Unresolved);
                    }
                    Some(perms) => {
                        let result =
                            matcher::evaluate_segment(&segment.effective_cmd, &perms);
                        results.push(result);
                    }
                }
            }
        }
    }

    // Aggregate results
    match matcher::aggregate(&results) {
        Some((decision, reason)) => {
            Ok(Some(HookOutput::new(decision, reason)))
        }
        None => Ok(None),
    }
}
