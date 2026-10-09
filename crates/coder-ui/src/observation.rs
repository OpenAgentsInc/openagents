//! Read-only task presentation. The application supplies admitted, bounded
//! records and their original source identities; these views own no transport.

use crate::workspace::Palette;
use rust_native::style::{Color, Space, Style};
use rust_native::{Axis, Element, MessageRole, Node, TextRole, ToolState, View};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// These views have no application effects.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObservationIntent {}

#[derive(Clone, Copy)]
pub struct Source<'a> {
    /// Stable, unique presentation key supplied by the application.
    pub key: &'a str,
    pub host: &'a str,
    pub workspace: &'a str,
    pub task: &'a str,
    pub attempt: Option<&'a str>,
    pub revision: &'a str,
}

pub struct Task<'a> {
    pub source: Source<'a>,
    pub title: &'a str,
    pub status: &'a str,
    pub execution: &'a str,
    pub checks: &'a str,
    pub delivery: &'a str,
    pub integration: &'a str,
    pub stop: &'a str,
    pub cleanup: &'a str,
    pub cost: &'a str,
}

pub struct Evidence<'a> {
    pub source: Source<'a>,
    pub label: &'a str,
    pub state: &'a str,
    pub summary: &'a str,
}

pub struct File<'a> {
    pub source: Source<'a>,
    pub label: &'a str,
    pub media_type: &'a str,
    pub bytes: &'a str,
    pub digest: &'a str,
    pub retention: &'a str,
    pub content: Option<&'a str>,
}

pub struct Step<'a> {
    pub source: Source<'a>,
    /// Original absolute index, independent of the displayed page position.
    pub index: u64,
    pub label: &'a str,
    pub record: &'a Value,
}

fn text(
    key: impl Into<String>,
    value: impl Into<String>,
    role: TextRole,
    color: Color,
) -> Node<ObservationIntent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(color),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

fn stack(
    key: impl Into<String>,
    children: Vec<Node<ObservationIntent>>,
) -> Node<ObservationIntent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(Space::Sm),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    }
}

fn source(source: Source<'_>, key: &str, palette: Palette) -> Vec<Node<ObservationIntent>> {
    [
        ("host", "Host", source.host),
        ("workspace", "Workspace", source.workspace),
        ("task", "Task", source.task),
        (
            "attempt",
            "Attempt",
            source.attempt.unwrap_or("Not recorded"),
        ),
        ("revision", "Revision", source.revision),
    ]
    .into_iter()
    .map(|(suffix, label, value)| {
        text(
            format!("{key}:{suffix}"),
            format!("{label}: {value}"),
            TextRole::Code,
            palette.secondary,
        )
    })
    .collect()
}

fn task_node(value: &Task<'_>, palette: Palette) -> Node<ObservationIntent> {
    let key = value.source.key;
    let mut children = vec![text(
        format!("{key}:title"),
        value.title,
        TextRole::Heading,
        palette.heading,
    )];
    children.extend(source(value.source, key, palette));
    children.extend(
        [
            ("status", "Status", value.status),
            ("execution", "Execution", value.execution),
            ("checks", "Checks", value.checks),
            ("delivery", "Delivery", value.delivery),
            ("integration", "Integration", value.integration),
            ("stop", "Stop", value.stop),
            ("cleanup", "Cleanup", value.cleanup),
            ("cost", "Cost", value.cost),
        ]
        .into_iter()
        .map(|(suffix, label, value)| {
            text(
                format!("{key}:{suffix}"),
                format!("{label}: {value}"),
                TextRole::Status,
                palette.text,
            )
        }),
    );
    stack(key, children)
}

pub fn task(value: &Task<'_>, palette: Palette) -> View<ObservationIntent> {
    View::new_v3("observation-task", 1, task_node(value, palette))
}

pub fn tasks(values: &[Task<'_>], palette: Palette) -> View<ObservationIntent> {
    View::new_v3(
        "observation-tasks",
        1,
        Node {
            key: "observation-tasks".into(),
            style: Style {
                gap: Some(Space::Md),
                ..Style::default()
            },
            element: Element::List {
                label: "Tasks".into(),
                children: values
                    .iter()
                    .map(|value| task_node(value, palette))
                    .collect(),
            },
        },
    )
}

pub fn evidence(value: &Evidence<'_>, palette: Palette) -> View<ObservationIntent> {
    let key = value.source.key;
    let mut children = vec![text(
        format!("{key}:label"),
        value.label,
        TextRole::Heading,
        palette.heading,
    )];
    children.extend(source(value.source, key, palette));
    children.push(text(
        format!("{key}:state"),
        value.state,
        TextRole::Status,
        palette.text,
    ));
    children.push(text(
        format!("{key}:summary"),
        value.summary,
        TextRole::Body,
        palette.text,
    ));
    View::new_v3("observation-evidence", 1, stack(key, children))
}

pub fn file(value: &File<'_>, palette: Palette) -> View<ObservationIntent> {
    let key = value.source.key;
    let mut children = vec![text(
        format!("{key}:label"),
        value.label,
        TextRole::Heading,
        palette.heading,
    )];
    children.extend(source(value.source, key, palette));
    children.extend(
        [
            ("media", "Media type", value.media_type),
            ("bytes", "Bytes", value.bytes),
            ("retention", "Kept", value.retention),
        ]
        .into_iter()
        .map(|(suffix, label, value)| {
            text(
                format!("{key}:{suffix}"),
                format!("{label}: {value}"),
                TextRole::Code,
                palette.secondary,
            )
        }),
    );
    children.push(text(
        format!("{key}:content"),
        value.content.unwrap_or("Contents not shown."),
        TextRole::Code,
        palette.text,
    ));
    View::new_v3("observation-file", 1, stack(key, children))
}

/// Preserve the original record as inert JSON instead of inferring an outcome
/// from tool text or admitting embedded links and markup.
pub fn original_step(value: &Step<'_>, palette: Palette) -> View<ObservationIntent> {
    let key = format!("{}:step-{}", value.source.key, value.index);
    let mut children = vec![text(
        format!("{key}:label"),
        format!("{} · Step {}", value.label, value.index),
        TextRole::Heading,
        palette.heading,
    )];
    children.extend(source(value.source, &key, palette));
    children.push(text(
        format!("{key}:record"),
        value.record.to_string(),
        TextRole::Code,
        palette.text,
    ));
    View::new_v3("observation-step", 1, stack(key, children))
}

fn content(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.into();
    }
    if let Some(parts) = value.as_array() {
        return parts
            .iter()
            .map(|part| {
                if part.get("type").and_then(Value::as_str) == Some("text") {
                    part.get("text")
                        .and_then(Value::as_str)
                        .map(str::to_owned)
                        .unwrap_or_else(|| part.to_string())
                } else {
                    format!("Attachment not loaded: {part}")
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    value.to_string()
}

/// Render original ATIF content without deriving task success from its prose.
/// Links remain inert, attachments are not fetched, and absent outcomes stay
/// unknown. The application may show [`original_step`] beside this view.
pub fn step(value: &Step<'_>, palette: Palette) -> View<ObservationIntent> {
    let key = format!("{}:step-{}", value.source.key, value.index);
    let mut children = vec![text(
        format!("{key}:label"),
        format!("{} · Step {}", value.label, value.index),
        TextRole::Heading,
        palette.heading,
    )];
    children.extend(source(value.source, &key, palette));
    if let Some(message) = value.record.get("message") {
        let message_key = format!("{key}:message");
        let body = Node {
            key: format!("{message_key}:body"),
            style: Style {
                foreground: Some(palette.text),
                ..Style::default()
            },
            element: Element::Markdown {
                blocks: rust_native::markdown::parse(&content(message)),
            },
        };
        let role = match value.record.get("source").and_then(Value::as_str) {
            Some("user") => Some(MessageRole::User),
            Some("agent") => Some(MessageRole::Assistant),
            Some("system") => Some(MessageRole::System),
            _ => None,
        };
        children.push(if let Some(role) = role {
            Node {
                key: message_key,
                style: Style::default(),
                element: Element::Message {
                    role,
                    note: value
                        .record
                        .get("timestamp")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    children: vec![body],
                },
            }
        } else {
            stack(
                message_key,
                vec![
                    text(
                        format!("{key}:unknown-source"),
                        "Message source is not recorded.",
                        TextRole::Status,
                        palette.secondary,
                    ),
                    body,
                ],
            )
        });
    }
    if let Some(reasoning) = value.record.get("reasoning_content") {
        children.push(text(
            format!("{key}:reasoning-label"),
            "Recorded reasoning",
            TextRole::Status,
            palette.secondary,
        ));
        children.push(text(
            format!("{key}:reasoning"),
            content(reasoning),
            TextRole::Code,
            palette.text,
        ));
    }
    let results = value
        .record
        .pointer("/observation/results")
        .and_then(Value::as_array);
    for (index, call) in value
        .record
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let call_key = format!("{key}:call-{index}");
        let id = call.get("tool_call_id").and_then(Value::as_str);
        let matched: Vec<_> = results
            .into_iter()
            .flatten()
            .filter(|result| {
                id.is_some() && result.get("source_call_id").and_then(Value::as_str) == id
            })
            .collect();
        let unique_call = id.is_some()
            && value
                .record
                .get("tool_calls")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .filter(|call| call.get("tool_call_id").and_then(Value::as_str) == id)
                .count()
                == 1;
        let state = if unique_call && matched.len() == 1 {
            match matched[0].pointer("/extra/status").and_then(Value::as_str) {
                Some("completed") => Some(ToolState::Done),
                Some("failed") => Some(ToolState::Failed),
                _ => None,
            }
        } else {
            None
        };
        let name = call
            .get("function_name")
            .and_then(Value::as_str)
            .unwrap_or("Unnamed tool");
        let mut outputs = vec![text(
            format!("{call_key}:arguments"),
            call.get("arguments").unwrap_or(&Value::Null).to_string(),
            TextRole::Code,
            palette.text,
        )];
        for (index, result) in matched.iter().enumerate() {
            if let Some(output) = result.get("content") {
                outputs.push(text(
                    format!("{call_key}:output-{index}"),
                    content(output),
                    TextRole::Code,
                    palette.text,
                ));
            }
        }
        children.push(if let Some(state) = state {
            Node {
                key: call_key,
                style: Style::default(),
                element: Element::Tool {
                    name: name.into(),
                    detail: id.unwrap_or_default().into(),
                    state,
                    children: outputs,
                },
            }
        } else {
            let outcome = if unique_call
                && matched.len() == 1
                && matched[0].pointer("/extra/status").and_then(Value::as_str) == Some("cancelled")
            {
                "Cancelled before execution"
            } else {
                "Outcome not recorded"
            };
            outputs.insert(
                0,
                text(
                    format!("{call_key}:state"),
                    format!("{name} · {outcome}"),
                    TextRole::Status,
                    palette.secondary,
                ),
            );
            stack(call_key, outputs)
        });
    }
    // Unassociated results stay visible without being attributed to a tool.
    for (index, result) in results.into_iter().flatten().enumerate() {
        let id = result.get("source_call_id").and_then(Value::as_str);
        let associated = id.is_some()
            && value
                .record
                .get("tool_calls")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .any(|call| call.get("tool_call_id").and_then(Value::as_str) == id);
        if !associated {
            children.push(text(
                format!("{key}:unassociated-{index}"),
                format!("Other result: {result}"),
                TextRole::Code,
                palette.text,
            ));
        }
    }
    View::new_v3("observation-step", 1, stack(key, children))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn palette() -> Palette {
        let color = Color::rgb(255, 255, 255);
        Palette {
            text: color,
            heading: color,
            secondary: color,
            border: color,
        }
    }
    fn identity() -> Source<'static> {
        Source {
            key: "source-1",
            host: "host-1",
            workspace: "workspace-1",
            task: "task-1",
            attempt: Some("attempt-2"),
            revision: "revision-7",
        }
    }
    #[test]
    fn executor_end_does_not_replace_independent_evidence_or_unknown_cost() {
        let view = task(
            &Task {
                source: identity(),
                title: "Candidate",
                status: "Needs review",
                execution: "Ended",
                checks: "Failed",
                delivery: "Candidate retained",
                integration: "Not attempted",
                stop: "Not requested",
                cleanup: "Unknown",
                cost: "Unknown",
            },
            palette(),
        )
        .validate()
        .unwrap();
        let value = serde_json::to_string(view.view()).unwrap();
        for original in [
            "Execution: Ended",
            "Checks: Failed",
            "Delivery: Candidate retained",
            "Integration: Not attempted",
            "Cleanup: Unknown",
            "Cost: Unknown",
            "Attempt: attempt-2",
            "Revision: revision-7",
        ] {
            assert!(value.contains(original));
        }
        assert!(!value.contains("\"button\""));
    }
    #[test]
    fn transcript_record_is_inert_and_keeps_its_absolute_source_position() {
        let record = serde_json::json!({"text":"<script>untrusted()</script>","step_id":31,"source":"original"});
        let view = original_step(
            &Step {
                source: identity(),
                index: 31,
                label: "Retained record",
                record: &record,
            },
            palette(),
        )
        .validate()
        .unwrap();
        let Element::Stack { children, .. } = &view.view().root.element else {
            panic!("expected a stack")
        };
        assert!(
            matches!(&children.last().unwrap().element, Element::Text { value, role:TextRole::Code } if value == &record.to_string())
        );
        assert_eq!(view.view().root.key, "source-1:step-31");
    }

    #[test]
    fn human_transcript_requires_an_exact_typed_tool_outcome() {
        let record = serde_json::json!({"source":"agent","message":"A **message** with [an inert link](https://example.invalid)","tool_calls":[{"tool_call_id":"call-1","function_name":"shell","arguments":{"command":"false"}}],"observation":{"results":[{"source_call_id":"call-1","content":"This prose claims success","extra":{"status":"failed"}}]}});
        let render = |record: &Value| {
            step(
                &Step {
                    source: identity(),
                    index: 2,
                    label: "Original record",
                    record,
                },
                palette(),
            )
            .validate()
            .unwrap()
        };
        let view = render(&record);
        let Element::Stack { children, .. } = &view.view().root.element else {
            panic!("expected a stack")
        };
        assert!(matches!(
            &children.last().unwrap().element,
            Element::Tool {
                state: ToolState::Failed,
                ..
            }
        ));
        let mut unknown = record.clone();
        unknown["observation"]["results"][0]["source_call_id"] =
            Value::String("another-call".into());
        let value = serde_json::to_value(render(&unknown).view()).unwrap();
        assert!(!value.to_string().contains("\"kind\":\"tool\""));
        assert!(value.to_string().contains("Outcome not recorded"));
        assert!(value.to_string().contains("Other result"));
        let mut ambiguous = record.clone();
        let call = ambiguous["tool_calls"][0].clone();
        ambiguous["tool_calls"].as_array_mut().unwrap().push(call);
        let value = serde_json::to_value(render(&ambiguous).view()).unwrap();
        assert!(!value.to_string().contains("\"kind\":\"tool\""));
    }
}
