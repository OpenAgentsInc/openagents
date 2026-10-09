//! Local ATIF import and export through the shared trajectory format.
//!
//! The export command follows the reference product's local-file design,
//! reimplemented with this repository's public `atif` crate.

use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use atif::{Call, Outcome, Session, Source, Step};
use serde_json::{Map, Value, json};

use crate::{
    App,
    live::{Chat, Entry},
};

/// Render a transcript without running a model or a tool.
pub fn document(chat: &Chat, id: &str, model: &str, cwd: &Path) -> Value {
    let mut session = Session::opening(
        id,
        model,
        "coder-new",
        &cwd.to_string_lossy(),
        env!("CARGO_PKG_VERSION"),
    );
    session.state = if chat.busy { "running" } else { "ended" }.into();
    let mut steps = Vec::new();
    for (index, entry) in chat.entries.iter().enumerate() {
        let step = match entry {
            Entry::User(text) => Step::said(Source::User, text),
            Entry::Assistant {
                text,
                model,
                elapsed_ms,
            } => {
                let mut step = Step::said(Source::Agent, text);
                if let Some(elapsed_ms) = elapsed_ms {
                    step = step.taking(*elapsed_ms);
                }
                if let Some(model) = model {
                    step.by(model)
                } else {
                    step
                }
            }
            Entry::Tool {
                name,
                input,
                output,
                running,
            } => {
                let mut extra = Map::new();
                extra.insert("running".into(), json!(running));
                if *running && !output.is_null() {
                    extra.insert("pending_output".into(), output.clone());
                }
                Step::called(Call {
                    id: format!("call-{}", index + 1),
                    name: name.clone(),
                    arguments: input.clone(),
                    output: serde_json::to_string(output).unwrap_or_default(),
                    outcome: if *running {
                        Outcome::Cancelled
                    } else if output.get("error").is_some() {
                        Outcome::Failed
                    } else {
                        Outcome::Completed
                    },
                    milliseconds: 0,
                    purpose: None,
                    extra,
                })
            }
            Entry::Delegation {
                id,
                name,
                task,
                output,
                running,
                ..
            } => {
                let mut extra = Map::new();
                extra.insert("schema".into(), json!("openagents.delegation.v1"));
                extra.insert("agent".into(), json!(name));
                extra.insert("running".into(), json!(running));
                Step::called(Call {
                    id: id.clone(),
                    name: "delegate".into(),
                    arguments: json!({"agent":name,"task":task}),
                    output: output.to_string(),
                    outcome: if *running {
                        Outcome::Cancelled
                    } else if output.get("error").is_some() {
                        Outcome::Failed
                    } else {
                        Outcome::Completed
                    },
                    milliseconds: 0,
                    purpose: Some(task.clone()),
                    extra,
                })
            }
        };
        if session.directive.is_empty() && step.source == Source::User {
            session.directive = step.message.clone();
        }
        steps.push(step);
    }
    if !chat.partial.is_empty() {
        let mut step = Step::said(Source::Agent, &chat.partial).noting("partial", json!(true));
        step.model = chat.partial_model.clone();
        if let Some(elapsed_ms) = chat.reply_elapsed_ms() {
            step = step.taking(elapsed_ms);
        }
        steps.push(step);
    }
    let mut result = atif::document(&session, &steps);
    for step in result["steps"].as_array_mut().into_iter().flatten() {
        if step
            .pointer("/tool_calls/0/extra/running")
            .and_then(Value::as_bool)
            == Some(true)
        {
            // A pending call has no observation or completed outcome yet.
            step.as_object_mut().unwrap().remove("observation");
        }
    }
    result["extra"]["timestamps"] = json!("export-time");
    result["final_metrics"]["extra"]["reported_total_tokens"] = json!(chat.tokens);
    if let Some(notice) = &chat.notice {
        result["extra"]["notice"] = json!(notice);
    }
    result
}

/// Export the selected chat, or the main chat with its delegated trajectories.
pub fn app_document(app: &App, cwd: &Path) -> Value {
    if app.mode == crate::Mode::Demo {
        return demo_document(app, cwd);
    }
    if let Some(index) = app.selected_agent
        && let Some(child) = app.delegations.get(index)
    {
        let mut result = document(&child.chat, &child.id, chat_model(&child.chat), cwd);
        app.plugins
            .execution_settings(cwd.to_owned())
            .redact(&mut result);
        return result;
    }
    main_document(app, cwd)
}

fn demo_document(app: &App, cwd: &Path) -> Value {
    use crate::agents::{DEMOS, MAIN_PLUGINS, MAIN_TOOLS};
    if let Some(index) = app.selected_agent
        && let Some(agent) = DEMOS.get(index)
    {
        let mut chat = demo_chat(agent);
        append_demo_messages(&mut chat, &app.messages);
        chat.notice.clone_from(&app.notice);
        let mut exported = document(&chat, &format!("demo-{}", agent.name), "demo/local", cwd);
        exported["extra"]["demo"] = json!(true);
        exported["extra"]["agent"] = json!(agent.name);
        exported["extra"]["task"] = json!(agent.task);
        return exported;
    }
    let mut chat = Chat::default();
    chat.entries
        .push(Entry::User("Review the terminal with four agents.".into()));
    chat.entries.extend(MAIN_TOOLS.iter().map(demo_tool));
    chat.entries.extend(MAIN_PLUGINS.iter().map(demo_plugin));
    for agent in &DEMOS {
        chat.entries.push(Entry::Delegation {
            id: format!("demo-{}", agent.name),
            name: agent.name.into(),
            task: agent.task.into(),
            running: true,
            output: Value::Null,
            progress: None,
        });
    }
    append_demo_messages(&mut chat, &app.messages);
    chat.notice.clone_from(&app.notice);
    let mut exported = document(&chat, "demo-main", "demo/local", cwd);
    exported["extra"]["demo"] = json!(true);
    exported["subagent_trajectories"] = json!(
        DEMOS
            .iter()
            .enumerate()
            .map(|(index, agent)| {
                let mut child = demo_chat(agent);
                append_demo_messages(&mut child, &app.saved_chats[index + 1].messages);
                let mut child =
                    document(&child, &format!("demo-{}", agent.name), "demo/local", cwd);
                child["extra"]["demo"] = json!(true);
                child["extra"]["agent"] = json!(agent.name);
                child["extra"]["task"] = json!(agent.task);
                child["final_metrics"]["extra"]["wall_seconds"] =
                    json!(agent.elapsed_seconds + app.elapsed_seconds);
                child
            })
            .collect::<Vec<_>>()
    );
    exported
}

fn append_demo_messages(chat: &mut Chat, messages: &[String]) {
    for message in messages {
        chat.entries.push(Entry::User(message.clone()));
        chat.entries.push(Entry::Assistant {
            elapsed_ms: None,
            text: "Preview message added. No agent is connected.".into(),
            model: Some("demo/local".into()),
        });
    }
}

fn demo_chat(agent: &crate::agents::DemoAgent) -> Chat {
    use crate::agents::DemoMessage;
    let mut chat = Chat::default();
    for message in agent.conversation {
        chat.entries.push(match message {
            DemoMessage::User(text) => Entry::User((*text).into()),
            DemoMessage::Assistant(text) => Entry::Assistant {
                elapsed_ms: None,
                text: (*text).into(),
                model: Some("demo/local".into()),
            },
            DemoMessage::Tool(call) => demo_tool(call),
            DemoMessage::Plugin(call) => demo_plugin(call),
        });
    }
    let multiplier = if agent.tokens.ends_with('k') {
        1_000.0
    } else {
        1.0
    };
    chat.tokens = (agent
        .tokens
        .trim_end_matches('k')
        .parse::<f64>()
        .unwrap_or(0.0)
        * multiplier) as u64;
    chat
}

fn demo_tool(call: &crate::tools::ToolCall) -> Entry {
    use crate::tools::ToolKind;
    demo_call(
        match call.kind {
            ToolKind::Read => "Read",
            ToolKind::Search => "Search",
            ToolKind::Edit => "Edit",
            ToolKind::Run => "Run",
        },
        call.input,
        call.output,
        call.state,
    )
}

fn demo_plugin(call: &crate::tools::PluginCall) -> Entry {
    demo_call(
        &format!("{}.{}", call.plugin, call.operation),
        call.input,
        call.output,
        call.state,
    )
}

fn demo_call(name: &str, input: &str, output: &str, state: crate::tools::ToolState) -> Entry {
    Entry::Tool {
        name: name.into(),
        input: json!(input),
        output: if state == crate::tools::ToolState::Failed {
            json!({"error":output})
        } else {
            json!(output)
        },
        running: state == crate::tools::ToolState::Running,
    }
}

/// Render the parent and all children regardless of the current UI selection.
pub fn main_document(app: &App, cwd: &Path) -> Value {
    let id = app
        .session_id()
        .map(str::to_owned)
        .unwrap_or_else(|| format!("coder-new-{}", atif::stamp(atif::now_ms())));
    let model = if app.plugins.enabled && app.plugins.key_configured {
        app.plugins.options.slug(&app.plugins.model)
    } else {
        match chat_model(&app.live) {
            "unknown" => "auto".into(),
            actual => actual.into(),
        }
    };
    let mut result = document(&app.live, &id, &model, cwd);
    if !app.delegations.is_empty() {
        result["subagent_trajectories"] = json!(
            app.delegations
                .iter()
                .map(|child| {
                    let mut result = document(&child.chat, &child.id, chat_model(&child.chat), cwd);
                    result["extra"]["agent"] = json!(child.name);
                    result["extra"]["task"] = json!(child.task);
                    result["final_metrics"]["extra"]["wall_seconds"] = json!(child.elapsed_seconds);
                    result
                })
                .collect::<Vec<_>>()
        );
    }
    app.plugins
        .execution_settings(cwd.to_owned())
        .redact(&mut result);
    result
}

fn chat_model(chat: &Chat) -> &str {
    chat.partial_model
        .as_deref()
        .or_else(|| {
            chat.entries.iter().rev().find_map(|entry| match entry {
                Entry::Assistant {
                    model: Some(model), ..
                } => Some(model.as_str()),
                _ => None,
            })
        })
        .unwrap_or("unknown")
}

/// Write a new local export. Existing files are preserved.
pub fn export_app(
    app: &App,
    path: Option<&Path>,
    cwd: &Path,
    openagents_root: Option<&Path>,
) -> Result<PathBuf, String> {
    let path = match path {
        Some(path) if path.is_absolute() => path.to_owned(),
        Some(path) => cwd.join(path),
        None => openagents_root
            .ok_or("Set a home directory or provide an export path.")?
            .join("exports")
            .join(format!(
                "coder-new-{}.atif.json",
                atif::log::session_id(atif::now_ms()),
            )),
    };
    write(&path, &app_document(app, cwd))?;
    Ok(path)
}

/// Write one ATIF document as an owner-readable file.
pub fn write(path: &Path, document: &Value) -> Result<(), String> {
    if !atif::validate(document).is_empty() {
        return Err("The trajectory is not valid ATIF.".into());
    }
    let parent = path
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut directory = fs::DirBuilder::new();
    directory.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        directory.mode(0o700);
    }
    directory
        .create(parent)
        .map_err(|_| "Cannot create the export directory.")?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|_| "Cannot create the export file. Existing files are preserved.")?;
    let mut bytes =
        serde_json::to_vec_pretty(document).map_err(|_| "Cannot encode the trajectory.")?;
    bytes.push(b'\n');
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| "Cannot write the export file.".to_owned())
}

/// Import text and observations for viewing or continuing. Import never reruns calls.
pub fn read(path: &Path) -> Result<Chat, String> {
    let file = fs::File::open(path).map_err(|_| "Cannot read the trajectory.")?;
    let mut bytes = Vec::new();
    file.take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read the trajectory.")?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("The trajectory exceeds 64 MiB.".into());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "The trajectory is not a JSON document.")?;
    from_document(&value)
}

pub fn from_document(value: &Value) -> Result<Chat, String> {
    if !atif::validate(value).is_empty() {
        return Err("The trajectory is not valid ATIF.".into());
    }
    let steps = value
        .get("steps")
        .and_then(Value::as_array)
        .ok_or("The trajectory has no steps array.")?;
    let mut chat = Chat::default();
    for step in steps {
        let text = text(step.get("message").unwrap_or(&Value::Null));
        match step.get("source").and_then(Value::as_str) {
            Some("user") => chat.entries.push(Entry::User(text)),
            Some("agent") => {
                if !text.is_empty() {
                    chat.entries.push(Entry::Assistant {
                        elapsed_ms: step
                            .get("extra")
                            .and_then(|extra| extra.get("duration_ms"))
                            .and_then(Value::as_u64),
                        text,
                        model: step
                            .get("model_name")
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
                    let id = call
                        .get("tool_call_id")
                        .and_then(Value::as_str)
                        .unwrap_or("");
                    let observation = step
                        .pointer("/observation/results")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .find(|result| {
                            result.get("source_call_id").and_then(Value::as_str) == Some(id)
                        });
                    let output = observation
                        .and_then(|result| result.get("content"))
                        .map(|output| {
                            output
                                .as_str()
                                .and_then(|text| serde_json::from_str(text).ok())
                                .unwrap_or_else(|| output.clone())
                        })
                        .or_else(|| call.pointer("/extra/pending_output").cloned())
                        .unwrap_or(Value::Null);
                    if call.pointer("/extra/schema").and_then(Value::as_str)
                        == Some("openagents.delegation.v1")
                    {
                        chat.entries.push(Entry::Delegation {
                            id: id.into(),
                            name: call
                                .pointer("/arguments/agent")
                                .and_then(Value::as_str)
                                .unwrap_or("subagent")
                                .into(),
                            task: call
                                .pointer("/arguments/task")
                                .and_then(Value::as_str)
                                .unwrap_or("")
                                .into(),
                            output,
                            running: false,
                            progress: None,
                        });
                        continue;
                    }
                    chat.entries.push(Entry::Tool {
                        name: call
                            .get("function_name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown")
                            .into(),
                        input: call.get("arguments").cloned().unwrap_or(Value::Null),
                        output,
                        running: false,
                    });
                }
            }
            Some("system") => {}
            _ => return Err("A trajectory step has an unsupported source.".into()),
        }
    }
    chat.tokens = value
        .pointer("/final_metrics/extra/reported_total_tokens")
        .and_then(Value::as_u64)
        .unwrap_or_else(|| {
            value
                .pointer("/final_metrics/total_prompt_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .saturating_add(
                    value
                        .pointer("/final_metrics/total_completion_tokens")
                        .and_then(Value::as_u64)
                        .unwrap_or(0),
                )
        });
    Ok(chat)
}

/// Restore retained chats without resuming recorded calls.
pub fn restore_app(app: &mut App, value: &Value) -> Result<(), String> {
    let main = from_document(value)?;
    let mut children = Vec::new();
    for child in value
        .get("subagent_trajectories")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let chat = from_document(child)?;
        let id = child
            .get("session_id")
            .and_then(Value::as_str)
            .ok_or("A subagent trajectory has no session ID.")?;
        if children
            .iter()
            .any(|previous: &crate::live::Delegation| previous.id == id)
        {
            return Err("Subagent trajectories must have distinct session IDs.".into());
        }
        children.push(crate::live::Delegation {
            id: id.into(),
            name: child
                .pointer("/extra/agent")
                .and_then(Value::as_str)
                .unwrap_or("subagent")
                .into(),
            task: child
                .pointer("/extra/task")
                .and_then(Value::as_str)
                .unwrap_or("")
                .into(),
            chat,
            elapsed_seconds: child
                .pointer("/final_metrics/extra/wall_seconds")
                .and_then(Value::as_u64)
                .unwrap_or(0),
            started_at: 0,
            running: false,
            draft: crate::Draft::default(),
            scroll: 0,
        });
    }
    app.live = main;
    app.delegations = children;
    app.selected_agent = None;
    Ok(())
}

fn text(value: &Value) -> String {
    if let Some(text) = value.as_str() {
        return text.into();
    }
    value
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn export_import_preserves_calls_models_and_full_results_without_execution() {
        let mut chat = Chat::default();
        chat.entries.push(Entry::User("Explain this".into()));
        chat.entries.push(Entry::Tool {
            name: "jev".into(),
            input: json!({"questions":{"valid":{"type":"boolean"}}}),
            output: json!({"answers":{"valid":{"noul":0.9}},"long":"x".repeat(20000)}),
            running: false,
        });
        chat.entries.push(Entry::Assistant {
            elapsed_ms: Some(5500),
            text: "**Valid**".into(),
            model: Some("openai/gpt-6-luna:low".into()),
        });
        chat.tokens = 42;
        let document = document(&chat, "test", "openrouter/free", Path::new("/workspace"));
        assert_eq!(document["schema_version"], "ATIF-v1.8");
        assert!(atif::validate(&document).is_empty());
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("export.json");
        write(&path, &document).unwrap();
        assert!(write(&path, &document).is_err());
        let imported = read(&path).unwrap();
        assert_eq!(imported.tokens, 42);
        assert_eq!(document["steps"][2]["extra"]["duration_ms"], 5500);
        assert!(matches!(
            &imported.entries[2],
            Entry::Assistant {
                elapsed_ms: Some(5500),
                ..
            }
        ));
        assert!(
            matches!(&imported.entries[1],Entry::Tool{output,..} if output["long"].as_str().unwrap().len()==20000)
        );
        assert!(
            matches!(&imported.entries[2],Entry::Assistant{model,..} if model.as_deref()==Some("openai/gpt-6-luna:low"))
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }
    #[test]
    fn foreign_or_malformed_trajectory_is_rejected() {
        assert!(from_document(&json!({"schema_version":"ATIF-v9","steps":[]})).is_err());
        assert!(from_document(&json!({"schema_version":"ATIF-v1.8"})).is_err());
        assert!(
            from_document(&json!({"schema_version":"ATIF-v1.8","steps":[{"source":"bad"}]}))
                .is_err()
        );
    }

    #[test]
    fn pending_calls_have_no_completed_observation() {
        let chat = Chat {
            entries: vec![Entry::Tool {
                name: "Run".into(),
                input: json!({"command":"cargo test"}),
                output: Value::Null,
                running: true,
            }],
            busy: true,
            ..Chat::default()
        };
        let exported = document(&chat, "pending", "auto", Path::new("/workspace"));
        assert_eq!(
            exported["steps"][0]["tool_calls"][0]["extra"]["running"],
            true
        );
        assert!(exported["steps"][0].get("observation").is_none());
        let restored = from_document(&exported).unwrap();
        assert!(matches!(
            &restored.entries[0],
            Entry::Tool { running: false, .. }
        ));
    }

    #[test]
    fn delegated_chats_remain_delegations_when_restored_and_can_be_exported_alone() {
        let mut app = App {
            mode: crate::Mode::Live,
            ..App::default()
        };
        app.live.entries.push(Entry::Delegation {
            id: "child".into(),
            name: "microcoder".into(),
            task: "Review the parser".into(),
            running: false,
            output: json!({"reply":"Reviewed"}),
            progress: None,
        });
        app.delegations.push(crate::live::Delegation {
            id: "child".into(),
            name: "microcoder".into(),
            task: "Review the parser".into(),
            chat: Chat {
                entries: vec![
                    Entry::User("Review the parser".into()),
                    Entry::Assistant {
                        elapsed_ms: None,
                        text: "Reviewed".into(),
                        model: Some("model/actual:low".into()),
                    },
                ],
                tokens: 23,
                ..Chat::default()
            },
            elapsed_seconds: 12,
            started_at: 0,
            running: false,
            draft: crate::Draft::default(),
            scroll: 0,
        });
        let document = app_document(&app, Path::new("/workspace"));
        let mut restored = App {
            mode: crate::Mode::Live,
            ..App::default()
        };
        restore_app(&mut restored, &document).unwrap();
        assert!(
            matches!(&restored.live.entries[0],Entry::Delegation{name,..} if name=="microcoder")
        );
        assert_eq!(restored.delegations[0].chat.tokens, 23);
        assert_eq!(restored.delegations[0].elapsed_seconds, 12);
        assert!(!restored.delegations[0].running);
        restored.selected_agent = Some(0);
        let selected = app_document(&restored, Path::new("/workspace"));
        assert_eq!(selected["session_id"], "child");
        assert_eq!(selected["steps"][1]["model_name"], "model/actual:low");
        assert!(selected.get("subagent_trajectories").is_none());
    }

    #[test]
    fn demo_exports_the_displayed_fixtures_and_each_conversations_messages() {
        let mut app = App::default();
        app.messages.push("Main-only message".into());
        app.select_agent(Some(0));
        app.messages.push("Child-only message".into());
        let child = app_document(&app, Path::new("/workspace"));
        assert_eq!(child["session_id"], "demo-claude-code");
        assert!(
            child
                .to_string()
                .contains("Review the composer keyboard navigation.")
        );
        assert!(child.to_string().contains("Child-only message"));
        assert!(!child.to_string().contains("Main-only message"));
        app.select_agent(None);
        let main = app_document(&app, Path::new("/workspace"));
        assert!(main["steps"].to_string().contains("Main-only message"));
        assert!(!main["steps"].to_string().contains("Child-only message"));
        assert!(
            main["subagent_trajectories"][0]
                .to_string()
                .contains("Child-only message")
        );
        assert_eq!(main["subagent_trajectories"].as_array().unwrap().len(), 4);
        assert!(atif::validate(&main).is_empty());
    }
}
