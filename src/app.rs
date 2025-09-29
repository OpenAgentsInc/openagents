use leptos::prelude::*;
use wasm_bindgen::JsCast;
use pulldown_cmark::{Parser, Options, html};
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use wasm_bindgen::JsValue;
use wasm_bindgen::closure::Closure;
use js_sys::Function as JsFunction;

#[wasm_bindgen]
extern "C" {
    // Tauri v2 global API
    #[wasm_bindgen(js_namespace = ["__TAURI__", "core"], js_name = invoke)]
    fn tauri_invoke(cmd: &str, args: JsValue) -> js_sys::Promise;

    // Tauri v2 event listen
    #[wasm_bindgen(js_namespace = ["__TAURI__", "event"], js_name = listen)]
    fn tauri_event_listen(event: &str, cb: &JsFunction) -> js_sys::Promise;
}

#[derive(Clone, Debug, Default, Deserialize)]
struct WorkspaceStatus { path: Option<String>, approval_mode: Option<String>, sandbox: Option<String>, agents_files: Vec<String> }
#[derive(Clone, Debug, Default, Deserialize)]
struct AccountStatus { signed_in_with: Option<String>, login: Option<String>, plan: Option<String> }
#[derive(Clone, Debug, Default, Deserialize)]
struct ModelStatus { name: Option<String>, provider: Option<String>, reasoning_effort: Option<String>, reasoning_summaries: Option<String> }
#[derive(Clone, Debug, Default, Deserialize)]
struct ClientStatus { cli_version: Option<String> }
#[derive(Clone, Debug, Default, Deserialize)]
struct TokenUsageStatus { session_id: Option<String>, input: Option<u64>, output: Option<u64>, total: Option<u64> }
#[derive(Clone, Debug, Default, Deserialize)]
struct UsageLimitsStatus { note: Option<String> }
#[derive(Clone, Debug, Default, Deserialize)]
struct FullStatus { workspace: WorkspaceStatus, account: AccountStatus, model: ModelStatus, client: ClientStatus, token_usage: TokenUsageStatus, usage_limits: UsageLimitsStatus }

async fn fetch_full_status() -> FullStatus {
    let args = js_sys::Object::new();
    let promise = tauri_invoke("get_full_status", JsValue::from(args));
    match JsFuture::from(promise).await {
        Ok(val) => serde_wasm_bindgen::from_value::<FullStatus>(val).unwrap_or_default(),
        Err(_) => FullStatus::default(),
    }
}

// ---- Chats API ----
#[derive(Clone, Debug, Default, Deserialize)]
struct UiChatSummary {
    #[allow(dead_code)]
    id: String,
    path: String,
    started_at: String,
    title: String,
    #[allow(dead_code)]
    cwd: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(tag = "kind")]
enum UiDisplayItem { #[default] Empty, User { text: String }, Assistant { text: String }, Reasoning { text: String }, Tool { title: String, text: String }, Instructions { ikind: String, text: String } }

async fn fetch_recent_chats(limit: usize) -> Vec<UiChatSummary> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "limit": limit })).unwrap_or(JsValue::UNDEFINED);
    let promise = tauri_invoke("list_recent_chats", args);
    match JsFuture::from(promise).await { Ok(val) => serde_wasm_bindgen::from_value::<Vec<UiChatSummary>>(val).unwrap_or_default(), Err(_) => vec![] }
}

async fn fetch_chat_items(path: String) -> Vec<UiDisplayItem> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "path": path })).unwrap_or(JsValue::UNDEFINED);
    let promise = tauri_invoke("load_chat", args);
    match JsFuture::from(promise).await { Ok(val) => serde_wasm_bindgen::from_value::<Vec<UiDisplayItem>>(val).unwrap_or_default(), Err(_) => vec![] }
}

// ---- Master Task API ----
#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
struct TaskMeta { id: String, name: String, status: String, #[allow(dead_code)] updated_at: String }

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
struct AutonomyBudget { approvals: String, sandbox: String, max_turns: Option<u32>, max_tokens: Option<u32>, max_minutes: Option<u32> }

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
struct StopConditions { #[allow(dead_code)] done_regex: Option<String> }

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
struct Metrics { #[allow(dead_code)] turns: u64, #[allow(dead_code)] tokens_in: u64, #[allow(dead_code)] tokens_out: u64, #[allow(dead_code)] wall_clock_minutes: u64 }

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
struct Subtask { id: String, title: String, status: String }

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
struct Task { version: u8, id: String, name: String, status: String, #[allow(dead_code)] created_at: String, updated_at: String, autonomy_budget: AutonomyBudget, stop_conditions: StopConditions, queue: Vec<Subtask>, metrics: Metrics }

async fn tasks_list() -> Vec<TaskMeta> {
    let promise = tauri_invoke("tasks_list_cmd", JsValue::UNDEFINED);
    match JsFuture::from(promise).await { Ok(v) => serde_wasm_bindgen::from_value::<Vec<TaskMeta>>(v).unwrap_or_default(), Err(_) => vec![] }
}

async fn task_get(id: &str) -> Option<Task> {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id })).unwrap_or(JsValue::UNDEFINED);
    match JsFuture::from(tauri_invoke("task_get_cmd", args)).await { Ok(v) => serde_wasm_bindgen::from_value::<Task>(v).ok(), Err(_) => None }
}

async fn task_create(name: &str) -> Result<Task, String> {
    // Default Master Task to read-only to prevent accidental writes.
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
        "name": name, "approvals":"never","sandbox":"read-only","max_turns": 50u32
    })).unwrap_or(JsValue::UNDEFINED);
    match JsFuture::from(tauri_invoke("task_create_cmd", args)).await {
        Ok(v) => serde_wasm_bindgen::from_value::<Task>(v).map_err(|e| format!("decode task: {}", e)),
        Err(e) => {
            let msg = js_sys::JSON::stringify(&e).ok().and_then(|v| v.as_string()).unwrap_or_else(|| "invoke error".into());
            Err(msg)
        }
    }
}

#[allow(dead_code)]
async fn task_delete(id: &str) -> bool {
    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id })).unwrap_or(JsValue::UNDEFINED);
    JsFuture::from(tauri_invoke("task_delete_cmd", args)).await.is_ok()
}

// ---- Streaming UI types ----
#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(tag = "kind")]
enum UiStreamEvent {
    #[default]
    Created,
    OutputTextDelta { text: String },
    ToolDelta { call_id: String, chunk: String, is_stderr: bool },
    OutputItemDoneMessage { text: String },
    Completed { #[serde(rename = "response_id")] _response_id: Option<String>, token_usage: Option<TokenUsageLite> },
    Raw { json: String },
    SystemNote { #[serde(rename = "text")] _text: String },
    ReasoningDelta { text: String },
    ReasoningSummary { text: String },
    ReasoningBreak {},
    ToolBegin { call_id: String, title: String },
    ToolEnd { call_id: String, #[serde(rename = "exit_code")] _exit_code: Option<i64> },
    SessionConfigured { session_id: String, rollout_path: Option<String> },
    TaskUpdate { task_id: String, status: String, message: Option<String> },
}

#[allow(dead_code)]
#[derive(Clone, Debug, Default, Deserialize)]
struct TokenUsageLite { input: u64, output: u64, total: u64 }

#[derive(Clone, Debug)]
#[allow(dead_code)]
enum ChatItem {
    User { text: String },
    Reasoning { text: String },
    Assistant { text: String, streaming: bool },
    Tool { call_id: String, title: String, segments: Vec<(String, bool)>, done: bool },
    System { text: String },
    Collapsible { label: String, text: String },
}

fn append_to_assistant(list: &mut Vec<ChatItem>, s: &str) {
    if let Some(last) = list.last_mut() {
        if let ChatItem::Assistant { text, .. } = last { text.push_str(s); return; }
    }
    list.push(ChatItem::Assistant { text: s.to_string(), streaming: true });
}

fn append_tool_chunk(list: &mut Vec<ChatItem>, call_id: &str, chunk: &str, is_stderr: bool) {
    if let Some(ChatItem::Tool { segments, .. }) = list.iter_mut().rev().find(|i| matches!(i, ChatItem::Tool { call_id: id, .. } if id == call_id )) {
        if let Some((last_text, last_err)) = segments.last_mut() {
            if *last_err == is_stderr {
                last_text.push_str(chunk);
                return;
            }
        }
        segments.push((chunk.to_string(), is_stderr));
    } else {
        list.push(ChatItem::Tool { call_id: call_id.to_string(), title: "exec".to_string(), segments: vec![(chunk.to_string(), is_stderr)], done: false });
    }
}

fn append_to_reasoning(list: &mut Vec<ChatItem>, s: &str) {
    if let Some(last) = list.last_mut() {
        if let ChatItem::Reasoning { text } = last {
            text.push_str(s);
            return;
        }
    }
    list.push(ChatItem::Reasoning { text: s.to_string() });
}

fn md_to_html(md: &str) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(md, opts);
    let mut out = String::new();
    html::push_html(&mut out, parser);
    out
}

#[component]
pub fn App() -> impl IntoView {
    // Load auth status on mount
    let full: RwSignal<FullStatus> = RwSignal::new(Default::default());
    let full_setter = full.write_only();
    // Sidebar toggles

    // Chat state
    let items: RwSignal<Vec<ChatItem>> = RwSignal::new(vec![]);
    let token_usage_sig: RwSignal<Option<TokenUsageLite>> = RwSignal::new(None);
    // Logs
    let raw_events: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let compact_logs: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let show_raw_json: RwSignal<bool> = RwSignal::new(false);
    let show_token_logs: RwSignal<bool> = RwSignal::new(false);
    let show_delta_logs: RwSignal<bool> = RwSignal::new(false);
    // Tracks if we have streamed any reasoning deltas for the current turn
    let reasoning_streamed: RwSignal<bool> = RwSignal::new(false);
    let raw_open: RwSignal<bool> = RwSignal::new(true);
    let status_open: RwSignal<bool> = RwSignal::new(false);

    // Recent chats
    let chats: RwSignal<Vec<UiChatSummary>> = RwSignal::new(vec![]);
    let chats_open: RwSignal<bool> = RwSignal::new(true);
    let chat_title: RwSignal<String> = RwSignal::new("New chat".to_string());
    let reasoning: RwSignal<String> = RwSignal::new("High".to_string());
    let bottom_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let raw_bottom_ref: NodeRef<leptos::html::Div> = NodeRef::new();
    let input_ref: NodeRef<leptos::html::Input> = NodeRef::new();
    // Session mode: Chat (default) vs Task (routes send() into master task flow)
    let session_mode: RwSignal<String> = RwSignal::new("Chat".to_string());
    // Master tasks
    let tasks: RwSignal<Vec<TaskMeta>> = RwSignal::new(vec![]);
    let _tasks_open: RwSignal<bool> = RwSignal::new(true);
    let selected_task: RwSignal<Option<String>> = RwSignal::new(None);
    let selected_task_detail: RwSignal<Option<Task>> = RwSignal::new(None);

    // Install event listener once (on mount)
    {
        let items_setter = items.write_only();
        let token_setter = token_usage_sig.write_only();
        let raw_setter = raw_events.write_only();
        let compact_setter = compact_logs.write_only();
        let show_tokens = show_token_logs.read_only();
        let show_deltas = show_delta_logs.read_only();
        let show_raw = show_raw_json.read_only();
        let chats_setter = chats.write_only();
        let tasks_setter = tasks.write_only();
        let sel_task_ro = selected_task.read_only();
        let sel_task_detail_setter = selected_task_detail.write_only();
        spawn_local(async move {
            let cb = Closure::wrap(Box::new(move |evt: JsValue| {
                let payload = js_sys::Reflect::get(&evt, &JsValue::from_str("payload")).unwrap_or(JsValue::NULL);
                if payload.is_null() || payload.is_undefined() { return; }
                let parsed: Result<UiStreamEvent, _> = serde_wasm_bindgen::from_value(payload);
                if let Ok(ev) = parsed {
                    // Write formatted/filtered logs
                    match &ev {
                        UiStreamEvent::Raw { json } => {
                            if show_raw.get() {
                                raw_setter.update(|v| v.push(json.clone()));
                            }
                        }
                        UiStreamEvent::SessionConfigured { session_id, .. } => {
                            compact_setter.update(|v| v.push(format!("session_configured: {}", session_id)));
                        }
                        UiStreamEvent::TaskUpdate { task_id, status, message } => {
                            let m = message.clone().unwrap_or_default();
                            compact_setter.update(|v| v.push(format!("task[{}]: {} {}", task_id, status, m)));
                        }
                        UiStreamEvent::ToolBegin { title, .. } => {
                            compact_setter.update(|v| v.push(format!("tool: begin {}", title)));
                        }
                        UiStreamEvent::ToolEnd { call_id, .. } => {
                            compact_setter.update(|v| v.push(format!("tool: end {}", call_id)));
                        }
                        UiStreamEvent::OutputItemDoneMessage { text } => {
                            let mut t = text.clone(); if t.len() > 200 { t.truncate(200); t.push_str("…"); }
                            compact_setter.update(|v| v.push(format!("assistant: {}", t)));
                        }
                        UiStreamEvent::Completed { token_usage, .. } => {
                            if let Some(tu) = token_usage.clone() {
                                compact_setter.update(|v| v.push(format!("completed: in={} out={} total={}", tu.input, tu.output, tu.total)));
                            } else {
                                compact_setter.update(|v| v.push("completed".to_string()));
                            }
                        }
                        UiStreamEvent::ReasoningDelta { text } => {
                            if show_deltas.get() {
                                let mut t = text.clone(); if t.len() > 160 { t.truncate(160); t.push_str("…"); }
                                compact_setter.update(|v| v.push(format!("reasoning: {}", t)));
                            }
                        }
                        UiStreamEvent::SystemNote { _text } => {
                            compact_setter.update(|v| v.push("note: (see UI)".to_string()));
                        }
                        _ => {}
                    }
                    if let UiStreamEvent::SessionConfigured { .. } = &ev {
                        // Refresh recent chats when a new session starts
                        let chats_setter = chats_setter.clone();
                        spawn_local(async move { chats_setter.set(fetch_recent_chats(30).await); });
                    }
                    if let UiStreamEvent::TaskUpdate { .. } = &ev {
                        // Refresh master task list and selected detail
                        let tasks_setter = tasks_setter.clone();
                        let sel_task_ro = sel_task_ro.clone();
                        let sel_task_detail_setter = sel_task_detail_setter.clone();
                        spawn_local(async move {
                            tasks_setter.set(tasks_list().await);
                            if let Some(id) = sel_task_ro.get() { sel_task_detail_setter.set(task_get(&id).await); }
                        });
                    }
                    items_setter.update(|list| {
                        match ev {
                            UiStreamEvent::Created => {}
                            UiStreamEvent::OutputTextDelta { text } => {
                                append_to_assistant(list, &text);
                            }
                            UiStreamEvent::ToolDelta { call_id, chunk, is_stderr } => {
                                append_tool_chunk(list, &call_id, &chunk, is_stderr);
                            }
                            UiStreamEvent::ToolBegin { call_id, title } => {
                                list.push(ChatItem::Tool { call_id, title, segments: Vec::new(), done: false });
                            }
                            UiStreamEvent::ToolEnd { call_id, _exit_code: _ } => {
                                if let Some(ChatItem::Tool { done, .. }) = list.iter_mut().rev().find(|i| matches!(i, ChatItem::Tool { call_id: id, .. } if *id == call_id )) {
                                    *done = true;
                                }
                            }
                            UiStreamEvent::OutputItemDoneMessage { text } => {
                                append_to_assistant(list, &format!("\n{}", text));
                                if let Some(ChatItem::Assistant { streaming, .. }) = list.last_mut() { *streaming = false; }
                            }
                            UiStreamEvent::Completed { _response_id: _, token_usage } => {
                                if let Some(ChatItem::Assistant { streaming, .. }) = list.last_mut() { *streaming = false; }
                                if let Some(ChatItem::Tool { done, .. }) = list.iter_mut().rev().find(|i| matches!(i, ChatItem::Tool { .. })) { *done = true; }
                                if let Some(tu) = token_usage { token_setter.set(Some(tu)); }
                            }
                            UiStreamEvent::Raw { .. } => {}
                            UiStreamEvent::SystemNote { .. } => { /* do not render in transcript */ }
                            UiStreamEvent::ReasoningDelta { text } => {
                                append_to_reasoning(list, &text);
                                reasoning_streamed.set(true);
                            }
                            UiStreamEvent::ReasoningBreak { .. } => {
                                list.push(ChatItem::Reasoning { text: String::new() });
                            }
                            UiStreamEvent::ReasoningSummary { text } => {
                                // Suppress end-of-turn summary if deltas already streamed this turn
                                if !reasoning_streamed.get() {
                                    list.push(ChatItem::Reasoning { text });
                                }
                            }
                            UiStreamEvent::SessionConfigured { .. } => { /* handled above */ }
                            UiStreamEvent::TaskUpdate { .. } => { /* future: reflect in master task panel */ }
                        }
                    });
                }
            }) as Box<dyn FnMut(JsValue)>);

            let _ = JsFuture::from(tauri_event_listen("codex:stream", cb.as_ref().unchecked_ref())).await;
            cb.forget(); // keep listener for the app lifetime
        });
    }
    spawn_local(async move {
        let f = fetch_full_status().await;
        full_setter.set(f);
    });
    {
        let chats_setter = chats.write_only();
        spawn_local(async move { chats_setter.set(fetch_recent_chats(30).await); });
    }
    {
        let tasks_setter = tasks.write_only();
        spawn_local(async move { tasks_setter.set(tasks_list().await); });
    }
    {
        let bottom = bottom_ref.clone();
        let items_ro = items.read_only();
        Effect::new(move |_| {
            let _ = items_ro.get().len();
            if let Some(el) = bottom.get() {
                use wasm_bindgen::JsCast;
                let e: web_sys::Element = el.unchecked_into();
                e.scroll_into_view();
            }
        });
    }
    {
        let raw_bottom = raw_bottom_ref.clone();
        let raw_ro = raw_events.read_only();
        let compact_ro = compact_logs.read_only();
        let show_raw_ro = show_raw_json.read_only();
        let raw_open_ro = raw_open.read_only();
        Effect::new(move |_| {
            let _ = raw_ro.get().len();
            let _ = compact_ro.get().len();
            let open = raw_open_ro.get();
            if open {
                if let Some(el) = raw_bottom.get() {
                    use wasm_bindgen::JsCast;
                    let e: web_sys::Element = el.unchecked_into();
                    e.scroll_into_view();
                }
            }
        });
    }

    view! {
        <div class="h-screen w-full">
            <div class="fixed top-0 left-0 bottom-0 w-80 p-3 border-r border-zinc-700 bg-white/5 flex flex-col overflow-hidden">
                <div class="text-lg mb-2">"OpenAgents"</div>
                <button class="text-xs underline text-white/80 hover:text-white cursor-pointer self-start mb-2"
                        on:click=move |_| {
                            items.set(Vec::new());
                            chat_title.set("New chat".to_string());
                            let mode = session_mode.get();
                            spawn_local(async move {
                                let _ = JsFuture::from(tauri_invoke("configure_session_mode", serde_wasm_bindgen::to_value(&serde_json::json!({ "mode": mode })).unwrap_or(JsValue::UNDEFINED))).await;
                                let _ = JsFuture::from(tauri_invoke("new_chat_session", JsValue::UNDEFINED)).await;
                            });
                            if let Some(el) = input_ref.get() { let _ = el.focus(); }
                        }>
                    "New chat"
                </button>
                <button class="text-xs underline text-white/80 hover:text-white cursor-pointer self-start mb-1"
                        on:click=move |_| status_open.update(|v| *v = !*v)>
                    {move || if status_open.get() { "Hide status".to_string() } else { "Show status".to_string() }}
                </button>
                {move || if status_open.get() {
                    view! {
                        <div class="mb-2 max-h-112 overflow-auto border border-white/20 bg-black/30 p-2 text-[12px] leading-5">
                            <div class="font-semibold mb-1 opacity-95">"📂 Workspace"</div>
                            <div class="ml-2 opacity-90">{move || format!("• Path: {}", full.get().workspace.path.clone().unwrap_or_else(|| "(unknown)".into()))}</div>
                            <div class="ml-2 opacity-90">{move || format!("• Approval Mode: {}", full.get().workspace.approval_mode.clone().unwrap_or_else(|| "(default)".into()))}</div>
                            <div class="ml-2 opacity-90">{move || format!("• Sandbox: {}", full.get().workspace.sandbox.clone().unwrap_or_else(|| "(default)".into()))}</div>
                            <div class="ml-2 opacity-90">{move || {
                                let files = full.get().workspace.agents_files.clone();
                                if files.is_empty() { "• AGENTS files: none".to_string() } else { format!("• AGENTS files: {}", files.join(", ")) }
                            }}</div>

                            <div class="font-semibold mt-2 mb-1 opacity-95">"👤 Account"</div>
                            <div class="ml-2 opacity-90">{move || format!("• Signed in with {}", full.get().account.signed_in_with.clone().unwrap_or_else(|| "Not logged in".into()))}</div>
                            <div class="ml-2 opacity-90">{move || format!("• Login: {}", full.get().account.login.clone().unwrap_or_default())}</div>
                            <div class="ml-2 opacity-90">{move || format!("• Plan: {}", full.get().account.plan.clone().unwrap_or_default())}</div>

                            <div class="font-semibold mt-2 mb-1 opacity-95">"🧠 Model"</div>
                            <div class="ml-2 opacity-90">{move || format!("• Name: {}", full.get().model.name.clone().unwrap_or_else(|| "gpt-5".into()))}</div>
                            <div class="ml-2 opacity-90">{move || format!("• Provider: {}", full.get().model.provider.clone().unwrap_or_else(|| "OpenAI".into()))}</div>
                            <div class="ml-2 opacity-90">{move || format!("• Reasoning Effort: {}", full.get().model.reasoning_effort.clone().unwrap_or_else(|| "Medium".into()))}</div>
                            <div class="ml-2 opacity-90">{move || format!("• Reasoning Summaries: {}", full.get().model.reasoning_summaries.clone().unwrap_or_else(|| "Auto".into()))}</div>

                            <div class="font-semibold mt-2 mb-1 opacity-95">"💻 Client"</div>
                            <div class="ml-2 opacity-90">{move || format!("• CLI Version: {}", full.get().client.cli_version.clone().unwrap_or_else(|| "0.0.0".into()))}</div>

                            <div class="font-semibold mt-2 mb-1 opacity-95">"📊 Token Usage"</div>
                            <div class="ml-2 opacity-90">{move || format!("• Session ID: {}", full.get().token_usage.session_id.clone().unwrap_or_else(|| "(not started)".into()))}</div>
                            <div class="ml-2 opacity-90">{move || format!("• Input: {}", full.get().token_usage.input.unwrap_or(0))}</div>
                            <div class="ml-2 opacity-90">{move || format!("• Output: {}", full.get().token_usage.output.unwrap_or(0))}</div>
                            <div class="ml-2 opacity-90">{move || format!("• Total: {}", full.get().token_usage.total.unwrap_or(0))}</div>

                            <div class="font-semibold mt-2 mb-1 opacity-95">"⏱️ Usage Limits"</div>
                            <div class="ml-2 opacity-90">{move || format!("• {}", full.get().usage_limits.note.clone().unwrap_or_else(|| "Rate limit data not available yet.".into()))}</div>
                        </div>
                    }.into_any()
                } else { view!{ <div></div> }.into_any() }}

                <button class="text-xs underline text-white/80 hover:text-white cursor-pointer self-start"
                        on:click=move |_| raw_open.update(|v| *v = !*v)>
                    {move || if raw_open.get() { "Hide event log".to_string() } else { "Show event log".to_string() }}
                </button>
                {move || if raw_open.get() {
                    view! {
                        <div class="mt-2 flex flex-col gap-2">
                            <div class="flex items-center gap-3 text-xs">
                                <label class="flex items-center gap-1"><input type="checkbox" prop:checked=move || show_raw_json.get() on:change=move |ev| { if let Some(i) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) { show_raw_json.set(i.checked()); } }/>"Raw JSON"</label>
                                <label class="flex items-center gap-1"><input type="checkbox" prop:checked=move || show_delta_logs.get() on:change=move |ev| { if let Some(i) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) { show_delta_logs.set(i.checked()); } }/>"Reasoning deltas"</label>
                                <label class="flex items-center gap-1"><input type="checkbox" prop:checked=move || show_token_logs.get() on:change=move |ev| { if let Some(i) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) { show_token_logs.set(i.checked()); } } disabled/>"Token counts"</label>
                                <button class="underline" on:click=move |_| { raw_events.set(vec![]); compact_logs.set(vec![]); } >"Clear"</button>
                            </div>
                            {if show_raw_json.get() {
                                view!{ <div class="flex-1 overflow-auto border border-white/20 bg-black/50 p-2">
                                    <pre class="text-[11px] leading-4 whitespace-pre-wrap">{raw_events.get().join("\n")}</pre>
                                    <div node_ref=raw_bottom_ref class="h-0"></div>
                                </div> }.into_any()
                            } else {
                                view!{ <div class="flex-1 overflow-auto border border-white/20 bg-black/50 p-2">
                                    <pre class="text-[11px] leading-4 whitespace-pre-wrap">{compact_logs.get().join("\n")}</pre>
                                    <div node_ref=raw_bottom_ref class="h-0"></div>
                                </div> }.into_any()
                            }}
                        </div>
                    }.into_any()
                } else { view! { <div class="mt-2"></div> }.into_any() }}

                <div class="mt-2 overflow-hidden flex-1 flex flex-col">
                    <div class="mb-2">
                        <div class="text-sm opacity-90 mb-1">"Master Tasks"</div>
                        <div class="flex items-center gap-2 mb-2">
                            { // new task inline
                                let tasks_setter = tasks.write_only();
                                let sel_setter = selected_task.write_only();
                                let sel_detail = selected_task_detail.write_only();
                                view!{
                                    <button class="text-xs underline text-white/80 hover:text-white cursor-pointer"
                                            on:click=move |_| {
                                                let tasks_setter = tasks_setter.clone();
                                                let sel_setter = sel_setter.clone();
                                                let sel_detail = sel_detail.clone();
                                                let items = items.write_only();
                                                spawn_local(async move {
                                                    match task_create("New Master Task").await {
                                                        Ok(t) => {
                                                            sel_setter.set(Some(t.id.clone()));
                                                            sel_detail.set(Some(t.clone()));
                                                            tasks_setter.set(tasks_list().await);
                                                        }
                                                        Err(err) => {
                                                            items.update(|list| list.push(ChatItem::System { text: format!("Failed to create task: {}", err) }));
                                                        }
                                                    }
                                                });
                                            }>"New task"</button>
                                }
                            }
                        </div>
                        { // task list
                            let tasks_ro = tasks.read_only();
                            let sel_setter = selected_task.write_only();
                            let sel_detail = selected_task_detail.write_only();
                            view!{
                                <div class="max-h-52 overflow-auto border border-zinc-700 bg-black/30">
                                    {move || tasks_ro.get().iter().map(|m| {
                                        let id = m.id.clone();
                                        let name = m.name.clone();
                                        let status = m.status.clone();
                                        view!{ <button class="w-full text-left px-2 py-1 border-b border-white/10 hover:bg-white/10 cursor-pointer"
                                            on:click=move |_| {
                                                let id2 = id.clone();
                                                let sel_setter = sel_setter.clone();
                                                let sel_detail = sel_detail.clone();
                                                spawn_local(async move {
                                                    sel_setter.set(Some(id2.clone()));
                                                    sel_detail.set(task_get(&id2).await);
                                                });
                                            }>
                                            <span class="text-[12px] truncate">{name}</span>
                                            <span class="text-[10px] opacity-70 float-right">{status}</span>
                                        </button> }.into_any() }).collect_view()}
                                </div>
                            }
                        }
                    </div>
                    <button class="text-xs underline text-white/80 hover:text-white cursor-pointer self-start mb-1"
                            on:click=move |_| chats_open.update(|v| *v = !*v)>
                        {move || if chats_open.get() { "Hide chats".to_string() } else { "Show chats".to_string() }}
                    </button>
                    {move || if chats_open.get() {
                        view! {
                            <div class="flex-1 overflow-auto border border-white/20 bg-black/30">
                                {move || chats.get().iter().map(|c| {
                                    let path = c.path.clone();
                                    let title = c.title.clone();
                                    let started = c.started_at.clone();
                                    view!{
                                        <button class="w-full text-left px-2 py-1 border-b border-white/10 hover:bg-white/10 cursor-pointer"
                                                on:click=move |_| {
                                                    let items_setter = items.write_only();
                                                    let path_clone = path.clone();
                                                    spawn_local(async move {
                                                        let loaded = fetch_chat_items(path_clone.clone()).await;
                                                        items_setter.set({
                                                            let mut v: Vec<ChatItem> = Vec::new();
                                                            for it in loaded.into_iter() {
                                                                match it {
                                                                    UiDisplayItem::User { text } => v.push(ChatItem::User { text }),
                                                                    UiDisplayItem::Assistant { text } => v.push(ChatItem::Assistant { text, streaming: false }),
                                                                    UiDisplayItem::Reasoning { text } => v.push(ChatItem::Reasoning { text }),
                                                                    UiDisplayItem::Tool { title, text } => v.push(ChatItem::Tool { call_id: String::new(), title, segments: vec![(text, false)], done: true }),
                                                                    UiDisplayItem::Instructions { ikind, text } => {
                                                                        let label = if ikind == "environment_context" { "context".to_string() } else { "instructions".to_string() };
                                                                        v.push(ChatItem::Collapsible { label, text });
                                                                    }
                                                                    UiDisplayItem::Empty => {}
                                                                }
                                                            }
                                                            v
                                                        });
                                                    });
                                                    chat_title.set(title.clone());
                                                }>
                                            <div class="truncate text-[12px]">{title.clone()}</div>
                                            <div class="text-[11px] opacity-70 truncate">{started}</div>
                                        </button>
                                    }.into_any()
                                }).collect_view()}
                            </div>
                        }.into_any()
                    } else { view!{ <div></div> }.into_any() }}
                </div>
            </div>

            <div class="pl-80 pt-6 pb-36 h-full overflow-auto">
                <div class="mx-auto w-full max-w-[768px] px-4">
                    <div class="flex items-center justify-between gap-3 mb-4">
                        <div class="text-sm opacity-90 truncate">{move || chat_title.get()}</div>
                        <div class="flex items-center gap-3">
                            <label class="text-xs opacity-80">"Reasoning"</label>
                            { // selector
                                let reasoning = reasoning;
                                view! {
                                    <select class="text-xs text-white bg-black border border-white rounded-none px-2 py-1 cursor-pointer appearance-none focus:outline-none"
                                            prop:value=move || reasoning.get()
                                            on:change=move |ev| {
                                                if let Some(sel) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok()) {
                                                    let val = sel.value();
                                                    reasoning.set(val.clone());
                                                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "effort": val.to_lowercase() })).unwrap_or(JsValue::UNDEFINED);
                                                    let _ = tauri_invoke("set_reasoning_effort", args);
                                                }
                                            }>
                                        <option>Minimal</option>
                                        <option>Low</option>
                                        <option>Medium</option>
                                        <option>High</option>
                                    </select>
                                }
                            }
                            <label class="text-xs opacity-80">"Mode"</label>
                            { // mode selector
                                let session_mode = session_mode;
                                view! {
                                    <select class="text-xs text-white bg-black border border-white rounded-none px-2 py-1 cursor-pointer appearance-none focus:outline-none"
                                            prop:value=move || session_mode.get()
                                            on:change=move |ev| {
                                                if let Some(sel) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlSelectElement>().ok()) {
                                                    session_mode.set(sel.value());
                                                }
                                            }>
                                        <option>Chat</option>
                                        <option>Task</option>
                                    </select>
                                }
                            }
                        </div>
                    </div>
                </div>
                <div class="mx-auto w-full max-w-[768px] px-4 space-y-3 text-[13px]">
                    { // master task detail header + controls (reactive)
                        let selected_task_detail = selected_task_detail.clone();
                        let tasks = tasks.clone();
                        { move || match selected_task_detail.get() {
                            Some(t) => {
                                let id = t.id.clone();
                                let name = t.name.clone();
                                let status = t.status.clone();
                                let updated = t.updated_at.clone();
                                let id_run = id.clone();
                                let id_pause = id.clone();
                                let id_cancel = id.clone();
                                let sel_detail_sig = selected_task_detail.write_only();
                                let tasks_setter = tasks.write_only();
                                view!{ <div class="w-full p-3 border border-zinc-700 bg-black/20 mb-3">
                                    <div class="flex items-center justify-between">
                                        <div class="text-sm opacity-90">{format!("{} [{}]", name, status)}</div>
                                        <div class="text-xs opacity-70">{format!("Updated: {}", updated)}</div>
                                    </div>
                                    <div class="flex items-center gap-3 mt-2 text-xs">
                                        <div class="opacity-80">"Allow writes"</div>
                                        {
                                            let id_toggle = id.clone();
                                            let sel_task_detail = sel_detail_sig.clone();
                                            let tasks_setter2 = tasks_setter.clone();
                                            let is_writable = t.autonomy_budget.sandbox.to_lowercase() != "read-only";
                                            view!{
                                                <input type="checkbox" prop:checked=is_writable on:change=move |ev| {
                                                    let checked = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()).map(|i| i.checked()).unwrap_or(false);
                                                    let id2 = id_toggle.clone();
                                                    let sel_task_detail = sel_task_detail.clone();
                                                    let tasks_setter2 = tasks_setter2.clone();
                                                    spawn_local(async move {
                                                        let sandbox = if checked { "danger-full-access" } else { "read-only" };
                                                        let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id2, "sandbox": sandbox })).unwrap_or(JsValue::UNDEFINED);
                                                        if let Ok(v) = JsFuture::from(tauri_invoke("task_set_sandbox_cmd", args)).await {
                                                            if let Ok(task) = serde_wasm_bindgen::from_value::<Task>(v) {
                                                                sel_task_detail.set(Some(task));
                                                                tasks_setter2.set(tasks_list().await);
                                                            }
                                                        }
                                                    });
                                                } />
                                            }
                                        }
                                        <div class="opacity-60">{format!("Sandbox: {}", t.autonomy_budget.sandbox)}</div>
                                    </div>
                                    <div class="flex items-center gap-3 mt-2">
                                        <button class="text-xs underline text-white/80 hover:text-white cursor-pointer" on:click={
                                            let id_capture = id_run.clone();
                                            let sel_task_detail = sel_detail_sig.clone();
                                            let tasks_setter = tasks_setter.clone();
                                            move |_| {
                                                let id2 = id_capture.clone();
                                                spawn_local(async move {
                                                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id2 })).unwrap_or(JsValue::UNDEFINED);
                                                    if let Ok(v) = JsFuture::from(tauri_invoke("task_run_cmd", args)).await {
                                                        if let Ok(task) = serde_wasm_bindgen::from_value::<Task>(v) {
                                                            sel_task_detail.set(Some(task));
                                                            tasks_setter.set(tasks_list().await);
                                                        }
                                                    }
                                                });
                                            }}>
                                            "Run next"
                                        </button>
                                        <button class="text-xs underline text-white/80 hover:text-white cursor-pointer" on:click={
                                            let id_capture = id_pause.clone();
                                            let sel_task_detail = sel_detail_sig.clone();
                                            move |_| {
                                                let id2 = id_capture.clone();
                                                spawn_local(async move {
                                                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id2 })).unwrap_or(JsValue::UNDEFINED);
                                                    let _ = JsFuture::from(tauri_invoke("task_pause_cmd", args)).await;
                                                    if let Some(t) = task_get(&id2).await { sel_task_detail.set(Some(t)); }
                                                });
                                            }}>
                                            "Pause"
                                        </button>
                                        <button class="text-xs underline text-white/80 hover:text-white cursor-pointer" on:click={
                                            let id_capture = id_cancel.clone();
                                            let sel_task_detail = sel_detail_sig.clone();
                                            move |_| {
                                                let id2 = id_capture.clone();
                                                spawn_local(async move {
                                                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id2 })).unwrap_or(JsValue::UNDEFINED);
                                                    let _ = JsFuture::from(tauri_invoke("task_cancel_cmd", args)).await;
                                                    if let Some(t) = task_get(&id2).await { sel_task_detail.set(Some(t)); }
                                                });
                                            }}>
                                            "Cancel"
                                        </button>
                                    </div>
                                    { // queue list (simple static snapshot)
                                        let text = t.queue.iter().map(|s| format!("[{}] {} — {}", s.status, s.id, s.title)).collect::<Vec<_>>().join("\n");
                                        view!{ <pre class="mt-3 p-2 text-xs border border-white/15 bg-black/20 max-h-56 overflow-auto whitespace-pre-wrap">{text}</pre> }.into_any()
                                    }
                                </div> }.into_any()
                            }
                            None => view!{ <div></div> }.into_any(),
                        } }
                    }
                    {move || items.get().into_iter().map(|item| match item {
                        ChatItem::User { text } => {
                            let html = md_to_html(&text);
                            view! { <div class="w-full p-3 border border-white/50 bg-black/20"><div class="assistant-md" inner_html=html></div></div> }.into_any()
                        }
                        ChatItem::Reasoning { text } => {
                            let html = md_to_html(&text);
                            view! { <div class="w-full text-[12px] italic opacity-85 px-3 reasoning-md" inner_html=html></div> }.into_any()
                        }
                        ChatItem::Assistant { text, streaming } => {
                            let html = md_to_html(&text);
                            view! { <div class="w-full p-3 border border-white/40 bg-white/10"><div class="assistant-md" inner_html=html></div>{if streaming { " ▌" } else { "" }.to_string()}</div> }.into_any()
                        }
                        ChatItem::Collapsible { label, text } => {
                            let open: RwSignal<bool> = RwSignal::new(false);
                            let html = md_to_html(&text);
                            view! {
                                <div class="w-full p-3 border border-white/30 bg-black/20">
                                    <button class="text-xs underline text-white/80 hover:text-white cursor-pointer"
                                            on:click=move |_| open.update(|v| *v = !*v)>
                                        {move || if open.get() { format!("Hide {}", label) } else { format!("Show {}", label) }}
                                    </button>
                                    {move || if open.get() { view!{ <div class="mt-2 assistant-md" inner_html=html.clone()></div> }.into_any() } else { view!{ <div></div> }.into_any() }}
                                </div>
                            }.into_any()
                        }
                        ChatItem::Tool { call_id: _, title, segments, done } => view! {
                            <div class="w-full p-3 border border-white/30 bg-black/40">
                                <div class="text-xs opacity-80 mb-1">{format!("{} {}", title, if done {"(done)"} else {"(running)"})}</div>
                                <pre class="whitespace-pre-wrap text-sm">
                                    {move || segments.iter().map(|(t, is_err)| {
                                        let cls = if *is_err { "text-red-400" } else { "text-white" };
                                        view!{ <span class={cls}>{t.clone()}</span> }.into_any()
                                    }).collect_view()}
                                </pre>
                            </div>
                        }.into_any(),
                        ChatItem::System { text } => view! { <div class="text-xs opacity-60">{text}</div> }.into_any(),
                    }).collect_view()}
                    <div node_ref=bottom_ref class="h-0"></div>
                </div>
                
            </div>

            // Chat bar
            <div class="fixed bottom-0 left-80 right-0 z-50">
                <div class="w-full bg-black border-t border-white/20">
                    <div class="mx-auto w-full max-w-[768px] px-4 py-3 flex gap-2">
                    { // input state
                        let msg: RwSignal<String> = RwSignal::new(String::new());
                        let send = {
                            let items = items.write_only();
                            let msg_get = msg.read_only();
                            let reasoning_streamed = reasoning_streamed;
                            let input_ref = input_ref.clone();
                            let session_mode = session_mode.read_only();
                            let tasks_setter = tasks.write_only();
                            let sel_setter = selected_task.write_only();
                            let sel_detail = selected_task_detail.write_only();
                            move || {
                                let text = msg_get.get();
                                if text.is_empty() { return; }
                                if session_mode.get() == "Task" {
                                    // Route to Master Task: create + plan + run
                                    let items2 = items.clone();
                                    let goal = text.clone();
                                    spawn_local(async move {
                                        // Create a quick task
                                        match task_create(&format!("Quick: {}", goal)).await {
                                            Ok(t) => {
                                                let id = t.id.clone();
                                                sel_setter.set(Some(id.clone()));
                                                sel_detail.set(Some(t));
                                                // Plan subtasks from goal
                                                let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id.clone(), "goal": goal })).unwrap_or(JsValue::UNDEFINED);
                                                let _ = JsFuture::from(tauri_invoke("task_plan_cmd", args)).await;
                                                // Run first pending
                                                let args2 = serde_wasm_bindgen::to_value(&serde_json::json!({ "id": id.clone() })).unwrap_or(JsValue::UNDEFINED);
                                                let _ = JsFuture::from(tauri_invoke("task_run_cmd", args2)).await;
                                                tasks_setter.set(tasks_list().await);
                                            }
                                            Err(err) => { items2.update(|list| list.push(ChatItem::System { text: format!("Failed to create task: {}", err) })); }
                                        }
                                    });
                                } else {
                                    // Standard chat
                                    items.update(|list| list.push(ChatItem::User { text: text.clone() }));
                                    reasoning_streamed.set(false);
                                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "prompt": text.clone() })).unwrap_or(JsValue::UNDEFINED);
                                    let items2 = items.clone();
                                    spawn_local(async move {
                                        match JsFuture::from(tauri_invoke("submit_chat", args)).await {
                                            Ok(_) => {}
                                            Err(e) => {
                                                let err = js_sys::JSON::stringify(&e).ok().and_then(|v| v.as_string()).unwrap_or_else(|| "invoke error".into());
                                                items2.update(|list| list.push(ChatItem::System { text: format!("submit_chat failed: {}", err) }));
                                            }
                                        }
                                    });
                                }
                                msg.set(String::new());
                                if let Some(el) = input_ref.get() { let _ = el.focus(); }
                            }
                        };
                        view! {
                            <input
                                node_ref=input_ref
                                prop:autofocus=true
                                class="flex-1 px-3 py-2 border border-white bg-black text-white placeholder-white/60 focus:outline-none"
                                type="text"
                                placeholder="Type a command or message…"
                                prop:value=move || msg.get()
                                on:input=move |ev| {
                                    if let Some(t) = ev.target().and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok()) {
                                        msg.set(t.value())
                                    }
                                }
                                on:keydown=move |ev| { if ev.key() == "Enter" { send(); } }
                            />
                            <button
                                class="px-3 py-2 border border-white bg-black hover:bg-black cursor-pointer"
                                on:click=move |_| send()
                                type="button"
                            >"Send"</button>
                        }
                    }
                    </div>
                </div>
            </div>
        </div>
    }
}
