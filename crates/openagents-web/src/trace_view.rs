//! A trace drawn as a conversation (#11621): an ATIF document's steps as
//! the person's [`Bubble`]s and the agent's [`Turn`]s, each turn's work
//! folded into a [`Trace`] under one summary line, its answer below, its
//! changed files as [`Attachments`], and the agents it asked nested under
//! the call that asked them (from the document's `subagent_trajectories`).
//!
//! Used by the trace pages in [`crate::traces`]: the owner's page, the
//! shared page, and an agent's page. `expand` opens every line.

use std::collections::{HashMap, HashSet};

use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::actions::{ButtonVariant, Color, ControlSize, CopyButton};
use openagents_ui::content::{
    Attachment, AttachmentKind, Attachments, Bubble, CodeBlock, CodeSize, MarkdownRoot,
    MarkdownSize, Marker, MessageScroller, Question, Questionnaire, Trace, TraceKind, TraceStatus,
    TraceStep, Turn,
};
use openagents_ui::icons::Icon;
use serde_json::Value;

/// The most steps drawn from one document.
pub(crate) const MAX_SHOWN_STEPS: usize = 400;
/// The most characters of one tool input or output drawn.
const MAX_SHOWN_RESULT: usize = 4000;
/// How deep agents asked by agents are drawn.
const MAX_DEPTH: usize = 4;

/// The agents a trace's steps started, by the tool call that started each:
/// where its page is and its title.
pub(crate) type AgentLinks = HashMap<String, (String, String)>;

fn steps(document: &Value) -> &[Value] {
    document
        .get("steps")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

/// A step's message as text: a string, or its content parts' text.
fn message_text(value: &Value) -> String {
    value.as_str().map(str::to_owned).unwrap_or_else(|| {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
    })
}

/// One line of at most `limit` characters.
fn line(value: &str, limit: usize) -> String {
    let joined = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut out: String = joined
        .chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect();
    if joined.chars().count() > limit {
        out.push('…');
    }
    out
}

fn clip(text: &str) -> String {
    secret_screen::head_and_tail(text, MAX_SHOWN_RESULT)
}

fn markdown(text: &str) -> Markup {
    MarkdownRoot::new(PreEscaped(crate::markdown::render(text))).render()
}

fn small_markdown(text: &str) -> Markup {
    MarkdownRoot::new(PreEscaped(crate::markdown::render(text)))
        .size(MarkdownSize::Sm)
        .render()
}

/// "4s", "3m", "1h 2m"; nothing under a second.
fn took(ms: u64) -> Option<String> {
    (ms >= 1000).then(|| crate::traces::duration_words(ms))
}

fn plural(n: usize, one: &str, many: &str) -> String {
    if n == 1 {
        format!("1 {one}")
    } else {
        format!("{n} {many}")
    }
}

/// An agent's name for people: "claude-code" → "Claude Code".
fn agent_name(raw: &str) -> String {
    let lower = raw.trim().to_ascii_lowercase();
    match lower.as_str() {
        "" => "an agent".into(),
        "claude-code" | "claude_code" | "claude" => "Claude Code".into(),
        "codex" => "Codex".into(),
        "opencode" => "OpenCode".into(),
        "cursor" | "cursor-agent" => "Cursor".into(),
        "microcoder" => "Microcoder".into(),
        "gemini" | "gemini-cli" => "Gemini".into(),
        name if name.contains("coder") => "Coder".into(),
        "openagents" | "openagents-web" | "openagents.com" => "OpenAgents".into(),
        _ => raw.trim().to_owned(),
    }
}

/// A tool call's arguments as an object (a JSON string is read too).
fn arguments(call: &Value) -> Value {
    match call.get("arguments") {
        Some(Value::String(text)) => {
            serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.clone()))
        }
        Some(value) => value.clone(),
        None => Value::Null,
    }
}

fn field<'a>(value: &'a Value, names: &[&str]) -> Option<&'a str> {
    names
        .iter()
        .find_map(|name| value.get(*name).and_then(Value::as_str))
        .filter(|text| !text.trim().is_empty())
}

/// Pretty JSON for an input or output, or the text as it is.
fn shown(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(text) => text.clone(),
        other => serde_json::to_string_pretty(other).unwrap_or_default(),
    }
}

fn file_language(path: &str) -> &str {
    path.rsplit_once('.').map_or("", |(_, ext)| ext)
}

/// A unified-looking diff of one replacement.
fn replacement_diff(old: &str, new: &str) -> (String, usize, usize) {
    let old_lines: Vec<&str> = if old.is_empty() {
        Vec::new()
    } else {
        old.lines().collect()
    };
    let new_lines: Vec<&str> = if new.is_empty() {
        Vec::new()
    } else {
        new.lines().collect()
    };
    // Keep the lines both share at the start and the end as context.
    let head = old_lines
        .iter()
        .zip(&new_lines)
        .take_while(|(a, b)| a == b)
        .count();
    let tail = old_lines[head..]
        .iter()
        .rev()
        .zip(new_lines[head..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let mut out = format!("@@ -{} +{} @@\n", old_lines.len(), new_lines.len());
    for l in &old_lines[..head] {
        out.push_str(&format!(" {l}\n"));
    }
    let removed = &old_lines[head..old_lines.len() - tail];
    let added = &new_lines[head..new_lines.len() - tail];
    for l in removed {
        out.push_str(&format!("-{l}\n"));
    }
    for l in added {
        out.push_str(&format!("+{l}\n"));
    }
    for l in &old_lines[old_lines.len() - tail..] {
        out.push_str(&format!(" {l}\n"));
    }
    (out, added.len(), removed.len())
}

/// The trajectories of the agents a document asked, by id.
struct Agents<'a> {
    all: Vec<&'a Value>,
    used: HashSet<usize>,
}

impl<'a> Agents<'a> {
    fn of(document: &'a Value) -> Self {
        Self {
            all: document
                .get("subagent_trajectories")
                .and_then(Value::as_array)
                .map(|list| list.iter().collect())
                .unwrap_or_default(),
            used: HashSet::new(),
        }
    }

    /// The trajectory a call started: named by its result's reference, by
    /// the result's `child` run, or by the call or result id itself.
    fn take(&mut self, call: &Value, result: Option<&Value>, output: &Value) -> Option<&'a Value> {
        let mut keys: Vec<String> = Vec::new();
        if let Some(refs) = result
            .and_then(|r| r.get("subagent_trajectory_ref"))
            .and_then(Value::as_array)
        {
            keys.extend(
                refs.iter()
                    .filter_map(|r| field(r, &["session_id", "trajectory_id"]).map(str::to_owned)),
            );
        }
        if let Some(child) = field(output, &["child"]) {
            keys.push(child.to_owned());
        }
        if let Some(id) = field(call, &["tool_call_id"]) {
            keys.push(id.to_owned());
        }
        if let Some(id) = result.and_then(|r| field(r, &["source_call_id"])) {
            keys.push(id.to_owned());
        }
        let index = self.all.iter().enumerate().position(|(i, trajectory)| {
            !self.used.contains(&i)
                && [
                    field(trajectory, &["trajectory_id"]),
                    field(trajectory, &["session_id"]),
                ]
                .into_iter()
                .flatten()
                .any(|id| {
                    keys.iter().any(|key| {
                        id == key
                            || id
                                .strip_suffix(key.as_str())
                                .is_some_and(|p| p.ends_with(':'))
                    })
                })
        })?;
        self.used.insert(index);
        Some(self.all[index])
    }

    fn rest(&self) -> Vec<&'a Value> {
        self.all
            .iter()
            .enumerate()
            .filter(|(i, _)| !self.used.contains(i))
            .map(|(_, t)| *t)
            .collect()
    }
}

/// What one agent step adds to a turn, in order.
enum Part {
    Line(TraceStep),
    Text(String),
    Card(Markup),
}

/// A turn's files: path, lines added, lines removed.
#[derive(Default)]
struct Files(Vec<(String, usize, usize)>);

impl Files {
    fn add(&mut self, path: &str, added: usize, removed: usize) {
        if let Some(entry) = self.0.iter_mut().find(|(p, _, _)| p == path) {
            entry.1 += added;
            entry.2 += removed;
        } else {
            self.0.push((path.to_owned(), added, removed));
        }
    }
}

struct Drawer<'a, 'd> {
    links: &'a AgentLinks,
    agents: Agents<'d>,
    expand: bool,
}

impl<'d> Drawer<'_, 'd> {
    /// The parts one agent step adds.
    fn step(&mut self, step: &'d Value, files: &mut Files, depth: usize) -> Vec<Part> {
        let mut parts = Vec::new();
        if let Some(reasoning) = step
            .get("reasoning_content")
            .and_then(Value::as_str)
            .filter(|r| !r.trim().is_empty())
        {
            let first = reasoning
                .lines()
                .map(|l| l.trim().trim_matches('*').trim())
                .find(|l| !l.is_empty())
                .unwrap_or_default();
            parts.push(Part::Line(
                TraceStep::new(TraceKind::Reasoning, "Thought")
                    .description(line(first, 120))
                    .body(small_markdown(reasoning))
                    .open(self.expand),
            ));
        }
        let results: Vec<&Value> = step
            .pointer("/observation/results")
            .and_then(Value::as_array)
            .map(|r| r.iter().collect())
            .unwrap_or_default();
        let calls = step.get("tool_calls").and_then(Value::as_array);
        for call in calls.into_iter().flatten() {
            let id = field(call, &["tool_call_id"]);
            let result = results
                .iter()
                .copied()
                .find(|r| id.is_some() && field(r, &["source_call_id"]) == id);
            parts.push(self.call(call, result, files, depth));
        }
        // Results with no call in this step (an agent reporting back).
        for result in &results {
            let source = field(result, &["source_call_id"]);
            let answered = calls
                .into_iter()
                .flatten()
                .any(|c| source.is_some() && field(c, &["tool_call_id"]) == source);
            if !answered && source.is_some() {
                let call = serde_json::json!({"function_name": "result", "tool_call_id": source});
                parts.push(self.call(&call, Some(result), files, depth));
            }
        }
        let text = message_text(step.get("message").unwrap_or(&Value::Null));
        if !text.trim().is_empty() {
            parts.push(Part::Text(text));
        }
        parts
    }

    fn call(
        &mut self,
        call: &Value,
        result: Option<&Value>,
        files: &mut Files,
        depth: usize,
    ) -> Part {
        let name = field(call, &["function_name"]).unwrap_or("tool");
        let args = arguments(call);
        let content = result
            .map(|r| message_text(r.get("content").unwrap_or(&Value::Null)))
            .unwrap_or_default();
        let output: Value = serde_json::from_str(&content).unwrap_or(Value::Null);
        let failed_status = result
            .and_then(|r| r.pointer("/extra/status"))
            .and_then(Value::as_str)
            .is_some_and(|s| matches!(s, "failed" | "error" | "cancelled"));
        let error = field(&output, &["error"]).map(str::to_owned);
        let exit = output
            .get("exit")
            .and_then(Value::as_i64)
            .filter(|code| *code != 0);
        let failed = failed_status
            || error.is_some()
            || exit.is_some()
            || field(&output, &["outcome"]) == Some("failed");
        let running = result.is_none()
            && call.pointer("/extra/running").and_then(Value::as_bool) == Some(true);
        let status = if failed {
            TraceStatus::Error
        } else if running {
            TraceStatus::Running
        } else {
            TraceStatus::Done
        };
        let ms = result
            .and_then(|r| r.pointer("/extra/duration_ms"))
            .and_then(Value::as_u64);
        let lower = name.to_ascii_lowercase();
        let path = field(
            &args,
            &["path", "file_path", "filePath", "file", "target_file"],
        );
        let output_text = field(&output, &["output", "stdout"])
            .map(str::to_owned)
            .unwrap_or_else(|| {
                if output.is_null() {
                    content.clone()
                } else {
                    String::new()
                }
            });
        let mut step = match lower.as_str() {
            "run" | "bash" | "shell" | "exec" | "exec_command" | "terminal"
            | "background_shell" | "run_terminal_cmd" => {
                let command = field(&args, &["command", "cmd"]).unwrap_or_default();
                let mut step = TraceStep::new(TraceKind::Command, "Ran").detail(line(command, 160));
                let shown = if output_text.trim().is_empty() {
                    content.clone()
                } else {
                    output_text.clone()
                };
                if !shown.trim().is_empty()
                    && !(output.is_object() && output_text.trim().is_empty())
                {
                    step = step.body(output_block(&clip(&shown), "Output"));
                } else if output_text.trim().is_empty() && output.is_object() {
                    step = step.description("No output");
                }
                if let Some(code) = exit {
                    step = step.description(format!("Exited with code {code}"));
                }
                step
            }
            "read" | "read_file" | "view" | "cat" => {
                let path = path.unwrap_or_default();
                let mut step = TraceStep::new(TraceKind::Read, "Read").detail(path);
                if let Some(total) = output.get("total_lines").and_then(Value::as_u64) {
                    step = step.meta(plural(total as usize, "line", "lines"));
                }
                let text = field(&output, &["content", "text"])
                    .map(str::to_owned)
                    .or_else(|| {
                        (output.is_null() && !content.trim().is_empty()).then(|| content.clone())
                    });
                if let Some(text) = text {
                    step = step.body(
                        CodeBlock::new(clip(&text))
                            .title(path)
                            .language(file_language(path))
                            .line_numbers(true)
                            .size(CodeSize::Sm)
                            .wrap_toggle(true),
                    );
                }
                step
            }
            "edit" | "multiedit" | "str_replace" | "str_replace_editor" | "edit_file"
            | "search_replace" | "apply_patch" | "write" | "write_file" | "create_file" => {
                let writes = lower.starts_with("write") || lower == "create_file";
                let path = path.unwrap_or_default();
                let mut step =
                    TraceStep::new(TraceKind::Edit, if writes { "Wrote" } else { "Edited" })
                        .detail(path);
                let edits: Vec<(String, String)> = match args.get("edits").and_then(Value::as_array)
                {
                    Some(list) => list
                        .iter()
                        .map(|e| {
                            (
                                field(e, &["old_string", "old_str"])
                                    .unwrap_or_default()
                                    .to_owned(),
                                field(e, &["new_string", "new_str"])
                                    .unwrap_or_default()
                                    .to_owned(),
                            )
                        })
                        .collect(),
                    None => match (
                        field(&args, &["old_string", "old_str"]),
                        field(&args, &["new_string", "new_str"]),
                    ) {
                        (old, Some(new)) => {
                            vec![(old.unwrap_or_default().to_owned(), new.to_owned())]
                        }
                        _ => Vec::new(),
                    },
                };
                if let Some(patch) = field(&args, &["patch", "diff", "input"])
                    .filter(|p| p.contains("\n+") || p.contains("\n-"))
                {
                    let added = patch
                        .lines()
                        .filter(|l| l.starts_with('+') && !l.starts_with("+++"))
                        .count();
                    let removed = patch
                        .lines()
                        .filter(|l| l.starts_with('-') && !l.starts_with("---"))
                        .count();
                    files.add(
                        if path.is_empty() { "a patch" } else { path },
                        added,
                        removed,
                    );
                    step = step.meta(format!("+{added} −{removed}")).body(
                        CodeBlock::new(clip(patch))
                            .title(if path.is_empty() { "Patch" } else { path })
                            .language("diff")
                            .diff(true)
                            .size(CodeSize::Sm)
                            .wrap_toggle(true),
                    );
                } else if !edits.is_empty() {
                    let mut diff = String::new();
                    let (mut added, mut removed) = (0, 0);
                    for (old, new) in &edits {
                        let (text, a, r) = replacement_diff(old, new);
                        diff.push_str(&text);
                        added += a;
                        removed += r;
                    }
                    files.add(path, added, removed);
                    step = step.meta(format!("+{added} −{removed}")).body(
                        CodeBlock::new(clip(&diff))
                            .title(path)
                            .language("diff")
                            .diff(true)
                            .size(CodeSize::Sm)
                            .wrap_toggle(true),
                    );
                } else if let Some(text) = field(&args, &["content", "contents", "text"]) {
                    let lines = text.lines().count();
                    files.add(path, lines, 0);
                    step = step.meta(plural(lines, "line", "lines")).body(
                        CodeBlock::new(clip(text))
                            .title(path)
                            .language(file_language(path))
                            .line_numbers(true)
                            .size(CodeSize::Sm)
                            .wrap_toggle(true),
                    );
                } else if !path.is_empty() {
                    files.add(path, 0, 0);
                }
                step
            }
            "grep" | "glob" | "search" | "find" | "codebase_search" | "web_search"
            | "websearch" | "search_files" | "ripgrep" => {
                let query =
                    field(&args, &["pattern", "query", "q", "glob", "regex"]).unwrap_or_default();
                let web = lower.contains("web");
                let mut step = TraceStep::new(
                    if web {
                        TraceKind::Source
                    } else {
                        TraceKind::Search
                    },
                    if web { "Searched the web" } else { "Searched" },
                )
                .detail(line(query, 120));
                let shown = if output_text.trim().is_empty() {
                    shown_value(&output, &content)
                } else {
                    output_text.clone()
                };
                if !shown.trim().is_empty() {
                    step = step.body(output_block(&clip(&shown), "Results"));
                }
                step
            }
            "web_fetch" | "webfetch" | "fetch" | "open_url" | "browse" => {
                let url = field(&args, &["url", "href"]).unwrap_or_default();
                TraceStep::new(TraceKind::Source, "Read")
                    .detail(line(url, 120))
                    .href(url)
            }
            "todowrite" | "update_plan" | "plan" | "todo_write" => {
                let items = args
                    .get("todos")
                    .or_else(|| args.get("plan"))
                    .and_then(Value::as_array)
                    .cloned()
                    .unwrap_or_default();
                let lines: Vec<TraceStep> = items
                    .iter()
                    .map(|item| {
                        let text =
                            field(item, &["content", "step", "text", "title"]).unwrap_or_default();
                        let done = matches!(field(item, &["status"]), Some("completed" | "done"));
                        TraceStep::new(TraceKind::Plan, line(text, 160))
                            .icon(if done {
                                Icon::CheckCircleFilled
                            } else {
                                Icon::EmptyCircle
                            })
                            .status(if done {
                                TraceStatus::Done
                            } else {
                                TraceStatus::Pending
                            })
                    })
                    .collect();
                let done = lines
                    .iter()
                    .filter(|l| l.current_status() == TraceStatus::Done)
                    .count();
                let mut step = TraceStep::new(TraceKind::Plan, "Updated the plan");
                if !lines.is_empty() {
                    step = step
                        .meta(format!("{done} of {} done", lines.len()))
                        .children(lines);
                }
                step
            }
            "ask_user" | "askuserquestion" | "ask_questions" | "clarify" | "questionnaire"
            | "request_user_input" => {
                if let Some(card) = questionnaire(&args, &output, call) {
                    return Part::Card(card);
                }
                TraceStep::new(TraceKind::Tool, "Asked you a question")
            }
            "load_tools" | "toolsearch" | "tool_search" => {
                let what = args
                    .get("families")
                    .and_then(Value::as_array)
                    .map(|f| {
                        f.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join(", ")
                    })
                    .or_else(|| field(&args, &["query"]).map(str::to_owned))
                    .unwrap_or_default();
                TraceStep::new(TraceKind::Tool, "Got tools ready").detail(what)
            }
            _ if is_delegation(&lower, &args) => {
                return Part::Line(
                    self.delegation(call, result, &args, &output, &content, status, ms, depth),
                );
            }
            _ => {
                let mut step = TraceStep::new(TraceKind::Tool, "Used").detail(name);
                let input = shown(&args);
                let shown = shown_value(&output, &content);
                let mut body = html! {};
                if !input.trim().is_empty() && input != "{}" {
                    body = html! { (body) (output_block(&clip(&input), "Input")) };
                }
                if !shown.trim().is_empty() {
                    body = html! { (body) (output_block(&clip(&shown), "Output")) };
                }
                if !body.0.is_empty() {
                    step = step.body(body);
                }
                step
            }
        };
        if let Some(error) = error {
            step = step.description(line(&error, 200));
        }
        if let Some((href, title)) =
            field(call, &["tool_call_id"]).and_then(|id| self.links.get(id))
        {
            step =
                step.child(TraceStep::new(TraceKind::Agent, format!("Started {title}")).href(href));
        }
        if let Some(took) = ms.and_then(took) {
            step = step.meta(took);
        }
        Part::Line(step.status(status).open(self.expand))
    }

    #[allow(clippy::too_many_arguments)]
    fn delegation(
        &mut self,
        call: &Value,
        result: Option<&Value>,
        args: &Value,
        output: &Value,
        content: &str,
        status: TraceStatus,
        ms: Option<u64>,
        depth: usize,
    ) -> TraceStep {
        let name = field(call, &["function_name"]).unwrap_or_default();
        let who = field(args, &["to", "agent", "subagent_type", "harness"])
            .or_else(|| field(output, &["harness"]))
            .unwrap_or(if name.eq_ignore_ascii_case("microcoder") {
                "microcoder"
            } else {
                ""
            });
        let task = field(args, &["task", "prompt", "description", "message"]).unwrap_or_default();
        let mut step = TraceStep::new(TraceKind::Agent, format!("Asked {}", agent_name(who)))
            .description(line(task, 200))
            .status(status);
        if let Some(took) = ms.and_then(took) {
            step = step.meta(took);
        }
        let reply = field(output, &["summary", "reply", "result", "answer"])
            .map(str::to_owned)
            .or_else(|| field(output, &["error"]).map(str::to_owned))
            .or_else(|| {
                (output.is_null() && !content.trim().is_empty()).then(|| content.to_owned())
            });
        let trajectory = if depth < MAX_DEPTH {
            self.agents.take(call, result, output)
        } else {
            None
        };
        let mut files = Files::default();
        if let Some(changed) = output.get("changed_files").and_then(Value::as_array) {
            for path in changed.iter().filter_map(Value::as_str) {
                files.add(path, 0, 0);
            }
        }
        let mut children = Vec::new();
        if let Some(trajectory) = trajectory {
            let mut texts = Vec::new();
            for inner in steps(trajectory).iter().take(MAX_SHOWN_STEPS) {
                if inner.get("source").and_then(Value::as_str) != Some("agent") {
                    continue;
                }
                for part in self.step(inner, &mut files, depth + 1) {
                    match part {
                        Part::Line(line) => children.push(line),
                        Part::Text(text) => texts.push(text),
                        Part::Card(_) => {}
                    }
                }
            }
            // Its words before its last are lines of their own.
            let last = texts.pop();
            for text in texts {
                children.push(said(&text));
            }
            if let Some(last) = last.filter(|_| reply.is_none()) {
                children.push(said(&last).open(true));
            }
            if !children.is_empty() {
                step = step.meta(plural(children.len(), "step", "steps"));
            }
        }
        let page = field(call, &["tool_call_id"]).and_then(|id| self.links.get(id));
        if let Some((href, title)) = page {
            if children.is_empty() && reply.is_none() {
                return step.href(href);
            }
            children.push(
                TraceStep::new(TraceKind::Agent, "Open its page")
                    .detail(line(title, 80))
                    .href(href),
            );
        }
        let body = html! {
            @if let Some(reply) = &reply { (small_markdown(reply)) }
            @if !files.0.is_empty() { (files_list(&files)) }
        };
        if reply.is_some() || !files.0.is_empty() {
            step = step.body(body);
        }
        step.children(children).open(self.expand)
    }
}

fn is_delegation(lower: &str, args: &Value) -> bool {
    matches!(
        lower,
        "delegate"
            | "task"
            | "agent"
            | "microcoder"
            | "spawn_agent"
            | "subagent"
            | "dispatch_agent"
    ) || (lower == "result" && field(args, &["agent"]).is_some())
}

fn shown_value(output: &Value, content: &str) -> String {
    if output.is_null() {
        content.to_owned()
    } else {
        shown(output)
    }
}

fn output_block(text: &str, title: &str) -> Markup {
    CodeBlock::new(text.trim_end().to_owned())
        .title(title)
        .size(CodeSize::Sm)
        .wrap_toggle(true)
        .render()
}

/// Words an agent said between its steps, as a line that opens to them.
fn said(text: &str) -> TraceStep {
    TraceStep::new(TraceKind::Summary, line(text, 140))
        .icon(Icon::Chat)
        .body(small_markdown(text))
}

fn files_list(files: &Files) -> Markup {
    Attachments::new(files.0.iter().map(|(path, added, removed)| {
        let meta = if *added + *removed > 0 {
            format!("+{added} −{removed}")
        } else {
            "Changed".to_owned()
        };
        Attachment::new(path.clone()).meta(meta)
    }))
    .label("Files changed")
    .render()
}

/// Clarifying questions asked through a tool, with the answers given.
fn questionnaire(args: &Value, output: &Value, call: &Value) -> Option<Markup> {
    let questions = args.get("questions").and_then(Value::as_array)?;
    let id = format!(
        "questions-{}",
        field(call, &["tool_call_id"]).unwrap_or("x")
    );
    let answers = output.get("answers");
    let mut card = Questionnaire::new(id, "Questions for you");
    for (index, q) in questions.iter().enumerate() {
        let prompt = field(q, &["question", "prompt", "text"]).unwrap_or_default();
        let mut question = Question::new(format!("q{index}"), prompt).multiple(
            q.get("multiSelect")
                .or_else(|| q.get("multiple"))
                .and_then(Value::as_bool)
                == Some(true),
        );
        for option in q
            .get("options")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            match option {
                Value::String(label) => question = question.option(label.clone(), None),
                other => {
                    if let Some(label) = field(other, &["label", "text", "value"]) {
                        question = question.option(label, field(other, &["description"]));
                    }
                }
            }
        }
        let answer = answers.and_then(|a| a.get(prompt).or_else(|| a.get(index)));
        let given: Vec<String> = match answer {
            Some(Value::String(text)) => text.split(", ").map(str::to_owned).collect(),
            Some(Value::Array(list)) => list
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            _ => Vec::new(),
        };
        card = card.question(question.answer(given));
    }
    Some(card.render())
}

fn month(m: u32) -> &'static str {
    [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ][(m.clamp(1, 12) - 1) as usize]
}

/// "October 14, 2026" and "13:02 UTC" from an ISO time.
fn day_and_time(iso: &str) -> Option<(String, String, String)> {
    let ms = atif::parse_iso(iso)?;
    let at = atif::iso(ms);
    let year = &at[..4];
    let m: u32 = at[5..7].parse().ok()?;
    let d: u32 = at[8..10].parse().ok()?;
    Some((
        format!("{} {d}, {year}", month(m)),
        format!("{} UTC", &at[11..16]),
        at,
    ))
}

/// The author shown on the document's turns.
fn author(document: &Value) -> String {
    // The web chat speaks as OpenAgents.
    if document
        .pointer("/agent/extra/door")
        .and_then(Value::as_str)
        == Some("openagents-web")
    {
        return "OpenAgents".into();
    }
    let name = document
        .pointer("/agent/name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if name.is_empty() {
        "Agent".into()
    } else {
        agent_name(name)
    }
}

/// One agent turn: its steps drawn as one [`Turn`].
fn agent_turn<'d>(
    drawer: &mut Drawer<'_, 'd>,
    document: &Value,
    run: &[&'d Value],
    index: usize,
) -> Markup {
    let mut files = Files::default();
    let mut parts = Vec::new();
    for step in run {
        parts.extend(drawer.step(step, &mut files, 0));
    }
    let answer = match parts.last() {
        Some(Part::Text(_)) => match parts.pop() {
            Some(Part::Text(text)) => Some(text),
            _ => None,
        },
        _ => None,
    };
    let mut lines = Vec::new();
    let mut cards = Vec::new();
    for part in parts {
        match part {
            Part::Line(step) => lines.push(step),
            Part::Text(text) => lines.push(said(&text).open(drawer.expand)),
            Part::Card(card) => cards.push(card),
        }
    }
    let times: Vec<u64> = run
        .iter()
        .filter_map(|s| s.get("timestamp").and_then(Value::as_str))
        .filter_map(atif::parse_iso)
        .collect();
    let span = times
        .iter()
        .max()
        .zip(times.iter().min())
        .map(|(a, b)| a - b);
    let worked: u64 = run
        .iter()
        .filter_map(|s| s.pointer("/extra/duration_ms").and_then(Value::as_u64))
        .sum();
    let duration = span
        .filter(|ms| *ms >= 1000)
        .or((worked >= 1000).then_some(worked));
    let tokens: u64 = run
        .iter()
        .filter_map(|s| s.get("metrics"))
        .map(|m| {
            m.get("prompt_tokens").and_then(Value::as_u64).unwrap_or(0)
                + m.get("completion_tokens")
                    .and_then(Value::as_u64)
                    .unwrap_or(0)
        })
        .sum();
    let model = run
        .iter()
        .find_map(|s| s.get("model_name").and_then(Value::as_str))
        .or_else(|| {
            document
                .pointer("/agent/model_name")
                .and_then(Value::as_str)
        })
        .unwrap_or_default();
    // "auto" names the router, not a model.
    let model = if model == "auto" { "" } else { model };
    let mut turn = Turn::new(author(document))
        .icon(Icon::Agent)
        .model(line(model, 60))
        .id(format!("turn-{index}"));
    if let Some((_, time, at)) = run
        .first()
        .and_then(|s| s.get("timestamp").and_then(Value::as_str))
        .and_then(day_and_time)
    {
        turn = turn.time(at, time);
    }
    let failed = lines
        .iter()
        .filter(|l| l.current_status() == TraceStatus::Error)
        .count();
    let empty = lines.is_empty() && answer.is_none() && cards.is_empty();
    if lines.len() == 1 && answer.is_some() {
        turn = turn.part(Trace::new(lines).label("What it did"));
    } else if !lines.is_empty() {
        let title = duration.map_or_else(
            || "Worked".to_owned(),
            |ms| format!("Worked for {}", crate::traces::duration_words(ms)),
        );
        let mut summary = TraceStep::new(TraceKind::Summary, title)
            .meta(plural(lines.len(), "step", "steps"))
            .open(drawer.expand || answer.is_none());
        if failed > 0 {
            summary = summary.description(if failed == 1 {
                "1 step didn't work".to_owned()
            } else {
                format!("{failed} steps didn't work")
            });
        }
        turn = turn.part(Trace::new([summary.children(lines)]).label("What it did"));
    }
    for card in cards {
        turn = turn.part(card);
    }
    if let Some(answer) = &answer {
        turn = turn.part(markdown(answer));
    }
    if !files.0.is_empty() {
        turn = turn.part(files_list(&files));
    }
    let made: Vec<Attachment> = run.iter().filter_map(|s| lab_card(s)).collect();
    if !made.is_empty() {
        turn = turn.part(Attachments::new(made.clone()).label("What it made"));
    }
    if let Some(answer) = &answer {
        turn = turn.action(
            CopyButton::new(answer.clone())
                .label("Copy")
                .aria_label("Copy the answer")
                .copy_icon(Icon::Copy)
                .size(ControlSize::Xs)
                .variant(ButtonVariant::Ghost)
                .color(Color::Secondary),
        );
    }
    if empty && files.0.is_empty() && made.is_empty() {
        return html! {
            (turn)
            (Marker::note("No answer came back"))
        };
    }
    let mut meta = Vec::new();
    if let Some(ms) = duration {
        meta.push(crate::traces::duration_words(ms));
    }
    if tokens > 0 {
        meta.push(format!("{} tokens", crate::traces::tokens_words(tokens)));
    }
    turn.meta(meta.join(" · ")).render()
}

/// A Lab card an answer carried (`extra.video_lab`, `extra.game_lab`):
/// the document and its version. The card's player lives on the person's
/// own computer, so it is named, not linked.
fn lab_card(step: &Value) -> Option<Attachment> {
    let (lab, card) = [
        ("Video Lab", "/extra/video_lab"),
        ("Game Lab", "/extra/game_lab"),
    ]
    .into_iter()
    .find_map(|(lab, pointer)| step.pointer(pointer).map(|card| (lab, card)))?;
    let title = field(card, &["title", "doc"])?;
    let version = card.get("version").and_then(Value::as_u64);
    let meta = version.map_or_else(|| lab.to_owned(), |v| format!("{lab} · version {v}"));
    Some(
        Attachment::new(line(title, 80))
            .kind(AttachmentKind::Image)
            .meta(meta),
    )
}

/// Files a person's step carries: image and file content parts.
fn sent_files(step: &Value) -> Vec<Attachment> {
    step.get("message")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|part| {
            let kind = field(part, &["type"])?;
            if kind == "text" {
                return None;
            }
            let name = field(part, &["name", "filename", "file_name", "path"])
                .or_else(|| part.pointer("/source/path").and_then(Value::as_str))
                .map(str::to_owned)
                .unwrap_or_else(|| {
                    if kind.contains("image") {
                        "Picture".into()
                    } else {
                        "File".into()
                    }
                });
            let mut attachment = Attachment::new(name);
            if kind.contains("image") {
                attachment = attachment.kind(AttachmentKind::Image).meta("Picture");
            }
            Some(attachment)
        })
        .collect()
}

fn flush<'d>(
    run: &mut Vec<&'d Value>,
    items: &mut Vec<Markup>,
    drawer: &mut Drawer<'_, 'd>,
    document: &Value,
    turns: &mut usize,
) {
    if !run.is_empty() {
        *turns += 1;
        items.push(agent_turn(drawer, document, run, *turns));
        run.clear();
    }
}

/// The whole document as a conversation.
pub(crate) fn transcript(document: &Value, links: &AgentLinks, expand: bool) -> Markup {
    let all = steps(document);
    let shown = &all[..all.len().min(MAX_SHOWN_STEPS)];
    let mut drawer = Drawer {
        links,
        agents: Agents::of(document),
        expand,
    };
    let mut scroller = MessageScroller::new().label("Conversation");
    let mut last_day = String::new();
    let mut run: Vec<&Value> = Vec::new();
    let mut turns = 0usize;
    let mut items: Vec<Markup> = Vec::new();
    for step in shown {
        if let Some((day, _, _)) = step
            .get("timestamp")
            .and_then(Value::as_str)
            .and_then(day_and_time)
            .filter(|(day, _, _)| *day != last_day)
        {
            flush(&mut run, &mut items, &mut drawer, document, &mut turns);
            items.push(Marker::divider(day.clone()).render());
            last_day = day;
        }
        match step
            .get("source")
            .and_then(Value::as_str)
            .unwrap_or("agent")
        {
            "user" => {
                flush(&mut run, &mut items, &mut drawer, document, &mut turns);
                turns += 1;
                let text = message_text(step.get("message").unwrap_or(&Value::Null));
                let mut bubble = Bubble::new(text.trim().to_owned()).id(format!("turn-{turns}"));
                for file in sent_files(step) {
                    bubble = bubble.attachment(file);
                }
                items.push(bubble.render());
            }
            "system" => {
                flush(&mut run, &mut items, &mut drawer, document, &mut turns);
                let text = message_text(step.get("message").unwrap_or(&Value::Null));
                if !text.trim().is_empty() {
                    items.push(
                        Trace::new([
                            TraceStep::new(TraceKind::Plan, "Instructions for the agent")
                                .icon(Icon::InfoCircle)
                                .description(line(&text, 120))
                                .body(small_markdown(&clip(&text)))
                                .open(expand),
                        ])
                        .render(),
                    );
                }
            }
            _ => run.push(step),
        }
    }
    flush(&mut run, &mut items, &mut drawer, document, &mut turns);
    // Agents the document carries that no call above started.
    let rest = drawer.agents.rest();
    if !rest.is_empty() {
        let lines: Vec<TraceStep> = rest
            .iter()
            .map(|trajectory| {
                let who = field(trajectory, &["agent"])
                    .or_else(|| trajectory.pointer("/extra/agent").and_then(Value::as_str))
                    .unwrap_or_default();
                let mut files = Files::default();
                let children: Vec<TraceStep> = steps(trajectory)
                    .iter()
                    .filter(|s| s.get("source").and_then(Value::as_str) == Some("agent"))
                    .flat_map(|s| drawer.step(s, &mut files, 1))
                    .map(|part| match part {
                        Part::Line(line) => line,
                        Part::Text(text) => said(&text),
                        Part::Card(card) => {
                            TraceStep::new(TraceKind::Tool, "Asked questions").body(card)
                        }
                    })
                    .collect();
                TraceStep::new(TraceKind::Agent, agent_name(who))
                    .meta(plural(children.len(), "step", "steps"))
                    .children(children)
                    .open(expand)
            })
            .collect();
        items.push(Trace::new(lines).label("Other agents").render());
    }
    if all.len() > shown.len() {
        items.push(
            Marker::note(format!(
                "The first {} of {} steps are shown",
                shown.len(),
                all.len()
            ))
            .render(),
        );
    }
    scroller = scroller.turns(items);
    scroller.render()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn export() -> Value {
        serde_json::from_str(include_str!("screens/delegation.atif.json")).unwrap()
    }

    #[test]
    fn a_real_export_reads_as_a_conversation() {
        let document = export();
        let html = transcript(&document, &AgentLinks::new(), false).into_string();
        assert!(
            html.contains("role=\"log\" aria-label=\"Conversation\""),
            "{html}"
        );
        // The person's words are bubbles; the agent's answers are turns.
        assert!(
            html.contains("<div class=\"oa-bubble\">who are you</div>"),
            "{html}"
        );
        assert!(html.contains("I am Coder, made by OpenAgents."), "{html}");
        assert!(
            html.contains("<h2 class=\"oa-turn__author\">Coder</h2>"),
            "{html}"
        );
        assert!(html.contains("google/gemini-4-argon"), "{html}");
        // Its work folds under one line; the agents it asked nest with
        // their own steps.
        assert!(html.contains("Worked for"), "{html}");
        assert!(html.contains("Asked Claude Code"), "{html}");
        assert!(html.contains("Asked OpenCode"), "{html}");
        assert!(html.contains("Asked Microcoder"), "{html}");
        assert!(html.contains("head -n 30 Cargo.toml"), "{html}");
        assert!(html.contains("data-status=\"error\""), "{html}");
        assert!(
            html.contains("role=\"separator\" aria-label=\"October 14, 2026\""),
            "{html}"
        );
        // Every agent the document carries is drawn under the call that
        // asked it, so none is left over.
        assert!(!html.contains("Other agents"), "{html}");
        assert!(!html.contains(" open>"), "folded by default");
        let open = transcript(&document, &AgentLinks::new(), true).into_string();
        assert!(open.contains(" open>"), "{open}");
    }

    #[test]
    fn reasoning_edits_plans_and_questions() {
        let document = serde_json::json!({
            "schema_version": "ATIF-v1.7",
            "session_id": "s",
            "agent": {"name": "openagents-coder", "version": "1", "model_name": "m"},
            "steps": [
                {"step_id": 1, "source": "user", "message": [
                    {"type": "text", "text": "Fix it"},
                    {"type": "image", "source": {"path": "shot.png"}}
                ]},
                {"step_id": 2, "source": "agent", "reasoning_content": "**Looking**\n\nThe test is stale.",
                 "tool_calls": [
                    {"tool_call_id": "a", "function_name": "Edit", "arguments": {"file_path": "src/a.rs", "old_string": "x\ny", "new_string": "x\nz"}},
                    {"tool_call_id": "b", "function_name": "TodoWrite", "arguments": {"todos": [{"content": "Fix", "status": "completed"}, {"content": "Push", "status": "pending"}]}},
                    {"tool_call_id": "c", "function_name": "AskUserQuestion", "arguments": {"questions": [{"question": "Push?", "options": [{"label": "Yes"}, {"label": "No"}]}]}}
                 ],
                 "observation": {"results": [{"source_call_id": "c", "content": "{\"answers\":{\"Push?\":\"Yes\"}}"}]}},
                {"step_id": 3, "source": "agent", "message": "Done."}
            ]
        });
        let html = transcript(&document, &AgentLinks::new(), true).into_string();
        assert!(html.contains("Thought"), "{html}");
        assert!(html.contains("The test is stale."), "{html}");
        assert!(html.contains("data-diff=\"del\">-y"), "{html}");
        assert!(html.contains("data-diff=\"add\">+z"), "{html}");
        assert!(html.contains("+1 −1"), "{html}");
        assert!(html.contains("1 of 2 done"), "{html}");
        assert!(html.contains("Questions for you"), "{html}");
        assert!(html.contains("data-chosen"), "{html}");
        assert!(html.contains("aria-label=\"Files changed\""), "{html}");
        assert!(html.contains("shot.png"), "{html}");
        assert!(html.contains("aria-label=\"Copy the answer\""), "{html}");
        crate::copy_guard::assert_plain("/trace/x", &html);
    }

    #[test]
    fn a_replacement_reads_as_a_diff() {
        let (diff, added, removed) = replacement_diff("a\nb\nc", "a\nB\nc");
        assert_eq!(diff, "@@ -3 +3 @@\n a\n-b\n+B\n c\n");
        assert_eq!((added, removed), (1, 1));
    }
}
