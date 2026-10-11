//! A replaceable plan for the current conversation.

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const TOOL: &str = "update_plan";
pub const PREFIX: &str = "Current plan:\n";

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Plan {
    plan: Vec<Step>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Step {
    step: String,
    status: Status,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Status {
    Pending,
    InProgress,
    Completed,
}

pub fn definition() -> Value {
    json!({"type":"function","function":{"name":TOOL,
        "description":"Replace the current plan. Use a short plan for multi-step work, update it as steps finish, and avoid repeating completed checks. An empty plan clears it.",
        "parameters":{"type":"object","additionalProperties":false,"required":["plan"],"properties":{"plan":{"type":"array","maxItems":12,"items":{"type":"object","additionalProperties":false,"required":["step","status"],"properties":{"step":{"type":"string","minLength":1,"maxLength":240},"status":{"type":"string","enum":["pending","in_progress","completed"]}}}}}}}})
}

pub fn execute(arguments: Value) -> Result<Value, String> {
    let plan: Plan = serde_json::from_value(arguments).map_err(|_| {
        "Use a plan array with step and status (pending, in_progress, or completed).".to_owned()
    })?;
    if plan.plan.len() > 12
        || plan.plan.iter().any(|s| {
            s.step.trim().is_empty()
                || s.step.chars().count() > 240
                || s.step.chars().any(char::is_control)
        })
    {
        return Err("Use at most 12 steps, each with 1–240 characters and no line breaks.".into());
    }
    if plan
        .plan
        .iter()
        .filter(|s| matches!(s.status, Status::InProgress))
        .count()
        > 1
    {
        return Err("Only one plan step can be in progress.".into());
    }
    serde_json::to_value(plan).map_err(|_| "The plan could not be saved.".into())
}

pub fn text(output: &Value) -> Option<String> {
    let plan: Plan = serde_json::from_value(output.clone()).ok()?;
    Some(
        plan.plan
            .iter()
            .map(|s| {
                format!(
                    "{} {}",
                    match s.status {
                        Status::Pending => "○",
                        Status::InProgress => "→",
                        Status::Completed => "✓",
                    },
                    s.step
                )
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

pub fn message(output: &Value) -> Option<openrouter::Message> {
    text(output).map(|text| openrouter::Message::system(format!("{PREFIX}{text}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn validates_and_clears_plans() {
        let output = execute(json!({"plan":[{"step":"Read files","status":"completed"},{"step":"Run tests","status":"in_progress"}]})).unwrap();
        assert_eq!(text(&output).unwrap(), "✓ Read files\n→ Run tests");
        assert_eq!(text(&execute(json!({"plan":[]})).unwrap()).unwrap(), "");
        for input in [
            json!({"plan":[{"step":"","status":"pending"}]}),
            json!({"plan":[{"step":"x","status":"wrong"}]}),
            json!({"plan":[],"extra":true}),
            json!({"plan":[{"step":"x","status":"in_progress"},{"step":"y","status":"in_progress"}]}),
        ] {
            assert!(execute(input).is_err());
        }
    }
    #[test]
    fn context_contains_only_the_latest_successful_plan() {
        let tool = |step: &str| crate::live::Entry::Tool {
            name: TOOL.into(),
            input: Value::Null,
            output: execute(json!({"plan":[{"step":step,"status":"pending"}]})).unwrap(),
            running: false,
        };
        let mut chat = crate::live::Chat::default();
        chat.entries = vec![
            tool("old step"),
            crate::live::Entry::User("continue".into()),
            tool("new step"),
        ];
        let messages = chat.messages();
        assert_eq!(messages.len(), 2);
        assert!(messages[0].content.contains("new step"));
        assert!(!messages.iter().any(|m| m.content.contains("old step")));
        chat.entries.push(crate::live::Entry::Tool {
            name: TOOL.into(),
            input: Value::Null,
            output: json!({"error":"bad plan"}),
            running: false,
        });
        assert_eq!(chat.messages(), messages);
    }
}
