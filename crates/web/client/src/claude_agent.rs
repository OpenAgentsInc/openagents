use std::cell::RefCell;
use std::rc::Rc;

use openagents_relay::{ClaudeChunk, ClaudeRequest, ChunkType, RelayMessage, ToolDefinition};
use serde::Deserialize;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::state::{AppState, ClaudeToolRequest};
use crate::utils::copy_to_clipboard;

#[derive(Deserialize)]
struct RegisterResponse {
    session_id: String,
    tunnel_url: String,
    browser_url: String,
}

pub(crate) fn start_claude_chat(state: Rc<RefCell<AppState>>, repo: String) {
    {
        let mut guard = state.borrow_mut();
        guard.claude_chat.show();
        guard.claude_chat.clear();
        guard.claude_state.reset();
        guard.claude_state.repo = Some(repo.clone());
        guard.claude_chat.push_system_message("Starting Claude tunnel...");
        guard.claude_chat.set_status("registering tunnel");
    }

    let state_clone = state.clone();
    wasm_bindgen_futures::spawn_local(async move {
        match register_tunnel(&repo).await {
            Ok(response) => {
                let connect_command = format!(
                    "openagents pylon connect --tunnel-url {}",
                    response.tunnel_url
                );

                {
                    let mut guard = state_clone.borrow_mut();
                    guard.claude_state.tunnel_session_id = Some(response.session_id);
                    guard.claude_state.tunnel_url = Some(response.tunnel_url.clone());
                    guard.claude_state.browser_url = Some(response.browser_url.clone());
                    guard.claude_state.connect_command = Some(connect_command.clone());
                    guard.claude_chat.set_connect_command(Some(connect_command.clone()));
                    guard.claude_chat.push_system_message(
                        "Tunnel registered. Start the local connector to continue.",
                    );
                    guard.claude_chat.set_status("waiting for tunnel");
                }

                if let Err(err) = connect_websocket(state_clone.clone(), response.browser_url) {
                    let mut guard = state_clone.borrow_mut();
                    guard
                        .claude_chat
                        .push_error_message(&format!("WebSocket error: {}", err));
                    guard.claude_chat.set_status("relay error");
                }
            }
            Err(err) => {
                let mut guard = state_clone.borrow_mut();
                guard
                    .claude_chat
                    .push_error_message(&format!("Tunnel registration failed: {}", err));
                guard.claude_chat.set_status("registration failed");
            }
        }
    });
}

pub(crate) fn send_prompt(state: Rc<RefCell<AppState>>, prompt: String) {
    let (ws, session_id, repo, tunnel_connected) = {
        let guard = state.borrow();
        (
            guard.claude_state.ws.clone(),
            guard.claude_state.claude_session_id.clone(),
            guard.claude_state.repo.clone(),
            guard.claude_state.tunnel_connected,
        )
    };

    let Some(ws) = ws else {
        let mut guard = state.borrow_mut();
        guard
            .claude_chat
            .push_error_message("Tunnel is not connected yet.");
        return;
    };

    if !tunnel_connected {
        let mut guard = state.borrow_mut();
        guard
            .claude_chat
            .push_error_message("Waiting for tunnel connection.");
        return;
    }

    {
        let mut guard = state.borrow_mut();
        guard.claude_chat.push_user_message(&prompt);
        guard.claude_chat.push_streaming_assistant();
        guard.claude_state.streaming_text.clear();
    }

    let payload = if let Some(existing_session) = session_id {
        RelayMessage::ClaudePrompt {
            session_id: existing_session,
            content: prompt,
        }
    } else {
        let session_id = generate_session_id();
        let request = ClaudeRequest {
            model: String::new(),
            system_prompt: repo.map(|repo| format!(
                "You are Claude running locally for OpenAgents. Repository: {}",
                repo
            )),
            initial_prompt: Some(prompt),
            tools: Vec::<ToolDefinition>::new(),
            max_cost_usd: None,
            autonomy: None,
            approval_required_tools: None,
        };
        {
            let mut guard = state.borrow_mut();
            guard.claude_state.claude_session_id = Some(session_id.clone());
        }
        RelayMessage::ClaudeCreateSession { session_id, request }
    };

    if ws.send_with_str(&payload.to_json()).is_err() {
        let mut guard = state.borrow_mut();
        guard
            .claude_chat
            .push_error_message("Failed to send prompt over tunnel.");
    }
}

pub(crate) fn send_current_input(state: Rc<RefCell<AppState>>) {
    let prompt = {
        let mut guard = state.borrow_mut();
        guard.claude_chat.take_input()
    };
    if prompt.trim().is_empty() {
        return;
    }
    send_prompt(state, prompt);
}

pub(crate) fn respond_tool_approval(state: Rc<RefCell<AppState>>, approved: bool) {
    let (ws, pending) = {
        let guard = state.borrow();
        (
            guard.claude_state.ws.clone(),
            guard.claude_state.pending_tool.clone(),
        )
    };

    let Some(ws) = ws else {
        return;
    };

    let Some(request) = pending else {
        return;
    };

    let payload = RelayMessage::ClaudeToolApprovalResponse {
        session_id: request.session_id.clone(),
        approved,
    };

    if ws.send_with_str(&payload.to_json()).is_ok() {
        let mut guard = state.borrow_mut();
        guard.claude_state.pending_tool = None;
        guard.claude_chat.set_pending_tool_label(None);
    }
}

pub(crate) fn copy_connect_command(state: Rc<RefCell<AppState>>) {
    let command = {
        let guard = state.borrow();
        guard.claude_state.connect_command.clone()
    };

    if let Some(command) = command {
        copy_to_clipboard(command);
    }
}

async fn register_tunnel(repo: &str) -> Result<RegisterResponse, String> {
    let window = web_sys::window().ok_or("No window")?;
    let body = serde_json::json!({
        "repo": repo,
    });

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_credentials(web_sys::RequestCredentials::Include);

    let headers = web_sys::Headers::new().map_err(|e| format!("Headers error: {:?}", e))?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|e| format!("Header set error: {:?}", e))?;
    opts.set_headers(&headers);
    opts.set_body(&wasm_bindgen::JsValue::from_str(&body.to_string()));

    let request = web_sys::Request::new_with_str_and_init("/api/tunnel/register", &opts)
        .map_err(|e| format!("Request error: {:?}", e))?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("Fetch error: {:?}", e))?;

    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|_| "Response cast error".to_string())?;

    if !resp.ok() {
        let status = resp.status();
        let text = JsFuture::from(resp.text().map_err(|e| format!("Text error: {:?}", e))?)
            .await
            .map_err(|e| format!("Text parse error: {:?}", e))?;
        return Err(format!("HTTP {}: {:?}", status, text));
    }

    let json = JsFuture::from(resp.json().map_err(|e| format!("JSON error: {:?}", e))?)
        .await
        .map_err(|e| format!("JSON parse error: {:?}", e))?;

    serde_wasm_bindgen::from_value(json).map_err(|e| format!("Deserialize error: {:?}", e))
}

fn connect_websocket(state: Rc<RefCell<AppState>>, browser_url: String) -> Result<(), String> {
    let ws = web_sys::WebSocket::new(&browser_url)
        .map_err(|e| format!("WebSocket error: {:?}", e))?;

    let state_clone = state.clone();
    let onmessage = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(
        move |event: web_sys::MessageEvent| {
            if let Some(text) = event.data().as_string() {
                handle_ws_message(&state_clone, &text);
            }
        },
    );
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let state_clone = state.clone();
    let onopen = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
        let mut guard = state_clone.borrow_mut();
        guard.claude_chat.set_status("relay connected");
    });
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let state_clone = state.clone();
    let onclose = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |_event: web_sys::CloseEvent| {
        let mut guard = state_clone.borrow_mut();
        guard.claude_state.tunnel_connected = false;
        guard.claude_chat.set_status("relay disconnected");
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    let state_clone = state.clone();
    let onerror = wasm_bindgen::closure::Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
        let mut guard = state_clone.borrow_mut();
        guard.claude_chat.set_status("relay error");
    });
    ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    {
        let mut guard = state.borrow_mut();
        guard.claude_state.ws = Some(ws);
    }

    Ok(())
}

fn handle_ws_message(state: &Rc<RefCell<AppState>>, text: &str) {
    if let Ok(message) = RelayMessage::from_json(text) {
        handle_relay_message(state, message);
        return;
    }

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        if let Some(msg_type) = value.get("type").and_then(|v| v.as_str()) {
            match msg_type {
                "tunnel_connected" => {
                    let mut guard = state.borrow_mut();
                    guard.claude_state.tunnel_connected = true;
                    guard.claude_chat.set_status("tunnel connected");
                }
                "tunnel_disconnected" => {
                    let mut guard = state.borrow_mut();
                    guard.claude_state.tunnel_connected = false;
                    guard.claude_chat.set_status("tunnel disconnected");
                }
                _ => {}
            }
        }
    }
}

fn handle_relay_message(state: &Rc<RefCell<AppState>>, message: RelayMessage) {
    match message {
        RelayMessage::Ping { timestamp } => {
            let ws = { state.borrow().claude_state.ws.clone() };
            if let Some(ws) = ws {
                let _ = ws.send_with_str(&RelayMessage::Pong { timestamp }.to_json());
            }
        }
        RelayMessage::TunnelConnected { capabilities, .. } => {
            let mut guard = state.borrow_mut();
            guard.claude_state.tunnel_connected = true;
            let status = if capabilities.is_empty() {
                "tunnel connected".to_string()
            } else {
                format!("tunnel connected ({})", capabilities.join(", "))
            };
            guard.claude_chat.set_status(status);
        }
        RelayMessage::TunnelDisconnected { .. } => {
            let mut guard = state.borrow_mut();
            guard.claude_state.tunnel_connected = false;
            guard.claude_chat.set_status("tunnel disconnected");
        }
        RelayMessage::ClaudeSessionCreated { session_id } => {
            let mut guard = state.borrow_mut();
            guard.claude_state.claude_session_id = Some(session_id);
            guard.claude_chat.set_status("session ready");
        }
        RelayMessage::ClaudeChunk { chunk } => {
            handle_claude_chunk(state, chunk);
        }
        RelayMessage::ClaudeToolApproval {
            session_id,
            tool,
            params,
        } => {
            let mut guard = state.borrow_mut();
            guard.claude_state.pending_tool = Some(ClaudeToolRequest {
                session_id,
                tool: tool.clone(),
                params: params.clone(),
            });
            guard
                .claude_chat
                .push_system_message(&format!("Tool approval needed: {}", tool));
            guard
                .claude_chat
                .set_pending_tool_label(Some(format!("Approve tool: {}", tool)));
        }
        RelayMessage::ClaudeError { error, .. } => {
            let mut guard = state.borrow_mut();
            guard
                .claude_chat
                .push_error_message(&format!("Claude error: {}", error));
            guard.claude_chat.set_status("error");
        }
        RelayMessage::Error { message, .. } => {
            let mut guard = state.borrow_mut();
            guard
                .claude_chat
                .push_error_message(&format!("Relay error: {}", message));
        }
        _ => {}
    }
}

fn handle_claude_chunk(state: &Rc<RefCell<AppState>>, chunk: ClaudeChunk) {
    let mut guard = state.borrow_mut();
    match chunk.chunk_type {
        ChunkType::Text => {
            if let Some(delta) = chunk.delta {
                guard.claude_state.streaming_text.push_str(&delta);
                guard
                    .claude_chat
                    .update_last_assistant(&guard.claude_state.streaming_text);
            }
        }
        ChunkType::ToolStart => {
            if let Some(tool) = chunk.tool {
                guard
                    .claude_chat
                    .push_tool_message(&format!("{} started", tool.name));
            }
        }
        ChunkType::ToolOutput => {
            if let Some(tool) = chunk.tool {
                if let Some(error) = tool.error {
                    guard
                        .claude_chat
                        .push_tool_message(&format!("{} error: {}", tool.name, error));
                    return;
                }
                let details = tool
                    .result
                    .as_ref()
                    .map(format_tool_value)
                    .unwrap_or_else(|| "tool output".to_string());
                guard
                    .claude_chat
                    .push_tool_message(&format!("{} output: {}", tool.name, details));
            }
        }
        ChunkType::ToolDone => {
            if let Some(tool) = chunk.tool {
                if let Some(error) = tool.error {
                    guard
                        .claude_chat
                        .push_tool_message(&format!("{} error: {}", tool.name, error));
                    return;
                }
                let details = tool
                    .result
                    .as_ref()
                    .map(format_tool_value)
                    .unwrap_or_else(|| "done".to_string());
                guard
                    .claude_chat
                    .push_tool_message(&format!("{} done: {}", tool.name, details));
            }
        }
        ChunkType::Done => {
            guard.claude_chat.set_status("complete");
            if let Some(delta) = chunk.delta {
                guard.claude_chat.push_system_message(&delta);
            }
        }
        ChunkType::Error => {
            let message = chunk
                .delta
                .unwrap_or_else(|| "Claude error".to_string());
            guard.claude_chat.push_error_message(&message);
            guard.claude_chat.set_status("error");
        }
    }
}

fn format_tool_value(value: &serde_json::Value) -> String {
    if let Some(text) = value.as_str() {
        truncate_text(text, 240)
    } else {
        truncate_text(&value.to_string(), 240)
    }
}

fn truncate_text(text: &str, max_len: usize) -> String {
    if text.len() <= max_len {
        text.to_string()
    } else {
        format!("{}...", &text[..max_len])
    }
}

fn generate_session_id() -> String {
    let now = js_sys::Date::now() as u64;
    let rand = (js_sys::Math::random() * 1_000_000.0) as u64;
    format!("claude-{}-{}", now, rand)
}
