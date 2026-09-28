use crate::{MAX_READABLE_RECORD_BYTES, Readable};
use serde_json::Value;

fn field(value: &Value, name: &str) -> Option<String> {
    value
        .get(name)
        .and_then(Value::as_str)
        .filter(|s| s.len() <= 128)
        .map(str::to_owned)
}

fn trim(value: &str, max: usize) -> (String, bool) {
    let mut end = value.len().min(max);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (value[..end].to_owned(), end < value.len())
}

/// Project a complete JSON record for display without executing its contents.
/// At most 256 KiB is parsed; larger records return None and remain available
/// through raw chunks. Unknown and malformed records are explicit projections.
/// Text is a 1 KiB preview, never a substitute for the original record bytes.
pub fn readable_record(raw: &[u8]) -> Option<Readable> {
    project(raw, 1024)
}

/// Project all recognized text from a locally assembled record. The input is
/// still capped at 256 KiB, but text is not shortened to the host-page preview.
/// The caller must page the resulting text for display and retain original
/// bytes for unknown fields and records larger than the parser bound.
pub fn readable_record_full(raw: &[u8]) -> Option<Readable> {
    project(raw, usize::MAX)
}

fn project(raw: &[u8], text_limit: usize) -> Option<Readable> {
    if raw.len() > MAX_READABLE_RECORD_BYTES {
        return None;
    }
    let parsed = serde_json::from_slice::<Value>(raw);
    let Ok(value) = parsed else {
        return Some(Readable {
            kind: "invalid_json".into(),
            native_id: None,
            role: None,
            timestamp: None,
            tool_name: None,
            call_id: None,
            text: "Unrecognized JSON record; original bytes retained.".into(),
            text_truncated: false,
            unknown: true,
        });
    };
    if let Some(readable) = atif(&value, text_limit) {
        return Some(readable);
    }
    if let Some(readable) = opencode(&value, text_limit) {
        return Some(readable);
    }
    if let Some(readable) = devin(&value, text_limit) {
        return Some(readable);
    }
    let outer = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let payload = value.get("payload").unwrap_or(&value);
    let kind = payload.get("type").and_then(Value::as_str).unwrap_or(outer);
    let message = payload
        .get("message")
        .filter(|v| v.is_object())
        .unwrap_or(payload);
    let tool = message
        .get("content")
        .and_then(Value::as_array)
        .and_then(|items| {
            items.iter().find(|item| {
                matches!(
                    item.get("type").and_then(Value::as_str),
                    Some("tool_use" | "tool_result")
                )
            })
        });
    let mut pieces = Vec::new();
    if let Some(content) = message.get("content") {
        collect(content, &mut pieces);
    }
    for key in ["text", "message", "arguments", "input", "output"] {
        if let Some(item) = payload
            .get(key)
            .filter(|v| !v.is_object() || key != "message")
        {
            if key == "input" && item.is_object() {
                pieces.push(item.to_string());
            } else {
                collect(item, &mut pieces);
            }
        }
    }
    let text = pieces.join("\n");
    let (text, text_truncated) = trim(&text, text_limit);
    let known = matches!(
        kind,
        "message"
            | "user"
            | "assistant"
            | "user_message"
            | "agent_message"
            | "function_call"
            | "function_call_output"
            | "custom_tool_call"
            | "custom_tool_call_output"
            | "tool_use"
            | "tool_result"
            | "reasoning"
            | "session_meta"
            | "turn_context"
            | "token_count"
            | "task_started"
            | "task_complete"
            | "turn_aborted"
            | "compacted"
            | "summary"
    );
    let role = field(message, "role").or_else(|| match kind {
        "user_message" | "user" => Some("user".into()),
        "agent_message" | "assistant" => Some("assistant".into()),
        _ => None,
    });
    Some(Readable {
        kind: trim(kind, 128).0,
        native_id: field(&value, "uuid").or_else(|| field(payload, "id")),
        role,
        timestamp: field(&value, "timestamp").or_else(|| field(payload, "timestamp")),
        tool_name: field(payload, "name").or_else(|| tool.and_then(|t| field(t, "name"))),
        call_id: field(payload, "call_id")
            .or_else(|| tool.and_then(|t| field(t, "id").or_else(|| field(t, "tool_use_id")))),
        text,
        text_truncated,
        unknown: !known,
    })
}

/// Project one record of a Coder task transcript (`*.atif.jsonl`, written by
/// `crates/atif`): a `session` header, a `step`, or an `end`. Returns None
/// for anything else, including a record with a `type` field, so Codex and
/// Claude records and unrecognized lines keep the generic projection.
///
/// A step becomes one record:
///
/// - `User`: a `message` with role `user`.
/// - `System` with Coder's loop event in `extensions.microcoder.event`: see
///   [`microcoder`].
/// - `System` with other host evidence in `extensions`: an `adapter` record
///   with no role and no text, so a reader can skip it.
/// - `System` with no extensions: a `message` with role `system`.
/// - `Agent` with message text: a `message` with role `assistant`. Each call
///   the step made follows the text after a blank line as `Tool: NAME ARGS`.
/// - `Agent` with no message text and a call: a `tool_call` named by the
///   call's function, whose text is the argument summary, then a blank line
///   and the call's result when the step recorded one.
/// - A step with only an observation: a `tool_result` with role `tool`.
/// - `Agent` with only reasoning: `reasoning`.
fn atif(value: &Value, text_limit: usize) -> Option<Readable> {
    if value.get("type").is_some() {
        return None;
    }
    let empty = |kind: &str| Readable {
        kind: kind.into(),
        native_id: None,
        role: None,
        timestamp: atif_time(value),
        tool_name: None,
        call_id: None,
        text: String::new(),
        text_truncated: false,
        unknown: false,
    };
    match value.get("record")?.as_str()? {
        "session" => {
            let session = value.get("session").filter(|s| s.is_object())?;
            Some(Readable {
                native_id: field(session, "id"),
                ..empty("session")
            })
        }
        "end" => Some(Readable {
            text: field(value, "state").unwrap_or_default(),
            ..empty("end")
        }),
        "step" => atif_step(value.get("step").filter(|s| s.is_object())?, text_limit),
        _ => None,
    }
}

/// A record's time: the log's `at` milliseconds, or the document's
/// `timestamp`.
fn atif_time(value: &Value) -> Option<String> {
    value
        .get("at")
        .and_then(Value::as_u64)
        .map(iso_ms)
        .or_else(|| field(value, "timestamp"))
}

/// One call a step made: its ID, function name, arguments, and result.
struct AtifCall {
    id: Option<String>,
    name: Option<String>,
    arguments: Option<Value>,
    output: Option<String>,
}

fn atif_step(step: &Value, text_limit: usize) -> Option<Readable> {
    let source = step.get("source")?.as_str()?;
    let message = step.get("message").map_or(Some(""), Value::as_str)?;
    let mut calls = atif_calls(step);
    // Observation results whose call this step does not name stand alone.
    let mut results = Vec::new();
    for result in step
        .get("observation")
        .and_then(|o| o.get("results"))
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let mut pieces = Vec::new();
        if let Some(content) = result.get("content") {
            collect(content, &mut pieces);
        }
        let id = field(result, "source_call_id");
        match calls
            .iter_mut()
            .find(|c| c.output.is_none() && id.is_some() && c.id == id)
        {
            Some(call) => call.output = Some(pieces.join("\n")),
            None => results.push((id, pieces.join("\n"))),
        }
    }
    let first = calls.first();
    let mut readable = Readable {
        kind: "message".into(),
        native_id: None,
        role: None,
        timestamp: atif_time(step),
        tool_name: first.and_then(|c| c.name.clone()),
        call_id: first.and_then(|c| c.id.clone()),
        text: String::new(),
        text_truncated: false,
        unknown: false,
    };
    let summaries = || {
        calls
            .iter()
            .map(|c| {
                let name = c.name.as_deref().unwrap_or("call");
                format!("Tool: {name} {}", summary(c.arguments.as_ref()))
                    .trim_end()
                    .to_owned()
            })
            .collect::<Vec<_>>()
    };
    let text = match source {
        "User" | "user" => {
            readable.role = Some("user".into());
            message.to_owned()
        }
        "System" | "system"
            if step
                .get("extensions")
                .and_then(Value::as_object)
                .is_some_and(|e| !e.is_empty()) =>
        {
            let (kind, role, tool, text) = step
                .get("extensions")
                .and_then(|e| e.get("microcoder"))
                .and_then(|m| m.get("event"))
                .and_then(microcoder)
                .unwrap_or(("adapter", None, None, String::new()));
            readable.kind = kind.into();
            readable.role = role.map(str::to_owned);
            if tool.is_some() {
                readable.call_id = None;
            }
            readable.tool_name = tool.map(str::to_owned).or(readable.tool_name);
            text
        }
        "System" | "system" => {
            readable.role = Some("system".into());
            [vec![message.to_owned()], summaries()]
                .concat()
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n")
        }
        "Agent" | "agent" if !message.trim().is_empty() => {
            readable.role = Some("assistant".into());
            [vec![message.to_owned()], summaries()]
                .concat()
                .join("\n\n")
        }
        "Agent" | "agent" if !calls.is_empty() => {
            readable.kind = "tool_call".into();
            let call = &calls[0];
            let mut parts = vec![summary(call.arguments.as_ref())];
            parts.extend(summaries().into_iter().skip(1));
            parts.extend(calls.iter().filter_map(|c| c.output.clone()));
            parts.extend(results.iter().map(|(_, text)| text.clone()));
            parts
                .into_iter()
                .filter(|p| !p.is_empty())
                .collect::<Vec<_>>()
                .join("\n\n")
        }
        "Agent" | "agent" if !results.is_empty() => {
            readable.kind = "tool_result".into();
            readable.role = Some("tool".into());
            readable.call_id = results[0].0.clone();
            results
                .iter()
                .map(|(_, text)| text.as_str())
                .collect::<Vec<_>>()
                .join("\n\n")
        }
        "Agent" | "agent" => match step.get("reasoning").and_then(Value::as_str) {
            Some(reasoning) => {
                readable.kind = "reasoning".into();
                reasoning.to_owned()
            }
            None => {
                readable.role = Some("assistant".into());
                String::new()
            }
        },
        _ => return None,
    };
    (readable.text, readable.text_truncated) = trim(&text, text_limit);
    Some(readable)
}

/// Project one event of Coder's loop (`crates/microcoder`, `run::Event`) as
/// `(kind, role, tool_name, text)`:
///
/// - `generated` with an `Ok` action: an `assistant` message with the
///   action's `reply`, the text the engine addresses to the user. Without a
///   reply, a finishing step (recorded before replies existed) shows its
///   rationale as the message, and a working step's rationale is
///   `reasoning`, the loop's own note, which conversation readers hide. No
///   status marker is appended.
/// - `generated` with an `Err`: a `system` message, `The model call failed: `
///   and the error's first line, at most 300 bytes.
/// - `ran`: a `shell` tool call whose text is the command, then `exit N`,
///   `timed out`, or `ended by a signal` on its own line when it failed, then
///   a blank line and the command's output.
/// - `tested`: a `tests` tool call with one `COMMAND: exit N` line per
///   acceptance test, naming each command by its first line.
/// - `ended`: a `system` record, `Coder finished in N steps.` as a
///   `task_complete` status record, or a `Coder stopped: ` message with the
///   reason.
///
/// Returns None for every other event, which the caller projects as an
/// `adapter` record.
fn microcoder(
    event: &Value,
) -> Option<(
    &'static str,
    Option<&'static str>,
    Option<&'static str>,
    String,
)> {
    let result_line = |result: &Value| {
        if result.get("timed_out").and_then(Value::as_bool) == Some(true) {
            return Some("timed out".to_owned());
        }
        match result.get("exit").and_then(Value::as_i64) {
            Some(0) => None,
            Some(code) => Some(format!("exit {code}")),
            None => Some("ended by a signal".to_owned()),
        }
    };
    match event.get("event")?.as_str()? {
        "generated" => {
            let action = event.get("generated")?.get("action")?;
            if let Some(next) = action.get("Ok") {
                let text = |name: &str| {
                    next.get(name)
                        .and_then(Value::as_str)
                        .map(str::trim)
                        .filter(|text| !text.is_empty())
                        .map(str::to_owned)
                };
                let finished = next.get("finished").and_then(Value::as_bool) == Some(true);
                match (text("reply"), text("rationale")) {
                    // The engine's reply to the user is the message.
                    (Some(reply), _) => Some(("message", Some("assistant"), None, reply)),
                    // A finishing step recorded before replies existed has
                    // only its rationale to show.
                    (None, Some(rationale)) if finished => {
                        Some(("message", Some("assistant"), None, rationale))
                    }
                    // A working step's rationale is the loop's own note.
                    (None, rationale) => {
                        Some(("reasoning", None, None, rationale.unwrap_or_default()))
                    }
                }
            } else {
                let error = action.get("Err")?.as_str()?;
                Some((
                    "message",
                    Some("system"),
                    None,
                    format!("The model call failed: {}", first_line(error, 300)),
                ))
            }
        }
        "ran" => {
            let result = event.get("result")?;
            let mut text = result.get("command")?.as_str()?.to_owned();
            if let Some(line) = result_line(result) {
                text.push('\n');
                text.push_str(&line);
            }
            if let Some(output) = result
                .get("output")
                .and_then(Value::as_str)
                .filter(|o| !o.is_empty())
            {
                text.push_str("\n\n");
                text.push_str(output);
            }
            Some(("tool_call", None, Some("shell"), text))
        }
        "tested" => {
            let lines = event
                .get("results")?
                .as_array()?
                .iter()
                .map(|result| {
                    let command = result
                        .get("command")
                        .and_then(Value::as_str)
                        .map(|c| first_line(c, 200))
                        .unwrap_or_default();
                    let status = result_line(result).unwrap_or_else(|| "exit 0".into());
                    format!("{command}: {status}")
                })
                .collect::<Vec<_>>();
            Some(("tool_call", None, Some("tests"), lines.join("\n")))
        }
        "ended" => {
            let outcome = event.get("outcome")?;
            let ending = outcome.get("ending")?;
            let reason = ending
                .get("reason")
                .and_then(Value::as_str)
                .or_else(|| ending.as_str())?;
            let text = match reason {
                // A status marker, not a message: readers that show only
                // the conversation leave `task_complete` records out.
                "finished" => {
                    let text = match outcome.get("steps").and_then(Value::as_u64) {
                        Some(1) => "Coder finished in 1 step.".to_owned(),
                        Some(steps) => format!("Coder finished in {steps} steps."),
                        None => "Coder finished.".to_owned(),
                    };
                    return Some(("task_complete", Some("system"), None, text));
                }
                // The question is the step's reply, already shown; the end
                // is a status marker too.
                "asked" => {
                    let approval = ending
                        .get("detail")
                        .and_then(|detail| detail.get("ask"))
                        .and_then(Value::as_str)
                        == Some("approval");
                    let text = if approval {
                        "Coder is waiting for your approval."
                    } else {
                        "Coder is waiting for your answer."
                    };
                    return Some(("task_complete", Some("system"), None, text.to_owned()));
                }
                "bad_replies" => format!(
                    "Coder stopped: {}",
                    first_line(
                        ending
                            .get("detail")
                            .and_then(Value::as_str)
                            .unwrap_or("bad replies"),
                        300
                    )
                ),
                other => format!("Coder stopped: {}", trim(&other.replace('_', " "), 64).0),
            };
            Some(("message", Some("system"), None, text))
        }
        _ => None,
    }
}

/// The first line of `text`, at most `max` bytes, marked when shortened.
fn first_line(text: &str, max: usize) -> String {
    let line = text.trim().lines().next().unwrap_or_default();
    let (mut line, cut) = trim(line, max);
    if cut {
        line.push('…');
    }
    line
}

/// The calls a step made, in either spelling: the log's single `call`, or
/// the exported document's `tool_calls`.
fn atif_calls(step: &Value) -> Vec<AtifCall> {
    let mut calls = Vec::new();
    if let Some(call) = step.get("call").filter(|c| c.is_object()) {
        calls.push(AtifCall {
            id: field(call, "id"),
            name: field(call, "name"),
            arguments: call.get("arguments").cloned(),
            output: call
                .get("output")
                .and_then(Value::as_str)
                .map(str::to_owned),
        });
    }
    for call in step
        .get("tool_calls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        calls.push(AtifCall {
            id: field(call, "tool_call_id"),
            name: field(call, "function_name"),
            arguments: call.get("arguments").cloned(),
            output: None,
        });
    }
    calls
}

/// Project one record of the host's OpenCode mirror (`opencode::mirror`):
/// the `opencode.session` header, an `opencode.part`, or an
/// `opencode.error`. Returns None for any other record.
///
/// | Record | `kind` | `role` | `text` |
/// | --- | --- | --- | --- |
/// | `opencode.session` | `session_meta` | none | empty; `native_id` is the session ID |
/// | `text` part | `message` | the message's role | the text |
/// | `text` part OpenCode added itself (`synthetic` or `ignored`) | `adapter` | none | empty |
/// | `reasoning` part | `reasoning` | none | the reasoning |
/// | `tool` part | `tool_call` (the tool's name) | none | the tool's title or input, then a blank line and its output or error |
/// | any other part | `adapter` | none | empty |
/// | `opencode.error` | `message` | `system` | `OpenCode stopped: ` and the error's message |
fn opencode(value: &Value, text_limit: usize) -> Option<Readable> {
    let kind = value.get("type").and_then(Value::as_str)?;
    let at = value.get("time").and_then(Value::as_u64).map(iso_ms);
    let readable = |kind: &str, role: Option<&str>, text: String| {
        let (text, text_truncated) = trim(&text, text_limit);
        Readable {
            kind: kind.into(),
            native_id: None,
            role: role.map(str::to_owned),
            timestamp: at.clone(),
            tool_name: None,
            call_id: None,
            text,
            text_truncated,
            unknown: false,
        }
    };
    match kind {
        "opencode.session" => Some(Readable {
            native_id: field(value, "session_id"),
            ..readable("session_meta", None, String::new())
        }),
        "opencode.error" => {
            let error = value.get("error").unwrap_or(&Value::Null);
            let said = error
                .pointer("/data/message")
                .and_then(Value::as_str)
                .or_else(|| error.get("name").and_then(Value::as_str))
                .unwrap_or("an error");
            Some(Readable {
                native_id: field(value, "message_id"),
                ..readable(
                    "message",
                    Some("system"),
                    format!("OpenCode stopped: {said}"),
                )
            })
        }
        "opencode.part" => {
            let part = value.get("part")?;
            let role = value.get("role").and_then(Value::as_str);
            let native_id = field(value, "part_id");
            let projected = match part.get("type").and_then(Value::as_str) {
                Some("text")
                    if part.get("synthetic").and_then(Value::as_bool) != Some(true)
                        && part.get("ignored").and_then(Value::as_bool) != Some(true) =>
                {
                    let role = role.filter(|r| matches!(*r, "user" | "assistant"));
                    readable(
                        "message",
                        role,
                        part.get("text")
                            .and_then(Value::as_str)
                            .unwrap_or_default()
                            .to_owned(),
                    )
                }
                Some("reasoning") => readable(
                    "reasoning",
                    None,
                    part.get("text")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                ),
                Some("tool") => {
                    let state = part.get("state").unwrap_or(&Value::Null);
                    let heading = state
                        .get("title")
                        .and_then(Value::as_str)
                        .filter(|t| !t.is_empty())
                        .map_or_else(|| summary(state.get("input")), str::to_owned);
                    let result = state
                        .get("output")
                        .or_else(|| state.get("error"))
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    let text = if result.is_empty() {
                        heading
                    } else {
                        format!("{heading}\n\n{result}")
                    };
                    Readable {
                        tool_name: field(part, "tool"),
                        call_id: field(part, "callID"),
                        ..readable("tool_call", None, text)
                    }
                }
                _ => readable("adapter", None, String::new()),
            };
            Some(Readable {
                native_id,
                ..projected
            })
        }
        _ => None,
    }
}

/// Project one record of the host's Devin mirror (`devin::mirror`): the
/// `devin.session` header or a `devin.item`. Returns None for any other
/// record.
///
/// | Record | `kind` | `role` | `text` |
/// | --- | --- | --- | --- |
/// | `devin.session` | `session_meta` | none | empty; `native_id` is the session ID |
/// | `message` item | `message` | `user` or `assistant` | the text |
/// | `reasoning` item | `reasoning` | none | the reasoning |
/// | `tool_call` item | `tool_call` (the tool's name) | none | a summary of the call's arguments |
/// | `tool_result` item | `tool_result` | `tool` | the result |
/// | an item cut to its kind, or another kind | `adapter` | none | empty |
///
/// An item's `native_id` is the Devin node it came from.
fn devin(value: &Value, text_limit: usize) -> Option<Readable> {
    let kind = value.get("type").and_then(Value::as_str)?;
    let at = value.get("time").and_then(Value::as_u64).map(iso_ms);
    let readable = |kind: &str, role: Option<&str>, text: &str| {
        let (text, text_truncated) = trim(text, text_limit);
        Readable {
            kind: kind.into(),
            native_id: None,
            role: role.map(str::to_owned),
            timestamp: at.clone(),
            tool_name: None,
            call_id: None,
            text,
            text_truncated,
            unknown: false,
        }
    };
    match kind {
        "devin.session" => Some(Readable {
            native_id: field(value, "session_id"),
            ..readable("session_meta", None, "")
        }),
        "devin.item" => {
            let text = value.get("text").and_then(Value::as_str);
            let projected = match (value.get("item").and_then(Value::as_str), text) {
                (Some("message"), Some(text)) => {
                    let role = value
                        .get("role")
                        .and_then(Value::as_str)
                        .filter(|r| matches!(*r, "user" | "assistant"));
                    readable("message", role, text)
                }
                (Some("reasoning"), Some(text)) => readable("reasoning", None, text),
                (Some("tool_call"), _) if value.get("tool").is_some() => Readable {
                    tool_name: field(value, "tool"),
                    call_id: field(value, "call_id"),
                    ..readable("tool_call", None, &summary(value.get("arguments")))
                },
                (Some("tool_result"), Some(text)) => Readable {
                    call_id: field(value, "call_id"),
                    ..readable("tool_result", Some("tool"), text)
                },
                _ => readable("adapter", None, ""),
            };
            Some(Readable {
                native_id: value
                    .get("node_id")
                    .and_then(Value::as_i64)
                    .map(|n| n.to_string()),
                ..projected
            })
        }
        _ => None,
    }
}

/// A short, single-line description of a call's arguments: a shell
/// command's text, a string argument, or compact JSON, at most 240 bytes.
fn summary(arguments: Option<&Value>) -> String {
    let text = match arguments {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(value) => match value.get("command").or_else(|| value.get("cmd")) {
            Some(Value::String(command)) => command.clone(),
            _ => value.to_string(),
        },
    };
    let line = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let (mut line, cut) = trim(&line, 240);
    if cut {
        line.push('…');
    }
    line
}

/// `YYYY-MM-DDTHH:MM:SS.mmmZ` for Unix milliseconds, in UTC.
fn iso_ms(milliseconds: u64) -> String {
    let seconds = utc(milliseconds / 1_000);
    format!(
        "{}.{:03}Z",
        seconds.trim_end_matches('Z'),
        milliseconds % 1_000
    )
}

/// `YYYY-MM-DDTHH:MM:SSZ` for Unix seconds, in UTC.
pub(crate) fn utc(seconds: u64) -> String {
    let days = (seconds / 86_400) as i64;
    let rest = seconds % 86_400;
    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

fn collect(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => out.push(text.clone()),
        Value::Array(values) => {
            for value in values {
                collect(value, out);
            }
        }
        Value::Object(value) => {
            if let Some(text) = value.get("text").and_then(Value::as_str) {
                out.push(text.to_owned());
            }
            if let Some(name) = value.get("name").and_then(Value::as_str) {
                out.push(format!("Tool: {name}"));
            }
            if let Some(input) = value.get("input") {
                out.push(input.to_string());
            }
            if let Some(content) = value.get("content") {
                collect(content, out);
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_full_projection_preserves_unicode_and_tool_text_beyond_preview() {
        let text = "日本語 👩🏽‍💻 ".repeat(300);
        let raw = serde_json::to_vec(&serde_json::json!({
            "type": "assistant",
            "message": {"role": "assistant", "content": [
                {"type": "text", "text": text},
                {"type": "tool_use", "id": "call-one", "name": "Read", "input": {"file": "sample.rs"}}
            ]}
        })).unwrap();
        let preview = readable_record(&raw).unwrap();
        assert!(preview.text_truncated);
        assert!(preview.text.len() <= 1024);
        let full = readable_record_full(&raw).unwrap();
        assert!(!full.text_truncated);
        assert!(full.text.starts_with(&text));
        assert!(full.text.contains("Tool: Read"));
        assert!(full.text.contains("sample.rs"));
        assert_eq!(full.call_id.as_deref(), Some("call-one"));
        assert!(readable_record_full(&vec![b' '; MAX_READABLE_RECORD_BYTES + 1]).is_none());
    }
}
