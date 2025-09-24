use leptos::prelude::*;
use wasm_bindgen::JsCast;
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

// ---- Streaming UI types ----
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(tag = "kind")]
enum UiStreamEvent {
    #[default]
    Created,
    OutputTextDelta { text: String },
    ToolDelta { call_id: String, chunk: String },
    OutputItemDoneMessage { text: String },
    Completed { response_id: Option<String>, token_usage: Option<TokenUsageLite> },
    Raw { json: String },
    SystemNote { text: String },
}

#[derive(Clone, Debug, Default, Deserialize)]
struct TokenUsageLite { input: u64, output: u64, total: u64 }

#[derive(Clone, Debug)]
enum ChatItem {
    User { text: String },
    Assistant { text: String, streaming: bool },
    Tool { call_id: String, output: String, done: bool },
    System { text: String },
}

fn append_to_assistant(list: &mut Vec<ChatItem>, s: &str) {
    if let Some(last) = list.last_mut() {
        if let ChatItem::Assistant { text, .. } = last { text.push_str(s); return; }
    }
    list.push(ChatItem::Assistant { text: s.to_string(), streaming: true });
}

fn append_tool_chunk(list: &mut Vec<ChatItem>, call_id: &str, chunk: &str) {
    if let Some(ChatItem::Tool { output, .. }) = list.iter_mut().rev().find(|i| matches!(i, ChatItem::Tool { call_id: id, .. } if id == call_id )) {
        output.push_str(chunk);
    } else {
        list.push(ChatItem::Tool { call_id: call_id.to_string(), output: chunk.to_string(), done: false });
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Load auth status on mount
    let full: RwSignal<FullStatus> = RwSignal::new(Default::default());
    let full_setter = full.write_only();
    // Default hidden
    let panel_open: RwSignal<bool> = RwSignal::new(false);

    // Chat state
    let items: RwSignal<Vec<ChatItem>> = RwSignal::new(vec![]);
    let token_usage_sig: RwSignal<Option<TokenUsageLite>> = RwSignal::new(None);
    let raw_events: RwSignal<Vec<String>> = RwSignal::new(vec![]);
    let raw_open: RwSignal<bool> = RwSignal::new(false);

    // Install event listener once (on mount)
    {
        let items_setter = items.write_only();
        let token_setter = token_usage_sig.write_only();
        let raw_setter = raw_events.write_only();
        spawn_local(async move {
            let cb = Closure::wrap(Box::new(move |evt: JsValue| {
                let payload = js_sys::Reflect::get(&evt, &JsValue::from_str("payload")).unwrap_or(JsValue::NULL);
                if payload.is_null() || payload.is_undefined() { return; }
                let parsed: Result<UiStreamEvent, _> = serde_wasm_bindgen::from_value(payload);
                if let Ok(ev) = parsed {
                    if let UiStreamEvent::Raw { json } = &ev {
                        raw_setter.update(|v| v.push(json.clone()));
                    }
                    items_setter.update(|list| {
                        match ev {
                            UiStreamEvent::Created => {
                                list.push(ChatItem::System { text: "Turn started".into() });
                            }
                            UiStreamEvent::OutputTextDelta { text } => {
                                append_to_assistant(list, &text);
                            }
                            UiStreamEvent::ToolDelta { call_id, chunk } => {
                                append_tool_chunk(list, &call_id, &chunk);
                            }
                            UiStreamEvent::OutputItemDoneMessage { text } => {
                                append_to_assistant(list, &format!("\n{}", text));
                                if let Some(ChatItem::Assistant { streaming, .. }) = list.last_mut() { *streaming = false; }
                            }
                            UiStreamEvent::Completed { response_id, token_usage } => {
                                if let Some(ChatItem::Assistant { streaming, .. }) = list.last_mut() { *streaming = false; }
                                if let Some(ChatItem::Tool { done, .. }) = list.iter_mut().rev().find(|i| matches!(i, ChatItem::Tool { .. })) { *done = true; }
                                if let Some(tu) = token_usage.clone() { token_setter.set(Some(tu)); }
                                if let Some(id) = response_id { list.push(ChatItem::System { text: format!("Completed: {id}") }); }
                                if let Some(tu) = token_usage { list.push(ChatItem::System { text: format!("Tokens in/out/total: {} / {} / {}", tu.input, tu.output, tu.total) }); }
                            }
                            UiStreamEvent::Raw { .. } => {}
                            UiStreamEvent::SystemNote { text } => { list.push(ChatItem::System { text }); }
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

    view! {
        <div class="h-screen w-full">
            <div class="fixed top-0 left-0 bottom-0 w-56 p-3 border-r border-white bg-white/5">
                <div class="text-lg mb-2">"OpenAgents"</div>
            </div>

            <div class="pl-56 pr-[26rem] pt-4 pb-28 h-full overflow-auto">
                <div class="space-y-3 text-[13px]">
                    {move || items.get().into_iter().map(|item| match item {
                        ChatItem::User { text } => view! { <div class="max-w-3xl p-3 border border-white/50 bg-black/20">{text}</div> }.into_any(),
                        ChatItem::Assistant { text, streaming } => view! { <div class="max-w-3xl p-3 border border-white/40 bg-white/10">{text}{if streaming { " ▌" } else { "" }.to_string()}</div> }.into_any(),
                        ChatItem::Tool { call_id, output, done } => view! { <div class="max-w-3xl p-3 border border-white/30 bg-black/40"><div class="text-xs opacity-70 mb-1">{format!("Tool {call_id} {}", if done {"(done)"} else {"(running)"})}</div><pre class="whitespace-pre-wrap text-sm">{output}</pre></div> }.into_any(),
                        ChatItem::System { text } => view! { <div class="text-xs opacity-60">{text}</div> }.into_any(),
                    }).collect_view()}
                </div>
                <div class="mt-4 max-w-3xl">
                    <button class="text-xs underline text-white/80 hover:text-white cursor-pointer" on:click=move |_| raw_open.update(|v| *v = !*v)>
                        {move || if raw_open.get() { "Hide raw event log".to_string() } else { "Show raw event log".to_string() }}
                    </button>
                    {move || if raw_open.get() {
                        view!{ <pre class="mt-2 max-h-72 overflow-auto text-[11px] leading-4 whitespace-pre-wrap border border-white/20 p-2 bg-black/50">{raw_events.get().join("\n")}</pre> }.into_any()
                    } else { view!{ <div></div> }.into_any() }}
                </div>
            </div>

            <button
                class="fixed top-2 right-3 z-10 text-sm underline text-white/80 hover:text-white cursor-pointer focus:outline-none"
                on:click=move |_| {
                    panel_open.update(|v| *v = !*v);
                }
                type="button"
            >
                {move || if panel_open.get() { "Hide status".to_string() } else { "Show status".to_string() }}
            </button>

            <div class=move || if panel_open.get() { "fixed top-12 right-3 bottom-3 w-96 overflow-auto p-3 border border-white rounded-none bg-black text-white text-[0.95rem] leading-6 z-50".to_string() } else { "hidden".to_string() }>
                <div class="space-y-1.5 mb-3">
                    <div class="font-semibold mb-1 opacity-95">"📂 Workspace"</div>
                    <div class="ml-2 opacity-90">{move || format!("• Path: {}", full.get().workspace.path.unwrap_or_else(|| "(unknown)".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Approval Mode: {}", full.get().workspace.approval_mode.unwrap_or_else(|| "(default)".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Sandbox: {}", full.get().workspace.sandbox.unwrap_or_else(|| "(default)".into()))}</div>
                    <div class="ml-2 opacity-90">{move || {
                        let files = full.get().workspace.agents_files;
                        if files.is_empty() { "• AGENTS files: none".to_string() } else { format!("• AGENTS files: {}", files.join(", ")) }
                    }}</div>
                </div>

                <div class="space-y-1.5 mb-3">
                    <div class="font-semibold mb-1 opacity-95">"👤 Account"</div>
                    <div class="ml-2 opacity-90">{move || format!("• Signed in with {}", full.get().account.signed_in_with.unwrap_or_else(|| "Not logged in".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Login: {}", full.get().account.login.unwrap_or_default())}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Plan: {}", full.get().account.plan.unwrap_or_default())}</div>
                </div>

                <div class="space-y-1.5 mb-3">
                    <div class="font-semibold mb-1 opacity-95">"🧠 Model"</div>
                    <div class="ml-2 opacity-90">{move || format!("• Name: {}", full.get().model.name.unwrap_or_else(|| "gpt-5".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Provider: {}", full.get().model.provider.unwrap_or_else(|| "OpenAI".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Reasoning Effort: {}", full.get().model.reasoning_effort.unwrap_or_else(|| "Medium".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Reasoning Summaries: {}", full.get().model.reasoning_summaries.unwrap_or_else(|| "Auto".into()))}</div>
                </div>

                <div class="space-y-1.5 mb-3">
                    <div class="font-semibold mb-1 opacity-95">"💻 Client"</div>
                    <div class="ml-2 opacity-90">{move || format!("• CLI Version: {}", full.get().client.cli_version.unwrap_or_else(|| "0.0.0".into()))}</div>
                </div>

                <div class="space-y-1.5 mb-3">
                    <div class="font-semibold mb-1 opacity-95">"📊 Token Usage"</div>
                    <div class="ml-2 opacity-90">{move || format!("• Session ID: {}", full.get().token_usage.session_id.unwrap_or_else(|| "(not started)".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Input: {}", full.get().token_usage.input.unwrap_or(0))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Output: {}", full.get().token_usage.output.unwrap_or(0))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Total: {}", full.get().token_usage.total.unwrap_or(0))}</div>
                </div>

                <div class="space-y-1.5 mb-1">
                    <div class="font-semibold mb-1 opacity-95">"⏱️ Usage Limits"</div>
                    <div class="ml-2 opacity-90">{move || format!("• {}", full.get().usage_limits.note.unwrap_or_else(|| "Rate limit data not available yet.".into()))}</div>
                </div>
            </div>

            // Chat bar
            <div class="fixed bottom-0 left-0 right-0 flex justify-center pb-4 z-10">
                <div class="w-full max-w-[600px] px-4 flex gap-2">
                    { // input state
                        let msg: RwSignal<String> = RwSignal::new(String::new());
                        let send = {
                            let items = items.write_only();
                            let msg_get = msg.read_only();
                            move || {
                                let text = msg_get.get();
                                if !text.is_empty() {
                                    items.update(|list| list.push(ChatItem::User { text: text.clone() }));
                                    items.update(|list| list.push(ChatItem::Assistant { text: String::new(), streaming: true }));
                                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "prompt": text })).unwrap_or(JsValue::UNDEFINED);
                                    let _ = tauri_invoke("submit_chat", args);
                                    msg.set(String::new());
                                }
                            }
                        };
                        view! {
                            <input
                                class="flex-1 px-3 py-2 border border-white bg-transparent text-white placeholder-white/50 focus:outline-none"
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
                                class="px-3 py-2 border border-white bg-white/10 hover:bg-white/20 cursor-pointer"
                                on:click=move |_| send()
                                type="button"
                            >"Send"</button>
                        }
                    }
                </div>
            </div>
        </div>
    }
}
