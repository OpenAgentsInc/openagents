use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use wgpui::{
    Bounds, Button, ButtonVariant, Component, EventContext, EventResult, InputEvent,
    PaintContext, Point, Quad, Scene, TextInput, TextSystem, theme,
};
use wgpui::components::Text;
use wgpui::components::hud::{StatusBar, StatusItem, StatusItemAlignment};
use wgpui::components::organisms::{ThreadEntry, ThreadEntryType};
use wgpui::components::sections::{
    CodeDiff, CodeLine, CodeLineKind, CodePane, LastPrSummary, MetricsPane, TerminalLine,
    TerminalPane, TerminalStream, ThreadView, UsageSummary,
};

use crate::state::{AppState, AppView};
use crate::utils::js_optional_string;

/// Context from /repo/:owner/:repo route
#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct HudContext {
    pub(crate) username: String,
    pub(crate) repo: String,
    #[serde(default)]
    pub(crate) is_owner: bool,
    #[serde(default = "default_true")]
    pub(crate) is_public: bool,
    #[serde(default)]
    pub(crate) embed_mode: bool,
    #[serde(default)]
    pub(crate) agent_id: Option<String>,
    #[serde(default)]
    pub(crate) stream_url: Option<String>,
    #[serde(default)]
    pub(crate) session_id: Option<String>,
    #[serde(default)]
    pub(crate) ws_url: Option<String>,
    #[serde(default = "default_status")]
    pub(crate) status: String, // "idle", "starting", "running", "completed", "failed"
}

fn default_true() -> bool {
    true
}

fn default_status() -> String {
    "idle".to_string()
}

#[derive(Clone, Deserialize)]
pub(crate) struct LiveIssue {
    pub(crate) label: String,
    pub(crate) url: String,
    #[serde(default)]
    pub(crate) title: Option<String>,
}

#[derive(Clone, Deserialize)]
struct LiveHudResponse {
    enabled: bool,
    #[serde(default)]
    hud_context: Option<HudContext>,
    #[serde(default)]
    issue: Option<LiveIssue>,
}

#[derive(Clone)]
pub(crate) struct LandingLive {
    pub(crate) hud_context: HudContext,
    pub(crate) issue: Option<LiveIssue>,
}

/// HUD event types from streaming or replay.
#[derive(Clone, Debug)]
pub(crate) enum HudEvent {
    SessionStart { session_id: Option<String> },
    SessionEnd { success: Option<bool> },
    TickStart { tick_id: Option<String>, cause: Option<String> },
    TickEnd { tick_id: Option<String>, success: Option<bool> },
    ToolStart { tool_name: String, tool_id: Option<String> },
    ToolDone { tool_id: Option<String>, output: Option<String>, success: Option<bool> },
    Chunk { text: String },
    FileDiff { path: String, lines: Vec<String>, additions: Option<u64>, deletions: Option<u64> },
    ContainerOutput { stream: TerminalStream, data: String },
    Usage { input_tokens: Option<u64>, output_tokens: Option<u64>, cost_usd: Option<f64> },
    Error { error: String },
}

#[derive(Clone, Debug, Default)]
pub(crate) struct HudSettingsData {
    pub(crate) public: bool,
    pub(crate) embed_allowed: bool,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct HudLayout {
    pub(crate) thread_bounds: Bounds,
    pub(crate) code_bounds: Bounds,
    pub(crate) terminal_bounds: Bounds,
    pub(crate) metrics_bounds: Bounds,
    pub(crate) wallet_bounds: Bounds,
    pub(crate) start_form_bounds: Bounds,
    pub(crate) start_prompt_bounds: Bounds,
    pub(crate) start_button_bounds: Bounds,
    pub(crate) share_button_bounds: Bounds,
    pub(crate) share_panel_bounds: Bounds,
    pub(crate) copy_url_bounds: Bounds,
    pub(crate) copy_embed_bounds: Bounds,
    pub(crate) status_bounds: Bounds,
    pub(crate) settings_public_bounds: Bounds,
    pub(crate) settings_embed_bounds: Bounds,
}

#[derive(Clone, Copy)]
pub(crate) enum HudAction {
    StartAutopilot,
    ToggleSharePanel,
    CopyShareUrl,
    CopyEmbedCode,
}

#[derive(Clone, Copy)]
enum ShareNotice {
    Url,
    Embed,
}

pub(crate) struct HudUi {
    pub(crate) thread: ThreadView,
    pub(crate) code: CodePane,
    pub(crate) terminal: TerminalPane,
    pub(crate) metrics: MetricsPane,
    pub(crate) status_bar: StatusBar,
    pub(crate) assistant_entry: Option<usize>,
    pub(crate) assistant_text: String,
    pub(crate) tool_entries: HashMap<String, usize>,
    pub(crate) status_text: String,
    pub(crate) settings: HudSettingsData,
    pub(crate) start_prompt_input: TextInput,
    pub(crate) start_button: Button,
    pub(crate) share_button: Button,
    pub(crate) copy_url_button: Button,
    pub(crate) copy_embed_button: Button,
    pub(crate) share_panel_open: bool,
    pub(crate) share_url_copied: bool,
    pub(crate) embed_code_copied: bool,
    pub(crate) share_url_timer: Option<i32>,
    pub(crate) embed_code_timer: Option<i32>,
    pub(crate) event_ctx: EventContext,
    actions: Rc<RefCell<Vec<HudAction>>>,
}

impl HudUi {
    pub(crate) fn new() -> Self {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let start_actions_handle = actions.clone();
        let share_actions_handle = actions.clone();
        let copy_url_actions_handle = actions.clone();
        let copy_embed_actions_handle = actions.clone();
        let start_button = Button::new("Start Autopilot")
            .variant(ButtonVariant::Primary)
            .padding(12.0, 5.0)
            .on_click(move || {
                start_actions_handle
                    .borrow_mut()
                    .push(HudAction::StartAutopilot);
            });
        let share_button = Button::new("Share")
            .variant(ButtonVariant::Secondary)
            .padding(10.0, 4.0)
            .on_click(move || {
                share_actions_handle
                    .borrow_mut()
                    .push(HudAction::ToggleSharePanel);
            });
        let copy_url_button = Button::new("Copy URL")
            .variant(ButtonVariant::Secondary)
            .padding(10.0, 4.0)
            .on_click(move || {
                copy_url_actions_handle
                    .borrow_mut()
                    .push(HudAction::CopyShareUrl);
            });
        let copy_embed_button = Button::new("Copy Embed")
            .variant(ButtonVariant::Secondary)
            .padding(10.0, 4.0)
            .on_click(move || {
                copy_embed_actions_handle
                    .borrow_mut()
                    .push(HudAction::CopyEmbedCode);
            });
        let mut status_bar = StatusBar::new();
        status_bar.set_items(vec![
            StatusItem::text("status", "idle").left(),
            StatusItem::text("mode", "HUD").center(),
        ]);
        Self {
            thread: ThreadView::new().auto_scroll(true),
            code: CodePane::new().auto_scroll(true),
            terminal: TerminalPane::new().auto_scroll(true),
            metrics: MetricsPane::new(),
            status_bar,
            assistant_entry: None,
            assistant_text: String::new(),
            tool_entries: HashMap::new(),
            status_text: "idle".to_string(),
            settings: HudSettingsData {
                public: true,
                embed_allowed: true,
            },
            start_prompt_input: TextInput::new()
                .value(DEFAULT_AUTOPILOT_PROMPT)
                .placeholder("Autopilot prompt")
                .font_size(11.0)
                .padding(8.0, 5.0),
            start_button,
            share_button,
            copy_url_button,
            copy_embed_button,
            share_panel_open: false,
            share_url_copied: false,
            embed_code_copied: false,
            share_url_timer: None,
            embed_code_timer: None,
            event_ctx: EventContext::new(),
            actions,
        }
    }

    pub(crate) fn handle_event(
        &mut self,
        event: &InputEvent,
        layout: &HudLayout,
        show_start: bool,
        show_share: bool,
    ) -> EventResult {
        let mut handled = EventResult::Ignored;

        if show_share {
            handled = merge_event_result(
                handled,
                self.share_button
                    .event(event, layout.share_button_bounds, &mut self.event_ctx),
            );

            if self.share_panel_open {
                handled = merge_event_result(
                    handled,
                    self.copy_url_button
                        .event(event, layout.copy_url_bounds, &mut self.event_ctx),
                );
                handled = merge_event_result(
                    handled,
                    self.copy_embed_button
                        .event(event, layout.copy_embed_bounds, &mut self.event_ctx),
                );
            }
        } else {
            self.share_panel_open = false;
        }

        if show_start {
            handled = merge_event_result(
                handled,
                self.start_prompt_input.event(
                    event,
                    layout.start_prompt_bounds,
                    &mut self.event_ctx,
                ),
            );
            handled = merge_event_result(
                handled,
                self.start_button
                    .event(event, layout.start_button_bounds, &mut self.event_ctx),
            );
        } else {
            self.start_prompt_input.blur();
        }
        handled
    }

    pub(crate) fn take_actions(&self) -> Vec<HudAction> {
        let mut actions = self.actions.borrow_mut();
        std::mem::take(&mut *actions)
    }
}

pub(crate) enum HudStreamHandle {
    EventSource(web_sys::EventSource),
    WebSocket(web_sys::WebSocket),
}

impl HudStreamHandle {
    pub(crate) fn close(self) {
        match self {
            HudStreamHandle::EventSource(source) => {
                source.close();
            }
            HudStreamHandle::WebSocket(ws) => {
                let _ = ws.close();
            }
        }
    }
}

pub(crate) fn get_hud_context() -> Option<HudContext> {
    let window = web_sys::window()?;
    let context = js_sys::Reflect::get(&window, &"HUD_CONTEXT".into()).ok()?;

    if context.is_undefined() || context.is_null() {
        return None;
    }

    let username = js_sys::Reflect::get(&context, &"username".into())
        .ok()
        .and_then(|v| v.as_string())?;
    let repo = js_sys::Reflect::get(&context, &"repo".into())
        .ok()
        .and_then(|v| v.as_string())?;
    let is_owner = js_sys::Reflect::get(&context, &"is_owner".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let is_public = js_sys::Reflect::get(&context, &"is_public".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let embed_mode = js_sys::Reflect::get(&context, &"embed_mode".into())
        .ok()
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let agent_id = js_sys::Reflect::get(&context, &"agent_id".into())
        .ok()
        .and_then(|v| js_optional_string(&v));
    let stream_url = js_sys::Reflect::get(&context, &"stream_url".into())
        .ok()
        .and_then(|v| js_optional_string(&v));
    let session_id = js_sys::Reflect::get(&context, &"session_id".into())
        .ok()
        .and_then(|v| js_optional_string(&v));
    let ws_url = js_sys::Reflect::get(&context, &"ws_url".into())
        .ok()
        .and_then(|v| js_optional_string(&v));
    let status = js_sys::Reflect::get(&context, &"status".into())
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_else(|| "idle".to_string());

    Some(HudContext {
        username,
        repo,
        is_owner,
        is_public,
        embed_mode,
        agent_id,
        stream_url,
        session_id,
        ws_url,
        status,
    })
}

fn parse_hud_event(data: &str) -> Option<HudEvent> {
    if let Ok(obj) = serde_json::from_str::<serde_json::Value>(data) {
        let event_type = obj
            .get("event_type")
            .or_else(|| obj.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("chunk");

        return match event_type {
            "session_start" => Some(HudEvent::SessionStart {
                session_id: obj.get("session_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            }),
            "session_end" => Some(HudEvent::SessionEnd {
                success: obj.get("success").and_then(|v| v.as_bool()),
            }),
            "tick_start" => Some(HudEvent::TickStart {
                tick_id: obj.get("tick_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                cause: obj.get("cause").and_then(|v| v.as_str()).map(|s| s.to_string()),
            }),
            "tick_end" => Some(HudEvent::TickEnd {
                tick_id: obj.get("tick_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                success: obj.get("success").and_then(|v| v.as_bool()),
            }),
            "tool_start" => Some(HudEvent::ToolStart {
                tool_name: obj
                    .get("tool_name")
                    .or_else(|| obj.get("tool"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("tool")
                    .to_string(),
                tool_id: obj.get("tool_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            }),
            "tool_done" => Some(HudEvent::ToolDone {
                tool_id: obj.get("tool_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
                output: obj
                    .get("result")
                    .or_else(|| obj.get("output"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                success: obj
                    .get("success")
                    .and_then(|v| v.as_bool())
                    .or_else(|| obj.get("is_error").and_then(|v| v.as_bool()).map(|e| !e)),
            }),
            "chunk" => Some(HudEvent::Chunk {
                text: obj
                    .get("text")
                    .or_else(|| obj.pointer("/delta/text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            }),
            "file_diff" => Some(HudEvent::FileDiff {
                path: obj
                    .get("path")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string(),
                lines: collect_diff_lines(&obj),
                additions: obj.get("additions").and_then(|v| v.as_u64()),
                deletions: obj.get("deletions").and_then(|v| v.as_u64()),
            }),
            "container_output" => Some(HudEvent::ContainerOutput {
                stream: match obj.get("stream").and_then(|v| v.as_str()) {
                    Some("stderr") => TerminalStream::Stderr,
                    _ => TerminalStream::Stdout,
                },
                data: obj
                    .get("data")
                    .or_else(|| obj.get("text"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string(),
            }),
            "usage" => Some(HudEvent::Usage {
                input_tokens: obj.get("input_tokens").and_then(|v| v.as_u64()),
                output_tokens: obj.get("output_tokens").and_then(|v| v.as_u64()),
                cost_usd: obj
                    .get("cost_usd")
                    .or_else(|| obj.get("total_cost_usd"))
                    .and_then(|v| v.as_f64()),
            }),
            "status" => Some(HudEvent::SessionStart {
                session_id: obj.get("task_id").and_then(|v| v.as_str()).map(|s| s.to_string()),
            }),
            "done" => Some(HudEvent::SessionEnd { success: Some(true) }),
            "error" => Some(HudEvent::Error {
                error: obj
                    .get("message")
                    .or_else(|| obj.get("error"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("error")
                    .to_string(),
            }),
            _ => None,
        };
    }

    Some(HudEvent::Chunk {
        text: data.to_string(),
    })
}

fn collect_diff_lines(obj: &serde_json::Value) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(hunks) = obj.get("hunks").and_then(|v| v.as_array()) {
        for hunk in hunks {
            if let Some(hunk_lines) = hunk.get("lines").and_then(|v| v.as_array()) {
                for line in hunk_lines {
                    if let Some(text) = line.as_str() {
                        lines.push(text.to_string());
                    }
                }
            } else if let Some(text) = hunk.as_str() {
                lines.extend(text.lines().map(|l| l.to_string()));
            }
        }
    } else if let Some(diff) = obj.get("diff").and_then(|v| v.as_str()) {
        lines.extend(diff.lines().map(|l| l.to_string()));
    }
    lines
}

fn apply_hud_event(hud: &mut HudUi, event: HudEvent) {
    match event {
        HudEvent::SessionStart { session_id } => {
            let label = session_id
                .map(|id| format!("session {}", id))
                .unwrap_or_else(|| "session started".to_string());
            hud.status_text = label;
        }
        HudEvent::SessionEnd { success } => {
            hud.status_text = if success == Some(true) {
                "session complete".to_string()
            } else {
                "session ended".to_string()
            };
        }
        HudEvent::TickStart { tick_id, cause } => {
            let mut text = "tick start".to_string();
            if let Some(id) = tick_id {
                text = format!("{} {}", text, id);
            }
            if let Some(cause) = cause {
                text = format!("{} ({})", text, cause);
            }
            hud.thread
                .push_entry(ThreadEntry::new(ThreadEntryType::System, Text::new(text)));
        }
        HudEvent::TickEnd { tick_id, success } => {
            let mut text = "tick end".to_string();
            if let Some(id) = tick_id {
                text = format!("{} {}", text, id);
            }
            if let Some(success) = success {
                text = format!("{} {}", text, if success { "ok" } else { "fail" });
            }
            hud.thread
                .push_entry(ThreadEntry::new(ThreadEntryType::System, Text::new(text)));
        }
        HudEvent::ToolStart { tool_name, tool_id } => {
            let text = format!("tool start {}", tool_name);
            hud.thread
                .push_entry(ThreadEntry::new(ThreadEntryType::Tool, Text::new(text)));
            if let Some(id) = tool_id {
                let idx = hud.thread.entry_count().saturating_sub(1);
                hud.tool_entries.insert(id, idx);
            }
            hud.assistant_entry = None;
        }
        HudEvent::ToolDone { tool_id, output, success } => {
            let summary = output.unwrap_or_default();
            let suffix = if let Some(success) = success {
                if success { "ok" } else { "fail" }
            } else {
                "done"
            };
            if let Some(id) = tool_id {
                if let Some(idx) = hud.tool_entries.get(&id).copied() {
                    if let Some(entry) = hud.thread.entry_mut(idx) {
                        let content = format!("tool {} {}", suffix, summary);
                        entry.set_content(Text::new(content));
                        return;
                    }
                }
            }
            let content = format!("tool {} {}", suffix, summary);
            hud.thread
                .push_entry(ThreadEntry::new(ThreadEntryType::Tool, Text::new(content)));
        }
        HudEvent::Chunk { text } => {
            if text.is_empty() {
                return;
            }
            hud.assistant_text.push_str(&text);
            if let Some(idx) = hud.assistant_entry {
                if let Some(entry) = hud.thread.entry_mut(idx) {
                    entry.set_content(Text::new(hud.assistant_text.clone()));
                    return;
                }
            }
            hud.thread.push_entry(ThreadEntry::new(
                ThreadEntryType::Assistant,
                Text::new(hud.assistant_text.clone()),
            ));
            hud.assistant_entry = Some(hud.thread.entry_count().saturating_sub(1));
        }
        HudEvent::FileDiff { path, lines, additions, deletions } => {
            let mut diff_lines = Vec::new();
            for line in lines {
                let (kind, text) = if let Some(rest) = line.strip_prefix('+') {
                    (CodeLineKind::Add, rest)
                } else if let Some(rest) = line.strip_prefix('-') {
                    (CodeLineKind::Remove, rest)
                } else {
                    (CodeLineKind::Context, line.as_str())
                };
                diff_lines.push(CodeLine::new(kind, text));
            }
            let diff = CodeDiff::new(path)
                .additions(additions.unwrap_or(0) as usize)
                .deletions(deletions.unwrap_or(0) as usize)
                .lines(diff_lines);
            hud.code.push_diff(diff);
        }
        HudEvent::ContainerOutput { stream, data } => {
            for line in data.lines() {
                if !line.is_empty() {
                    hud.terminal
                        .push_line(TerminalLine::new(stream.clone(), line.to_string()));
                }
            }
        }
        HudEvent::Usage { input_tokens, output_tokens, cost_usd } => {
            let usage = UsageSummary {
                input_tokens: input_tokens.unwrap_or(0),
                output_tokens: output_tokens.unwrap_or(0),
                cost_usd: cost_usd.unwrap_or(0.0),
            };
            hud.metrics.set_usage(Some(usage));
        }
        HudEvent::Error { error } => {
            hud.status_text = "error".to_string();
            hud.thread
                .push_entry(ThreadEntry::new(ThreadEntryType::Error, Text::new(error)));
        }
    }
}

fn connect_event_source(state: Rc<RefCell<AppState>>, stream_url: &str) -> Option<HudStreamHandle> {
    let source = web_sys::EventSource::new(stream_url).ok()?;
    let state_clone = state.clone();
    let onmessage = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MessageEvent| {
        if let Some(data) = event.data().as_string() {
            if let Some(hud_event) = parse_hud_event(&data) {
                let mut state = state_clone.borrow_mut();
                apply_hud_event(&mut state.hud_ui, hud_event);
            }
        }
    });
    source.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let state_clone = state.clone();
    let onerror = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
        let mut state = state_clone.borrow_mut();
        state.hud_ui.status_text = "stream error".to_string();
    });
    source.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    onerror.forget();

    let state_clone = state.clone();
    let onopen = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
        let mut state = state_clone.borrow_mut();
        state.hud_ui.status_text = "streaming".to_string();
    });
    source.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    Some(HudStreamHandle::EventSource(source))
}

fn connect_websocket(state: Rc<RefCell<AppState>>, ws_url: &str) -> Option<HudStreamHandle> {
    let window = web_sys::window()?;
    let protocol = if window.location().protocol().unwrap_or_default() == "https:" {
        "wss:"
    } else {
        "ws:"
    };
    let host = window.location().host().unwrap_or_default();
    let full_url = format!("{}//{}{}", protocol, host, ws_url);

    let ws = web_sys::WebSocket::new(&full_url).ok()?;
    let state_clone = state.clone();
    let onmessage = Closure::<dyn FnMut(_)>::new(move |event: web_sys::MessageEvent| {
        if let Some(data) = event.data().as_string() {
            if let Some(hud_event) = parse_hud_event(&data) {
                let mut state = state_clone.borrow_mut();
                apply_hud_event(&mut state.hud_ui, hud_event);
            }
        }
    });
    ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    onmessage.forget();

    let state_clone = state.clone();
    let onopen = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::Event| {
        let mut state = state_clone.borrow_mut();
        state.hud_ui.status_text = "streaming".to_string();
    });
    ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    onopen.forget();

    let state_clone = state.clone();
    let onclose = Closure::<dyn FnMut(_)>::new(move |_event: web_sys::CloseEvent| {
        let mut state = state_clone.borrow_mut();
        state.hud_ui.status_text = "disconnected".to_string();
    });
    ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
    onclose.forget();

    Some(HudStreamHandle::WebSocket(ws))
}

pub(crate) async fn fetch_live_hud() -> Option<LandingLive> {
    let window = web_sys::window()?;
    let resp = JsFuture::from(window.fetch_with_str("/api/hud/live")).await.ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }

    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    let payload: LiveHudResponse = serde_json::from_str(
        &js_sys::JSON::stringify(&json)
            .ok()?
            .as_string()?,
    )
    .ok()?;
    if !payload.enabled {
        return None;
    }
    let context = payload.hud_context?;
    Some(LandingLive {
        hud_context: context,
        issue: payload.issue,
    })
}

pub(crate) fn init_hud_runtime(state: Rc<RefCell<AppState>>) {
    let (context, replay_speed) = {
        let state = state.borrow();
        (state.hud_context.clone(), replay_speed_from_query())
    };

    let Some(context) = context else {
        return;
    };

    if let Some(speed) = replay_speed {
        if let Some(agent_id) = context.agent_id.clone() {
            start_replay(state, agent_id, speed);
        } else {
            state.borrow_mut().hud_ui.status_text = "replay unavailable".to_string();
        }
        return;
    }

    if state.borrow().hud_stream.is_none() {
        let stream_url = context
            .stream_url
            .clone()
            .or_else(|| {
                context
                    .agent_id
                    .as_ref()
                    .map(|id| format!("/agents/{}/hud/stream?watch=1", id))
            });
        let handle = if let Some(url) = stream_url.as_deref() {
            connect_event_source(state.clone(), url)
        } else if let Some(ws_url) = context.ws_url.as_deref() {
            connect_websocket(state.clone(), ws_url)
        } else {
            None
        };
        state.borrow_mut().hud_stream = handle;
    }

    if let Some(agent_id) = context.agent_id.clone() {
        start_metrics_poll(state.clone(), agent_id.clone());
    }

    if context.is_owner && !state.borrow().hud_settings_loaded {
        let state_clone = state.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(repo) = state_clone
                .borrow()
                .hud_context
                .as_ref()
                .map(|ctx| format!("{}/{}", ctx.username, ctx.repo))
            {
                if let Some(settings) = fetch_hud_settings(&repo).await {
                    let mut state = state_clone.borrow_mut();
                    state.hud_ui.settings = settings;
                    state.hud_settings_loaded = true;
                }
            }
        });
    }
}

fn apply_hud_session(state: &mut AppState, repo: &str, session: HudSessionResponse) -> bool {
    let matches_repo = state
        .hud_context
        .as_ref()
        .map(|ctx| format!("{}/{}", ctx.username, ctx.repo) == repo)
        .unwrap_or(false);
    if !matches_repo {
        return false;
    }

    if let Some(ctx) = state.hud_context.as_mut() {
        ctx.session_id = session.session_id.clone();
        ctx.ws_url = session.ws_url.clone();
        ctx.status = session.status.clone();
    }
    state.hud_ui.status_text = session.status;
    state.hud_settings_loaded = false;
    if let Some(handle) = state.hud_stream.take() {
        handle.close();
    }
    true
}

pub(crate) async fn ensure_hud_session(state: Rc<RefCell<AppState>>, repo: String) {
    let existing = fetch_hud_session(&repo).await;
    let session = match existing {
        Some(session) if session.session_id.is_some() => Some(session),
        Some(session) if session.can_start == Some(true) => {
            start_hud_session(&repo, DEFAULT_AUTOPILOT_PROMPT).await.ok()
        }
        Some(session) => Some(session),
        None => start_hud_session(&repo, DEFAULT_AUTOPILOT_PROMPT).await.ok(),
    };

    let Some(session) = session else {
        let mut guard = state.borrow_mut();
        guard.hud_ui.status_text = "start failed".to_string();
        return;
    };

    let matches_repo = {
        let mut guard = state.borrow_mut();
        apply_hud_session(&mut guard, &repo, session)
    };
    if !matches_repo {
        return;
    }
    init_hud_runtime(state);
}

fn replay_speed_from_query() -> Option<f64> {
    let replay = query_param("replay");
    if let Some(value) = replay {
        if value.is_empty() {
            return Some(20.0);
        }
        return value.parse::<f64>().ok().or(Some(20.0));
    }
    query_param("speed").and_then(|v| v.parse::<f64>().ok())
}

fn query_param(name: &str) -> Option<String> {
    let window = web_sys::window()?;
    let search = window.location().search().ok()?;
    let params = web_sys::UrlSearchParams::new_with_str(&search).ok()?;
    params.get(name)
}

#[derive(Default)]
struct MetricsPayload {
    apm: Option<f32>,
    queue_depth: Option<u64>,
    oldest_issue: Option<String>,
    last_pr: LastPrSummary,
}

fn start_metrics_poll(state: Rc<RefCell<AppState>>, agent_id: String) {
    {
        let mut guard = state.borrow_mut();
        if guard.hud_metrics_polling {
            return;
        }
        guard.hud_metrics_polling = true;
    }

    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };

    let state_clone = state.clone();
    let closure = Closure::<dyn FnMut()>::new(move || {
        let state_inner = state_clone.clone();
        let agent_id = agent_id.clone();
        wasm_bindgen_futures::spawn_local(async move {
            if let Some(payload) = fetch_metrics(&agent_id).await {
                let mut state = state_inner.borrow_mut();
                state.hud_ui.metrics.set_apm(payload.apm);
                state
                    .hud_ui
                    .metrics
                    .set_queue(payload.queue_depth, payload.oldest_issue);
                state.hud_ui.metrics.set_last_pr(payload.last_pr);
            }
        });
    });

    let interval_id = window.set_interval_with_callback_and_timeout_and_arguments_0(
        closure.as_ref().unchecked_ref(),
        4000,
    );
    if let Ok(id) = interval_id {
        let mut guard = state.borrow_mut();
        guard.hud_metrics_timer = Some(id);
    }
    closure.forget();
}

pub(crate) fn stop_metrics_poll(state: &mut AppState) {
    if let Some(window) = web_sys::window() {
        if let Some(id) = state.hud_metrics_timer.take() {
            window.clear_interval_with_handle(id);
        }
    }
    state.hud_metrics_polling = false;
}

pub(crate) fn dispatch_hud_event(state: &Rc<RefCell<AppState>>, event: InputEvent) -> EventResult {
    let show_start_form = {
        let guard = state.borrow();
        guard.view == AppView::RepoView
            && guard
                .hud_context
                .as_ref()
                .map(|ctx| ctx.is_owner && ctx.status == "idle")
                .unwrap_or(false)
    };
    let show_share = {
        let guard = state.borrow();
        guard.view == AppView::RepoView
            && guard
                .hud_context
                .as_ref()
                .map(|ctx| ctx.is_owner && ctx.is_public)
                .unwrap_or(false)
    };

    let (handled, actions) = {
        let mut guard = state.borrow_mut();
        let layout = guard.hud_layout;
        let handled = guard
            .hud_ui
            .handle_event(&event, &layout, show_start_form, show_share);
        let actions = guard.hud_ui.take_actions();
        (handled, actions)
    };

    if !actions.is_empty() {
        queue_hud_actions(state.clone(), actions);
    }

    handled
}

fn queue_hud_actions(state: Rc<RefCell<AppState>>, actions: Vec<HudAction>) {
    for action in actions {
        match action {
            HudAction::StartAutopilot => {
                let (repo, prompt) = {
                    let mut guard = state.borrow_mut();
                    let repo = guard
                        .hud_context
                        .as_ref()
                        .map(|ctx| format!("{}/{}", ctx.username, ctx.repo));
                    let prompt = guard
                        .hud_ui
                        .start_prompt_input
                        .get_value()
                        .trim()
                        .to_string();
                    guard.hud_ui.start_prompt_input.blur();
                    if let Some(ctx) = guard.hud_context.as_mut() {
                        ctx.status = "starting".to_string();
                    }
                    guard.hud_ui.status_text = "starting".to_string();
                    if let Some(handle) = guard.hud_stream.take() {
                        handle.close();
                    }
                    (repo, prompt)
                };

                let Some(repo) = repo else { continue };
                let prompt = if prompt.is_empty() {
                    DEFAULT_AUTOPILOT_PROMPT.to_string()
                } else {
                    prompt
                };

                let state_clone = state.clone();
                wasm_bindgen_futures::spawn_local(async move {
                    match start_hud_session(&repo, &prompt).await {
                        Ok(session) => {
                            let matches_repo = {
                                let mut guard = state_clone.borrow_mut();
                                apply_hud_session(&mut guard, &repo, session)
                            };
                            if matches_repo {
                                init_hud_runtime(state_clone);
                            }
                        }
                        Err(_) => {
                            let mut guard = state_clone.borrow_mut();
                            guard.hud_ui.status_text = "start failed".to_string();
                        }
                    }
                });
            }
            HudAction::ToggleSharePanel => {
                let mut guard = state.borrow_mut();
                guard.hud_ui.share_panel_open = !guard.hud_ui.share_panel_open;
                if !guard.hud_ui.share_panel_open {
                    guard.hud_ui.share_url_copied = false;
                    guard.hud_ui.embed_code_copied = false;
                }
            }
            HudAction::CopyShareUrl => {
                let share_url = {
                    let guard = state.borrow();
                    guard.hud_context.as_ref().map(hud_share_url)
                };
                if let Some(share_url) = share_url {
                    let state_clone = state.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if copy_to_clipboard(&share_url).await {
                            set_share_notice(state_clone, ShareNotice::Url);
                        }
                    });
                }
            }
            HudAction::CopyEmbedCode => {
                let embed_code = {
                    let guard = state.borrow();
                    guard.hud_context.as_ref().map(hud_embed_code)
                };
                if let Some(embed_code) = embed_code {
                    let state_clone = state.clone();
                    wasm_bindgen_futures::spawn_local(async move {
                        if copy_to_clipboard(&embed_code).await {
                            set_share_notice(state_clone, ShareNotice::Embed);
                        }
                    });
                }
            }
        }
    }
}

fn hud_share_url(ctx: &HudContext) -> String {
    format!("openagents.com/hud/@{}/{}", ctx.username, ctx.repo)
}

fn hud_embed_code(ctx: &HudContext) -> String {
    format!(
        "<iframe src=\"openagents.com/embed/@{}/{}\">",
        ctx.username, ctx.repo
    )
}

async fn copy_to_clipboard(text: &str) -> bool {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return false,
    };
    let clipboard = window.navigator().clipboard();
    let promise = clipboard.write_text(text);
    JsFuture::from(promise).await.is_ok()
}

fn set_share_notice(state: Rc<RefCell<AppState>>, notice: ShareNotice) {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };

    {
        let mut guard = state.borrow_mut();
        match notice {
            ShareNotice::Url => {
                guard.hud_ui.share_url_copied = true;
                if let Some(id) = guard.hud_ui.share_url_timer.take() {
                    window.clear_timeout_with_handle(id);
                }
            }
            ShareNotice::Embed => {
                guard.hud_ui.embed_code_copied = true;
                if let Some(id) = guard.hud_ui.embed_code_timer.take() {
                    window.clear_timeout_with_handle(id);
                }
            }
        }
    }

    let state_clone = state.clone();
    let cb = Closure::once(move || {
        let mut guard = state_clone.borrow_mut();
        match notice {
            ShareNotice::Url => {
                guard.hud_ui.share_url_copied = false;
                guard.hud_ui.share_url_timer = None;
            }
            ShareNotice::Embed => {
                guard.hud_ui.embed_code_copied = false;
                guard.hud_ui.embed_code_timer = None;
            }
        }
    });
    if let Ok(id) = window.set_timeout_with_callback_and_timeout_and_arguments_0(
        cb.as_ref().unchecked_ref(),
        1500,
    ) {
        let mut guard = state.borrow_mut();
        match notice {
            ShareNotice::Url => guard.hud_ui.share_url_timer = Some(id),
            ShareNotice::Embed => guard.hud_ui.embed_code_timer = Some(id),
        }
    }
    cb.forget();
}

async fn fetch_metrics(agent_id: &str) -> Option<MetricsPayload> {
    let apm = fetch_metric_json(&format!("/agents/{}/metrics/apm", agent_id)).await;
    let queue = fetch_metric_json(&format!("/agents/{}/metrics/queue", agent_id)).await;
    let last_pr = fetch_metric_json(&format!("/agents/{}/metrics/last_pr", agent_id)).await;

    let apm_value = apm
        .as_ref()
        .and_then(|value| value.get("value"))
        .and_then(|value| value.as_f64())
        .map(|value| value as f32);

    let queue_depth = queue
        .as_ref()
        .and_then(|value| value.get("depth"))
        .and_then(|value| value.as_u64());
    let oldest_issue = queue
        .as_ref()
        .and_then(|value| value.get("oldest_issue"))
        .and_then(|value| value.as_str())
        .map(|s| s.to_string());

    let last_pr_summary = LastPrSummary {
        url: last_pr
            .as_ref()
            .and_then(|value| value.get("url"))
            .and_then(|value| value.as_str())
            .map(|s| s.to_string()),
        title: last_pr
            .as_ref()
            .and_then(|value| value.get("title"))
            .and_then(|value| value.as_str())
            .map(|s| s.to_string()),
        merged: last_pr
            .as_ref()
            .and_then(|value| value.get("merged"))
            .and_then(|value| value.as_bool()),
    };

    Some(MetricsPayload {
        apm: apm_value,
        queue_depth,
        oldest_issue,
        last_pr: last_pr_summary,
    })
}

async fn fetch_metric_json(url: &str) -> Option<serde_json::Value> {
    let window = web_sys::window()?;
    let resp = JsFuture::from(window.fetch_with_str(url)).await.ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    serde_json::from_str(&js_sys::JSON::stringify(&json).ok()?.as_string()?).ok()
}

async fn fetch_hud_settings(repo: &str) -> Option<HudSettingsData> {
    let window = web_sys::window()?;
    let repo_param = js_sys::encode_uri_component(repo);
    let url = format!("/api/hud/settings?repo={}", repo_param);
    let resp = JsFuture::from(window.fetch_with_str(&url)).await.ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    let value: serde_json::Value = serde_json::from_str(
        &js_sys::JSON::stringify(&json)
            .ok()?
            .as_string()?,
    )
    .ok()?;

    Some(HudSettingsData {
        public: value.get("is_public").and_then(|v| v.as_bool()).unwrap_or(true),
        embed_allowed: value
            .get("embed_allowed")
            .and_then(|v| v.as_bool())
            .unwrap_or(true),
    })
}

pub(crate) async fn update_hud_settings(
    repo: &str,
    settings: HudSettingsData,
) -> Result<(), String> {
    let window = web_sys::window().ok_or("No window available")?;
    let url = "/api/hud/settings";
    let body = serde_json::json!({
        "repo": repo,
        "is_public": settings.public,
        "embed_allowed": settings.embed_allowed,
    });

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body.to_string()));

    let headers = web_sys::Headers::new().map_err(|_| "Failed to create headers".to_string())?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|_| "Failed to set headers".to_string())?;
    opts.set_headers(&headers);

    let resp = JsFuture::from(window.fetch_with_str_and_init(&url, &opts))
        .await
        .map_err(|_| "Request failed".to_string())?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "Response invalid".to_string())?;
    if !resp.ok() {
        return Err(format!("Settings update failed ({})", resp.status()));
    }

    Ok(())
}

const DEFAULT_AUTOPILOT_PROMPT: &str =
    "Work the highest priority open issues and report progress in the HUD.";

#[derive(Clone, Deserialize)]
struct HudSessionResponse {
    status: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    ws_url: Option<String>,
    #[serde(default)]
    can_start: Option<bool>,
}

async fn fetch_hud_session(repo: &str) -> Option<HudSessionResponse> {
    let window = web_sys::window()?;
    let repo_param = js_sys::encode_uri_component(repo);
    let url = format!("/api/hud/session?repo={}", repo_param);
    let resp = JsFuture::from(window.fetch_with_str(&url)).await.ok()?;
    let resp: web_sys::Response = resp.dyn_into().ok()?;
    if !resp.ok() {
        return None;
    }
    let json = JsFuture::from(resp.json().ok()?).await.ok()?;
    serde_json::from_str(
        &js_sys::JSON::stringify(&json)
            .ok()?
            .as_string()?,
    )
    .ok()
}

async fn start_hud_session(repo: &str, prompt: &str) -> Result<HudSessionResponse, String> {
    let window = web_sys::window().ok_or("No window available")?;
    let body = serde_json::json!({
        "repo": repo,
        "prompt": prompt,
    });

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body.to_string()));

    let headers = web_sys::Headers::new().map_err(|_| "Failed to create headers".to_string())?;
    headers
        .set("Content-Type", "application/json")
        .map_err(|_| "Failed to set headers".to_string())?;
    opts.set_headers(&headers);

    let resp = JsFuture::from(window.fetch_with_str_and_init("/api/hud/start", &opts))
        .await
        .map_err(|_| "Request failed".to_string())?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "Response invalid".to_string())?;
    if !resp.ok() {
        return Err(format!("HUD start failed ({})", resp.status()));
    }

    let json = JsFuture::from(resp.json().map_err(|_| "Invalid response".to_string())?)
        .await
        .map_err(|_| "Invalid response".to_string())?;
    serde_json::from_str(
        &js_sys::JSON::stringify(&json)
            .map_err(|_| "Invalid response".to_string())?
            .as_string()
            .ok_or_else(|| "Invalid response".to_string())?,
    )
    .map_err(|_| "Invalid response".to_string())
}

#[derive(Deserialize)]
struct TraceLine {
    timestamp: u64,
    data: String,
}

fn start_replay(state: Rc<RefCell<AppState>>, agent_id: String, speed: f64) {
    let state_clone = state.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let events = fetch_trajectory_events(&agent_id).await;
        schedule_replay(state_clone, events, speed);
    });
}

async fn fetch_trajectory_events(agent_id: &str) -> Vec<(u64, HudEvent)> {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return Vec::new(),
    };
    let url = format!("/agents/{}/logs/trajectory", agent_id);
    let resp = match JsFuture::from(window.fetch_with_str(&url)).await {
        Ok(resp) => resp,
        Err(_) => return Vec::new(),
    };
    let resp: web_sys::Response = match resp.dyn_into() {
        Ok(resp) => resp,
        Err(_) => return Vec::new(),
    };
    if !resp.ok() {
        return Vec::new();
    }
    let text_promise = match resp.text() {
        Ok(promise) => promise,
        Err(_) => return Vec::new(),
    };
    let text = JsFuture::from(text_promise)
        .await
        .ok()
        .and_then(|v| v.as_string())
        .unwrap_or_default();

    let mut events = Vec::new();
    for line in text.lines() {
        if let Ok(trace) = serde_json::from_str::<TraceLine>(line) {
            if let Some(event) = parse_hud_event(&trace.data) {
                events.push((trace.timestamp, event));
            }
        }
    }
    events
}

fn schedule_replay(state: Rc<RefCell<AppState>>, events: Vec<(u64, HudEvent)>, speed: f64) {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };
    if events.is_empty() {
        state.borrow_mut().hud_ui.status_text = "replay empty".to_string();
        return;
    }
    state.borrow_mut().hud_ui.status_text = format!("replay {}x", speed);
    let start = events[0].0;
    for (timestamp, event) in events {
        let delay = ((timestamp.saturating_sub(start)) as f64 / speed).round() as i32;
        let state_clone = state.clone();
        let event = event.clone();
        let cb = Closure::once(move || {
            let mut state = state_clone.borrow_mut();
            apply_hud_event(&mut state.hud_ui, event);
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            cb.as_ref().unchecked_ref(),
            delay,
        );
        cb.forget();
    }
}

pub(crate) fn draw_hud_view(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    state: &mut AppState,
    width: f32,
    height: f32,
    scale_factor: f32,
) {
    scene.draw_quad(
        Quad::new(Bounds::new(0.0, 0.0, width, height)).with_background(theme::bg::APP),
    );

    let status_h = 28.0;
    let padding = 6.0;
    let gutter = 6.0;
    let content_x = padding;
    let content_y = padding;
    let content_w = width - padding * 2.0;
    let content_h = height - status_h - padding * 2.0;

    let mut layout = HudLayout::default();

    let mut thread_bounds;
    let code_bounds;
    let terminal_bounds;
    let metrics_bounds;
    let wallet_bounds;
    let show_start_form = state
        .hud_context
        .as_ref()
        .map(|ctx| ctx.is_owner && ctx.status == "idle")
        .unwrap_or(false);
    let show_share = state
        .hud_context
        .as_ref()
        .map(|ctx| ctx.is_owner && ctx.is_public)
        .unwrap_or(false);

    if width < 900.0 {
        let pane_h = ((content_h - gutter * 4.0) / 5.0).max(0.0);
        thread_bounds = Bounds::new(content_x, content_y, content_w, pane_h);
        code_bounds = Bounds::new(content_x, content_y + pane_h + gutter, content_w, pane_h);
        terminal_bounds =
            Bounds::new(content_x, content_y + (pane_h + gutter) * 2.0, content_w, pane_h);
        metrics_bounds =
            Bounds::new(content_x, content_y + (pane_h + gutter) * 3.0, content_w, pane_h);
        wallet_bounds =
            Bounds::new(content_x, content_y + (pane_h + gutter) * 4.0, content_w, pane_h);
    } else {
        let left_w = (content_w * 0.34).max(280.0).min(420.0);
        let right_w = (content_w * 0.28).max(240.0).min(360.0);
        let center_w = (content_w - left_w - right_w - gutter * 2.0).max(220.0);
        let left_x = content_x;
        let center_x = left_x + left_w + gutter;
        let right_x = center_x + center_w + gutter;
        thread_bounds = Bounds::new(left_x, content_y, left_w, content_h);
        code_bounds = Bounds::new(center_x, content_y, center_w, content_h);
        let mut terminal_h = (content_h * 0.6).max(0.0);
        terminal_h = terminal_h.min(content_h - gutter * 2.0).max(0.0);
        let remaining_h = (content_h - terminal_h - gutter * 2.0).max(0.0);
        let metrics_h = remaining_h * 0.5;
        let wallet_h = remaining_h - metrics_h;
        terminal_bounds = Bounds::new(right_x, content_y, right_w, terminal_h);
        metrics_bounds = Bounds::new(
            right_x,
            content_y + terminal_h + gutter,
            right_w,
            metrics_h,
        );
        wallet_bounds = Bounds::new(
            right_x,
            content_y + terminal_h + gutter + metrics_h + gutter,
            right_w,
            wallet_h,
        );
    }

    layout.settings_public_bounds = Bounds::ZERO;
    layout.settings_embed_bounds = Bounds::ZERO;
    layout.start_form_bounds = Bounds::ZERO;
    layout.start_prompt_bounds = Bounds::ZERO;
    layout.start_button_bounds = Bounds::ZERO;
    layout.share_button_bounds = Bounds::ZERO;
    layout.share_panel_bounds = Bounds::ZERO;
    layout.copy_url_bounds = Bounds::ZERO;
    layout.copy_embed_bounds = Bounds::ZERO;
    if let Some(ctx) = state.hud_context.as_ref() {
        if ctx.is_owner {
            let settings_height = 58.0;
            let settings_bounds = Bounds::new(
                thread_bounds.origin.x,
                thread_bounds.origin.y,
                thread_bounds.size.width,
                settings_height,
            );
            let (public_bounds, embed_bounds) = draw_hud_settings(
                scene,
                text_system,
                &state.hud_ui.settings,
                settings_bounds,
            );
            layout.settings_public_bounds = public_bounds;
            layout.settings_embed_bounds = embed_bounds;
            thread_bounds.origin.y += settings_height + gutter;
            thread_bounds.size.height -= settings_height + gutter;
        }
    }

    let show_start_form = show_start_form && thread_bounds.size.height > 72.0;
    if show_start_form {
        let start_height = 72.0;
        let start_bounds = Bounds::new(
            thread_bounds.origin.x,
            thread_bounds.origin.y,
            thread_bounds.size.width,
            start_height,
        );
        let (prompt_bounds, button_bounds) = start_form_layout(&state.hud_ui, start_bounds);
        layout.start_form_bounds = start_bounds;
        layout.start_prompt_bounds = prompt_bounds;
        layout.start_button_bounds = button_bounds;
        thread_bounds.origin.y += start_height + gutter;
        thread_bounds.size.height -= start_height + gutter;
    }

    layout.thread_bounds = thread_bounds;
    layout.code_bounds = code_bounds;
    layout.terminal_bounds = terminal_bounds;
    layout.metrics_bounds = metrics_bounds;
    layout.wallet_bounds = wallet_bounds;
    layout.status_bounds = Bounds::new(0.0, height - status_h, width, status_h);
    if show_share {
        let (button_w, button_h) = state.hud_ui.share_button.size_hint();
        let button_w = button_w.unwrap_or(64.0);
        let button_h = button_h
            .unwrap_or(layout.status_bounds.size.height - 6.0)
            .min(layout.status_bounds.size.height - 6.0);
        let button_x = layout.status_bounds.origin.x + layout.status_bounds.size.width - button_w - 6.0;
        let button_y = layout.status_bounds.origin.y
            + (layout.status_bounds.size.height - button_h) / 2.0;
        layout.share_button_bounds = Bounds::new(button_x, button_y, button_w, button_h);

        if state.hud_ui.share_panel_open {
            let panel_w = 340.0;
            let panel_h = 128.0;
            let panel_x = (layout.status_bounds.origin.x + layout.status_bounds.size.width - panel_w - 6.0)
                .max(6.0);
            let panel_y = (layout.status_bounds.origin.y - panel_h - 6.0).max(6.0);
            layout.share_panel_bounds = Bounds::new(panel_x, panel_y, panel_w, panel_h);
            let (copy_url_bounds, copy_embed_bounds) =
                share_panel_layout(&state.hud_ui, layout.share_panel_bounds);
            layout.copy_url_bounds = copy_url_bounds;
            layout.copy_embed_bounds = copy_embed_bounds;
        }
    } else {
        state.hud_ui.share_panel_open = false;
    }
    state.hud_layout = layout;

    let mut cx = PaintContext::new(scene, text_system, scale_factor);
    if show_start_form {
        draw_start_form(
            &mut cx,
            &mut state.hud_ui,
            state.hud_layout.start_form_bounds,
            state.hud_layout.start_prompt_bounds,
            state.hud_layout.start_button_bounds,
        );
    } else {
        state.hud_ui.start_prompt_input.blur();
    }
    state.hud_ui.thread.paint(thread_bounds, &mut cx);
    state.hud_ui.code.paint(code_bounds, &mut cx);
    state.hud_ui.terminal.paint(terminal_bounds, &mut cx);
    state.hud_ui.metrics.paint(metrics_bounds, &mut cx);
    let show_wallet = state
        .hud_context
        .as_ref()
        .map(|ctx| ctx.is_owner)
        .unwrap_or(false);
    if show_wallet {
        state.wallet.paint(wallet_bounds, &mut cx);
    }

    if state.hud_ui.thread.entry_count() == 0 {
        let placeholder = cx.text.layout(
            "No events yet",
            Point::new(thread_bounds.origin.x + 10.0, thread_bounds.origin.y + 10.0),
            theme::font_size::XS,
            theme::text::MUTED,
        );
        cx.scene.draw_text(placeholder);
    }

    if let Some(ctx) = state.hud_context.as_ref() {
        let repo = format!("{}/{}", ctx.username, ctx.repo);
        let scope = if ctx.embed_mode {
            "embed"
        } else if ctx.is_public {
            "public"
        } else {
            "private"
        };
        let mut items = vec![
            StatusItem::text("status", state.hud_ui.status_text.clone())
                .align(StatusItemAlignment::Left),
            StatusItem::text("scope", scope).center(),
        ];
        if show_share {
            items.push(StatusItem::text("repo", repo).left());
        } else {
            items.push(StatusItem::text("repo", repo).right());
        }
        state.hud_ui.status_bar.set_items(items);
    }
    state
        .hud_ui
        .status_bar
        .paint(state.hud_layout.status_bounds, &mut cx);

    if show_share {
        state
            .hud_ui
            .share_button
            .paint(state.hud_layout.share_button_bounds, &mut cx);
        if state.hud_ui.share_panel_open {
            if let Some(ctx) = state.hud_context.as_ref() {
                let share_url = hud_share_url(ctx);
                let embed_code = hud_embed_code(ctx);
                draw_share_panel(
                    &mut cx,
                    &mut state.hud_ui,
                    state.hud_layout.share_panel_bounds,
                    state.hud_layout.copy_url_bounds,
                    state.hud_layout.copy_embed_bounds,
                    &share_url,
                    &embed_code,
                );
            }
        }
    }
}

fn start_form_layout(hud_ui: &HudUi, bounds: Bounds) -> (Bounds, Bounds) {
    let padding = 8.0;
    let header_h = 20.0;
    let gap = 8.0;
    let available_h = (bounds.size.height - header_h - padding).max(0.0);
    let (button_w, button_h) = hud_ui.start_button.size_hint();
    let (_, input_h) = hud_ui.start_prompt_input.size_hint();
    let button_w = button_w.unwrap_or(120.0);
    let button_h = button_h.unwrap_or(available_h);
    let input_h = input_h.unwrap_or(24.0);
    let row_h = available_h.min(input_h.max(button_h));
    let row_y = bounds.origin.y + header_h;
    let button_bounds = Bounds::new(
        bounds.origin.x + bounds.size.width - padding - button_w,
        row_y,
        button_w,
        row_h,
    );
    let prompt_bounds = Bounds::new(
        bounds.origin.x + padding,
        row_y,
        (bounds.size.width - padding * 2.0 - button_w - gap).max(0.0),
        row_h,
    );
    (prompt_bounds, button_bounds)
}

fn draw_start_form(
    cx: &mut PaintContext,
    hud_ui: &mut HudUi,
    bounds: Bounds,
    prompt_bounds: Bounds,
    button_bounds: Bounds,
) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let label = cx.text.layout(
        "Start Autopilot",
        Point::new(bounds.origin.x + 8.0, bounds.origin.y + 6.0),
        10.0,
        theme::text::PRIMARY,
    );
    cx.scene.draw_text(label);

    hud_ui.start_prompt_input.paint(prompt_bounds, cx);
    hud_ui.start_button.paint(button_bounds, cx);
}

fn share_panel_layout(hud_ui: &HudUi, bounds: Bounds) -> (Bounds, Bounds) {
    let padding = 10.0;
    let (url_w, url_h) = hud_ui.copy_url_button.size_hint();
    let (embed_w, embed_h) = hud_ui.copy_embed_button.size_hint();
    let button_w = url_w
        .unwrap_or(90.0)
        .max(embed_w.unwrap_or(90.0))
        .max(90.0);
    let button_h = url_h
        .unwrap_or(20.0)
        .max(embed_h.unwrap_or(20.0));
    let copy_url_bounds = Bounds::new(
        bounds.origin.x + bounds.size.width - padding - button_w,
        bounds.origin.y + 44.0,
        button_w,
        button_h,
    );
    let copy_embed_bounds = Bounds::new(
        bounds.origin.x + bounds.size.width - padding - button_w,
        bounds.origin.y + 76.0,
        button_w,
        button_h,
    );
    (copy_url_bounds, copy_embed_bounds)
}

fn draw_share_panel(
    cx: &mut PaintContext,
    hud_ui: &mut HudUi,
    bounds: Bounds,
    copy_url_bounds: Bounds,
    copy_embed_bounds: Bounds,
    share_url: &str,
    embed_code: &str,
) {
    cx.scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let title = cx.text.layout(
        "Share HUD",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 8.0),
        11.0,
        theme::text::PRIMARY,
    );
    cx.scene.draw_text(title);

    let url_label = cx.text.layout(
        "URL",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 32.0),
        9.0,
        theme::text::MUTED,
    );
    cx.scene.draw_text(url_label);

    let url_text = cx.text.layout(
        share_url,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 44.0),
        10.0,
        theme::text::PRIMARY,
    );
    cx.scene.draw_text(url_text);

    let embed_label = cx.text.layout(
        "Embed",
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 64.0),
        9.0,
        theme::text::MUTED,
    );
    cx.scene.draw_text(embed_label);

    let embed_text = cx.text.layout(
        embed_code,
        Point::new(bounds.origin.x + 10.0, bounds.origin.y + 76.0),
        10.0,
        theme::text::PRIMARY,
    );
    cx.scene.draw_text(embed_text);

    hud_ui.copy_url_button.paint(copy_url_bounds, cx);
    hud_ui.copy_embed_button.paint(copy_embed_bounds, cx);

    if hud_ui.share_url_copied {
        let copied = cx.text.layout(
            "Copied!",
            Point::new(copy_url_bounds.origin.x - 56.0, copy_url_bounds.origin.y + 6.0),
            9.0,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(copied);
    }

    if hud_ui.embed_code_copied {
        let copied = cx.text.layout(
            "Copied!",
            Point::new(copy_embed_bounds.origin.x - 56.0, copy_embed_bounds.origin.y + 6.0),
            9.0,
            theme::accent::PRIMARY,
        );
        cx.scene.draw_text(copied);
    }
}

fn draw_hud_settings(
    scene: &mut Scene,
    text_system: &mut TextSystem,
    settings: &HudSettingsData,
    bounds: Bounds,
) -> (Bounds, Bounds) {
    scene.draw_quad(
        Quad::new(bounds)
            .with_background(theme::bg::SURFACE)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let padding = 10.0;
    let label = text_system.layout(
        "Share",
        Point::new(bounds.origin.x + padding, bounds.origin.y + 8.0),
        11.0,
        theme::text::PRIMARY,
    );
    scene.draw_text(label);

    let toggle_w = 100.0;
    let toggle_h = 22.0;
    let toggle_y = bounds.origin.y + 28.0;

    let public_bounds = Bounds::new(
        bounds.origin.x + padding,
        toggle_y,
        toggle_w,
        toggle_h,
    );
    let embed_bounds = Bounds::new(
        bounds.origin.x + padding + toggle_w + 10.0,
        toggle_y,
        toggle_w,
        toggle_h,
    );

    let public_bg = if settings.public {
        theme::accent::PRIMARY.with_alpha(0.2)
    } else {
        theme::bg::APP
    };
    let embed_bg = if settings.embed_allowed {
        theme::accent::PRIMARY.with_alpha(0.2)
    } else {
        theme::bg::APP
    };

    scene.draw_quad(
        Quad::new(public_bounds)
            .with_background(public_bg)
            .with_border(theme::border::DEFAULT, 1.0),
    );
    scene.draw_quad(
        Quad::new(embed_bounds)
            .with_background(embed_bg)
            .with_border(theme::border::DEFAULT, 1.0),
    );

    let public_text = text_system.layout(
        "Public",
        Point::new(public_bounds.origin.x + 10.0, public_bounds.origin.y + 5.0),
        10.0,
        if settings.public {
            theme::accent::PRIMARY
        } else {
            theme::text::MUTED
        },
    );
    scene.draw_text(public_text);

    let embed_text = text_system.layout(
        "Embed",
        Point::new(embed_bounds.origin.x + 10.0, embed_bounds.origin.y + 5.0),
        10.0,
        if settings.embed_allowed {
            theme::accent::PRIMARY
        } else {
            theme::text::MUTED
        },
    );
    scene.draw_text(embed_text);

    (public_bounds, embed_bounds)
}

fn merge_event_result(lhs: EventResult, rhs: EventResult) -> EventResult {
    if matches!(lhs, EventResult::Handled) || matches!(rhs, EventResult::Handled) {
        EventResult::Handled
    } else {
        EventResult::Ignored
    }
}
