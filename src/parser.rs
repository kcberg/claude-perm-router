/// A raw split segment before classification
#[derive(Debug, Clone, PartialEq)]
pub struct RawSegment {
    pub text: String,
    pub is_pipe: bool,
}

/// Split a command string on unquoted &&, ||, ;, and |.
/// Tracks whether each segment was preceded by a pipe operator.
pub fn split_command(cmd: &str) -> Vec<RawSegment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut is_pipe = false;
    let chars: Vec<char> = cmd.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let c = chars[i];

        // Track quote state
        if c == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            current.push(c);
            i += 1;
            continue;
        }
        if c == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            current.push(c);
            i += 1;
            continue;
        }

        // Only split when not inside quotes
        if !in_single_quote && !in_double_quote {
            // Check for && or ||
            if i + 1 < len
                && ((c == '&' && chars[i + 1] == '&')
                    || (c == '|' && chars[i + 1] == '|'))
            {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    segments.push(RawSegment {
                        text: trimmed,
                        is_pipe,
                    });
                }
                current.clear();
                is_pipe = false; // && and || are chain operators
                i += 2;
                continue;
            }

            // Check for ; (chain) or | (pipe)
            if c == ';' {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    segments.push(RawSegment {
                        text: trimmed,
                        is_pipe,
                    });
                }
                current.clear();
                is_pipe = false;
                i += 1;
                continue;
            }

            if c == '|' {
                let trimmed = current.trim().to_string();
                if !trimmed.is_empty() {
                    segments.push(RawSegment {
                        text: trimmed,
                        is_pipe,
                    });
                }
                current.clear();
                is_pipe = true;
                i += 1;
                continue;
            }
        }

        current.push(c);
        i += 1;
    }

    let trimmed = current.trim().to_string();
    if !trimmed.is_empty() {
        segments.push(RawSegment {
            text: trimmed,
            is_pipe,
        });
    }

    segments
}
