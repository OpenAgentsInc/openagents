#[cfg(feature = "jsonl_components")]
use crate::jsonl::{JsonlMessage, MessageRow};
#[cfg(feature = "jsonl_components")]
use crate::library::{LibraryContent, LibraryPage, LibrarySidebar};
#[cfg(not(feature = "jsonl_components"))]
#[derive(Clone, Debug, PartialEq, serde::Deserialize)]
struct MessageRow {
    #[allow(dead_code)] id: Option<String>,
    role: Option<String>,
    kind: Option<String>,
    text: Option<String>,
    #[allow(dead_code)] data: Option<serde_json::Value>,
    ts: f64,
}
#[cfg(not(feature = "jsonl_components"))]
impl MessageRow {
    fn stable_key(&self) -> String {
        let ts_bits = self.ts.to_bits();
        format!("{}-{ts_bits}", self.kind.clone().unwrap_or_else(|| "message".into()))
    }
}
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{MessageEvent, WebSocket};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "event"], js_name = listen)]
    async fn tauri_listen(event: &str, handler: &js_sys::Function) -> JsValue;
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct ConvexStatus { healthy: bool, url: String }

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct BridgeStatus {
    bind: String,
    codex_pid: Option<u32>,
    last_thread_id: Option<String>,
    convex_url: String,
    convex_healthy: bool,
}

#[component]
pub fn App() -> impl IntoView {
    let (ws_connected, set_ws_connected) = signal(false);
    let (threads, set_threads) = signal::<Vec<serde_json::Value>>(vec![]);
    let (selected_thread_id, set_selected_thread_id) = signal::<Option<String>>(None);
    let (selected_thread_doc_id, set_selected_thread_doc_id) = signal::<Option<String>>(None);
    let (messages, set_messages) = signal::<Vec<MessageRow>>(vec![]);
    let (convex_status, set_convex_status) = signal::<Option<ConvexStatus>>(None);
    let (bridge_status, set_bridge_status) = signal::<Option<BridgeStatus>>(None);
    use std::rc::Rc; use std::cell::Cell;
    let local_convex_override = Rc::new(Cell::new(false));
    // Ref to the composer input for autofocus on thread switches
    let composer_ref = NodeRef::new();

    // Connect to the local bridge websocket only after backend signals readiness (reduces failed attempts)
    {
        let set_ws_connected = set_ws_connected.clone();
        let set_convex_status = set_convex_status.clone();
        let set_bridge_status = set_bridge_status.clone();
        // Prefer local sidecar status (3210). Once we receive it, ignore bridge convex status.
        let local_convex_seen = local_convex_override.clone();
        fn schedule<F: 'static + FnOnce()>(delay_ms: i32, f: F) { if let Some(win) = web_sys::window() { let cb = Closure::once_into_js(Box::new(f) as Box<dyn FnOnce()>); let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), delay_ms); } }
        fn connect(set_ws_connected: WriteSignal<bool>, set_convex_status: WriteSignal<Option<ConvexStatus>>, set_bridge_status: WriteSignal<Option<BridgeStatus>>, attempt: u32, local_convex_seen: Rc<Cell<bool>>) {
            let ws = match WebSocket::new("ws://127.0.0.1:8787/ws") { Ok(ws) => ws, Err(_) => { let delay = (250 * (attempt as i32)).min(1500); let local_pref = local_convex_seen.clone(); schedule(delay, move || connect(set_ws_connected, set_convex_status, set_bridge_status, attempt.saturating_add(1), local_pref)); return; }};
            let scheduled = Rc::new(Cell::new(false));
            // onopen
            { let set_ws_connected = set_ws_connected.clone(); let ws_clone = ws.clone(); let onopen = Closure::wrap(Box::new(move || { set_ws_connected.set(true); if let Some(win) = web_sys::window() { let _ = js_sys::Reflect::set(&win, &JsValue::from_str("__BRIDGE_WS"), &JsValue::from(ws_clone.clone())); } /* don't request bridge convex.status; prefer local sidecar */ let _ = ws_clone.send_with_str("{\"control\":\"bridge.status\"}"); }) as Box<dyn Fn()>); ws.set_onopen(Some(onopen.as_ref().unchecked_ref())); onopen.forget(); }
            // onmessage
            { let set_convex_status = set_convex_status.clone(); let set_bridge_status = set_bridge_status.clone(); let local_pref = local_convex_seen.clone(); let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| { if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() { let s: String = txt.into(); if let Ok(v) = serde_json::from_str::<JsonValue>(&s) { if let Some(t) = v.get("type").and_then(|x| x.as_str()) { match t { "bridge.convex_status" => { if !local_pref.get() { let healthy = v.get("healthy").and_then(|x| x.as_bool()).unwrap_or(false); let url = v.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string(); set_convex_status.set(Some(ConvexStatus { healthy, url })); } } "bridge.status" => { let bs: BridgeStatus = serde_json::from_value(v).unwrap_or_default(); set_bridge_status.set(Some(bs)); } _ => {} } } } } }) as Box<dyn FnMut(MessageEvent)>); ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref())); onmessage.forget(); }
            // onclose/onerror → single retry
            { let set_ws_connected_c = set_ws_connected.clone(); let scheduled_flag = scheduled.clone(); let local_pref = local_convex_seen.clone(); let onclose = Closure::wrap(Box::new(move || { if scheduled_flag.replace(true) { return; } set_ws_connected_c.set(false); if let Some(win) = web_sys::window() { let _ = js_sys::Reflect::set(&win, &JsValue::from_str("__BRIDGE_WS"), &JsValue::UNDEFINED); } let lp = local_pref.clone(); schedule(500, move || connect(set_ws_connected_c, set_convex_status, set_bridge_status, 2, lp)); }) as Box<dyn Fn()>); ws.set_onclose(Some(onclose.as_ref().unchecked_ref())); onclose.forget(); let set_ws_connected_e = set_ws_connected.clone(); let set_convex_status_e = set_convex_status.clone(); let set_bridge_status_e = set_bridge_status.clone(); let scheduled_flag_err = scheduled.clone(); let local_pref_err = local_convex_seen.clone(); let onerror = Closure::wrap(Box::new(move || { if scheduled_flag_err.replace(true) { return; } set_ws_connected_e.set(false); if let Some(win) = web_sys::window() { let _ = js_sys::Reflect::set(&win, &JsValue::from_str("__BRIDGE_WS"), &JsValue::UNDEFINED); } let lp = local_pref_err.clone(); schedule(500, move || connect(set_ws_connected_e, set_convex_status_e, set_bridge_status_e, 2, lp)); }) as Box<dyn Fn()>); ws.set_onerror(Some(onerror.as_ref().unchecked_ref())); onerror.forget(); }
        }
        // Listen for backend readiness
        let handler = Closure::wrap(Box::new({ let set_ws_connected = set_ws_connected.clone(); let set_convex_status = set_convex_status.clone(); let set_bridge_status = set_bridge_status.clone(); let local_pref = local_convex_seen.clone(); move |_e: JsValue| { let lp = local_pref.clone(); connect(set_ws_connected, set_convex_status, set_bridge_status, 1, lp); } }) as Box<dyn FnMut(JsValue)>);
        let _ = tauri_listen("bridge:ready", handler.as_ref().unchecked_ref());
        handler.forget();
        // Safety fallback: if no event in 1s, attempt once (no reactive read to avoid warnings)
        let attempted = Rc::new(Cell::new(false));
        let attempted_clone = attempted.clone();
        schedule(1000, { let set_ws_connected = set_ws_connected.clone(); let set_convex_status = set_convex_status.clone(); let set_bridge_status = set_bridge_status.clone(); let local_pref = local_convex_seen.clone(); move || { if !attempted_clone.replace(true) { let lp = local_pref.clone(); connect(set_ws_connected, set_convex_status, set_bridge_status, 1, lp); } } });
    }

    // Fetch recent threads on mount
    {
        let set_threads = set_threads.clone();
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "limit": 10 })).unwrap();
            let res = invoke("list_recent_threads", args).await;
            // Expect an array
            let arr = js_sys::Array::from(&res);
            if arr.length() > 0 {
                // Convert JsValue -> serde_json::Value
                if let Ok(v) = serde_wasm_bindgen::from_value::<Vec<serde_json::Value>>(res.clone()) {
                    set_threads.set(v);
                }
            } else {
                // try parse non-array fallback
                if let Ok(v) = serde_wasm_bindgen::from_value::<Vec<serde_json::Value>>(res) {
                    set_threads.set(v);
                }
            }
        });
    }

    // Live subscription for recent threads (filtered to count>0). Also get updates when first message appears.
    {
        let set_threads = set_threads.clone();
        spawn_local(async move {
            let handler = Closure::wrap(Box::new(move |e: JsValue| {
                if let Ok(payload) = js_sys::Reflect::get(&e, &JsValue::from_str("payload")) {
                    if let Ok(rows) = serde_wasm_bindgen::from_value::<Vec<serde_json::Value>>(payload) {
                        // Debug: log counts to console
                        web_sys::console::log_1(&JsValue::from_str(&format!("threads batch {} items", rows.len())));
                        if let Some(first) = rows.get(0) {
                            let count = first.get("count").and_then(|x| x.as_i64())
                                .or_else(|| first.get("messageCount").and_then(|x| x.as_i64()))
                                .or_else(|| first.get("message_count").and_then(|x| x.as_i64()))
                                .unwrap_or(-1);
                            web_sys::console::log_1(&JsValue::from_str(&format!("first row count={}", count)));
                        }
                        set_threads.set(rows);
                    }
                }
            }) as Box<dyn FnMut(JsValue)>);
            let _unlisten = tauri_listen("convex:threads", handler.as_ref().unchecked_ref()).await;
            handler.forget();
        });
        // Start server-side subscription
        spawn_local(async move {
            let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "limit": 10 })).unwrap();
            let _ = invoke("subscribe_recent_threads", args).await;
        });
    }

    // Listen for live message updates from Tauri backend subscription
    {
        let set_messages = set_messages.clone();
        spawn_local(async move {
            let handler = Closure::wrap(Box::new(move |e: JsValue| {
                if let Ok(payload) = js_sys::Reflect::get(&e, &JsValue::from_str("payload")) {
                    if let Ok(rows) = serde_wasm_bindgen::from_value::<Vec<MessageRow>>(payload) {
                        set_messages.set(rows);
                    }
                }
            }) as Box<dyn FnMut(JsValue)>);
            let _unlisten = tauri_listen("convex:messages", handler.as_ref().unchecked_ref()).await;
            handler.forget();
        });
    }

    // Listen for local convex status emitted by Tauri (sidecar on 3210)
    {
        let set_convex_status = set_convex_status.clone();
        let local_pref = local_convex_override.clone();
        spawn_local(async move {
            // Initial probe in case the event fired before we subscribed
            let initial = invoke("get_local_convex_status", JsValue::NULL).await;
            if let Ok(status) = serde_wasm_bindgen::from_value::<ConvexStatus>(initial.clone()) {
                local_pref.set(status.healthy);
                set_convex_status.set(Some(status));
            }
            let handler = Closure::wrap(Box::new(move |e: JsValue| {
                if let Ok(payload) = js_sys::Reflect::get(&e, &JsValue::from_str("payload")) {
                    if let Ok(status) = serde_wasm_bindgen::from_value::<ConvexStatus>(payload) {
                        local_pref.set(true);
                        set_convex_status.set(Some(status));
                    }
                }
            }) as Box<dyn FnMut(JsValue)>);
            let _unlisten = tauri_listen("convex:local_status", handler.as_ref().unchecked_ref()).await;
            handler.forget();
        });
    }

    // When a thread is selected, load its messages
    let on_select_thread = {
        let set_selected_thread_id = set_selected_thread_id.clone();
        let set_selected_thread_doc_id = set_selected_thread_doc_id.clone();
        let set_messages = set_messages.clone();
        move |tid: String, doc_id_opt: Option<String>| {
            set_selected_thread_id.set(Some(tid.clone()));
            set_selected_thread_doc_id.set(doc_id_opt.clone());
            set_messages.set(vec![]);
            let tid_fetch = tid.clone();
            let doc_id_fetch = doc_id_opt.clone();
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "threadId": tid_fetch, "docId": doc_id_fetch, "limit": 400 })).unwrap();
                let res = invoke("list_messages_for_thread", args).await;
                if let Ok(v) = serde_wasm_bindgen::from_value::<Vec<MessageRow>>(res) {
                    set_messages.set(v);
                }
            });
            // Start live subscription; updates are delivered via 'convex:messages' events
            let tid_sub = tid.clone();
            let doc_id_sub = doc_id_opt.clone();
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "threadId": tid_sub, "docId": doc_id_sub, "limit": 400 })).unwrap();
                let _ = invoke("subscribe_thread_messages", args).await;
            });
            // Autofocus composer (query by class for robustness)
            if let Some(doc) = web_sys::window().and_then(|w| w.document()) {
                if let Ok(Some(el)) = doc.query_selector(".compose-input") {
                    let _ = el.dyn_into::<web_sys::HtmlElement>().map(|e| e.focus());
                } else {
                    // delayed attempt if not mounted yet
                    if let Some(win) = web_sys::window() {
                        let cb = Closure::once_into_js(Box::new(move || {
                            if let Some(doc2) = web_sys::window().and_then(|w| w.document()) {
                                if let Ok(Some(el2)) = doc2.query_selector(".compose-input") {
                                    let _ = el2.dyn_into::<web_sys::HtmlElement>().map(|e| e.focus());
                                }
                            }
                        }) as Box<dyn FnOnce()>);
                        let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), 60);
                    }
                }
            }
        }
    };

    // Local tab state: App vs Components (Library)
    #[cfg(feature = "jsonl_components")]
    let (show_library, set_show_library) = signal(false);
    #[cfg(feature = "jsonl_components")]
    let (lib_page, set_lib_page) = signal(LibraryPage::Markdown);

    // Helper: render messages list (feature-aware)
    #[cfg(feature = "jsonl_components")]
    fn render_messages(messages: ReadSignal<Vec<MessageRow>>) -> impl IntoView {
        view! {
            <For
                each=move || messages.get()
                key=|row: &MessageRow| row.stable_key()
                children=move |row| { view! { <JsonlMessage row=row /> } }
            />
        }
    }
    #[cfg(not(feature = "jsonl_components"))]
    fn render_messages(messages: ReadSignal<Vec<MessageRow>>) -> impl IntoView {
        view! {
            <For
                each=move || messages.get()
                key=|row: &MessageRow| row.stable_key()
                children=move |row| {
                    let role = row.role.clone().unwrap_or_else(|| "assistant".into());
                    let text = row.text.clone().unwrap_or_default();
                    let cls = if role == "user" { "msg user" } else { "msg assistant" };
                    view! { <div class=cls><div class="bubble">{text}</div></div> }
                }
            />
        }
    }

    // Shared: status + threads block
    let status_threads = {
        let ws_connected = ws_connected;
        let convex_status = convex_status;
        let bridge_status = bridge_status;
        let threads = threads;
        let selected_thread_id = selected_thread_id;
        let on_select_thread = on_select_thread.clone();
        // create thread (subscription will update list; no auto-select)
        let create_new_thread = move || {
            spawn_local(async move {
                let _ = invoke("create_thread", JsValue::NULL).await;
            });
        };
        move || -> AnyView {
            view! {
                <div class="status-list">
                    <div class="status-row">
                        <span class={ move || if ws_connected.get() { "dot dot-ok" } else { "dot dot-bad" } }></span>
                        <span>"Bridge"</span>
                        <span class="muted">{ move || if ws_connected.get() { "Connected".to_string() } else { "Disconnected".to_string() } }</span>
                    </div>
                    <div class="status-row">
                        <span class={ move || if convex_status.get().map(|s| s.healthy).unwrap_or(false) { "dot dot-ok" } else { "dot dot-bad" } }></span>
                        <span>"Convex"</span>
                        <span class="muted">{ move || convex_status.get().map(|s| s.url).unwrap_or_else(|| "".into()) }</span>
                    </div>
                    <div class="status-row">
                        <span class={ move || if bridge_status.get().and_then(|s| s.codex_pid).is_some() { "dot dot-ok" } else { "dot dot-bad" } }></span>
                        <span>"Codex"</span>
                        <span class="muted">{ move || bridge_status.get().and_then(|s| s.codex_pid).map(|p| format!("PID {p}" )).unwrap_or_else(|| "Not running".into()) }</span>
                    </div>
                </div>
                <div class="threads">
                    <div class="threads-title">"Recent Threads"</div>
                    <div class="thread-new">
                        <button class="compose-send" on:click=move |_| create_new_thread()>{"New"}</button>
                    </div>
                    <div class="thread-list">
                        { move || threads.get().into_iter().map(|row| {
                            let tid = row.get("thread_id").and_then(|x| x.as_str()).map(|s| s.to_string())
                                .or_else(|| row.get("threadId").and_then(|x| x.as_str()).map(|s| s.to_string()))
                                .or_else(|| row.get("id").and_then(|x| x.as_str()).map(|s| s.to_string()))
                                .unwrap_or_default();
                            let doc_id = row.get("id").and_then(|x| x.as_str()).map(|s| s.to_string());
                            let title = row.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            let updated = row.get("updated_at").and_then(|x| x.as_f64()).unwrap_or(0.0);
                            let ts = if updated > 0.0 { Some(updated) } else { None };
                            let count = row.get("count").and_then(|x| x.as_i64())
                                .or_else(|| row.get("messageCount").and_then(|x| x.as_i64()))
                                .or_else(|| row.get("message_count").and_then(|x| x.as_i64()));
                            let is_selected = selected_thread_id.get().as_deref() == Some(&tid);
                            view! {
                                <div class={ move || if is_selected { "thread-item selected" } else { "thread-item" } }
                                     on:click={
                                        let tid = tid.clone();
                                        let doc_id = doc_id.clone();
                                        let on_select_thread = on_select_thread.clone();
                                        move |_| on_select_thread(tid.clone(), doc_id.clone())
                                     }>
                                    <div class="thread-row-flex">
                                        <div class="thread-text">
                                            <div class="thread-title">{ title.clone() }</div>
                                            { ts.map(|ms| {
                                                let dt = js_sys::Date::new(&JsValue::from_f64(ms)).to_locale_string("en-US", &JsValue::UNDEFINED);
                                                let text = dt.as_string().unwrap_or_default();
                                                view! { <div class="thread-sub">{ text }</div> }
                                            }) }
                                        </div>
                                        { count.map(|c| view! { <div class="thread-badge">{ c }</div> }) }
                                    </div>
                                </div>
                            }
                        }).collect::<Vec<_>>() }
                    </div>
                </div>
            }.into_any()
        }
    };

    // Feature-aware: library tabs and body
    #[cfg(feature = "jsonl_components")]
    let sidebar_tabs: Option<AnyView> = Some(view! {
        <div class="threads" style="margin-bottom: 12px;">
            <div class="thread-list">
                <div class=move || if !show_library.get() { "thread-item selected" } else { "thread-item" }
                     on:click=move |_| set_show_library.set(false)>
                    <div class="thread-title">{"Messages"}</div>
                </div>
                <div class=move || if show_library.get() { "thread-item selected" } else { "thread-item" }
                     on:click=move |_| set_show_library.set(true)>
                    <div class="thread-title">{"Components"}</div>
                </div>
            </div>
        </div>
    }.into_any());
    #[cfg(not(feature = "jsonl_components"))]
    let sidebar_tabs: Option<AnyView> = None;

    #[cfg(feature = "jsonl_components")]
    let sidebar_body = {
        let lib_page = lib_page;
        move || -> AnyView {
            if show_library.get() {
                view! { <LibrarySidebar selected=lib_page on_select=set_lib_page /> }.into_any()
            } else {
                status_threads()
            }
        }
    };
    #[cfg(not(feature = "jsonl_components"))]
    let sidebar_body = move || -> AnyView { status_threads() };

    // Feature-aware: main content chooser
    #[cfg(feature = "jsonl_components")]
    let main_content = {
        let lib_page = lib_page;
        let messages_ro = messages;
        let _can_send = move || selected_thread_doc_id.get().is_some();
        let do_send = move |text: String| {
            // Persist via Convex mutation
            if let Some(doc_id) = selected_thread_doc_id.get() {
                let text2 = text.clone();
                let doc_id_clone = doc_id.clone();
                spawn_local(async move {
                    let args = serde_wasm_bindgen::to_value(&serde_json::json!({
                        "threadDocId": doc_id_clone,
                        "text": text2,
                        "role": "user",
                    })).unwrap();
                    let _ = invoke("enqueue_run", args).await;
                });
                // Send control message to bridge
                if let Some(win) = web_sys::window() {
                    if let Ok(v) = js_sys::Reflect::get(&win, &JsValue::from_str("__BRIDGE_WS")) {
                        if let Ok(ws) = v.dyn_into::<WebSocket>() {
                            let payload = serde_json::json!({
                                "control": "run.submit",
                                "threadDocId": doc_id,
                                "text": text,
                            });
                            let _ = ws.send_with_str(&payload.to_string());
                        }
                    }
                }
            }
        };
        move || -> AnyView {
            if show_library.get() {
                view! { <LibraryContent page=lib_page /> }.into_any()
            } else {
                view! {
                    <div class="messages">{ render_messages(messages_ro) }</div>
                    <div class="composer-wrap">
                        <crate::composer::ChatComposer on_send=do_send placeholder="Ask Codex".to_string() node_ref=composer_ref/>
                    </div>
                }.into_any()
            }
        }
    };
    #[cfg(not(feature = "jsonl_components"))]
    let main_content = {
        let messages_ro = messages;
        move || -> AnyView { view! { <div class="messages">{ render_messages(messages_ro) }</div> }.into_any() }
    };

    view! {
        <main class="app">
            <aside class="sidebar">
                { sidebar_tabs }
                { move || sidebar_body() }
            </aside>
            <section class="content">
                { move || main_content() }
            </section>
        </main>
    }
}
