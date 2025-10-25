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
    let (convex_status, set_convex_status) = signal::<Option<ConvexStatus>>(None);
    let (bridge_status, set_bridge_status) = signal::<Option<BridgeStatus>>(None);

    // Connect to the local bridge websocket and request statuses
    spawn_local(async move {
        // Try to open a websocket to the bridge
        let ws = match WebSocket::new("ws://127.0.0.1:8787/ws") {
            Ok(ws) => ws,
            Err(_) => return,
        };
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
        // onclose
        {
            let set_ws_connected = set_ws_connected.clone();
            let onclose = Closure::wrap(Box::new(move || {
                set_ws_connected.set(false);
            }) as Box<dyn Fn()>);
            ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
            onclose.forget();
        }
    });

    view! {
        <main class="container">
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
        </main>
    }
}
