//! The `github` tool (#11166): GitHub issues and Project boards as one
//! typed tool. Each call becomes an `openagents issue|project` command
//! and runs through the bundled CLI, so it uses this computer's GitHub
//! sign-in and the same approval gate as every other command: reads
//! (`issue_list`, `issue_view`, `project_list`) run, and writes wait for
//! the person's confirm when the host gates commands.

use serde::Deserialize;
use serde_json::{Value, json};

/// The tool's definition.
pub fn definition() -> Value {
    json!({"type":"function","function":{
        "name":"github",
        "description":"Manage GitHub issues and Project boards for the repository here (or `repo`). Actions: issue_create (title, body, labels, project, status), issue_comment (number, body), issue_close (number, reason completed|not_planned, comment; also moves the issue to Done on its boards), issue_reopen (number, comment), issue_list (state, labels, limit), issue_view (number), project_list (project, status: items with that status, \"none\" for unset), project_add (number, project, status), project_move (number, status, project: without it, on every open board the issue is on). Writes change GitHub for everyone; call them only for work the user asked for.",
        "parameters":{"type":"object","properties":{
            "action":{"type":"string","enum":["issue_create","issue_comment","issue_close","issue_reopen","issue_list","issue_view","project_list","project_add","project_move"]},
            "repo":{"type":"string","description":"OWNER/NAME; the checkout's repository when left out."},
            "number":{"type":"integer","minimum":1,"description":"The issue number."},
            "title":{"type":"string","maxLength":256},
            "body":{"type":"string","maxLength":65536,"description":"Issue or comment text."},
            "comment":{"type":"string","maxLength":65536,"description":"A comment posted before closing or reopening."},
            "labels":{"type":"array","items":{"type":"string"},"maxItems":20},
            "project":{"type":"integer","minimum":1,"description":"The board number, such as 22."},
            "status":{"type":"string","maxLength":100,"description":"A board status, such as Todo, In Progress, Blocked, Done."},
            "reason":{"type":"string","enum":["completed","not_planned"]},
            "state":{"type":"string","enum":["open","closed","all"]},
            "limit":{"type":"integer","minimum":1,"maximum":1000}
        },"required":["action"],"additionalProperties":false}
    }})
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    action: String,
    repo: Option<String>,
    number: Option<u64>,
    title: Option<String>,
    body: Option<String>,
    comment: Option<String>,
    #[serde(default)]
    labels: Vec<String>,
    project: Option<u64>,
    status: Option<String>,
    reason: Option<String>,
    state: Option<String>,
    limit: Option<u64>,
}

/// The `openagents` arguments for one call.
///
/// # Errors
/// The call misses a field its action needs, or names no known action.
pub fn arguments(input: Value) -> Result<Vec<String>, String> {
    let call: Call = serde_json::from_value(input)
        .map_err(|error| format!("github: the arguments do not match the schema: {error}"))?;
    let number = || {
        call.number
            .map(|n| n.to_string())
            .ok_or_else(|| format!("github: {} needs `number`", call.action))
    };
    let mut words: Vec<String> = Vec::new();
    let flag = |words: &mut Vec<String>, name: &str, value: Option<String>| {
        if let Some(value) = value {
            words.push(format!("--{name}"));
            words.push(value);
        }
    };
    let project = call.project.map(|n| n.to_string());
    match call.action.as_str() {
        "issue_create" => {
            let title = call
                .title
                .clone()
                .filter(|t| !t.trim().is_empty())
                .ok_or("github: issue_create needs `title`")?;
            words.extend(["issue".into(), "create".into()]);
            flag(&mut words, "title", Some(title));
            flag(&mut words, "body", call.body.clone());
            for label in &call.labels {
                flag(&mut words, "label", Some(label.clone()));
            }
            flag(&mut words, "project", project);
            flag(&mut words, "status", call.status.clone());
        }
        "issue_comment" => {
            words.extend(["issue".into(), "comment".into(), number()?]);
            let body = call
                .body
                .clone()
                .filter(|b| !b.trim().is_empty())
                .ok_or("github: issue_comment needs `body`")?;
            flag(&mut words, "body", Some(body));
        }
        "issue_close" => {
            words.extend(["issue".into(), "close".into(), number()?]);
            flag(&mut words, "reason", call.reason.clone());
            flag(&mut words, "comment", call.comment.clone());
            flag(&mut words, "project", project);
        }
        "issue_reopen" => {
            words.extend(["issue".into(), "reopen".into(), number()?]);
            flag(&mut words, "comment", call.comment.clone());
        }
        "issue_list" => {
            words.extend(["issue".into(), "list".into()]);
            flag(&mut words, "state", call.state.clone());
            for label in &call.labels {
                flag(&mut words, "label", Some(label.clone()));
            }
            flag(&mut words, "limit", call.limit.map(|n| n.to_string()));
        }
        "issue_view" => words.extend(["issue".into(), "view".into(), number()?]),
        "project_list" => {
            words.extend(["project".into(), "list".into()]);
            flag(&mut words, "project", project);
            flag(&mut words, "status", call.status.clone());
        }
        "project_add" => {
            words.extend(["project".into(), "add".into(), number()?]);
            flag(&mut words, "project", project);
            flag(&mut words, "status", call.status.clone());
        }
        "project_move" => {
            words.extend(["project".into(), "move".into(), number()?]);
            let status = call
                .status
                .clone()
                .ok_or("github: project_move needs `status`")?;
            flag(&mut words, "status", Some(status));
            flag(&mut words, "project", project);
        }
        other => return Err(format!("github: unknown action `{other}`")),
    }
    flag(&mut words, "repo", call.repo.clone());
    Ok(words)
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder::task::agent::{Effect, effect};

    fn shown(words: &[String]) -> String {
        std::iter::once("openagents".to_owned())
            .chain(words.iter().map(|w| crate::bundled_runtime::shell_word(w)))
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn each_action_becomes_one_cli_command_and_writes_need_approval() {
        let cases = [
            (
                json!({"action":"issue_create","title":"Ship it","body":"Why.\nHow.","labels":["a"],"project":22,"status":"Todo"}),
                "openagents issue create --title 'Ship it' --body 'Why.\nHow.' --label a --project 22 --status Todo",
                false,
            ),
            (
                json!({"action":"issue_comment","number":5,"body":"Done"}),
                "openagents issue comment 5 --body Done",
                false,
            ),
            (
                json!({"action":"issue_close","number":5,"reason":"not_planned"}),
                "openagents issue close 5 --reason not_planned",
                false,
            ),
            (
                json!({"action":"issue_reopen","number":5}),
                "openagents issue reopen 5",
                false,
            ),
            (
                json!({"action":"project_move","number":5,"status":"In Progress","repo":"a/b"}),
                "openagents project move 5 --status 'In Progress' --repo a/b",
                false,
            ),
            (
                json!({"action":"project_add","number":5,"project":22}),
                "openagents project add 5 --project 22",
                false,
            ),
            (
                json!({"action":"issue_list","state":"all","limit":5}),
                "openagents issue list --state all --limit 5",
                true,
            ),
            (
                json!({"action":"issue_view","number":5}),
                "openagents issue view 5",
                true,
            ),
            (
                json!({"action":"project_list","project":22,"status":"Todo"}),
                "openagents project list --project 22 --status Todo",
                true,
            ),
        ];
        for (input, want, read) in cases {
            let words = arguments(input).unwrap();
            let line = shown(&words);
            assert_eq!(line, want);
            let classified = effect(&line);
            if read {
                assert_eq!(classified, Effect::ReadOnly, "{line}");
            } else {
                assert!(matches!(classified, Effect::Approval(_)), "{line}");
            }
        }
    }

    #[test]
    fn a_call_missing_what_its_action_needs_is_refused() {
        for input in [
            json!({"action":"issue_create"}),
            json!({"action":"issue_comment","number":1}),
            json!({"action":"issue_close"}),
            json!({"action":"project_move","number":1}),
            json!({"action":"delete_repo"}),
            json!({"action":"issue_view","number":1,"token":"x"}),
        ] {
            assert!(arguments(input).is_err());
        }
    }
}
