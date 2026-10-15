//! Common observations, separate from SDK hooks and execution permissions.

pub use route_contract::observation::*;
use serde_json::Value;

/// One producer for one admitted provider attempt. The owner assigns its scope.
#[derive(Debug)]
pub struct Recorder {
    scope: Scope,
    seq: u64,
    session: Option<String>,
}

impl Recorder {
    pub fn scope(&self) -> &Scope {
        &self.scope
    }
    pub fn new(scope: Scope) -> Self {
        Self {
            scope,
            seq: 0,
            session: None,
        }
    }

    pub fn session(&mut self, session: &str) {
        self.session = Some(session.chars().take(256).collect());
    }

    pub fn next(&mut self, mut item: Option<String>, mut body: Body) -> Observation {
        match &mut body {
            Body::Text {
                text, truncated, ..
            }
            | Body::ToolResult {
                text, truncated, ..
            } => {
                *truncated |= text.len() > MAX_TEXT;
                *text = bounded(text);
            }
            Body::ToolStarted { name, input } => {
                *name = name.chars().take(256).collect();
                *input = bounded(input);
            }
            Body::Plan { items } => {
                items.truncate(64);
                for item in items {
                    item.text = item.text.chars().take(300).collect();
                }
            }
            Body::Request { text, choices, .. } => {
                *text = bounded(text);
                choices.truncate(64);
                for choice in choices {
                    *choice = choice.chars().take(300).collect();
                }
            }
            Body::Ended {
                message: Some(message),
                ..
            } => *message = bounded(message),
            _ => {}
        }
        if item.as_ref().is_some_and(|item| item.len() > 256) {
            item = None;
            body = Body::Gap {
                after: Some(self.seq),
            };
        }
        self.seq += 1;
        Observation {
            schema: SCHEMA.into(),
            scope: self.scope.clone(),
            seq: self.seq,
            native_session: self.session.clone(),
            item,
            child: None,
            body,
        }
    }
}

/// Normalize the Claude SDK's full messages, before executor-summary clipping.
#[derive(Debug)]
pub struct Claude {
    recorder: Recorder,
    messages: u64,
}

impl Claude {
    pub fn scope(&self) -> &Scope {
        self.recorder.scope()
    }
    pub fn new(recorder: Recorder) -> Self {
        Self {
            recorder,
            messages: 0,
        }
    }

    pub fn next(&mut self, item: Option<String>, body: Body) -> Observation {
        self.recorder.next(item, body)
    }

    pub fn session(&mut self, session: &str) -> Observation {
        self.recorder.session(session);
        self.next(
            None,
            Body::Session {
                model: None,
                version: None,
            },
        )
    }

    pub fn assistant(&mut self, message: &Value) -> Vec<Observation> {
        self.messages += 1;
        let id = message["id"]
            .as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| format!("message-{}", self.messages));
        let mut out = Vec::new();
        let text = message["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|block| block["type"] == "text")
            .filter_map(|block| block["text"].as_str())
            .collect::<Vec<_>>()
            .join("");
        let mut text_emitted = false;
        if let Some(model) = message["model"].as_str() {
            out.push(self.next(
                None,
                Body::Session {
                    model: Some(model.into()),
                    version: None,
                },
            ));
        }
        for (index, block) in message["content"]
            .as_array()
            .into_iter()
            .flatten()
            .enumerate()
        {
            let item = Some(format!("{id}:{index}"));
            match block["type"].as_str() {
                Some("text") if !text_emitted => {
                    text_emitted = true;
                    if !text.is_empty() {
                        out.push(self.next(
                            Some(format!("{id}:text")),
                            Body::Text {
                                channel: Channel::Text,
                                mode: TextMode::Snapshot,
                                text: bounded(&text),
                                complete: true,
                                truncated: text.len() > MAX_TEXT,
                            },
                        ));
                    }
                }
                Some("thinking") => {
                    let text = block["thinking"].as_str().unwrap_or("");
                    if !text.is_empty() {
                        out.push(self.next(
                            item,
                            Body::Text {
                                channel: Channel::Reasoning,
                                mode: TextMode::Snapshot,
                                text: bounded(text),
                                complete: true,
                                truncated: text.len() > MAX_TEXT,
                            },
                        ));
                    }
                }
                Some("tool_use") => {
                    let Some(id) = block["id"].as_str().filter(|id| !id.is_empty()) else {
                        continue;
                    };
                    let name = block["name"].as_str().unwrap_or("Tool");
                    let input = &block["input"];
                    out.push(self.next(
                        Some(id.into()),
                        Body::ToolStarted {
                            name: name.chars().take(256).collect(),
                            input: bounded(&serde_json::to_string(input).unwrap_or_default()),
                        },
                    ));
                    if matches!(
                        name,
                        "TodoWrite" | "todowrite" | "todo_write" | "update_plan"
                    ) {
                        let rows = input
                            .get("todos")
                            .or_else(|| input.get("plan"))
                            .and_then(Value::as_array);
                        if let Some(rows) = rows {
                            let items = rows
                                .iter()
                                .take(64)
                                .filter_map(|row| {
                                    Some(PlanItem {
                                        text: row
                                            .get("content")
                                            .or_else(|| row.get("step"))?
                                            .as_str()?
                                            .chars()
                                            .take(300)
                                            .collect(),
                                        status: match row["status"].as_str() {
                                            Some("completed" | "complete" | "done") => {
                                                PlanStatus::Completed
                                            }
                                            Some("in_progress" | "inProgress" | "active") => {
                                                PlanStatus::InProgress
                                            }
                                            _ => PlanStatus::Pending,
                                        },
                                    })
                                })
                                .collect();
                            out.push(self.next(None, Body::Plan { items }));
                        }
                    }
                }
                _ => {}
            }
        }
        out
    }

    pub fn user(&mut self, message: &Value) -> Vec<Observation> {
        let mut out = Vec::new();
        for block in message["content"].as_array().into_iter().flatten() {
            if block["type"] != "tool_result" {
                continue;
            }
            let Some(id) = block["tool_use_id"].as_str().filter(|id| !id.is_empty()) else {
                continue;
            };
            let text = match &block["content"] {
                Value::String(text) => text.clone(),
                Value::Array(parts) => parts
                    .iter()
                    .filter_map(|part| part["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("\n"),
                _ => String::new(),
            };
            out.push(self.next(
                Some(id.into()),
                Body::ToolResult {
                    text: bounded(&text),
                    failed: block["is_error"].as_bool().unwrap_or(false),
                    truncated: text.len() > MAX_TEXT,
                },
            ));
        }
        out
    }

    pub fn usage(&mut self, usage: &Value) -> Observation {
        self.next(
            None,
            Body::Usage {
                usage: Usage {
                    input: usage["input_tokens"].as_u64(),
                    output: usage["output_tokens"].as_u64(),
                    cache_read: usage["cache_read_input_tokens"].as_u64(),
                    cache_write: usage["cache_creation_input_tokens"].as_u64(),
                },
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn full_messages_keep_ids_text_tools_plans_and_unknown_usage() {
        let mut adapter = Claude::new(Recorder::new(Scope {
            host: "local".into(),
            task: "task".into(),
            turn: 1,
            generation: 1,
        }));
        let text = "full output ".repeat(600);
        let lines = adapter.assistant(&json!({"id":"m1","model":"model","content":[
            {"type":"text","text":text}, {"type":"text","text":" tail"},
            {"type":"tool_use","id":"c1","name":"TodoWrite","input":{"todos":[{"content":"Test","status":"in_progress"}]}}
        ]}));
        assert!(
            matches!(&lines[1].body, Body::Text { text: actual, .. } if actual == &format!("{text} tail"))
        );
        assert_eq!(lines[1].item.as_deref(), Some("m1:text"));
        assert_eq!(lines[2].item.as_deref(), Some("c1"));
        assert!(matches!(&lines[3].body, Body::Plan { items } if items.len() == 1));
        let result = adapter
            .user(&json!({"content":[{"type":"tool_result","tool_use_id":"c1","content":"done"}]}));
        assert_eq!(result[0].item, lines[2].item);
        let usage = adapter.usage(&json!({"output_tokens":0}));
        assert!(matches!(
            usage.body,
            Body::Usage {
                usage: Usage {
                    input: None,
                    output: Some(0),
                    ..
                }
            }
        ));
        assert_eq!(usage.seq, 6);
    }

    #[test]
    fn oversized_output_is_marked_and_oversized_item_ids_report_a_gap() {
        let mut recorder = Recorder::new(Scope {
            host: "local".into(),
            task: "task".into(),
            turn: 1,
            generation: 1,
        });
        let observation = recorder.next(
            Some("item".into()),
            Body::Text {
                channel: Channel::Text,
                mode: TextMode::Snapshot,
                text: "日".repeat(MAX_TEXT),
                complete: true,
                truncated: false,
            },
        );
        assert!(
            matches!(observation.body, Body::Text { text, truncated: true, .. } if text.len() <= MAX_TEXT)
        );
        let observation = recorder.next(
            Some("x".repeat(257)),
            Body::ToolResult {
                text: "done".into(),
                failed: false,
                truncated: false,
            },
        );
        assert!(matches!(observation.body, Body::Gap { after: Some(1) }));
        assert!(observation.item.is_none());
    }
}
