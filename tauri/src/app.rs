use leptos::task::spawn_local;
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use web_sys::{MessageEvent, WebSocket};

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "core"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
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
    let (messages, set_messages) = signal::<Vec<serde_json::Value>>(vec![]);
    let (convex_status, set_convex_status) = signal::<Option<ConvexStatus>>(None);
    let (bridge_status, set_bridge_status) = signal::<Option<BridgeStatus>>(None);

    // Connect to the local bridge websocket with retry/backoff
    {
        let set_ws_connected = set_ws_connected.clone();
        let set_convex_status = set_convex_status.clone();
        let set_bridge_status = set_bridge_status.clone();
        fn schedule<F: 'static + FnOnce()>(delay_ms: i32, f: F) {
            if let Some(win) = web_sys::window() {
                let cb = Closure::once_into_js(Box::new(f) as Box<dyn FnOnce()>);
                let _ = win.set_timeout_with_callback_and_timeout_and_arguments_0(cb.as_ref().unchecked_ref(), delay_ms);
                // cb drops after timeout fires
            }
        }
        fn connect(set_ws_connected: WriteSignal<bool>, set_convex_status: WriteSignal<Option<ConvexStatus>>, set_bridge_status: WriteSignal<Option<BridgeStatus>>, attempt: u32) {
            let ws = match WebSocket::new("ws://127.0.0.1:8787/ws") { Ok(ws) => ws, Err(_) => {
                // retry later
                schedule((1000 * attempt.min(10)) as i32, move || connect(set_ws_connected, set_convex_status, set_bridge_status, attempt.saturating_add(1)));
                return;
            }};
            // onopen
            {
                let set_ws_connected = set_ws_connected.clone();
                let ws_clone = ws.clone();
                let onopen = Closure::wrap(Box::new(move || {
                    set_ws_connected.set(true);
                    let _ = ws_clone.send_with_str("{\"control\":\"convex.status\"}");
                    let _ = ws_clone.send_with_str("{\"control\":\"bridge.status\"}");
                }) as Box<dyn Fn()>);
                ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
                onopen.forget();
            }
            // onmessage
            {
                let set_convex_status = set_convex_status.clone();
                let set_bridge_status = set_bridge_status.clone();
                let onmessage = Closure::wrap(Box::new(move |e: MessageEvent| {
                    if let Ok(txt) = e.data().dyn_into::<js_sys::JsString>() {
                        let s: String = txt.into();
                        if let Ok(v) = serde_json::from_str::<JsonValue>(&s) {
                            if let Some(t) = v.get("type").and_then(|x| x.as_str()) {
                                match t {
                                    "bridge.convex_status" => {
                                        let healthy = v.get("healthy").and_then(|x| x.as_bool()).unwrap_or(false);
                                        let url = v.get("url").and_then(|x| x.as_str()).unwrap_or("").to_string();
                                        set_convex_status.set(Some(ConvexStatus { healthy, url }));
                                    }
                                    "bridge.status" => {
                                        let bs: BridgeStatus = serde_json::from_value(v).unwrap_or_default();
                                        set_bridge_status.set(Some(bs));
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }) as Box<dyn FnMut(MessageEvent)>);
                ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
                onmessage.forget();
            }
            // onclose/onerror → retry
            {
                let set_ws_connected_c = set_ws_connected.clone();
                let retry = move || {
                    set_ws_connected_c.set(false);
                    schedule(1000, move || connect(set_ws_connected_c, set_convex_status, set_bridge_status, 1));
                };
                let onclose = Closure::wrap(Box::new(retry) as Box<dyn Fn()>);
                ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
                onclose.forget();
                let set_ws_connected_e = set_ws_connected.clone();
                let set_convex_status_e = set_convex_status.clone();
                let set_bridge_status_e = set_bridge_status.clone();
                let onerror = Closure::wrap(Box::new(move || {
                    set_ws_connected_e.set(false);
                    schedule(1000, move || connect(set_ws_connected_e, set_convex_status_e, set_bridge_status_e, 1));
                }) as Box<dyn Fn()>);
                ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
                onerror.forget();
            }
        }
        connect(set_ws_connected, set_convex_status, set_bridge_status, 1);
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

    // When a thread is selected, load its messages
    let on_select_thread = {
        let set_selected_thread_id = set_selected_thread_id.clone();
        let set_messages = set_messages.clone();
        move |tid: String| {
            set_selected_thread_id.set(Some(tid.clone()));
            spawn_local(async move {
                let args = serde_wasm_bindgen::to_value(&serde_json::json!({ "thread_id": tid, "limit": 400 })).unwrap();
                let res = invoke("list_messages_for_thread", args).await;
                if let Ok(v) = serde_wasm_bindgen::from_value::<Vec<serde_json::Value>>(res) {
                    set_messages.set(v);
                }
            });
        }
    };

    view! {
        <main class="app">
            <aside class="sidebar">
                <div class="status-list">
                    <div class="status-row">
                        <span class={ move || if ws_connected.get() { "dot dot-ok" } else { "dot dot-bad" } }></span>
                        <span>"Bridge WS"</span>
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
                    <div class="thread-list">
                        { move || threads.get().into_iter().map(|row| {
                            let tid = row.get("thread_id").and_then(|x| x.as_str()).map(|s| s.to_string())
                                .or_else(|| row.get("threadId").and_then(|x| x.as_str()).map(|s| s.to_string()))
                                .or_else(|| row.get("id").and_then(|x| x.as_str()).map(|s| s.to_string()))
                                .unwrap_or_default();
                            let title = row.get("title").and_then(|x| x.as_str()).unwrap_or("").to_string();
                            let is_selected = selected_thread_id.get().as_deref() == Some(&tid);
                            view! {
                                <div class={ move || if is_selected { "thread-item selected" } else { "thread-item" } }
                                     on:click={
                                        let tid = tid.clone();
                                        let on_select_thread = on_select_thread.clone();
                                        move |_| on_select_thread(tid.clone())
                                     }>
                                    <div class="thread-title">{ title.clone() }</div>
                                </div>
                            }
                        }).collect::<Vec<_>>() }
                    </div>
                </div>
            </aside>
            <section class="content">
                <div class="messages">
                    { move || messages.get().into_iter().map(|m| {
                        let role = m.get("role").and_then(|x| x.as_str()).unwrap_or("assistant");
                        let text = m.get("text").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let cls = if role == "user" { "msg user" } else { "msg assistant" };
                        view! {
                            <div class={ cls }><div class="bubble">{ text }</div></div>
                        }
                    }).collect::<Vec<_>>() }
                </div>
            </section>
        </main>
    }
}
