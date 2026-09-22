//! The one action a generator returns per step, and its parser.

use serde::Deserialize;

/// What the generator asks the host to do next.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Run one shell command in the task checkout. `reason` is the
    /// model's note on what the command is for; it stays in the history so
    /// later steps see the plan.
    Shell {
        command: String,
        #[serde(default)]
        reason: Option<String>,
    },
    /// Stop. The title and summary become the pull request's.
    Finished { title: String, summary: String },
}

impl Action {
    /// Parses a generator's reply.
    ///
    /// The reply must be one JSON object. A single surrounding Markdown
    /// code fence is tolerated because models add one often; anything else
    /// around the object is an error.
    pub fn parse(reply: &str) -> Result<Self, String> {
        let body = unfence(reply.trim());
        // Models often put literal newlines inside a JSON string, most of
        // all in heredocs. Escape them before refusing the reply.
        let action: Action = serde_json::from_str(body)
            .or_else(|_| serde_json::from_str(&escape_raw_newlines(body)))
            .map_err(|error| format!("not a valid action: {error}"))?;
        if let Action::Shell { command, .. } = &action
            && command.trim().is_empty()
        {
            return Err("the shell command is empty".to_string());
        }
        Ok(action)
    }
}

/// `text` with each raw newline or tab inside a JSON string escaped.
fn escape_raw_newlines(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut in_string = false;
    let mut escaped = false;
    for ch in text.chars() {
        match ch {
            '\n' if in_string => out.push_str("\\n"),
            '\t' if in_string => out.push_str("\\t"),
            '\r' if in_string => {}
            _ => {
                if in_string && !escaped && ch == '"' {
                    in_string = false;
                } else if !in_string && ch == '"' {
                    in_string = true;
                }
                escaped = in_string && !escaped && ch == '\\';
                out.push(ch);
                continue;
            }
        }
        escaped = false;
    }
    out
}

/// The text inside one surrounding code fence, or the text itself.
fn unfence(text: &str) -> &str {
    let Some(rest) = text.strip_prefix("```") else {
        return text;
    };
    let Some(rest) = rest.strip_suffix("```") else {
        return text;
    };
    // Drop the fence's language tag, if any.
    match rest.split_once('\n') {
        Some((_tag, inner)) => inner.trim(),
        None => rest.trim(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_both_actions() {
        assert_eq!(
            Action::parse(r#"{"action":"shell","command":"ls"}"#),
            Ok(Action::Shell {
                command: "ls".to_string(),
                reason: None
            })
        );
        assert_eq!(
            Action::parse(r#"{"action":"finished","title":"Fix it","summary":"Done."}"#),
            Ok(Action::Finished {
                title: "Fix it".to_string(),
                summary: "Done.".to_string()
            })
        );
    }

    #[test]
    fn accepts_raw_newlines_inside_strings() {
        let reply =
            "{\"action\":\"shell\",\"command\":\"cat <<'EOF' > a.py\nprint(\\\"hi\\\")\nEOF\"}";
        assert_eq!(
            Action::parse(reply),
            Ok(Action::Shell {
                command: "cat <<'EOF' > a.py\nprint(\"hi\")\nEOF".to_string(),
                reason: None
            })
        );
    }

    #[test]
    fn tolerates_one_fence() {
        let reply = "```json\n{\"action\":\"shell\",\"command\":\"cargo test\"}\n```";
        assert_eq!(
            Action::parse(reply),
            Ok(Action::Shell {
                command: "cargo test".to_string(),
                reason: None
            })
        );
    }

    #[test]
    fn refuses_prose_unknown_fields_and_empty_commands() {
        assert!(Action::parse("I will run ls.").is_err());
        assert!(Action::parse(r#"{"action":"shell","command":"ls","why":"x"}"#).is_err());
        assert!(Action::parse(r#"{"action":"shell","command":"  "}"#).is_err());
        assert!(Action::parse(r#"{"action":"push"}"#).is_err());
    }
}
