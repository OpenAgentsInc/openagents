//! The next action as JSON in plain text, for a model reached without a
//! structured-output guarantee: an OpenAI-compatible server that ignores
//! or refuses the strict `json_schema` response format (Flint through
//! `rec`), and the Pylon conversation lane, which carries text only.
//!
//! [`reminder`] is appended to the system prompt and spells out the
//! object the reply must be. [`parse`] reads the reply robustly: it drops
//! a reasoning block and code fences, takes the first JSON object that
//! reads as a [`NextAction`], and fills the members a small model tends to
//! leave out with their empty values.

use serde_json::Value;

use crate::models::{NextAction, next_action_schema};

/// What the system prompt says when the action comes back as text.
#[must_use]
pub fn reminder() -> String {
    let schema = next_action_schema().to_string();
    format!(
        "\n\n# Reply format\n\nReply with exactly one JSON object and nothing else: no prose \
         before or after it, no code fences. It is your next action, and it must match this \
         JSON schema, with every required member present:\n\n{schema}\n\nAn example of a step \
         that runs commands:\n{{\"reply\": \"\", \"ask\": \"none\", \"rationale\": \"See what \
         is here.\", \"commands\": [\"ls -la\"], \"view\": [], \"freeze_tests\": false, \
         \"expand\": [], \"finished\": false}}\n"
    )
}

/// A shorter [`reminder`] for a model with little room for its prompt:
/// the members and their types without the schema's descriptions, which
/// the system prompt already covers.
#[must_use]
pub fn brief_reminder() -> String {
    "\n\n# Reply format\n\nReply with exactly one JSON object and nothing else (no prose, no \
     code fences), with these members: reply (string), ask (\"none\" or \"question\"), \
     rationale (string), commands (array of bash scripts as strings), view (array of file \
     paths), freeze_tests (boolean), expand (array of strings), finished (boolean). Example: \
     {\"reply\": \"\", \"ask\": \"none\", \"rationale\": \"See what is here.\", \
     \"commands\": [\"ls -la\"], \"view\": [], \"freeze_tests\": false, \"expand\": [], \
     \"finished\": false}\n"
        .to_string()
}

/// The action in `raw`, or why there is none. The error quotes the start
/// of the reply.
///
/// # Errors
///
/// When no JSON object in the reply reads as an action.
pub fn parse(raw: &str) -> Result<NextAction, String> {
    let text = without_reasoning(raw);
    let mut first_error = None;
    for (start, _) in text.match_indices('{') {
        let mut values = serde_json::Deserializer::from_str(&text[start..]).into_iter::<Value>();
        let Some(Ok(value)) = values.next() else {
            continue;
        };
        if !value.is_object() {
            continue;
        }
        match action(value) {
            Ok(action) => return Ok(action),
            Err(why) => {
                first_error.get_or_insert(why);
            }
        }
    }
    let excerpt: String = raw.trim().chars().take(400).collect();
    Err(match first_error {
        Some(why) => format!("the reply didn't match the schema ({why}): {excerpt}"),
        None => format!("the reply holds no JSON object: {excerpt}"),
    })
}

/// `raw` without a `<think>` block that leaked into the content.
fn without_reasoning(raw: &str) -> &str {
    match raw.rfind("</think>") {
        Some(end) => &raw[end + "</think>".len()..],
        None => raw,
    }
}

/// `value` as an action, with the members a model left out at their empty
/// values. An object with none of the action's members isn't one.
fn action(mut value: Value) -> Result<NextAction, String> {
    let object = value.as_object_mut().ok_or("not an object")?;
    const MEMBERS: [&str; 4] = ["commands", "finished", "rationale", "reply"];
    if !MEMBERS.iter().any(|member| object.contains_key(*member)) {
        return Err("an object with none of the action's members".to_string());
    }
    // Commands written as one string, or as objects that hold the script
    // (`{"bash": "ls"}`), or a whole action nested in the list.
    if let Some(commands) = object.remove("commands") {
        let mut scripts = Vec::new();
        flatten(&commands, &mut scripts);
        object.insert("commands".into(), Value::Array(scripts));
    }
    for (member, empty) in [
        ("rationale", Value::String(String::new())),
        ("reply", Value::String(String::new())),
        ("commands", Value::Array(Vec::new())),
        ("view", Value::Array(Vec::new())),
        ("expand", Value::Array(Vec::new())),
        ("freeze_tests", Value::Bool(false)),
        ("finished", Value::Bool(false)),
    ] {
        if object.get(member).is_none_or(Value::is_null) {
            object.insert(member.into(), empty);
        }
    }
    if object
        .get("ask")
        .and_then(Value::as_str)
        .is_none_or(|ask| !["none", "question", "approval"].contains(&ask))
    {
        object.insert("ask".into(), Value::String("none".into()));
    }
    serde_json::from_value(value).map_err(|error| error.to_string())
}

/// The scripts in a `commands` value, in order: strings as they are, and
/// from an object its `bash`, `command`, `cmd`, `script`, or `run` string,
/// or a nested action's own `commands`. Anything else is dropped.
fn flatten(value: &Value, scripts: &mut Vec<Value>) {
    match value {
        Value::String(script) => scripts.push(Value::String(script.clone())),
        Value::Array(items) => {
            for item in items {
                flatten(item, scripts);
            }
        }
        Value::Object(object) => {
            if let Some(script) = ["bash", "command", "cmd", "script", "run"]
                .iter()
                .find_map(|key| object.get(*key).and_then(Value::as_str))
            {
                scripts.push(Value::String(script.to_string()));
            } else if let Some(nested) = object.get("commands") {
                flatten(nested, scripts);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_fenced_object_after_prose_and_reasoning() {
        let raw = "<think>{\"commands\": [\"rm -rf /\"]}</think>Here it is:\n```json\n{\"rationale\": \"look\", \"commands\": [\"ls\"], \"finished\": false}\n```\nDone.";
        let action = parse(raw).unwrap();
        assert_eq!(action.commands, vec!["ls".to_string()]);
        assert_eq!(action.rationale, "look");
        assert!(!action.finished);
        assert!(action.view.is_empty());
    }

    #[test]
    fn fills_missing_members_and_a_single_command() {
        let action = parse("{\"commands\": \"cargo test\", \"ask\": null}").unwrap();
        assert_eq!(action.commands, vec!["cargo test".to_string()]);
        assert_eq!(action.ask, crate::models::Ask::None);
        let finished = parse("{\"finished\": true, \"reply\": \"Fixed it.\"}").unwrap();
        assert!(finished.finished);
        assert_eq!(finished.reply, "Fixed it.");
    }

    #[test]
    fn reads_commands_written_as_objects_or_a_nested_action() {
        let raw = r#"{"rationale": "look", "commands": [{"reply": "", "commands": [{"bash": "pwd"}]}, {"command": "ls"}, "id"], "finished": false}"#;
        assert_eq!(
            parse(raw).unwrap().commands,
            vec!["pwd".to_string(), "ls".to_string(), "id".to_string()]
        );
    }

    #[test]
    fn skips_objects_that_are_not_actions() {
        let raw = "The schema is {\"type\": \"object\"}; my action: {\"commands\": [\"pwd\"], \"finished\": false}";
        assert_eq!(parse(raw).unwrap().commands, vec!["pwd".to_string()]);
    }

    #[test]
    fn says_why_when_there_is_no_action() {
        assert!(
            parse("I will run ls now.")
                .unwrap_err()
                .contains("no JSON object")
        );
        assert!(
            parse("{\"finished\": \"soon\"}")
                .unwrap_err()
                .contains("didn't match the schema")
        );
    }

    #[test]
    fn the_reminder_carries_the_schema() {
        let reminder = reminder();
        assert!(reminder.contains("\"freeze_tests\""));
        assert!(reminder.contains("exactly one JSON object"));
        let brief = brief_reminder();
        assert!(brief.len() < reminder.len() / 3);
        // The brief reminder's example is itself an action.
        let example = &brief[brief.find('{').unwrap()..];
        assert_eq!(parse(example).unwrap().commands, vec!["ls -la".to_string()]);
    }
}
