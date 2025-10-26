use leptos::prelude::*;
use pulldown_cmark::{html, Options, Parser};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::Value as JsonValue;

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct MessageRow {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "threadId")]
    pub thread_id: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub data: Option<JsonValue>,
    #[serde(default)]
    pub ts: f64,
}

impl MessageRow {
    pub fn stable_key(&self) -> String {
        if let Some(id) = &self.id {
            return id.clone();
        }
        let ts_bits = self.ts.to_bits();
        format!("{}-{ts_bits}", self.kind_slug())
    }

    pub fn kind_slug(&self) -> String {
        self.kind
            .as_deref()
            .unwrap_or("message")
            .to_lowercase()
    }
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CommandExecutionPayload {
    #[serde(default)]
    pub command: Option<String>,
    #[serde(default)]
    pub aggregated_output: Option<String>,
    #[serde(default)]
    pub exit_code: Option<i32>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
pub struct FileChangePayload {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub changes: Vec<FileChangeEntry>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
pub struct FileChangeEntry {
    #[serde(default)]
    pub path: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
pub struct WebSearchPayload {
    #[serde(default)]
    pub query: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
pub struct McpToolCallPayload {
    #[serde(default)]
    pub server: Option<String>,
    #[serde(default)]
    pub tool: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
pub struct TodoListPayload {
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub items: Vec<TodoItemPayload>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
pub struct TodoItemPayload {
    #[serde(default)]
    pub text: Option<String>,
    #[serde(default)]
    pub completed: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
pub struct TurnUsagePayload {
    #[serde(default)]
    pub input_tokens: Option<i64>,
    #[serde(default)]
    pub cached_input_tokens: Option<i64>,
    #[serde(default)]
    pub output_tokens: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
pub struct TurnEventPayload {
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub usage: Option<TurnUsagePayload>,
    #[serde(default)]
    pub message: Option<String>,
    #[serde(default)]
    pub duration_ms: Option<f64>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
pub struct ItemLifecyclePayload {
    #[serde(default)]
    pub phase: Option<String>,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default, rename = "item_type")]
    pub item_type: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Default)]
pub struct ExecBeginPayload {
    #[serde(default)]
    pub command: ExecCommandField,
    #[serde(default)]
    pub cwd: Option<String>,
    #[serde(default)]
    pub parsed: Option<JsonValue>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum ExecCommandField {
    List(Vec<String>),
    One(String),
    Missing,
}

impl Default for ExecCommandField {
    fn default() -> Self {
        ExecCommandField::Missing
    }
}

impl ExecCommandField {
    fn display(&self) -> String {
        match self {
            ExecCommandField::List(items) => items.join(" "),
            ExecCommandField::One(cmd) => cmd.clone(),
            ExecCommandField::Missing => String::new(),
        }
    }
}

fn markdown_to_html(md: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);
    let parser = Parser::new_ext(md, options);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

fn quote_text(text: &str) -> String {
    if text.trim().is_empty() {
        return ">".to_string();
    }
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .lines()
        .map(|line| format!("> {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn sanitize_headline(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return "**Reasoning**".to_string();
    }
    let normalized = trimmed
        .trim_matches('*')
        .trim_matches('`')
        .trim()
        .to_ascii_lowercase();
    match normalized.as_str() {
        "unknown" | "n/a" | "none" | "null" => "**Reasoning**".to_string(),
        _ => trimmed.to_string(),
    }
}

fn reasoning_headline_text(text: &str) -> String {
    if text.trim().is_empty() {
        return "**Reasoning**".to_string();
    }
    let bytes = text.as_bytes();
    let mut headline: Option<&str> = None;
    let mut idx = 0;
    while idx + 1 < bytes.len() {
        if &bytes[idx..idx + 2] == b"**" {
            if let Some(end) = text[idx + 2..].find("**") {
                let end_idx = idx + 2 + end + 2;
                headline = Some(&text[idx..end_idx]);
                break;
            }
        }
        idx += 1;
    }
    if let Some(h) = headline {
        sanitize_headline(h)
    } else {
        let fallback = text
            .lines()
            .find(|line| !line.trim().is_empty())
            .unwrap_or("**Reasoning**");
        sanitize_headline(fallback)
    }
}

fn parse_payload<T: DeserializeOwned>(row: &MessageRow) -> Option<T> {
    if let Some(data) = &row.data {
        serde_json::from_value(data.clone()).ok()
    } else if let Some(text) = &row.text {
        serde_json::from_str(text).ok()
    } else {
        None
    }
}

fn pick_path(value: &JsonValue) -> Option<String> {
    if let Some(s) = value.as_str() {
        return Some(s.to_string());
    }
    if let Some(obj) = value.as_object() {
        for key in ["path", "name", "file", "filename"] {
            if let Some(v) = obj.get(key).and_then(|val| val.as_str()) {
                return Some(v.to_string());
            }
        }
        if let Some(arr) = obj.get("files").and_then(|v| v.as_array()) {
            if let Some(first) = arr.get(0).and_then(|v| v.as_str()) {
                return Some(first.to_string());
            }
        }
        if let Some(cmd_val) = obj.get("cmd") {
            if let Some(s) = cmd_val.as_str() {
                return Some(s.to_string());
            }
            if let Some(arr) = cmd_val.as_array() {
                let joined = arr
                    .iter()
                    .filter_map(|part| part.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if !joined.is_empty() {
                    return Some(joined);
                }
            }
        }
    }
    None
}

fn label_for(kind: &str) -> &str {
    match kind {
        "ReadFile" | "Read" => "Read",
        "WriteFile" | "Write" => "Write",
        "OpenFile" | "Open" => "Open",
        "ListFiles" => "ListFiles",
        "Search" => "Search",
        _ => kind,
    }
}

fn shorten_path(path: &str) -> String {
    for marker in ["/openagents/", "/expo/", "/crates/", "/docs/"] {
        if let Some(idx) = path.find(marker) {
            let start = idx + marker.len();
            if start < path.len() {
                return path[start..].to_string();
            }
        }
    }
    path.to_string()
}

struct ExecPretty {
    action: String,
    path: Option<String>,
    is_list: bool,
}

fn extract_pretty_exec(parsed: &JsonValue) -> Option<ExecPretty> {
    let arr = parsed.as_array()?;
    let first = arr.first()?.as_object()?;
    if first.len() != 1 {
        return None;
    }
    let (k, v) = first.iter().next()?;
    let action = label_for(k).to_string();
    let mut path = pick_path(v);

    if action.eq_ignore_ascii_case("unknown") {
        path = path.or_else(|| pick_path(v));
    }

    let is_list = action == "ListFiles";
    Some(ExecPretty {
        action,
        path,
        is_list,
    })
}

fn format_plain(row: &MessageRow) -> impl IntoView {
    let text = row.text.clone().unwrap_or_default();
    view! { <div class="jsonl-plain">{text}</div> }
}

#[component]
pub fn MarkdownBlock(markdown: String) -> impl IntoView {
    let html = markdown_to_html(&markdown);
    view! { <div class="markdown-block" inner_html=html></div> }
}

#[component]
pub fn AgentMessageCard(text: String) -> impl IntoView {
    view! {
        <div class="jsonl-card agent-message">
            <MarkdownBlock markdown=text/>
        </div>
    }
}

#[component]
pub fn UserMessageRow(text: String) -> impl IntoView {
    let quoted = quote_text(&text);
    let html = markdown_to_html(&quoted);
    view! { <div class="user-message markdown-block" inner_html=html></div> }
}

#[component]
pub fn ReasoningHeadline(text: String) -> impl IntoView {
    let headline = reasoning_headline_text(&text);
    let html = markdown_to_html(&headline);
    view! { <div class="reasoning-headline markdown-block" inner_html=html></div> }
}

#[component]
pub fn ReasoningCard(text: String) -> impl IntoView {
    view! {
        <div class="jsonl-card reasoning-card">
            <MarkdownBlock markdown=text/>
        </div>
    }
}

#[component]
pub fn ExecBeginRow(
    payload: ExecBeginPayload,
    #[prop(optional)] full: bool,
    #[prop(optional)] show_prefix: Option<bool>,
) -> impl IntoView {
    let ExecBeginPayload { command, cwd, parsed } = payload;
    let cmd = command.display();
    let pretty = parsed
        .as_ref()
        .and_then(|parsed| extract_pretty_exec(parsed));
    let show_prefix = show_prefix.unwrap_or(true);
    let prefix = if show_prefix { "[exec]".to_string() } else { String::new() };

    // Compute label and path uniformly to keep DOM shape consistent
    let (label_text, path_text) = if let Some(pretty) = pretty {
        let is_list = pretty.is_list;
        let shown_path = pretty.path.as_ref().map(|path| {
            if is_list { if path.ends_with('/') { path.clone() } else { format!("{path}/") } } else { shorten_path(path) }
        });
        (pretty.action.clone(), shown_path.unwrap_or_else(String::new))
    } else {
        (prefix.clone(), cmd.clone())
    };
    let cwd_view = if full {
        if let Some(dir) = cwd.as_ref() {
            view! { <div class="exec-cwd">{"in "}{dir.clone()}</div> }
        } else {
            view! { <div class="exec-cwd">{"in "}{String::new()}</div> }
        }
    } else {
        view! { <div class="exec-cwd">{"in "}{String::new()}</div> }
    };
    view! {
        <div class="exec-row">
            <div class="exec-action">
                <span class="exec-label">{label_text}</span>
                <span class="exec-path">{path_text}</span>
            </div>
            {cwd_view}
        </div>
    }
}

#[component]
pub fn CommandExecutionCard(
    command: String,
    status: Option<String>,
    exit_code: Option<i32>,
    sample: Option<String>,
    output_len: Option<usize>,
    #[prop(optional)] show_exit_code: bool,
    #[prop(optional)] show_output_len: bool,
    #[prop(optional)] collapsed: bool,
    #[prop(optional)] max_body_height: u32,
) -> impl IntoView {
    let max_body_height = if max_body_height == 0 { 120 } else { max_body_height };
    let is_fail = status.as_deref() == Some("failed") || exit_code.map(|c| c != 0).unwrap_or(false);
    let is_done = status.as_deref() == Some("completed") || exit_code == Some(0);
    let icon = if is_fail { "×" } else if is_done { "✓" } else { "…" };
    let status_class = if is_fail {
        "status-icon fail"
    } else if is_done {
        "status-icon ok"
    } else {
        "status-icon pending"
    };

    view! {
        <div class="jsonl-card command-card">
            <div class="command-header">
                <span class=status_class>{icon}</span>
                <span class="command-title">{command}</span>
            </div>
            {if show_exit_code {
                if let Some(code) = exit_code {
                    view! { <div class="command-meta">{"exit_code: "}{code}</div> }
                } else {
                    view! { <div class="command-meta">{"exit_code: "}{0}</div> }
                }
            } else {
                view! { <div class="command-meta">{"exit_code: "}{0}</div> }
            }}
            {if show_output_len {
                if let Some(len) = output_len {
                    view! { <div class="command-meta">{"output ~"}{len}{"B"}</div> }
                } else {
                    view! { <div class="command-meta">{"output ~"}{0usize}{"B"}</div> }
                }
            } else {
                view! { <div class="command-meta">{"output ~"}{0usize}{"B"}</div> }
            }}
            {if let Some(code) = sample {
                let style = if collapsed {
                    format!("max-height: {}px; overflow: hidden;", max_body_height)
                } else {
                    String::new()
                };
                view! {
                    <div class="code-snippet" style=style>
                        <pre><code>{code}</code></pre>
                    </div>
                }
            } else {
                let style = String::new();
                let code = String::new();
                view! { <div class="code-snippet" style=style><pre><code>{code}</code></pre></div> }
            }}
        </div>
    }
}

#[component]
pub fn FileChangeCard(
    changes: Vec<FileChangeEntry>,
    status: Option<String>,
    limit: Option<usize>,
) -> impl IntoView {
    let mut adds = 0;
    let mut updates = 0;
    let mut deletes = 0;
    for change in &changes {
        match change.kind.as_deref() {
            Some("add") => adds += 1,
            Some("delete") => deletes += 1,
            _ => updates += 1,
        }
    }
    let summary = [
        if adds > 0 { Some(format!("+{adds}")) } else { None },
        if updates > 0 { Some(format!("~{updates}")) } else { None },
        if deletes > 0 { Some(format!("-{deletes}")) } else { None },
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" ");
    let cap = limit.unwrap_or(8);
    let total = changes.len();

    view! {
        <div class="jsonl-card file-card">
            <div class="file-title">
                {"file_change"}
            {status.map(|s| view! { <span>{format!(" ({s})")}</span> })}
            </div>
            {(!summary.is_empty()).then(|| view! { <div class="file-summary">{"Summary: "}{summary.clone()}</div> })}
            <div class="file-list">
                {changes.into_iter().take(cap).map(|entry| {
                    let marker = match entry.kind.as_deref() {
                        Some("add") => "+",
                        Some("delete") => "-",
                        _ => "~",
                    };
                    let path = entry.path.unwrap_or_default();
                    view! { <div class="file-row">{marker} {path}</div> }
                }).collect::<Vec<_>>()}
                {(total > cap).then(|| view! { <div class="file-more">{"… "}{total - cap}{" more"}</div> })}
            </div>
        </div>
    }
}

#[component]
pub fn WebSearchRow(query: String) -> impl IntoView {
    view! {
        <div class="web-search-row">
            <span class="label">{"WebSearch"}</span>
            <span>{query}</span>
        </div>
    }
}

#[component]
pub fn McpToolCallRow(server: String, tool: String, status: Option<String>) -> impl IntoView {
    let status_view = status.map(|s| view! { <span>{" ("}{s}{")"}</span> });
    view! {
        <div class="mcp-row">
            <span class="label">{"MCP"}</span>
            <span class="mcp-server">{server}</span>
            <span>{" · "}{tool}</span>
            {status_view}
        </div>
    }
}

#[component]
pub fn TodoListCard(items: Vec<TodoItemPayload>, status: Option<String>) -> impl IntoView {
    let total = items.len();
    let done = items.iter().filter(|item| item.completed.unwrap_or(false)).count();
    let status_badge = status.map(|s| view! { <span>{format!(" ({s})")}</span> });
    view! {
        <div class="jsonl-card todo-card">
            <div class="todo-title">
                {"todo_list"}
                {status_badge}
            </div>
            <div class="todo-summary">{done}{"/"}{total}{" complete"}</div>
            <div class="todo-items">
                {items.into_iter().enumerate().map(|(idx, item)| {
                    let text = item.text.unwrap_or_default();
                    let completed = item.completed.unwrap_or(false);
                    let class = if completed { "todo-text done" } else { "todo-text" };
                    let idx_attr = idx.to_string();
                    view! {
                        <div class="todo-row" data-index=idx_attr>
                            <div class={if completed { "todo-check done" } else { "todo-check" }}></div>
                            <div class=class>{text}</div>
                        </div>
                    }
                }).collect::<Vec<_>>()}
            </div>
        </div>
    }
}

#[component]
pub fn ErrorRow(message: String) -> impl IntoView {
    view! {
        <div class="error-row">
            <div class="error-title">{"Error"}</div>
            <div class="error-body">{message}</div>
        </div>
    }
}

#[component]
pub fn ItemLifecycleRow(
    phase: Option<String>,
    item_type: Option<String>,
    status: Option<String>,
    id: Option<String>,
) -> impl IntoView {
    let phase_label = phase.unwrap_or_else(|| "updated".into());
    let item_label = item_type.unwrap_or_else(|| "item".into());
    let status_badge = status.map(|s| view! { <span>{" ("}{s}{")"}</span> });
    let id_badge = id.map(|value| view! { <span>{" · "}{value}</span> });
    view! {
        <div class="item-row">
            <span class="item-type">{item_label}</span>
            <span class="item-phase">{" "}{phase_label}</span>
            {status_badge}
            {id_badge}
        </div>
    }
}

#[component]
pub fn TurnEventRow(
    phase: Option<String>,
    usage: Option<TurnUsagePayload>,
    message: Option<String>,
    #[prop(optional)] show_usage: bool,
    duration_ms: Option<f64>,
) -> impl IntoView {
    let phase_value = phase.unwrap_or_else(|| "started".into());
    let color_class = match phase_value.as_str() {
        "failed" => "turn-row failed",
        "completed" => "turn-row completed",
        _ => "turn-row started",
    };
    let duration = duration_ms.map(|ms| format!(" ({:.1}s)", ms / 1000.0)).unwrap_or_default();
    let usage_text = if show_usage {
        if let Some(u) = usage.clone() {
            let input = u.input_tokens.unwrap_or(0);
            let cached = u.cached_input_tokens.unwrap_or(0);
            let output = u.output_tokens.unwrap_or(0);
            format!("usage: in {} (+{} cached) out {}", input, cached, output)
        } else { String::new() }
    } else { String::new() };
    let fail_msg = if phase_value == "failed" { message.clone().unwrap_or_default() } else { String::new() };

    view! {
        <div class={color_class}>
            <div class="turn-title">
                {match phase_value.as_str() {
                    "failed" => "Turn failed",
                    "completed" => "Turn completed",
                    _ => "Turn started",
                }}{duration}
            </div>
            <div class="turn-usage">{usage_text}</div>
            <div class="turn-message">{fail_msg}</div>
        </div>
    }
}

#[component]
pub fn ThreadStartedRow(thread_id: String) -> impl IntoView {
    view! {
        <div class="thread-row">
            <span class="thread-label">{"Thread started · "}</span>
            <span class="thread-id">{thread_id}</span>
        </div>
    }
}

#[component]
pub fn JsonlMessage(row: MessageRow) -> impl IntoView {
    let kind = row.kind_slug();
    let is_message = kind.as_str() == "message";
    let is_md = kind.as_str() == "md";
    let is_reason = kind.as_str() == "reason";
    let is_exec_begin = kind.as_str() == "exec_begin";
    let is_cmd = kind.as_str() == "cmd";
    let is_file = kind.as_str() == "file";
    let is_search = kind.as_str() == "search";
    let is_mcp = kind.as_str() == "mcp";
    let is_todo = kind.as_str() == "todo";
    let is_turn = kind.as_str() == "turn";
    let is_err = kind.as_str() == "err" || kind.as_str() == "error";
    let is_thread = kind.as_str() == "thread";
    let is_summary = kind.as_str() == "summary";
    let is_item = kind.as_str() == "item_lifecycle";

    // Pre-parse payloads where needed, one per Show
    let payload_exec_val = parse_payload::<ExecBeginPayload>(&row);
    let payload_cmd_val = parse_payload::<CommandExecutionPayload>(&row);
    let payload_file_val = parse_payload::<FileChangePayload>(&row);
    let payload_search_val = parse_payload::<WebSearchPayload>(&row);
    let payload_mcp_val = parse_payload::<McpToolCallPayload>(&row);
    let payload_todo_val = parse_payload::<TodoListPayload>(&row);
    let payload_turn_val = parse_payload::<TurnEventPayload>(&row);
    let payload_item_val = parse_payload::<ItemLifecyclePayload>(&row);

    let role = row.role.clone();
    let role1 = role.clone();
    let role2 = role.clone();
    let text_for_assistant = row.text.clone().unwrap_or_default();
    let text_for_user = row.text.clone().unwrap_or_default();
    let text_for_md = row.text.clone().unwrap_or_default();
    let text_for_reason = row.text.clone().unwrap_or_default();
    let text_for_err_fallback = row.text.clone().unwrap_or_else(|| "Unknown error".into());
    let text_for_thread = row.text.clone().unwrap_or_else(|| "thread".into());
    let text_for_summary = row.text.clone().unwrap_or_default();
    let err_msg = row
        .text
        .as_ref()
        .and_then(|txt| serde_json::from_str::<JsonValue>(txt).ok())
        .and_then(|value| value.get("message").and_then(|m| m.as_str()).map(|s| s.to_string()));

    view! {
        <div class="jsonl-row" data-kind=kind.clone()>
            <Show when=move || is_message && role1.as_deref() == Some("assistant")>
                {let text = text_for_assistant.clone(); view! { <AgentMessageCard text=text/> }}
            </Show>
            <Show when=move || is_message && role2.as_deref() != Some("assistant")>
                {let text = text_for_user.clone(); view! { <UserMessageRow text=text/> }}
            </Show>
            <Show when=move || is_md>
                {let text = text_for_md.clone(); view! { <MarkdownBlock markdown=text/> }}
            </Show>
            <Show when=move || is_reason>
                {let text = text_for_reason.clone(); view! { <ReasoningHeadline text=text/> }}
            </Show>
            { payload_exec_val.as_ref().map(|p| view! { <ExecBeginRow payload=p.clone() full=true /> }) }
            { payload_cmd_val.as_ref().map(|payload| {
                let command = payload.command.clone().unwrap_or_else(|| "shell".into());
                let mut sample = payload.aggregated_output.clone().unwrap_or_default();
                let output_len = if sample.is_empty() { None } else { Some(sample.len()) };
                if sample.len() > 600 { sample.truncate(600); }
                let sample_opt = if sample.trim().is_empty() { None } else { Some(sample) };
                view! { <CommandExecutionCard command=command status=payload.status.clone() exit_code=payload.exit_code sample=sample_opt output_len=output_len show_exit_code=true collapsed=true /> }
            }) }
            { payload_file_val.as_ref().map(|p| view! { <FileChangeCard changes=p.changes.clone() status=p.status.clone() limit=Some(8) /> }) }
            { payload_search_val.as_ref().map(|p| { let query = p.query.clone().unwrap_or_default(); view! { <WebSearchRow query=query/> } }) }
            { payload_mcp_val.as_ref().map(|p| { let server = p.server.clone().unwrap_or_default(); let tool = p.tool.clone().unwrap_or_default(); view! { <McpToolCallRow server=server tool=tool status=p.status.clone()/> } }) }
            { payload_todo_val.as_ref().map(|p| view! { <TodoListCard items=p.items.clone() status=p.status.clone()/> }) }
            { payload_turn_val.as_ref().map(|p| view! { <TurnEventRow phase=p.phase.clone() usage=p.usage.clone() message=p.message.clone() show_usage=true duration_ms=p.duration_ms /> }) }
            { if is_err { err_msg.as_ref().map(|m| view! { <ErrorRow message=m.clone()/> }) } else { None } }
            { if is_err && err_msg.is_none() { Some(view! { <ErrorRow message=text_for_err_fallback.clone()/> }) } else { None } }
            <Show when=move || is_thread>
                { let id = text_for_thread.clone(); view! { <ThreadStartedRow thread_id=id/> } }
            </Show>
            <Show when=move || is_summary>
                { let text = text_for_summary.clone(); view! { <ReasoningCard text=text/> } }
            </Show>
            { payload_item_val.as_ref().map(|p| view! { <ItemLifecycleRow phase=p.phase.clone() item_type=p.item_type.clone() status=p.status.clone() id=p.id.clone() /> }) }
            <Show when=move || !(is_message || is_md || is_reason || is_exec_begin || is_cmd || is_file || is_search || is_mcp || is_todo || is_turn || is_err || is_thread || is_summary || is_item)>
                { format_plain(&row) }
            </Show>
        </div>
    }
}
//! UI primitives for rendering JSONL‑derived rows (desktop demo components).
//!
//! Contains small structs that map message payloads and Leptos components that
//! render Markdown, reasoning blocks, command/file cards, and other rows.
