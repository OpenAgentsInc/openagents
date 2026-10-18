//! The tool catalog and its dispatch.
//!
//! Every tool is a thin composition of engine reads/writes from
//! [`OpenAgents`]; the only logic that lives here is argument resolution (chat
//! by prefix, project by path), sender attribution, and the "how do I
//! deliver a message to a chat in this state" choice the composer makes
//! for humans.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use openagents_doc::SessionCommandPayload;
use openagents_proto::{
    Chat, ChatConfig, HarnessId, ReasoningLevel, RunRequest, SandboxLevel, Session, SessionStatus,
    SidebarPinChange, SidebarPreferencesState, Space, UserInputAnswer,
};

use crate::inbox::Inbox;
use crate::transcript::{RenderOptions, RenderedMessage, render_entries};
use crate::openagents::{HarnessInfo, TurnOutcome, OpenAgents, session_for, short};

const MAX_BATCH: usize = 32;
/// Default and ceiling for every blocking wait (#11969): a chat stuck in one
/// long call can't show progress or take new messages.
const MAX_WAIT_SECS: u64 = 120;
/// A session row older than this is not trusted to still be working
/// (the UI's staleness window): a crashed host must not read as busy forever.
const SESSION_STALE: chrono::Duration = chrono::Duration::seconds(45);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub struct Tools {
    openagents: Arc<OpenAgents>,
    // Remember successful sends on this MCP connection so wait_for_turn after
    // wait:false also waits for a newly created chat with no session row yet.
    pending_turns: tokio::sync::Mutex<HashMap<String, Arc<PendingTurn>>>,
    inbox: tokio::sync::Mutex<std::sync::Weak<Inbox>>,
    /// Who answers `judge` (#12042): the Decisions API, or a fake in tests.
    decider: Arc<dyn crate::judge::Decider>,
}

/// OpenAgents #11971: the agent build gate's status, on Unix.
fn build_status_tool() -> Option<ToolDef> {
    #[cfg(unix)]
    {
        Some(crate::build_status::tool())
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// OpenAgents #12039: read-only machine facts, on Unix.
fn machine_status_tool() -> Option<ToolDef> {
    #[cfg(unix)]
    {
        Some(crate::machine_status::tool())
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// Terminal agent sessions on this computer, on Unix.
fn list_runs_tool() -> Option<ToolDef> {
    #[cfg(unix)]
    {
        Some(crate::runs::tool())
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// OpenAgents #12033: granted, logged machine actions, on Unix.
fn machine_action_tool() -> Option<ToolDef> {
    #[cfg(unix)]
    {
        Some(crate::machine_action::tool())
    }
    #[cfg(not(unix))]
    {
        None
    }
}

/// OpenAgents #12056: upgrading this computer's app, on Unix.
fn upgrade_host_tool() -> Option<ToolDef> {
    #[cfg(unix)]
    {
        Some(crate::upgrade_host::tool())
    }
    #[cfg(not(unix))]
    {
        None
    }
}

fn chat_key_schema(extra: Value) -> Value {
    let mut properties = json!({
        "chat": {
            "type": "string",
            "description": "Chat id, unique id prefix, or exact title."
        }
    });
    if let (Some(base), Some(more)) = (properties.as_object_mut(), extra.as_object()) {
        for (k, v) in more {
            base.insert(k.clone(), v.clone());
        }
    }
    json!({ "type": "object", "properties": properties, "required": ["chat"] })
}

fn timeout_schema() -> Value {
    json!({
        "type": "integer", "minimum": 1, "maximum": MAX_WAIT_SECS, "default": MAX_WAIT_SECS,
        "description": "Seconds to wait, at most 120. The wait also ends early when a new message arrives for you."
    })
}

fn device_schema() -> Value {
    json!({ "type": "string", "description": "Host device id or exact name. Omit for the local engine." })
}

fn catalog() -> Vec<ToolDef> {
    let mut tools = vec![
        ToolDef {
            name: "notify_owner",
            description: "Ask your owner to open your chat for an alarm or a question. Requires permission for your saved agent. Use one stable request_id for each event; retries reuse it. At most six new notes per hour.",
            input_schema: json!({"type":"object","additionalProperties":false,"properties":{"request_id":{"type":"string","description":"Stable 64-character lowercase hexadecimal event ID."},"title":{"type":"string","description":"Plain words under 120 bytes."},"body":{"type":"string","description":"What happened or what you need, up to 1024 bytes."}},"required":["request_id","title","body"]}),
        },
        ToolDef {
            name: "whoami",
            description: "Which chat and device this server speaks for, plus the engine's workspace mode. Call this first when you need to know your own chat id.",
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "list_devices",
            description: "Devices in this workspace (the local engine's device is flagged). Chats and projects are hosted on a device.",
            input_schema: json!({ "type": "object", "properties": {} }),
        },
        ToolDef {
            name: "land_status",
            description: "Read the shared landing queue with this app's signed-in account: whether the integrator is running, changes waiting or landing, and failed entries. Check it at every check-in; report a stopped integrator first.",
            input_schema: json!({ "type": "object", "properties": {}, "additionalProperties": false }),
        },
        ToolDef {
            name: "list_projects",
            description: "Projects: folders on specific devices. Filter by device (id or name); omit for all projects. Use ids when names or paths repeat.",
            input_schema: json!({ "type": "object", "properties": { "device": device_schema() } }),
        },
        ToolDef {
            name: "list_harnesses",
            description: "Agent harnesses (claude-code, codex, cursor, …) available on the chosen device. Omit device for the local engine.",
            input_schema: json!({ "type": "object", "properties": { "device": device_schema() } }),
        },
        ToolDef {
            name: "list_models",
            description: "Models a harness offers on the chosen device (local engine when omitted). Model ids are harness-specific strings; pass one to create_chat.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "harness": { "type": "string", "description": "Harness id, e.g. claude-code or codex." },
                    "device": device_schema()
                },
                "required": ["harness"]
            }),
        },
        ToolDef {
            name: "list_chats",
            description: "Chats with their project, host device, harness/model, harness session id (the id issue claims name), pinned and archived states, live status, and last activity. Newest activity first. Pinned is null while sidebar pins are loading.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Only chats in this project (id, path, or name)." },
                    "device": { "type": "string", "description": "Only chats hosted on this device (id or name)." },
                    "include_archived": { "type": "boolean", "default": false },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 500, "default": 50 }
                }
            }),
        },
        ToolDef {
            name: "get_chat",
            description: "One chat's metadata, pinned state, live status, and any question its agent is currently blocked on. Pinned is null while sidebar pins are loading.",
            input_schema: chat_key_schema(json!({})),
        },
        ToolDef {
            name: "changed_paths",
            description: "Read-only. For each chat on this device, the files its checkout changed against origin/main (committed, uncommitted, and untracked), plus which chats share a changed file. Push chats in `pushInOrder` groups one at a time; chats in `pushFreely` can push as soon as their own tests pass. Pass `dir` when the chat works in a worktree other than its folder.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "chats": {
                        "type": "array", "minItems": 1, "maxItems": MAX_BATCH,
                        "items": {
                            "type": "object",
                            "properties": {
                                "chat": { "type": "string", "description": "Chat id, unique id prefix, or exact title." },
                                "dir": { "type": "string", "description": "Worktree to read instead of the chat's folder." }
                            },
                            "required": ["chat"]
                        }
                    },
                    "base": { "type": "string", "default": crate::changed_paths::DEFAULT_BASE, "description": "Ref to compare against. Not fetched first." }
                },
                "required": ["chats"]
            }),
        },
        ToolDef {
            name: "create_chat",
            description: "Create a chat. It is a standalone session listed in Sessions, like one the user started. Select a project and/or device; catalogs are validated on that host. Optionally send a first prompt and wait for the reply. Returns the new chat id. For parallel delegation use create_chats, or leave wait=false on every launch and wait only after all chats have been started.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "project": { "type": "string", "description": "Project id, path, or name. Belongs to a specific device; when device is supplied, search only there. Optional." },
                    "device": { "type": "string", "description": "Host device (id or name). With project, it must own that project. Without either, use the local engine." },
                    "harness": { "type": "string", "description": "Harness id (see list_harnesses with the chosen device). Defaults to claude-code when available." },
                    "model": { "type": "string", "description": "Model id from list_models on the chosen device. Omit for the harness default." },
                    "reasoning": { "type": "string", "description": "Reasoning level the model supports (e.g. low, medium, high, max)." },
                    "sandbox": { "type": "string", "enum": ["read-only", "workspace-write", "danger-full-access"], "default": "workspace-write" },
                    "title": { "type": "string", "description": "Sidebar title. Otherwise the engine titles it from the first exchange." },
                    "branch": { "type": "string", "description": "Branch label to record on the chat." },
                    "cwd": { "type": "string", "description": "Working directory override (an existing worktree path). Defaults to the project folder." },
                    "prompt": { "type": "string", "description": "First message to send right away." },
                    "wait": { "type": "boolean", "default": false, "description": "With prompt: block until the first turn finishes and return the reply." },
                    "timeout_secs": timeout_schema()
                }
            }),
        },
        ToolDef {
            name: "read_chat",
            description: "Read a chat's transcript as plain messages (newest window by default). Tool calls are summarized one per line.",
            input_schema: chat_key_schema(json!({
                "limit": { "type": "integer", "minimum": 1, "maximum": 500, "default": 40, "description": "How many messages, counted from the newest." },
                "offset": { "type": "integer", "minimum": 0, "default": 0, "description": "Skip this many newest messages (paging backwards)." },
                "include_reasoning": { "type": "boolean", "default": false },
                "include_tools": { "type": "boolean", "default": true }
            })),
        },
        ToolDef {
            name: "send_message",
            description: "Send a message to a chat. Messages are attributed to your chat. mode 'auto' starts a turn when the chat is idle, steers a running turn through its live mailbox (at the next supported input boundary without interrupting the agent). Use mode 'queue' only to explicitly hold a message for later. With wait=true, blocks until the turn finishes and returns the assistant's reply. For parallel work use send_messages, or send to all chats with wait=false before waiting.",
            input_schema: chat_key_schema(json!({
                "text": { "type": "string" },
                "mode": { "type": "string", "enum": ["auto", "run", "steer", "queue"], "default": "auto" },
                "wait": { "type": "boolean", "default": false },
                "timeout_secs": timeout_schema()
            })),
        },
        ToolDef {
            name: "wait_for_turn",
            description: "Block until a chat is no longer working, for at most 120 seconds: returns completed, awaitingInput (answer with respond_to_input), errored, timedOut (call again to keep waiting), or interrupted when a new message arrives for you (read it before waiting again), with the newest assistant message.",
            input_schema: chat_key_schema(json!({
                "timeout_secs": timeout_schema()
            })),
        },
        ToolDef {
            name: "interrupt_chat",
            description: "Stop the chat's running turn.",
            input_schema: chat_key_schema(json!({})),
        },
        ToolDef {
            name: "respond_to_input",
            description: "Answer a question the chat's agent is blocked on (see get_chat / wait_for_turn pendingInput). Labels are the option strings, or free text for open questions.",
            input_schema: chat_key_schema(json!({
                "request_id": { "type": "string", "description": "The pending request id. Defaults to the chat's current pending question." },
                "answers": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "properties": {
                            "question_id": { "type": "string" },
                            "labels": { "type": "array", "items": { "type": "string" } }
                        },
                        "required": ["question_id", "labels"]
                    }
                }
            })),
        },
        ToolDef {
            name: "archive_chat",
            description: "Archive or restore any chat, for example when the owner asks. Archiving hides it from the active sidebar list; pass archived=false to restore.",
            input_schema: chat_key_schema(json!({
                "archived": { "type": "boolean", "default": true }
            })),
        },
        ToolDef {
            name: "pin_chat",
            description: "Pin or unpin any chat in the sidebar, for example when the owner asks. Pass pinned=false to unpin it. New pins go at the end; pinning an already pinned chat keeps its position.",
            input_schema: chat_key_schema(json!({
                "pinned": { "type": "boolean", "default": true }
            })),
        },
        ToolDef {
            name: "plan_move",
            description: "Plan moving running chats to a cloud computer so they keep running while this computer restarts or updates. Omit chats to plan every agent running on this computer. Returns the plan with each chat's state and the estimated paid cloud computer time. Nothing moves until the person approves the plan in the OpenAgents app; tell them what it will cost and ask them to approve it.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "chats": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Chat ids, unique id prefixes, or exact titles. Omit for every running agent."
                    },
                    "computer": { "type": "string", "description": "An existing cloud computer's id. Omit for a new one." }
                }
            }),
        },
        ToolDef {
            name: "start_move",
            description: "Start an approved move plan, or continue one that stopped partway. A retry never makes a second computer or a second continuation. Refused while the plan waits for the person's approval in the OpenAgents app.",
            input_schema: json!({
                "type": "object",
                "properties": { "plan_id": { "type": "string" } },
                "required": ["plan_id"]
            }),
        },
        ToolDef {
            name: "move_status",
            description: "Read move plans, newest first, or one plan: each chat's state (planned, pausing, captured, provisioned, restored, continued, released, stays_here, brought_back), the reason when a chat stays on this computer, and any Bring back.",
            input_schema: json!({
                "type": "object",
                "properties": { "plan_id": { "type": "string" } }
            }),
        },
        ToolDef {
            name: "bring_back",
            description: "Ask to bring a chat that moved to a cloud computer back to this computer: its work is fetched onto a local branch, the chat continues here, and the cloud computer is stopped (default), deleted, or kept. It runs once the person approves it in the OpenAgents app.",
            input_schema: chat_key_schema(json!({
                "after": { "type": "string", "enum": ["stop", "delete", "keep"], "default": "stop" }
            })),
        },
        ToolDef {
            name: RECORD_DECISION,
            description: "For a saved agent with check-ins: add one entry to your decision log, which the owner sees in the app and your next check-in reads. Call it once per check-in with what you saw, what you decided and what you did. It works only from the agent's own chat.",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "saw": { "type": "string", "description": "What you found, briefly." },
                    "decided": { "type": "string", "description": "What you decided and why." },
                    "did": { "type": "string", "description": "What you changed, or \"nothing\"." },
                    "working": {
                        "type": "array",
                        "maxItems": openagents_proto::agent_wakes::MAX_WORK_ITEMS,
                        "description": "Your whole working state now, replacing the saved one. Leave it out to keep the saved state.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "issue": { "type": "integer", "description": "The issue number." },
                                "chat": { "type": "string", "description": "The chat ID that owns it." },
                                "status": { "type": "string", "enum": ["working", "waiting", "handoff", "stalled", "watching"] },
                                "note": { "type": "string", "description": "What it waits on or what's next, briefly." }
                            },
                            "required": ["status"]
                        }
                    },
                    "judgments": {
                        "type": "array",
                        "items": { "type": "object" },
                        "description": "The `receipt` of each judge call this decision used, as judge returned it."
                    }
                },
                "required": ["saw", "decided", "did"]
            }),
        },
    ];
    for (name, single, description) in [
        (
            "create_chats",
            "create_chat",
            "Create multiple standalone chats concurrently. Put each chat's prompt in its request to start all work together. Prefer this for parallel delegation, including harnesses that execute tool calls sequentially. Each request has create_chat arguments; wait defaults to false. Results preserve request order and include per-request errors; successful requests are not rolled back.",
        ),
        (
            "send_messages",
            "send_message",
            "Send messages to multiple independent chats concurrently. Prefer this to delegate parallel work to existing chats. Each request has send_message arguments; wait defaults to false. If wait=true, requests still run concurrently. Results preserve request order and include per-request errors; successful sends are not rolled back. Do not include dependent messages to the same chat.",
        ),
    ] {
        let item_schema = tools
            .iter()
            .find(|tool| tool.name == single)
            .unwrap()
            .input_schema
            .clone();
        tools.push(ToolDef {
            name, description,
            input_schema: json!({
                "type": "object",
                "properties": {"requests": {"type": "array", "minItems": 1, "maxItems": MAX_BATCH, "items": item_schema}},
                "required": ["requests"],
            }),
        });
    }
    tools.push(ToolDef {
        name: SHOW_VISUAL,
        description: SHOW_VISUAL_DESCRIPTION,
        input_schema: json!({
            "type": "object",
            "properties": {
                "script": {
                    "type": "string",
                    "description": "The Luau visual script: setup at the top level, ending with `return function() return ui.column { ... } end`."
                },
                "title": { "type": "string", "description": "Short name for the visual." },
                "assets": {
                    "type": "object",
                    "additionalProperties": { "type": "string" },
                    "description": "Files the script uses, by the name it uses them: { \"prism.glb\": \"models/prism.glb\", \"sky.hdr\": \"/tmp/sky.hdr\" }. Paths are on this machine, relative to your working directory. glTF models (.glb, or .gltf with everything embedded), PNG, JPEG, WebP, and Radiance .hdr images; at most 16 files, 8 MB each, 10 MB together."
                }
            },
            "required": ["script", "title"]
        }),
    });
    tools
}

/// Shows a page in the CALLING chat, so it is offered only to an agent that
/// OpenAgents launched (`OPENAGENTS_CHAT_ID` set).
const SHOW_VISUAL: &str = "show_visual";
const RECORD_DECISION: &str = "record_decision";

const SHOW_VISUAL_DESCRIPTION: &str = "\
Show an interactive visual (chart, table, dashboard, diagram, simulation, 3D view) inline in this \
chat, above your final text reply; call it before writing that reply. Use it when a visual says more \
than prose. The reader already sees it, so the reply should not announce, locate, or restate it: add \
only what the visual doesn't say.\n\
\n\
A visual is a Luau script (Lua 5.1 plus `+=`, `continue`, and `if` expressions). OpenAgents runs it in a \
sandbox (no I/O or network; math.random is seeded, so data is the same on every render) and renders \
what it returns with OpenAgents's own native interface. Build data and state at the top level, then return \
a view function. OpenAgents calls the view again after every handler, so handlers just assign to state:\n\
\n\
local s = ui.state { metric = \"p95\" }\n\
local data = { { region = \"us-east\", p95 = 182, p99 = 240 }, { region = \"eu-west\", p95 = 140, p99 = 210 } }\n\
return function()\n\
  return ui.column { gap = 16,\n\
    ui.text { \"Latency by region\", size = \"lg\", weight = 600 },\n\
    ui.tabs { options = { \"p95\", \"p99\" }, value = s.metric, on_change = function(v) s.metric = v end },\n\
    ui.chart { type = \"bar\", height = 220, unit = \"ms\",\n\
      x = ui.map(data, function(d) return d.region end),\n\
      series = { { name = s.metric, values = ui.map(data, function(d) return d[s.metric] end) } } },\n\
  }\n\
end\n\
\n\
OpenAgents runs the script before showing it (setup, render, every draw, and each tab, toggle, slider, and \
button once) and returns the first error with its line; fix it and call again.\n\
\n\
Nodes are props tables: children are the array part, strings are plain text, nil and false children \
are skipped, and lists of nodes flatten (so ui.map results can be children). Unknown props are errors.\n\
- ui.column { gap = 8, align = \"start|center|end\", ... }, ui.row { gap = 12, align, justify = \
\"start|center|end|between\", wrap = true, ... }, ui.grid { columns = 3, gap = 16, ... }. Any node \
takes width = px or grow = true inside a row (sliders, charts, tables, canvases, and scenes fill a \
row's spare width by default), and key = \"name\" to keep its hover or camera when the tree changes \
shape. A row or column with on_click = function() end is clickable; hover = true gives \
it the hover wash.\n\
- ui.text { \"plain \", ui.span { \"bold\", weight = 600, color = \"accent\", mono = true }, size = \
\"xs|sm|body|lg|xl|2xl\", weight = 400-700, color, mono, italic, align, truncate }, or ui.text \"string\".\n\
- ui.stat { value = \"182 ms\", label = \"p95 latency\", delta = \"-12%\", tone = \"success\" }: a headline \
figure over a muted label; tone colors the delta.\n\
- ui.table { columns = { \"Region\", { label = \"p95\", align = \"end\" } }, rows = { { \"us-east\", 182 }, \
... } }: laid out like a markdown table (bold header, hairline row rules), numbers right-aligned; cells \
are strings, numbers, or nodes.\n\
- ui.chart { type = \"bar|line|area|scatter\", height = 220, x = { labels }, series = { { name, values = \
{ ... }, color } }, stacked, horizontal (bar), min, max, unit = \"ms\", prefix = \"$\", decimals, legend }. \
Scatter series take points = { { x, y }, ... } (x_unit, x_decimals). Axes, gridlines, the legend, and a \
hover readout are built in.\n\
- ui.rule {} is a hairline divider; ui.space { size = 16 }; ui.link { label, url = \"https://...\" } \
opens like a chat link.\n\
- Controls: ui.tabs { options = { ... }, value, on_change = function(option) end }, ui.toggle { label, \
value = bool, on_change = function(on) end }, ui.slider { label, value, min, max, step, unit, on_change \
= function(n) end }, ui.button { label, on_click = function() end, primary = bool }.\n\
- ui.canvas { height = 240, draw = function(g, w, h, t) ... end, animate = bool, on_click = \
function(x, y) end }: 2D drawing in pixels, origin top-left, w = the reply column's width. \
g:line(x1, y1, x2, y2, { color, width, dash }), g:polyline(pts, { color, width }), g:polygon(pts, \
{ fill, stroke, width }), g:rect(x, y, w, h, { fill, stroke, radius }), g:circle(x, y, r, { fill, \
stroke }), g:arc(x, y, r, a0, a1, { color } or { fill, inner = radius } for pie and donut sectors; \
radians clockwise from 3 o'clock), g:text(x, y, str, { size, color, weight, align = \
\"start|center|end\", baseline = \"top|middle|bottom\", mono }). pts are { x1, y1, x2, y2, ... } or \
{ { x, y }, ... }. g.pointer is { x, y, down } while the pointer is over the canvas (draw hover \
effects with it); t is seconds, and animate = true redraws every frame while visible.\n\
- ui.scene { height = 320, draw = function(s, t) ... end, orbit = true, spin = 0.3 (turntable \
radians per second), camera = { yaw = 0.6, pitch = 0.45, zoom = 1 }, animate = bool }: real 3D, y up, \
rendered on the GPU with depth, soft shadows from a key light, and bloom. OpenAgents frames the camera on \
your geometry and the reader drags to orbit. Solids (box, sphere, cylinder, surface, mesh) take \
material props: roughness (0 mirror to 1 matte), metal (0-1), glow (emits its color and blooms; \
use for lamps, beams, highlights), glass = true (refracts what's behind it; ior = 1.5, dispersion \
= 0-1 splits it into a rainbow, as a prism does). Build real forms from several primitives (legs, \
tops, rims), not single boxes, and give materials so surfaces read as wood, metal, or glass. s:box { at = { x, y, z }, size = { w, h, d } or n, color }, s:sphere { at, radius, color }, \
s:cylinder { at (base center), radius, height, color }, s:surface { x = { min, max }, z = { min, max }, \
f = function(x, z) return y end or values = { { ... } }, detail = 32, colors = { \"chart1\", \"chart3\" } \
(a ramp by height), wire = bool }, s:mesh { vertices = { { x, y, z }, ... }, faces = { { 1, 2, 3 }, ... }, \
color }, s:line(a, b, { color, width }), s:path(points, { color, width }), s:point(at, { radius, \
color, hint = \"shown on hover\" }), s:label(at, text, { color, size }), s:grid { size = 10, step = 1 }, \
s:axes { size = 1, labels = { \"x\", \"y\", \"z\" } }. Physical colors may be \"#rrggbb\" in scenes \
(a material's own color: gold, glass, wood), and only there.\n\
- Assets: pass real files in show_visual's assets (name -> path) and scripts use them by name. \
s:model { asset = \"prism.glb\", at, rotate = { x, y, z } degrees, scale, fit = 2 (center it and size \
its longest side), color and material props override the model's own } draws a glTF model with its \
materials and textures. Solids take texture = \"wood.jpg\" (multiplied by color; mesh takes uvs = { { u, v \
}, ... } per vertex). s:environment { asset = \"studio.hdr\", intensity = 1, background = false } lights \
the scene from an equirectangular image (an HDR is best) that metal and glass reflect and glass bends; \
background = true also draws it behind the scene. s:sprite { at, size = 48 (pixels), color, glow = 1, \
asset = \"flare.png\" } adds a camera-facing light (a flare or glint; a soft round glow without an \
asset) that blooms. ui.shader takes textures = { \"rock.jpg\", ... } as iTexture0-iTexture3 \
(repeating, mipmapped). Find or make assets with your own tools (download, convert KTX2 or EXR to PNG \
or HDR) before calling.\n\
- ui.shader { height = 360, code = [[ GLSL ]], buffers = { [[ GLSL ]], ... }, inputs = { speed = \
s.speed }, blend = \"alpha|light|opaque\", bloom = 0-3, orbit = bool, spin, camera = { yaw, pitch, \
zoom }, animate = true }: GPU shaders for what geometry can't express: raymarched SDFs and \
fractals, light and optics (beams, refraction, dispersion), volumes, fluids, particles, generative \
art. Write each pass as on Shadertoy: void mainImage(out vec4 fragColor, in vec2 fragCoord), with \
iResolution (vec3: width and height in pixels, then the display's pixel density), iTime, \
iTimeDelta, iFrame, iMouse (pixels from the bottom-left; xy where the pointer is, zw where it went \
down while it's held, negative otherwise), iCamera (vec4 yaw, pitch, zoom, 0; orbit = true lets the reader drag \
it), every inputs name as a float (bind them to state so sliders and toggles drive the shader), \
iNoise (a tiling 256x256 random RGBA texture), and iChannel0-iChannel3 with iChannelResolution: \
buffers run first, in order, each into a float texture that every pass reads as iChannel<i> (an \
earlier buffer's current frame; a buffer's own previous frame), for multi-pass work like blur, \
trails, accumulation, and simulation. The shader sits on the chat, never in a box: don't paint a \
background, vignette, or floor color. By default (blend = \"light\") the output is emitted light: \
brightness is opacity and black is clear, right for beams, glows, sparks, and light through glass. \
For solid shaded objects (fractals, SDF bodies) set blend = \"alpha\" and write alpha 0 where the ray \
misses. bloom = 0.6-1.5 adds a soft glow around bright light. \
The width varies with the reader (a phone is about 350px wide, often narrower than the shader is \
tall): scale the scene by min(iResolution.x, iResolution.y), not one axis, and keep the subject \
centered with room on both sides, so it fits and stays centered at any aspect. \
GLSL errors come back with the pass and line number.\n\
- Helpers: ui.scale(d0, d1, r0, r1) returns a mapping function; ui.ticks(lo, hi, n); ui.fmt(n, { unit, \
prefix, decimals }) groups digits; ui.map(list, function(v, i) end); ui.range(a, b, step); ui.clamp; \
ui.lerp; ui.series(i) is \"chart<i>\"; ui.color(token, alpha); ui.mix(a, b, t). print(...) output comes \
back to you.\n\
\n\
Colors are theme tokens, never hex or color names (except scene materials): text; muted (labels, axes, captions); faint \
(tertiary notes); accent (the one thing to notice); on-accent (text on an accent mark); success, \
warning, danger (status only); border (hairlines, gridlines); chart1, chart2, chart3, chart4, \
chart5, chart6 (series, in order; \
chart1 is the accent). They follow the reader's theme, light or dark, on frosted glass or an opaque \
surface, so never assume a background color.\n\
\n\
Style, enforced: layout nodes have no fill, border, shadow, padding, or font props. A visual is part \
of the reply, not a widget, so lay content directly on the chat with type, spacing, and alignment: no \
cards, tiles, boxed panels, banners, or canvas backgrounds. Stats are a row of plain ui.stat figures; \
a chart is a plain heading, then the chart; separate sections with gap or one ui.rule. Text renders \
in OpenAgents's interface font at the reader's text size: body for most text, sm and xs for labels and \
notes, lg and xl for headings, 2xl for a headline figure. The width is the reply column (about 720px, \
as narrow as 360px), so lay out fluidly with grow, wrap, and grid, and give charts, canvases, and \
scenes heights of 160-420px.";

/// Read `show_visual`'s asset files (paths relative to the agent's working
/// directory), base64-encoded by name for the engine.
async fn read_visual_assets(
    assets: &std::collections::BTreeMap<String, String>,
) -> anyhow::Result<serde_json::Map<String, Value>> {
    use base64::Engine as _;
    use openagents_proto::{
        VISUAL_MAX_ASSET_BYTES as MAX_ASSET_BYTES, VISUAL_MAX_ASSETS as MAX_ASSETS,
        VISUAL_MAX_TOTAL_ASSET_BYTES as MAX_TOTAL_ASSET_BYTES,
        valid_visual_asset_name as valid_name,
    };
    anyhow::ensure!(
        assets.len() <= MAX_ASSETS,
        "a visual takes at most {MAX_ASSETS} assets"
    );
    let mut out = serde_json::Map::new();
    let mut total = 0;
    let mut seen = std::collections::HashSet::new();
    for (name, path) in assets {
        anyhow::ensure!(
            valid_name(name),
            "asset name '{name}' must be a plain file name (letters, digits, '.', '-', '_')"
        );
        // Stored as files, and file systems often ignore case.
        anyhow::ensure!(
            seen.insert(name.to_ascii_lowercase()),
            "asset names must differ by more than case ('{name}')"
        );
        let metadata = tokio::fs::metadata(path)
            .await
            .with_context(|| format!("asset '{name}': can't read {path}"))?;
        anyhow::ensure!(metadata.is_file(), "asset '{name}': {path} is not a file");
        anyhow::ensure!(
            metadata.len() <= MAX_ASSET_BYTES as u64,
            "asset '{name}' is over {} MB",
            MAX_ASSET_BYTES / (1024 * 1024)
        );
        // Bounded however the file changes (or lies about its size).
        let mut bytes = Vec::new();
        {
            use tokio::io::AsyncReadExt as _;
            tokio::fs::File::open(path)
                .await
                .with_context(|| format!("asset '{name}': can't read {path}"))?
                .take(MAX_ASSET_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .await
                .with_context(|| format!("asset '{name}': can't read {path}"))?;
        }
        anyhow::ensure!(
            bytes.len() <= MAX_ASSET_BYTES,
            "asset '{name}' is over {} MB",
            MAX_ASSET_BYTES / (1024 * 1024)
        );
        total += bytes.len();
        anyhow::ensure!(
            total <= MAX_TOTAL_ASSET_BYTES,
            "a visual's assets total at most {} MB",
            MAX_TOTAL_ASSET_BYTES / (1024 * 1024)
        );
        out.insert(
            name.clone(),
            Value::String(base64::engine::general_purpose::STANDARD.encode(bytes)),
        );
    }
    Ok(out)
}

// ---- argument shapes ---------------------------------------------------------

#[derive(Deserialize)]
struct ShowVisualArgs {
    script: String,
    title: String,
    #[serde(default)]
    assets: std::collections::BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct BatchArgs {
    requests: Vec<Value>,
}

#[derive(Deserialize)]
struct ChatArgs {
    chat: String,
}

#[derive(Deserialize)]
struct DecisionArgs {
    #[serde(default)]
    saw: String,
    #[serde(default)]
    decided: String,
    #[serde(default)]
    did: String,
    /// The whole working state, replacing the saved one (#12057).
    #[serde(default)]
    working: Option<Vec<openagents_proto::agent_wakes::WorkItem>>,
    /// Receipts from `judge` for the judgments this decision used.
    #[serde(default)]
    judgments: Vec<openagents_proto::agent_wakes::JudgmentReceipt>,
}

#[derive(Deserialize)]
struct ChangedPathsArgs {
    chats: Vec<ChangedPathsChat>,
    base: Option<String>,
}

#[derive(Deserialize)]
struct ChangedPathsChat {
    chat: String,
    dir: Option<String>,
}

#[derive(Deserialize)]
struct ListModelsArgs {
    harness: String,
    device: Option<String>,
}

#[derive(Deserialize, Default)]
struct ListChatsArgs {
    project: Option<String>,
    device: Option<String>,
    #[serde(default)]
    include_archived: bool,
    limit: Option<usize>,
}

#[derive(Deserialize, Default)]
struct DeviceArgs {
    device: Option<String>,
}

/// Messages already in the transcript before a send. Replies are matched by
/// id, not by time: `createdAt` is stamped on the host's clock, which can run
/// behind the caller's when the chat lives on another device.
#[derive(Clone, Default)]
struct TurnStart {
    message_ids: Vec<String>,
}

#[derive(Clone)]
struct PendingTurn {
    baseline: Option<Session>,
    start: TurnStart,
}

#[derive(Deserialize, Default)]
struct CreateChatArgs {
    project: Option<String>,
    device: Option<String>,
    harness: Option<String>,
    model: Option<String>,
    reasoning: Option<String>,
    sandbox: Option<String>,
    title: Option<String>,
    branch: Option<String>,
    cwd: Option<String>,
    prompt: Option<String>,
    #[serde(default)]
    wait: bool,
    timeout_secs: Option<u64>,
}

#[derive(Deserialize)]
struct ReadChatArgs {
    chat: String,
    limit: Option<usize>,
    #[serde(default)]
    offset: usize,
    #[serde(default)]
    include_reasoning: bool,
    include_tools: Option<bool>,
}

#[derive(Deserialize)]
struct SendArgs {
    chat: String,
    text: String,
    mode: Option<String>,
    #[serde(default)]
    wait: bool,
    timeout_secs: Option<u64>,
}

#[derive(Deserialize)]
struct WaitArgs {
    chat: String,
    timeout_secs: Option<u64>,
}

#[derive(Deserialize)]
struct ArchiveArgs {
    chat: String,
    archived: Option<bool>,
}

#[derive(Deserialize)]
struct PinArgs {
    chat: String,
    pinned: Option<bool>,
}

#[derive(Deserialize)]
struct PlanMoveArgs {
    #[serde(default)]
    chats: Vec<String>,
    computer: Option<String>,
}

#[derive(Deserialize)]
struct PlanIdArgs {
    plan_id: Option<String>,
}

#[derive(Deserialize)]
struct BringBackArgs {
    chat: String,
    #[serde(default)]
    after: Option<openagents_rpc::moves::AfterBringBack>,
}

/// What the agent tells the person when a move waits for them.
const APPROVE_IN_APP: &str = "Waiting for approval: the person using this computer approves moves and Bring backs in the OpenAgents app.";

/// A plan as the tools return it, with the next step in words.
fn plan_reply(plan: Value) -> anyhow::Result<Value> {
    let view: openagents_rpc::moves::PlanView = serde_json::from_value(plan.clone())?;
    let mut reply = plan;
    reply["estimate"] = json!(openagents_rpc::moves::estimate_label(view.estimate_minutes));
    if view.needs_approval() {
        reply["next"] = json!(APPROVE_IN_APP);
    }
    Ok(reply)
}

#[derive(Deserialize)]
struct RespondArgs {
    chat: String,
    request_id: Option<String>,
    #[serde(default)]
    answers: Vec<AnswerArg>,
}

#[derive(Deserialize)]
struct AnswerArg {
    question_id: String,
    labels: Vec<String>,
}

fn parse<T: serde::de::DeserializeOwned>(args: Value) -> Result<T, String> {
    serde_json::from_value(args).map_err(|e| format!("invalid arguments: {e}"))
}

/// The wait to use, and a note for the result when the request was over the cap.
fn wait_duration(secs: Option<u64>) -> (Duration, Option<String>) {
    let requested = secs.unwrap_or(MAX_WAIT_SECS).max(1);
    let used = requested.min(MAX_WAIT_SECS);
    let note = (requested > used).then(|| {
        format!("timeout_secs is capped at {MAX_WAIT_SECS}; waited {used} seconds instead of {requested}. Call wait_for_turn to keep waiting.")
    });
    (Duration::from_secs(used), note)
}

#[cfg(test)]
fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

fn parse_enum<T: serde::de::DeserializeOwned>(what: &str, raw: &str) -> Result<T, String> {
    serde_json::from_value(Value::String(raw.trim().to_owned()))
        .map_err(|_| format!("unknown {what}: {raw:?}"))
}

// ---- summaries ---------------------------------------------------------------

/// Live posture of one chat as the tools report it.
fn status_of(session: Option<&Session>) -> (String, Option<i64>) {
    let Some(session) = session else {
        return ("idle".into(), None);
    };
    let age = chrono::Utc::now() - session.updated_at;
    let stale = age > SESSION_STALE;
    let label = match session.status {
        SessionStatus::Working if stale => "working (stale — host may be offline)",
        SessionStatus::Working => "working",
        SessionStatus::AwaitingInput => "awaitingInput",
        SessionStatus::Errored => "errored",
        SessionStatus::Idle => "idle",
    };
    (label.into(), Some(age.num_seconds().max(0)))
}

fn summarize_chat(
    chat: &Chat,
    spaces: &[Space],
    sessions: &[Session],
    sidebar: &SidebarPreferencesState,
) -> Value {
    let space = chat
        .space_id
        .as_deref()
        .and_then(|id| spaces.iter().find(|s| s.id == id));
    let session = session_for(sessions, chat);
    let (status, status_age) = status_of(session.as_ref());
    json!({
        "id": chat.id,
        "title": chat.title,
        "project": space.map(|s| json!({
            "id": s.id,
            "name": s.display_name(),
            "path": s.path,
        })),
        "deviceId": chat.device_id,
        "cwd": chat.cwd.clone().or_else(|| space.map(|s| s.path.clone())),
        "branch": chat.branch,
        "harness": chat.config.as_ref().map(|c| c.harness),
        "model": chat.config.as_ref().and_then(|c| c.model.clone()),
        "reasoning": chat.config.as_ref().and_then(|c| c.reasoning),
        "archived": chat.archived,
        "pinned": sidebar.can_edit().then(|| sidebar.pinned_session_ids.contains(&chat.id)),
        // The harness's own session id (Claude Code session, Codex thread):
        // the id an issue claim marker names (`session=claude-code:ID`).
        // An empty id is the engine's "do not resume" tombstone, not a session.
        "harnessSessionId": chat.harness_session_id.as_deref().filter(|id| !id.is_empty()),
        "status": status,
        "statusAgeSecs": status_age,
        "lastMessageAt": chat.last_message_at,
        "lastMessagePreview": chat.last_message_preview,
        "createdAt": chat.created_at,
    })
}

fn last_pending_input(messages: &[RenderedMessage]) -> Option<Value> {
    messages.iter().rev().find_map(|m| m.pending_input.clone())
}

// ---- dispatch ----------------------------------------------------------------

impl Tools {
    fn voice_relay(&self) -> bool {
        self.openagents
            .origin()
            .chat_id
            .as_deref()
            .is_some_and(openagents_proto::voice::is_orchestrator_chat)
    }

    fn voice_tool(name: &str) -> bool {
        matches!(name, "get_voice_agent" | "send_message" | "wait_for_turn")
    }

    pub fn new(openagents: Arc<OpenAgents>) -> Self {
        Self {
            openagents,
            pending_turns: Default::default(),
            inbox: Default::default(),
            decider: Arc::new(crate::judge::Api),
        }
    }

    /// The same tools with `judge` answered by `decider`.
    #[must_use]
    pub fn with_decider(mut self, decider: Arc<dyn crate::judge::Decider>) -> Self {
        self.decider = decider;
        self
    }

    pub fn list(&self) -> Vec<ToolDef> {
        let in_chat = self.openagents.origin().chat_id.is_some();
        catalog()
            .into_iter()
            .chain(std::iter::once(ToolDef {
                name: "get_voice_agent",
                description: "Read the account's selected saved agent and chat for the next spoken question. Re-read before each send so a change during the call takes effect.",
                input_schema: json!({"type":"object","properties":{},"additionalProperties":false}),
            }))
            .filter(|tool| {
                in_chat
                    || (tool.name != SHOW_VISUAL
                        && tool.name != RECORD_DECISION
                        && tool.name != "notify_owner")
            })
            .chain(crate::video_lab::tools().iter().cloned())
            .chain(std::iter::once(crate::work::tool()))
            .chain(std::iter::once(crate::judge::tool()))
            .chain(build_status_tool())
            .chain(list_runs_tool())
            .chain(machine_action_tool())
            .chain(upgrade_host_tool().filter(|_| in_chat))
            .chain(machine_status_tool())
            .filter(|tool| !self.voice_relay() || Self::voice_tool(tool.name))
            .collect()
    }

    pub fn has(&self, name: &str) -> bool {
        self.list().iter().any(|t| t.name == name)
    }

    /// `Ok` is the tool's structured result; `Err` is a message the model
    /// should read (surfaced as `isError`, never as a protocol error).
    pub async fn call(&self, name: &str, args: Value) -> Result<Value, String> {
        // Keep callers' futures small as tools add more concurrent reads.
        Box::pin(self.dispatch(name, args)).await
    }

    async fn dispatch(&self, name: &str, args: Value) -> Result<Value, String> {
        if self.voice_relay() && !Self::voice_tool(name) {
            return Err("Voice can only ask your chosen agent and wait for its answer.".into());
        }
        let result = match name {
            "get_voice_agent" => {
                self.openagents
                    .call(openagents_rpc::methods::GET_VOICE_AGENT, json!({}))
                    .await
            }
            crate::work::NAME => return crate::work::call(&self.openagents, args).await,
            "whoami" => self.whoami().await,
            "list_devices" => self.list_devices().await,
            "land_status" => {
                self.openagents
                    .call(openagents_rpc::methods::LAND_STATUS, json!({}))
                    .await
            }
            "list_projects" => self.list_projects(parse(args)?).await,
            "list_harnesses" => self.list_harnesses(parse(args)?).await,
            "list_models" => self.list_models(parse(args)?).await,
            "list_chats" => self.list_chats(parse(args)?).await,
            "get_chat" => self.get_chat(parse(args)?).await,
            "changed_paths" => self.changed_paths(parse(args)?).await,
            "create_chat" => self.create_chat(parse(args)?).await,
            "create_chats" => self.batch(parse(args)?, true).await,
            "send_messages" => self.batch(parse(args)?, false).await,
            "read_chat" => self.read_chat(parse(args)?).await,
            "send_message" => self.send_message(parse(args)?).await,
            "wait_for_turn" => self.wait_for_turn(parse(args)?).await,
            "interrupt_chat" => self.interrupt_chat(parse(args)?).await,
            "respond_to_input" => self.respond_to_input(parse(args)?).await,
            "archive_chat" => self.archive_chat(parse(args)?).await,
            "pin_chat" => self.pin_chat(parse(args)?).await,
            "plan_move" => self.plan_move(parse(args)?).await,
            "start_move" => self.start_move(parse(args)?).await,
            "move_status" => self.move_status(parse(args)?).await,
            "bring_back" => self.bring_back(parse(args)?).await,
            SHOW_VISUAL => self.show_visual(parse(args)?).await,
            RECORD_DECISION => self.record_decision(parse(args)?).await,
            "notify_owner" => {
                #[derive(Deserialize)]
                #[serde(deny_unknown_fields)]
                struct Note {
                    request_id: String,
                    title: String,
                    body: String,
                }
                let note: Note = parse(args)?;
                let chat = self
                    .openagents
                    .origin()
                    .chat_id
                    .as_ref()
                    .ok_or_else(|| "Choose a saved agent's chat.".to_owned())?;
                self.openagents.call(openagents_rpc::methods::NOTIFY_OWNER, json!({"originChatId":chat,"requestId":note.request_id,"title":note.title,"body":note.body})).await
            }
            crate::judge::NAME => return crate::judge::call(self.decider.as_ref(), args).await,
            #[cfg(unix)]
            crate::build_status::NAME => return crate::build_status::call(args),
            #[cfg(unix)]
            crate::machine_status::NAME => {
                return crate::machine_status::call(
                    self.openagents.origin().chat_id.as_deref(),
                    args,
                )
                .await;
            }
            #[cfg(unix)]
            crate::runs::NAME => return crate::runs::call(args).await,
            #[cfg(unix)]
            crate::machine_action::NAME => {
                let chat = self.openagents.origin().chat_id.clone();
                return crate::machine_action::call(chat.as_deref(), args);
            }
            #[cfg(unix)]
            crate::upgrade_host::NAME => {
                let chat = self.openagents.origin().chat_id.clone();
                return crate::upgrade_host::call(args, chat.as_deref());
            }
            lab if crate::video_lab::has(lab) => return self.video_lab(lab, args).await,
            other => return Err(format!("unknown tool: {other}")),
        };
        result.map_err(|e| format!("{e:#}"))
    }

    /// Poll every request together, including its optional wait. A waiting
    /// first chat must not prevent subsequent chats from receiving their work.
    async fn batch(&self, args: BatchArgs, create: bool) -> anyhow::Result<Value> {
        anyhow::ensure!(
            (1..=MAX_BATCH).contains(&args.requests.len()),
            "requests must contain between 1 and {MAX_BATCH} items"
        );
        let results = futures::future::join_all(args.requests.into_iter().enumerate().map(
            |(index, args)| async move {
                let result = if create {
                    match serde_json::from_value(args) {
                        Ok(args) => self.create_chat(args).await,
                        Err(error) => Err(error.into()),
                    }
                } else {
                    match serde_json::from_value(args) {
                        Ok(args) => self.send_message(args).await,
                        Err(error) => Err(error.into()),
                    }
                };
                match result {
                    Ok(result) => json!({"index": index, "isError": false, "result": result}),
                    Err(error) => {
                        json!({"index": index, "isError": true, "error": error.to_string()})
                    }
                }
            },
        ))
        .await;
        Ok(json!({"results": results}))
    }

    async fn whoami(&self) -> anyhow::Result<Value> {
        let origin = self.openagents.origin().clone();
        let local_device = self.openagents.local_device_id().await?;
        let engine = self.openagents.engine_info().await.unwrap_or(Value::Null);
        let chat = match origin.chat_id.as_deref() {
            Some(id) => match self.openagents.resolve_chat(id).await {
                Ok(chat) => {
                    let (spaces, sessions, sidebar) = tokio::try_join!(
                        self.openagents.spaces(),
                        self.openagents.sessions(),
                        self.openagents.sidebar_preferences()
                    )?;
                    summarize_chat(&chat, &spaces, &sessions, &sidebar)
                }
                Err(_) => json!({ "id": id }),
            },
            None => Value::Null,
        };
        Ok(json!({
            "chat": chat,
            "originDeviceId": origin.device_id,
            "localDeviceId": local_device,
            "workspaceScope": engine.get("workspaceScope").cloned().unwrap_or(Value::Null),
            "note": if origin.chat_id.is_some() {
                "Messages you send are attributed to this chat; it cannot message itself."
            } else {
                "Not running inside a chat: messages are sent without attribution."
            },
        }))
    }

    async fn list_devices(&self) -> anyhow::Result<Value> {
        let (devices, local) =
            tokio::try_join!(self.openagents.devices(), self.openagents.local_device_id())?;
        Ok(json!({
            "devices": devices.iter().map(|d| json!({
                "id": d.id,
                "name": d.name,
                "platform": d.platform,
                "local": d.id == local,
                "lastSeenAt": d.last_seen_at,
                "version": d.version,
            })).collect::<Vec<_>>()
        }))
    }

    async fn list_projects(&self, args: DeviceArgs) -> anyhow::Result<Value> {
        let (mut spaces, devices) = tokio::try_join!(self.openagents.spaces(), self.openagents.devices())?;
        if let Some(device) = args.device.as_deref() {
            let device = self.openagents.resolve_device_id(Some(device)).await?;
            spaces.retain(|s| s.device_id == device);
        }
        let device_name = |id: &str| devices.iter().find(|d| d.id == id).map(|d| d.name.clone());
        Ok(json!({
            "projects": spaces.iter().map(|s| json!({
                "id": s.id,
                "name": s.display_name(),
                "path": s.path,
                "deviceId": s.device_id,
                "deviceName": device_name(&s.device_id),
                "git": s.git_detected,
            })).collect::<Vec<_>>()
        }))
    }

    async fn list_harnesses(&self, args: DeviceArgs) -> anyhow::Result<Value> {
        let device = match args.device.as_deref() {
            Some(key) => Some(self.openagents.resolve_device_id(Some(key)).await?),
            None => None,
        };
        let harnesses = self.openagents.harnesses_on(device.as_deref()).await?;
        Ok(json!({
            "harnesses": harnesses.iter().map(|h| json!({
                "id": h.id,
                "name": h.name,
                "available": h.available(),
                "installed": h.installed,
                "steersMidTurn": h.steers_mid_turn(),
                "reasoningLevels": h.reasoning_levels,
            })).collect::<Vec<_>>()
        }))
    }

    async fn list_models(&self, args: ListModelsArgs) -> anyhow::Result<Value> {
        let harness: HarnessId =
            parse_enum("harness", &args.harness).map_err(anyhow::Error::msg)?;
        let device = match args.device.as_deref() {
            Some(key) => Some(self.openagents.resolve_device_id(Some(key)).await?),
            None => None,
        };
        let models = self.openagents.models_on(harness, device.as_deref()).await?;
        Ok(json!({
            "harness": harness,
            "models": models.iter().map(|m| json!({
                "id": m.id,
                "label": m.label,
                "description": m.description,
                "reasoningLevels": m.reasoning_levels,
            })).collect::<Vec<_>>()
        }))
    }

    async fn list_chats(&self, args: ListChatsArgs) -> anyhow::Result<Value> {
        let (mut chats, spaces, sessions, sidebar) = tokio::try_join!(
            self.openagents.chats(),
            self.openagents.spaces(),
            self.openagents.sessions(),
            self.openagents.sidebar_preferences()
        )?;
        if let Some(project) = args.project.as_deref() {
            let (space, _) = self
                .openagents
                .resolve_target(Some(project), args.device.as_deref())
                .await?;
            let space = space.expect("project provided");
            chats.retain(|c| c.space_id.as_deref() == Some(space.id.as_str()));
        }
        if args.device.is_some() {
            let device = self.openagents.resolve_device_id(args.device.as_deref()).await?;
            chats.retain(|c| c.device_id == device);
        }
        if !args.include_archived {
            chats.retain(|c| !c.archived);
        }
        chats.sort_by(|a, b| {
            let a_at = a.last_message_at.unwrap_or(a.created_at);
            let b_at = b.last_message_at.unwrap_or(b.created_at);
            b_at.cmp(&a_at)
        });
        let total = chats.len();
        let limit = args.limit.unwrap_or(50).clamp(1, 500);
        Ok(json!({
            "total": total,
            "chats": chats.iter().take(limit)
                .map(|c| summarize_chat(c, &spaces, &sessions, &sidebar))
                .collect::<Vec<_>>()
        }))
    }

    async fn get_chat(&self, args: ChatArgs) -> anyhow::Result<Value> {
        let chat = self.openagents.resolve_chat(&args.chat).await?;
        let (spaces, sessions, entries, sidebar) = tokio::try_join!(
            self.openagents.spaces(),
            self.openagents.sessions(),
            self.openagents
                .transcript_on(&chat.id, Some(&chat.device_id)),
            self.openagents.sidebar_preferences()
        )?;
        let rendered = render_entries(&entries, RenderOptions::default());
        let mut summary = summarize_chat(&chat, &spaces, &sessions, &sidebar);
        summary["messageCount"] = json!(rendered.len());
        summary["pendingInput"] = last_pending_input(&rendered).unwrap_or(Value::Null);
        summary["lastMessage"] = rendered.last().map(|m| json!(m)).unwrap_or(Value::Null);
        Ok(summary)
    }

    async fn changed_paths(&self, args: ChangedPathsArgs) -> anyhow::Result<Value> {
        anyhow::ensure!(
            (1..=MAX_BATCH).contains(&args.chats.len()),
            "chats must contain between 1 and {MAX_BATCH} items"
        );
        let base = args
            .base
            .as_deref()
            .unwrap_or(crate::changed_paths::DEFAULT_BASE);
        let (local, spaces) =
            tokio::try_join!(self.openagents.local_device_id(), self.openagents.spaces())?;
        let reports = futures::future::join_all(args.chats.into_iter().map(|item| {
            let (local, spaces) = (&local, &spaces);
            async move {
                let chat = match self.openagents.resolve_chat(&item.chat).await {
                    Ok(chat) => chat,
                    Err(e) => {
                        return (
                            None,
                            json!({ "chat": item.chat, "error": format!("{e:#}") }),
                        );
                    }
                };
                let dir = item.dir.clone().or_else(|| {
                    let space = chat
                        .space_id
                        .as_deref()
                        .and_then(|id| spaces.iter().find(|s| s.id == id));
                    chat.harness_session_cwd
                        .clone()
                        .or_else(|| chat.cwd.clone())
                        .or_else(|| space.map(|s| s.path.clone()))
                });
                let mut report = json!({ "chat": chat.id, "title": chat.title, "dir": dir });
                let result = match &dir {
                    _ if item.dir.is_none() && chat.device_id != *local => Err(anyhow::anyhow!(
                        "this chat runs on another device; pass dir for a local copy"
                    )),
                    None => Err(anyhow::anyhow!("this chat has no folder; pass dir")),
                    Some(dir) => {
                        crate::changed_paths::changed_files(std::path::Path::new(dir), base).await
                    }
                };
                match result {
                    Ok(paths) => {
                        report["paths"] = json!(paths);
                        (Some((chat.id, paths)), report)
                    }
                    Err(e) => {
                        report["error"] = json!(format!("{e:#}"));
                        (None, report)
                    }
                }
            }
        }))
        .await;
        let read: Vec<(String, Vec<String>)> =
            reports.iter().filter_map(|(r, _)| r.clone()).collect();
        let mut out = crate::changed_paths::overlaps(&read);
        out["base"] = json!(base);
        out["chats"] = json!(reports.into_iter().map(|(_, r)| r).collect::<Vec<_>>());
        Ok(out)
    }

    async fn create_chat(&self, args: CreateChatArgs) -> anyhow::Result<Value> {
        // Every chat an agent creates is standalone and listed in Sessions.
        // The chat it was created from is reported back, never stored as a
        // parent (older `kind` and `parent` arguments are ignored).
        let origin = self
            .openagents
            .origin()
            .chat_id
            .clone()
            .filter(|origin| !openagents_proto::voice::is_orchestrator_chat(origin));
        let (space, device_id) = self
            .openagents
            .resolve_target(args.project.as_deref(), args.device.as_deref())
            .await?;
        let harnesses = self.openagents.harnesses_on(Some(&device_id)).await?;
        let harness = match args.harness.as_deref() {
            Some(raw) => {
                let id: HarnessId = parse_enum("harness", raw).map_err(anyhow::Error::msg)?;
                anyhow::ensure!(
                    harnesses.iter().any(|h| h.id == id && h.available()),
                    "harness {raw} is not available on device {device_id} (see list_harnesses with device)"
                );
                id
            }
            None => default_harness(&harnesses).with_context(|| format!("device {device_id}"))?,
        };
        if let Some(model) = args.model.as_deref() {
            let models = self.openagents.models_on(harness, Some(&device_id)).await?;
            anyhow::ensure!(
                models.iter().any(|m| m.id == model),
                "model {model:?} is not offered by {harness:?} on device {device_id}; available: {}",
                models
                    .iter()
                    .map(|m| m.id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        let reasoning: Option<ReasoningLevel> = match args.reasoning.as_deref() {
            Some(raw) => Some(parse_enum("reasoning level", raw).map_err(anyhow::Error::msg)?),
            None => None,
        };
        let sandbox: SandboxLevel = match args.sandbox.as_deref() {
            Some(raw) => parse_enum("sandbox", raw).map_err(anyhow::Error::msg)?,
            None => SandboxLevel::WorkspaceWrite,
        };
        let config = ChatConfig {
            harness,
            model: args.model.clone(),
            reasoning,
            model_options: Default::default(),
            sandbox,
        };

        let chat_id = uuid::Uuid::new_v4().to_string();
        let mut mutate = json!({
            "op": "createChat",
            "chatId": chat_id,
            "deviceId": device_id,
            "config": config,
        });
        if let Some(space) = &space {
            mutate["spaceId"] = json!(space.id);
        }
        if let Some(branch) = args
            .branch
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
        {
            mutate["branch"] = json!(branch);
        }
        let mut cwd = args
            .cwd
            .as_deref()
            .map(str::trim)
            .filter(|c| !c.is_empty())
            .map(str::to_owned);
        // A projectless chat runs in its own scratch folder on a host that
        // creates them; naming it here keeps createChat and the first run
        // in agreement.
        if space.is_none()
            && cwd.is_none()
            && self.openagents.devices().await?.iter().any(|d| {
                d.id == device_id
                    && d.capabilities
                        .iter()
                        .any(|c| c == openagents_proto::capabilities::PROJECTLESS_SCRATCH_V1)
            })
        {
            cwd = Some(openagents_proto::projectless::scratch_cwd(
                &chat_id,
                chrono::Utc::now(),
            ));
        }
        if let Some(cwd) = &cwd {
            mutate["cwd"] = json!(cwd);
        }
        self.openagents.mutate(mutate).await?;
        if let Some(title) = args
            .title
            .as_deref()
            .map(str::trim)
            .filter(|t| !t.is_empty())
        {
            self.openagents
                .mutate(json!({ "op": "renameChat", "chatId": chat_id, "title": title }))
                .await?;
        }

        let mut result = json!({
            "chatId": chat_id,
            "kind": "chat",
            "deviceId": device_id,
            "project": space.as_ref().map(|s| json!({ "id": s.id, "name": s.display_name(), "path": s.path })),
            "harness": harness,
            "model": args.model,
            "reasoning": reasoning,
            "title": args.title,
            "createdFrom": origin,
        });
        if let Some(prompt) = args.prompt.filter(|p| !p.trim().is_empty()) {
            // The row may not have folded into WatchChats yet; build the
            // chat locally from what we just wrote rather than re-reading.
            let chat = Chat {
                links: Default::default(),
                id: chat_id.clone(),
                device_id: device_id.clone(),
                title: args.title.clone(),
                archived: false,
                cwd: cwd.clone(),
                branch: args.branch.clone(),
                checkout_id: None,
                source_context: None,
                config: Some(config),
                last_message_preview: None,
                last_message_at: None,
                created_at: chrono::Utc::now(),
                harness_session_id: None,
                harness_session_cwd: None,
                parent_chat_id: None,
                space_id: space.as_ref().map(|s| s.id.clone()),
                last_seen_at: None,
                room_gen: None,
            };
            let sent = self
                .deliver(&chat, space.as_ref(), &harnesses, None, prompt, "run")
                .await?;
            result["sent"] = sent;
            let pending = Arc::new(PendingTurn {
                baseline: None,
                // A new chat has no earlier messages.
                start: TurnStart::default(),
            });
            self.pending_turns
                .lock()
                .await
                .insert(chat.id.clone(), pending.clone());
            if args.wait {
                result["turn"] = self
                    .await_pending_turn(&chat, pending, wait_duration(args.timeout_secs))
                    .await?;
            }
        }
        Ok(result)
    }

    async fn read_chat(&self, args: ReadChatArgs) -> anyhow::Result<Value> {
        let chat = self.openagents.resolve_chat(&args.chat).await?;
        let entries = self
            .openagents
            .transcript_on(&chat.id, Some(&chat.device_id))
            .await?;
        let rendered = render_entries(
            &entries,
            RenderOptions {
                include_reasoning: args.include_reasoning,
                include_tools: args.include_tools.unwrap_or(true),
            },
        );
        let total = rendered.len();
        let limit = args.limit.unwrap_or(40).clamp(1, 500);
        let end = total.saturating_sub(args.offset);
        let start = end.saturating_sub(limit);
        let window = &rendered[start..end];
        Ok(json!({
            "chatId": chat.id,
            "title": chat.title,
            "total": total,
            "returned": window.len(),
            "olderRemaining": start,
            "newerSkipped": total - end,
            "pendingInput": last_pending_input(&rendered),
            "messages": window,
        }))
    }

    async fn send_message(&self, mut args: SendArgs) -> anyhow::Result<Value> {
        let voice = self.voice_relay();
        let mut target_host = None;
        let mut context = Value::Null;
        if voice {
            // Re-read here too: a choice can change after the model read it.
            let choice = self
                .openagents
                .call(openagents_rpc::methods::GET_VOICE_AGENT, json!({}))
                .await?;
            args.chat = choice["target"]["chat_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Choose a voice agent in Settings."))?
                .to_owned();
            target_host = Some(
                choice["target"]["host_id"]
                    .as_str()
                    .ok_or_else(|| anyhow::anyhow!("Choose a voice agent in Settings."))?
                    .to_owned(),
            );
            context = self
                .openagents
                .call(
                    openagents_rpc::methods::GET_VOICE_CONTEXT,
                    json!({"chatId":self.openagents.origin().chat_id}),
                )
                .await?;
            args.mode = Some("auto".into());
            args.wait = false;
        }
        let text = args.text.trim();
        if text.is_empty() {
            anyhow::bail!("text is empty");
        }
        let chat = self.openagents.resolve_chat(&args.chat).await?;
        if let Some(host) = target_host {
            anyhow::ensure!(
                chat.id == args.chat && chat.device_id == host,
                "The chosen agent's chat is unavailable. Choose another voice agent in Settings."
            );
        }
        if self.openagents.origin().chat_id.as_deref() == Some(chat.id.as_str()) {
            anyhow::bail!(
                "refusing to send a message to your own chat ({})",
                short(&chat.id)
            );
        }
        let (spaces, sessions, harnesses) = tokio::try_join!(
            self.openagents.spaces(),
            self.openagents.sessions(),
            self.openagents.harnesses_on(Some(&chat.device_id))
        )?;
        let space = chat
            .space_id
            .as_deref()
            .and_then(|id| spaces.iter().find(|s| s.id == id));
        let baseline = session_for(&sessions, &chat);
        let mode = if voice
            && baseline
                .as_ref()
                .is_some_and(|s| s.status == SessionStatus::AwaitingInput)
        {
            "queue"
        } else {
            args.mode.as_deref().unwrap_or("auto")
        };
        let message_ids = self
            .openagents
            .transcript_on(&chat.id, Some(&chat.device_id))
            .await?
            .into_iter()
            .map(|m| m.id)
            .collect();
        let body = if voice {
            let mut body = format!("Spoken:\n\n{text}");
            if let Some(id) = context["chatId"].as_str() {
                let label = match context["title"].as_str() {
                    Some(title) => format!("{} ({id})", serde_json::to_string(title)?),
                    None => id.to_owned(),
                };
                body.push_str(&format!("\n\n[Open chat: {label}]"));
            }
            body
        } else {
            self.attribute(&chat, text).await
        };
        let mut result = json!({
            "chatId": chat.id,
            "title": chat.title,
        });
        result["sent"] = self
            .deliver(&chat, space, &harnesses, baseline.as_ref(), body, mode)
            .await?;
        let pending = Arc::new(PendingTurn {
            baseline,
            start: TurnStart { message_ids },
        });
        self.pending_turns
            .lock()
            .await
            .insert(chat.id.clone(), pending.clone());
        if args.wait {
            result["turn"] = self
                .await_pending_turn(&chat, pending, wait_duration(args.timeout_secs))
                .await?;
        }
        Ok(result)
    }

    async fn wait_for_turn(&self, args: WaitArgs) -> anyhow::Result<Value> {
        if self.voice_relay() {
            anyhow::ensure!(
                self.pending_turns.lock().await.contains_key(&args.chat),
                "Wait for the chat returned by send_message."
            );
        }
        let chat = self.openagents.resolve_chat(&args.chat).await?;
        let pending = self.pending_turns.lock().await.get(&chat.id).cloned();
        let turn = match pending {
            Some(pending) => {
                self.await_pending_turn(&chat, pending, wait_duration(args.timeout_secs))
                    .await?
            }
            None => {
                self.await_turn(
                    &chat,
                    None,
                    false,
                    wait_duration(args.timeout_secs),
                    TurnStart::default(),
                )
                .await?
            }
        };
        Ok(json!({ "chatId": chat.id, "title": chat.title, "turn": turn }))
    }

    async fn archive_chat(&self, args: ArchiveArgs) -> anyhow::Result<Value> {
        let chat = self.openagents.resolve_chat(&args.chat).await?;
        let archived = args.archived.unwrap_or(true);
        self.openagents
            .mutate(json!({ "op": "setChatArchived", "chatId": chat.id, "archived": archived }))
            .await?;
        Ok(json!({ "chatId": chat.id, "title": chat.title, "archived": archived }))
    }

    async fn pin_chat(&self, args: PinArgs) -> anyhow::Result<Value> {
        let chat = self.openagents.resolve_chat(&args.chat).await?;
        let pinned = args.pinned.unwrap_or(true);
        let sidebar = self.openagents.sidebar_preferences().await?;
        anyhow::ensure!(
            sidebar.can_edit(),
            "Sidebar pins are still loading. Try again."
        );
        if sidebar.pinned_session_ids.contains(&chat.id) != pinned {
            let change = if pinned {
                SidebarPinChange::Pin {
                    session_id: chat.id.clone(),
                    after: None,
                    before: None,
                }
            } else {
                SidebarPinChange::Unpin {
                    session_id: chat.id.clone(),
                }
            };
            self.openagents
                .mutate(json!({ "op": "changeSidebarPin", "change": change }))
                .await?;
        }
        Ok(json!({ "chatId": chat.id, "title": chat.title, "pinned": pinned }))
    }

    async fn plan_move(&self, args: PlanMoveArgs) -> anyhow::Result<Value> {
        let mut chats = Vec::new();
        for key in &args.chats {
            chats.push(self.openagents.resolve_chat(key).await?.id);
        }
        let plan = self
            .openagents
            .call(
                openagents_rpc::moves::PLAN_MOVE,
                json!({ "chats": chats, "computer": args.computer, "requestedBy": "agent" }),
            )
            .await?;
        plan_reply(plan)
    }

    async fn start_move(&self, args: PlanIdArgs) -> anyhow::Result<Value> {
        let plan_id = args.plan_id.context("plan_id is required")?;
        let plan = self
            .openagents
            .call(
                openagents_rpc::moves::START_MOVE,
                json!({ "planId": plan_id }),
            )
            .await?;
        plan_reply(plan)
    }

    async fn move_status(&self, args: PlanIdArgs) -> anyhow::Result<Value> {
        let status = self
            .openagents
            .call(
                openagents_rpc::moves::MOVE_STATUS,
                json!({ "planId": args.plan_id }),
            )
            .await?;
        let plans = status["plans"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(plan_reply)
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok(json!({ "plans": plans }))
    }

    async fn bring_back(&self, args: BringBackArgs) -> anyhow::Result<Value> {
        let chat = self.openagents.resolve_chat(&args.chat).await?;
        let plan = self
            .openagents
            .call(
                openagents_rpc::moves::BRING_BACK,
                json!({ "chat": chat.id, "after": args.after.unwrap_or_default(), "requestedBy": "agent" }),
            )
            .await?;
        plan_reply(plan)
    }

    /// An agent's decision log lives on its chat's host (#11965, #12044).
    async fn record_decision(&self, args: DecisionArgs) -> anyhow::Result<Value> {
        let origin = self
            .openagents
            .origin()
            .chat_id
            .clone()
            .context("record_decision only works from an agent's own chat")?;
        self.openagents
            .call(
                openagents_rpc::methods::RECORD_AGENT_DECISION,
                json!({
                    "originChatId": origin,
                    "saw": args.saw,
                    "decided": args.decided,
                    "did": args.did,
                    "working": args.working,
                    "judgments": args.judgments,
                }),
            )
            .await?;
        Ok(json!({ "recorded": true }))
    }

    async fn show_visual(&self, args: ShowVisualArgs) -> anyhow::Result<Value> {
        let chat_id = self
            .openagents
            .origin()
            .chat_id
            .clone()
            .context("show_visual only works from an agent OpenAgents launched in a chat")?;
        let assets = read_visual_assets(&args.assets).await?;
        let shown = self
            .openagents
            .call(
                openagents_rpc::methods::PUBLISH_VISUAL,
                json!({
                    "chatId": chat_id,
                    "title": args.title,
                    "script": args.script,
                    "assets": assets,
                }),
            )
            .await?;
        Ok(json!({
            "visual": shown,
            "message": "Shown to the reader above your reply. Don't mention or describe the visual; reply with only what it doesn't already say.",
        }))
    }

    /// Run a Video Lab tool and, inside a chat, show its card above the
    /// reply (the same native visual `show_visual` publishes).
    async fn video_lab(&self, name: &str, args: Value) -> Result<Value, String> {
        let mut result = crate::video_lab::call(name, args).await?;
        let Some(chat_id) = self.openagents.origin().chat_id.clone() else {
            return Ok(result);
        };
        let Some(card) = result.get("card").cloned() else {
            return Ok(result);
        };
        // A platform card's document lives in the space the result names.
        let space = result.get("space").and_then(Value::as_str);
        let Some(visual) = crate::video_lab::card_visual(&card, space).await else {
            return Ok(result);
        };
        let mut paths = std::collections::BTreeMap::new();
        if let Some((name, path)) = &visual.picture {
            paths.insert(name.clone(), path.display().to_string());
        }
        let shown = match read_visual_assets(&paths).await {
            Ok(assets) => self
                .openagents
                .call(
                    openagents_rpc::methods::PUBLISH_VISUAL,
                    json!({
                        "chatId": chat_id,
                        "title": visual.title,
                        "script": visual.script,
                        "assets": assets,
                    }),
                )
                .await
                .map_err(|e| format!("{e:#}")),
            Err(e) => Err(format!("{e:#}")),
        };
        if let Some(object) = result.as_object_mut() {
            match shown {
                Ok(_) => {
                    object.insert(
                        "shown".into(),
                        json!("OpenAgents shows this result's Video Lab card (picture, render state, links to play it and to open the Lab player) above your reply. Don't describe the card; end with the player link."),
                    );
                }
                Err(error) => {
                    tracing::warn!(%error, "video lab card not shown");
                }
            }
        }
        Ok(result)
    }

    async fn interrupt_chat(&self, args: ChatArgs) -> anyhow::Result<Value> {
        let chat = self.openagents.resolve_chat(&args.chat).await?;
        let command_id = self
            .openagents
            .queue_command(&chat.id, &SessionCommandPayload::Interrupt {})
            .await?;
        Ok(json!({ "chatId": chat.id, "commandId": command_id }))
    }

    async fn respond_to_input(&self, args: RespondArgs) -> anyhow::Result<Value> {
        let chat = self.openagents.resolve_chat(&args.chat).await?;
        let request_id = match args.request_id {
            Some(id) => id,
            None => {
                let entries = self
                    .openagents
                    .transcript_on(&chat.id, Some(&chat.device_id))
                    .await?;
                let rendered = render_entries(&entries, RenderOptions::default());
                last_pending_input(&rendered)
                    .and_then(|p| {
                        p.get("requestId")
                            .and_then(Value::as_str)
                            .map(str::to_owned)
                    })
                    .ok_or_else(|| {
                        anyhow::anyhow!("chat {} has no pending question", short(&chat.id))
                    })?
            }
        };
        if args.answers.is_empty() {
            anyhow::bail!("answers is empty");
        }
        let answers = args
            .answers
            .into_iter()
            .map(|a| UserInputAnswer {
                question_id: a.question_id,
                labels: a.labels,
            })
            .collect();
        let command_id = self
            .openagents
            .queue_command(
                &chat.id,
                &SessionCommandPayload::RespondInput {
                    request_id: request_id.clone(),
                    answers,
                },
            )
            .await?;
        Ok(json!({ "chatId": chat.id, "requestId": request_id, "commandId": command_id }))
    }

    // ---- shared pieces -------------------------------------------------------

    /// Prefix the sender's identity when this server speaks for a chat, so
    /// the receiving agent (and the human reading that transcript) can tell
    /// an agent-to-agent message from a typed one. Reply instructions always
    /// use the full id: display prefixes need not uniquely identify a chat.
    async fn attribute(&self, target: &Chat, text: &str) -> String {
        let Some(origin_id) = self.openagents.origin().chat_id.as_deref() else {
            return text.to_owned();
        };
        if origin_id == target.id {
            return text.to_owned();
        }
        let title = match self.openagents.resolve_chat(origin_id).await {
            Ok(chat) => chat.title,
            Err(_) => None,
        };
        let label = match title {
            Some(title) if !title.trim().is_empty() => {
                format!("{} ({})", title.trim(), short(origin_id))
            }
            _ => short(origin_id).to_owned(),
        };
        format!(
            "[Message from OpenAgents chat {label}. Reply to it with the OpenAgents `send_message` tool, chat {}.]\n\n{text}",
            origin_id
        )
    }

    /// Pick and perform the delivery the composer would.
    async fn deliver(
        &self,
        chat: &Chat,
        space: Option<&Space>,
        harnesses: &[HarnessInfo],
        session: Option<&Session>,
        text: String,
        mode: &str,
    ) -> anyhow::Result<Value> {
        let harness = chat
            .config
            .as_ref()
            .map(|c| c.harness)
            .map_or_else(|| default_harness(harnesses), Ok)?;
        let (status, _) = status_of(session);
        // A quiet tool call can outlive the UI's stale-status window. Route
        // through steering and let the host decide whether a live run exists.
        let live = session.map(|s| s.status).unwrap_or(SessionStatus::Idle);
        let chosen = match mode {
            "auto" => match live {
                SessionStatus::Idle | SessionStatus::Errored => "run",
                SessionStatus::Working => "steer",
                SessionStatus::AwaitingInput => anyhow::bail!(
                    "chat {} is waiting for an answer; use respond_to_input (or mode 'queue' to hold this message for after the turn)",
                    short(&chat.id)
                ),
            },
            "run" | "steer" | "queue" => mode,
            other => anyhow::bail!("unknown mode {other:?}; expected auto, run, steer, or queue"),
        };
        let id = match chosen {
            "run" => {
                let config = chat.config.clone();
                let cwd = chat
                    .cwd
                    .clone()
                    .or_else(|| space.map(|s| s.path.clone()))
                    .unwrap_or_else(|| "~".into());
                let request = RunRequest {
                    mcp: None,
                    prompt: text,
                    harness: Some(harness),
                    model: config.as_ref().and_then(|c| c.model.clone()),
                    reasoning: config.as_ref().and_then(|c| c.reasoning),
                    model_options: config
                        .as_ref()
                        .map(|c| c.model_options.clone())
                        .unwrap_or_default(),
                    cwd,
                    sandbox: config
                        .as_ref()
                        .map(|c| c.sandbox)
                        .unwrap_or(SandboxLevel::WorkspaceWrite),
                    auto_approve: false,
                    resume: None,
                    attachments: Vec::new(),
                    worktree: None,
                };
                self.openagents
                    .queue_command(
                        &chat.id,
                        &SessionCommandPayload::Run {
                            request,
                            message_id: uuid::Uuid::new_v4().to_string(),
                        },
                    )
                    .await?
            }
            "steer" => {
                self.openagents
                    .queue_command(
                        &chat.id,
                        &SessionCommandPayload::Steer {
                            prompt: text,
                            message_id: Some(uuid::Uuid::new_v4().to_string()),
                        },
                    )
                    .await?
            }
            _ => self.openagents.queue_message(&chat.id, &text).await?,
        };
        Ok(json!({
            "delivery": chosen,
            "id": id,
            "chatStatusAtSend": status,
        }))
    }

    async fn await_pending_turn(
        &self,
        chat: &Chat,
        pending: Arc<PendingTurn>,
        timeout: (Duration, Option<String>),
    ) -> anyhow::Result<Value> {
        let turn = self
            .await_turn(
                chat,
                pending.baseline.as_ref(),
                true,
                timeout,
                pending.start.clone(),
            )
            .await?;
        // A timed-out or interrupted wait leaves the turn pending for the next.
        let settled = !matches!(turn["outcome"].as_str(), Some("timedOut" | "interrupted"))
            && !(self.voice_relay() && turn["outcome"] == "awaitingInput");
        if settled {
            let mut turns = self.pending_turns.lock().await;
            // A concurrent send may already have installed a newer baseline.
            if turns
                .get(&chat.id)
                .is_some_and(|p| Arc::ptr_eq(p, &pending))
            {
                turns.remove(&chat.id);
            }
        }
        Ok(turn)
    }

    /// Wait, then report the outcome with the assistant messages that
    /// are not in `start`'s baseline.
    async fn await_turn(
        &self,
        chat: &Chat,
        baseline: Option<&Session>,
        expect_turn: bool,
        (timeout, note): (Duration, Option<String>),
        start: TurnStart,
    ) -> anyhow::Result<Value> {
        let deadline = Instant::now() + timeout;
        let waited = self
            .openagents
            .wait_for_turn(chat, baseline, expect_turn, timeout);
        let inbox = self.inbox().await;
        let (mut outcome, session, new_messages) = match inbox.as_deref() {
            Some(inbox) => tokio::select! {
                waited = waited => {
                    let (outcome, session) = waited?;
                    (outcome, session, None)
                }
                messages = inbox.arrived() => {
                    let session = match self.openagents.sessions().await {
                        Ok(sessions) => session_for(&sessions, chat),
                        Err(_) => None,
                    };
                    (TurnOutcome::Interrupted, session, Some(messages))
                }
            },
            None => {
                let (outcome, session) = waited.await?;
                (outcome, session, None)
            }
        };
        // Registry session updates can arrive before the separate transcript doc.
        // Keep the same deadline while waiting for a response to this send.
        let mut rendered = Vec::new();
        let mut replies = Vec::new();
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                if outcome == TurnOutcome::Completed && expect_turn {
                    outcome = TurnOutcome::TimedOut;
                }
                break;
            }
            let entries = match tokio::time::timeout(
                remaining,
                self.openagents.transcript_on(&chat.id, Some(&chat.device_id)),
            )
            .await
            {
                Ok(entries) => entries?,
                Err(_) => {
                    outcome = TurnOutcome::TimedOut;
                    break;
                }
            };
            rendered = render_entries(&entries, RenderOptions::default());
            replies = rendered
                .iter()
                .filter(|m| m.role == openagents_doc::MessageRole::Assistant)
                .filter(|m| !start.message_ids.contains(&m.id))
                .cloned()
                .collect();
            if !expect_turn
                || outcome != TurnOutcome::Completed
                || replies
                    .iter()
                    .any(|m| m.status != Some(openagents_doc::MessageStatus::Streaming))
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(100).min(remaining)).await;
        }
        let (status, _) = status_of(session.as_ref());
        let mut turn = json!({
            "outcome": outcome,
            "status": status,
            "timedOut": outcome == TurnOutcome::TimedOut,
            "pendingInput": last_pending_input(&rendered),
            "replies": replies,
        });
        if let Some(messages) = new_messages {
            turn["interrupted"] = json!("newMessage");
            turn["newMessages"] = json!(messages);
            turn["note"] = json!(
                "A new message arrived for you, so this wait ended early. Read it and answer it first. The chat you were waiting on may still be working; call wait_for_turn to keep waiting."
            );
        } else if let Some(note) = note {
            turn["note"] = json!(note);
        }
        Ok(turn)
    }

    /// The new-message watch for the chat this server speaks for, shared by
    /// the blocking calls running at the same time. One that has already
    /// fired is not reused: its message was reported.
    async fn inbox(&self) -> Option<Arc<Inbox>> {
        let mut slot = self.inbox.lock().await;
        if let Some(inbox) = slot.upgrade().filter(|inbox| !inbox.fired()) {
            return Some(inbox);
        }
        let inbox = Arc::new(Inbox::open(self.openagents.clone()).await?);
        *slot = Arc::downgrade(&inbox);
        Some(inbox)
    }
}

/// claude-code when it is offered here, else the first available harness.
fn default_harness(harnesses: &[HarnessInfo]) -> anyhow::Result<HarnessId> {
    harnesses
        .iter()
        .find(|h| h.id == HarnessId::ClaudeCode && h.available())
        .or_else(|| {
            harnesses
                .iter()
                .find(|h| h.available() && h.id != HarnessId::Mock)
        })
        .map(|h| h.id)
        .ok_or_else(|| {
            anyhow::anyhow!("no harness is available on this device (see list_harnesses)")
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openagents::Origin;
    use async_trait::async_trait;
    use futures::StreamExt;
    use std::sync::Mutex;
    use openagents_rpc::{RpcError, RpcReply, RpcService, memory_client, methods};

    /// Two devices with distinct catalogs and repeated project paths, plus
    /// two chats with a two-message transcript. Writes are recorded for assertions.
    #[derive(Default)]
    struct World {
        writes: Mutex<Vec<(String, Value)>>,
        pins: Mutex<Vec<String>>,
        pins_unknown: bool,
        pin_error: Option<&'static str>,
        voice_target: Mutex<Value>,
        sessions: Vec<Session>,
        dispatch_barrier: Option<tokio::sync::Barrier>,
        beta_parent: Option<String>,
        beta_remote: bool,
        extra_chats: Vec<Value>,
        catalog_error: Option<&'static str>,
        reads: Mutex<Vec<(String, Value)>>,
        /// Where a new message for chat-beta-2 shows up mid-wait:
        /// "transcript" or "queue".
        new_message_in: Option<&'static str>,
    }

    /// `first` now, `then` a moment later, and the stream stays open.
    fn later(first: Value, then: Value) -> RpcReply {
        RpcReply::Stream(
            futures::stream::once(async move { first })
                .chain(futures::stream::once(async move {
                    tokio::time::sleep(Duration::from_millis(300)).await;
                    then
                }))
                .chain(futures::stream::pending())
                .boxed(),
        )
    }

    fn beta_transcript(with_new_message: bool) -> Value {
        let mut entries = vec![json!({ "id": "u1", "role": "user", "createdAt": 1,
            "deviceId": "dev-local", "parts": [{ "kind": "text", "id": "t", "text": "work" }] })];
        if with_new_message {
            entries.push(json!({ "id": "u2", "role": "user", "createdAt": 3,
                "deviceId": "dev-local", "parts": [{ "kind": "text", "id": "t", "text": "stop and look at this" }] }));
        }
        json!({ "reset": entries })
    }

    fn stream(item: Value) -> RpcReply {
        RpcReply::Stream(futures::stream::iter(vec![item]).boxed())
    }

    #[async_trait]
    impl RpcService for World {
        async fn handle(&self, method: &str, params: Value) -> Result<RpcReply, RpcError> {
            if matches!(method, methods::LIST_HARNESSES | methods::LIST_MODELS) {
                self.reads
                    .lock()
                    .unwrap()
                    .push((method.into(), params.clone()));
                if self.catalog_error == Some(method) {
                    return Err(RpcError::Failed("catalog unavailable".into()));
                }
            }
            let remote = params["targetDeviceId"] == "dev-remote";
            Ok(match method {
                methods::GET_VOICE_AGENT => {
                    RpcReply::Value(self.voice_target.lock().unwrap().clone())
                }
                methods::LAND_STATUS => {
                    self.reads.lock().unwrap().push((method.into(), params));
                    RpcReply::Value(json!({
                        "integrator": {"running": false},
                        "waiting": [], "landing": [], "failed": []
                    }))
                }
                methods::GET_VOICE_CONTEXT => {
                    RpcReply::Value(json!({"chatId":"caller-chat", "title":"Chat X"}))
                }
                methods::LOCAL_DEVICE => RpcReply::Value(json!({ "deviceId": "dev-local" })),
                methods::ENGINE_INFO => RpcReply::Value(json!({
                    "deviceId": "dev-local", "workspaceScope": "local"
                })),
                methods::WATCH_DEVICES => stream(json!([{
                    "id": "dev-local", "name": "Laptop", "platform": "linux",
                    "lastSeenAt": null
                }, {"id": "dev-remote", "name": "Worker", "platform": "linux", "lastSeenAt": null}])),
                methods::WATCH_SPACES => stream(json!([{
                    "id": "space-1", "deviceId": "dev-local", "path": "/repo/comet",
                    "gitDetected": true, "createdAt": "2026-09-01T00:00:00Z"
                }, {
                    "id": "space-remote", "deviceId": "dev-remote", "path": "/repo/comet",
                    "gitDetected": true, "createdAt": "2026-09-01T00:00:00Z"
                }, {
                    "id": "space-unique", "deviceId": "dev-remote", "path": "/repo/unique",
                    "gitDetected": true, "createdAt": "2026-09-01T00:00:00Z"
                }])),
                methods::WATCH_CHATS => {
                    let mut chats = json!([
                    {
                        "id": "chat-alpha-1", "deviceId": "dev-local", "title": "Alpha",
                        "archived": false, "spaceId": "space-1",
                        "config": { "harness": "claude-code", "model": "opus", "reasoning": null, "sandbox": "workspace-write" },
                        "harnessSessionId": "claude-session-alpha",
                        "createdAt": "2026-09-01T00:00:00Z"
                    },
                    {
                        "id": "chat-beta-2", "deviceId": if self.beta_remote { "dev-remote" } else { "dev-local" }, "title": "Beta",
                        "parentChatId": self.beta_parent,
                        "archived": false, "spaceId": "space-1",
                        "createdAt": "2026-09-02T00:00:00Z"
                    }
                    ]);
                    chats
                        .as_array_mut()
                        .unwrap()
                        .extend(self.extra_chats.clone());
                    stream(chats)
                }
                methods::WATCH_SESSIONS => stream(json!(self.sessions)),
                methods::WATCH_SIDEBAR_PREFERENCES => stream(json!({
                    "initialized": !self.pins_unknown,
                    "synced": false,
                    "pinnedSessionIds": *self.pins.lock().unwrap(),
                })),
                methods::LIST_HARNESSES if remote => RpcReply::Value(json!([
                    {"id":"codex", "name":"Remote Codex", "installed":true},
                    {"id":"cursor", "name":"Disabled Cursor", "installed":true, "enabled":false}
                ])),
                methods::LIST_HARNESSES => RpcReply::Value(json!([
                    { "id": "claude-code", "name": "Claude Code", "supportsSteering": true,
                      "steeringMode": "step-boundary", "reasoningLevels": [], "installed": true, "enabled": true },
                    { "id": "codex", "name": "Codex", "supportsSteering": true,
                      "steeringMode": "turn-boundary", "reasoningLevels": [], "installed": false, "enabled": true }
                ])),
                methods::LIST_MODELS if remote => RpcReply::Value(json!([
                    {"id":"remote-model", "label":"Remote model"}
                ])),
                methods::LIST_MODELS => RpcReply::Value(json!([
                    { "id": "opus", "label": "Opus" }, { "id": "sonnet", "label": "Sonnet" }
                ])),
                methods::WATCH_DOC_MESSAGES
                    if self.new_message_in == Some("transcript")
                        && params["chatId"] == "chat-beta-2" =>
                {
                    later(beta_transcript(false), beta_transcript(true))
                }
                methods::WATCH_QUEUE if self.new_message_in == Some("queue") => later(
                    json!({ "items": [] }),
                    json!({ "items": [{ "id": "q-9", "text": "stop and look at this",
                        "issuedBy": "dev-local", "issuedAt": 3 }] }),
                ),
                methods::WATCH_QUEUE => stream(json!({ "items": [] })),
                methods::WATCH_DOC_MESSAGES => stream(json!({ "reset": [
                    { "id": "u1", "role": "user", "createdAt": 1, "deviceId": "dev-local",
                      "parts": [{ "kind": "text", "id": "t", "text": "hi" }] },
                    { "id": "a1", "role": "assistant", "createdAt": 2, "deviceId": "dev-local",
                      "status": "complete",
                      "parts": [{ "kind": "text", "id": "t", "text": "hello back" }] }
                ]})),
                methods::PUBLISH_VISUAL => {
                    self.writes
                        .lock()
                        .unwrap()
                        .push((method.to_owned(), params.clone()));
                    RpcReply::Value(json!({ "id": "v:1", "title": params["title"] }))
                }
                openagents_rpc::moves::START_MOVE => {
                    return Err(RpcError::Failed(
                        "This move isn't approved yet. The person using this computer can approve it in the OpenAgents app.".into(),
                    ));
                }
                openagents_rpc::moves::PLAN_MOVE
                | openagents_rpc::moves::MOVE_STATUS
                | openagents_rpc::moves::BRING_BACK => {
                    self.writes
                        .lock()
                        .unwrap()
                        .push((method.to_owned(), params.clone()));
                    let plan = json!({
                        "id": "plan-0123456789abcdef", "createdUnix": 1, "requestedBy": "agent",
                        "approved": false, "computer": null, "destination": "A new cloud computer",
                        "estimateMinutes": 67,
                        "chats": [{ "chat": "chat-alpha-1", "title": "Alpha", "agent": "Claude Code",
                                    "state": "planned", "reason": null, "bringBack": null }]
                    });
                    if method == openagents_rpc::moves::MOVE_STATUS {
                        RpcReply::Value(json!({ "plans": [plan] }))
                    } else {
                        RpcReply::Value(plan)
                    }
                }
                methods::RECORD_AGENT_DECISION | methods::NOTIFY_OWNER => {
                    self.writes
                        .lock()
                        .unwrap()
                        .push((method.to_owned(), params));
                    RpcReply::Value(json!({ "recorded": true }))
                }
                methods::MUTATE if params["op"] == "changeSidebarPin" => {
                    let change: SidebarPinChange = serde_json::from_value(params["change"].clone())
                        .map_err(|error| RpcError::BadParams(error.to_string()))?;
                    self.writes.lock().unwrap().push((method.into(), params));
                    if let Some(error) = self.pin_error {
                        return Err(RpcError::Failed(error.into()));
                    }
                    change.project(&mut self.pins.lock().unwrap());
                    RpcReply::Value(json!({ "ok": true }))
                }
                methods::MUTATE | methods::QUEUE_COMMAND | methods::QUEUE_MESSAGE => {
                    self.writes
                        .lock()
                        .unwrap()
                        .push((method.to_owned(), params));
                    if method == methods::QUEUE_COMMAND
                        && let Some(barrier) = &self.dispatch_barrier
                    {
                        // No dispatch can finish until both chats have work.
                        // A sequential batch deadlocks here, before any waits.
                        barrier.wait().await;
                    }
                    RpcReply::Value(json!({ "commandId": "cmd-1", "id": "q-1" }))
                }
                other => return Err(RpcError::UnknownMethod(other.into())),
            })
        }
    }

    fn tools(world: Arc<World>, origin: Origin) -> Tools {
        let client = memory_client(world);
        Tools::new(Arc::new(OpenAgents::with_client(client, origin)))
    }

    #[tokio::test]
    async fn a_field_the_tool_does_not_declare_is_refused_not_dropped() {
        let tools = tools(Arc::new(World::default()), Origin::default());
        let call = |arguments: Value| {
            crate::jsonrpc::handle_request(
                &tools,
                json!(1),
                "tools/call",
                json!({ "name": "judge", "arguments": arguments }),
            )
        };
        let refused = call(json!({ "set": "run-progress", "state": {}, "working": [] })).await;
        assert_eq!(refused["result"]["isError"], true);
        let text = refused["result"]["content"][0]["text"].as_str().unwrap();
        assert!(text.contains("`working`") && text.contains("`set`"), "{text}");
        // Every tool's own declared fields pass the check.
        for tool in tools.list() {
            let declared: serde_json::Map<String, Value> = tool.input_schema["properties"]
                .as_object()
                .into_iter()
                .flatten()
                .map(|(key, _)| (key.clone(), Value::Null))
                .collect();
            assert_eq!(
                crate::jsonrpc::unknown_fields(tool.name, &tool.input_schema, &Value::Object(declared)),
                None,
                "{}",
                tool.name
            );
        }
        // A schema that allows other fields checks none.
        let open = json!({ "type": "object", "properties": {}, "additionalProperties": true });
        assert_eq!(crate::jsonrpc::unknown_fields("x", &open, &json!({ "y": 1 })), None);
    }

    /// Runs the stdio server over `input`, returning every line it wrote.
    async fn serve_lines(
        input: &str,
        binary: Option<crate::reexec::Binary>,
        exec: Option<Box<crate::jsonrpc::Exec>>,
        resumed: bool,
    ) -> Vec<Value> {
        let tools = Arc::new(tools(Arc::new(World::default()), Origin::default()));
        let input = crate::reexec::Input::new(
            Box::new(std::io::Cursor::new(input.as_bytes().to_vec())),
            Vec::new(),
        );
        let (writer, mut reader) = tokio::io::duplex(1 << 20);
        crate::jsonrpc::serve(tools, input, writer, binary, exec, resumed)
            .await
            .unwrap();
        let mut out = String::new();
        tokio::io::AsyncReadExt::read_to_string(&mut reader, &mut out)
            .await
            .unwrap();
        out.lines().map(|line| serde_json::from_str(line).unwrap()).collect()
    }

    #[tokio::test]
    async fn an_upgraded_binary_is_execed_with_the_call_and_the_rest_of_the_input() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("openagents");
        std::fs::write(&path, b"old build").unwrap();
        let binary = crate::reexec::Binary::at(path.clone());
        let next = dir.path().join("next");
        std::fs::write(&next, b"new build after the upgrade").unwrap();
        std::fs::rename(&next, &path).unwrap();
        let handed: Arc<std::sync::Mutex<Vec<(std::path::PathBuf, String)>>> = Arc::default();
        let seen = handed.clone();
        let exec: Box<crate::jsonrpc::Exec> = Box::new(move |binary, resume| {
            let text = std::fs::read_to_string(resume).unwrap();
            seen.lock().unwrap().push((binary.path().to_path_buf(), text));
            std::io::Error::other("not in a test")
        });
        let call = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"whoami","arguments":{}}}"#;
        let ping = r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#;
        let lines = serve_lines(&format!("{call}\n{ping}\n"), Some(binary), Some(exec), false).await;
        let handed = handed.lock().unwrap();
        // Once, with the new binary, the call, and the ping read with it.
        assert_eq!(handed.len(), 1);
        assert_eq!(handed[0].0, path);
        assert_eq!(handed[0].1, format!("{call}\n{ping}\n"));
        // The exec failed here, so this process answered both.
        let ids: Vec<&Value> = lines.iter().map(|line| &line["id"]).collect();
        assert!(ids.contains(&&json!(1)) && ids.contains(&&json!(2)), "{lines:?}");
    }

    #[tokio::test]
    async fn an_unchanged_binary_is_never_execed_and_a_resumed_one_says_its_tools_changed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("openagents");
        std::fs::write(&path, b"build").unwrap();
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counted = calls.clone();
        let exec: Box<crate::jsonrpc::Exec> = Box::new(move |_, _| {
            counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            std::io::Error::other("no")
        });
        let call = r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"whoami","arguments":{}}}"#;
        let lines = serve_lines(
            &format!("{call}\n"),
            Some(crate::reexec::Binary::at(path)),
            Some(exec),
            true,
        )
        .await;
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert_eq!(lines[0]["method"], "notifications/tools/list_changed");
        assert_eq!(lines[1]["id"], 1);
    }

    #[tokio::test]
    async fn voice_choice_is_read_again_for_the_next_question() {
        let world = Arc::new(World::default());
        let tools = tools(
            world.clone(),
            Origin {
                chat_id: Some("voice-orchestrator-fixture".into()),
                device_id: Some("dev-local".into()),
            },
        );
        assert_eq!(tools.list().len(), 3);
        assert!(tools.list().iter().all(|t| Tools::voice_tool(t.name)));
        for name in [
            "create_chat",
            "create_chats",
            "send_messages",
            "interrupt_chat",
            "machine_status",
            "machine_action",
            "upgrade_host",
            "notify_owner",
            crate::work::NAME,
        ] {
            let error = tools.call(name, json!({})).await.unwrap_err();
            assert!(error.contains("Voice can only"), "{name}: {error}");
        }
        assert!(world.writes.lock().unwrap().is_empty());
        assert!(
            tools
                .call(
                    "wait_for_turn",
                    json!({"chat":"chat-beta-2", "timeout_secs":1})
                )
                .await
                .is_err()
        );
        for chat in ["chat-alpha-1", "chat-beta-2"] {
            *world.voice_target.lock().unwrap() = json!({"revision":1,"target":{"agent_id":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","chat_id":chat,"host_id":"dev-local"}});
            let selected = tools.call("get_voice_agent", json!({})).await.unwrap();
            assert_eq!(selected["target"]["chat_id"], chat);
            tools.call("send_message", json!({"chat":"unselected-chat","text":"My entire next question","mode":"steer","wait":true})).await.unwrap();
            let writes = world.writes.lock().unwrap();
            assert!(writes.last().unwrap().1.to_string().contains(chat));
            let body = writes.last().unwrap().1.to_string();
            assert!(body.contains("Spoken:") && body.contains("My entire next question"));
            assert!(body.contains("caller-chat") && body.contains("Chat X"));
            assert!(!body.contains("Message from OpenAgents chat"));
        }
    }

    #[tokio::test]
    async fn catalog_is_well_formed() {
        let defs = catalog();
        let mut names: Vec<&str> = defs.iter().map(|d| d.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), defs.len(), "tool names must be unique");
        for def in &defs {
            assert_eq!(def.input_schema["type"], "object", "{}", def.name);
            assert!(!def.description.is_empty());
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn terminal_run_tool_is_available_to_every_agent() {
        let tools = tools(Arc::new(World::default()), Origin::default());
        let tool = tools
            .list()
            .into_iter()
            .find(|tool| tool.name == "list_runs")
            .unwrap();
        assert_eq!(tool.input_schema["type"], "object");
        assert_eq!(
            tool.input_schema["properties"]["stall_minutes"]["default"],
            20
        );
        let error = tools
            .call("list_runs", json!({ "stall_minutes": 0 }))
            .await
            .unwrap_err();
        assert_eq!(error, "stall_minutes must be between 1 and 1440");
    }

    #[tokio::test]
    async fn landing_status_is_available_to_any_app_client() {
        let world = Arc::new(World::default());
        let tools = tools(world.clone(), Origin::default());
        assert!(tools.has("land_status"));
        let status = tools.call("land_status", json!({})).await.unwrap();
        assert_eq!(status["integrator"]["running"], false);
        assert_eq!(
            world.reads.lock().unwrap().as_slice(),
            &[(methods::LAND_STATUS.into(), json!({}))]
        );
    }

    #[tokio::test]
    async fn move_tools_plan_as_the_agent_and_never_approve() {
        let world = Arc::new(World::default());
        let tools = tools(world.clone(), Origin::default());
        assert!(
            !catalog().iter().any(|t| t.name.contains("approve")),
            "only the app approves a move"
        );
        let plan = tools
            .call("plan_move", json!({ "chats": ["Alpha"] }))
            .await
            .unwrap();
        assert_eq!(plan["estimate"], "About 1 hr 7 min of cloud computer time");
        assert_eq!(plan["next"], APPROVE_IN_APP);
        let (method, params) = world.writes.lock().unwrap().last().cloned().unwrap();
        assert_eq!(method, openagents_rpc::moves::PLAN_MOVE);
        assert_eq!(params["chats"], json!(["chat-alpha-1"]));
        assert_eq!(params["requestedBy"], "agent");

        let refused = tools
            .call("start_move", json!({ "plan_id": "plan-0123456789abcdef" }))
            .await
            .unwrap_err();
        assert!(
            refused.contains("approve it in the OpenAgents app"),
            "{refused}"
        );

        tools
            .call("bring_back", json!({ "chat": "Alpha", "after": "delete" }))
            .await
            .unwrap();
        let (_, params) = world.writes.lock().unwrap().last().cloned().unwrap();
        assert_eq!(params["after"], "delete");
        assert_eq!(params["requestedBy"], "agent");

        let status = tools.call("move_status", json!({})).await.unwrap();
        assert_eq!(status["plans"][0]["next"], APPROVE_IN_APP);
    }

    #[test]
    fn the_show_visual_example_passes_its_own_check() {
        let start = SHOW_VISUAL_DESCRIPTION.find("local s = ui.state").unwrap();
        let end = start
            + SHOW_VISUAL_DESCRIPTION[start..]
                .find("\n\nOpenAgents runs the script")
                .unwrap();
        let report = openagents_visual::check(&SHOW_VISUAL_DESCRIPTION[start..end], 1).unwrap();
        assert_eq!(report.interactions, 2, "both tabs");
    }

    #[tokio::test]
    async fn show_visual_is_offered_only_inside_a_chat() {
        let world = Arc::new(World::default());
        let outside = tools(world.clone(), Origin::default());
        assert!(!outside.has("show_visual"));
        assert!(
            outside
                .call(
                    "show_visual",
                    json!({ "script": "return ui.text 'x'", "title": "x" })
                )
                .await
                .is_err()
        );

        let inside = tools(
            world.clone(),
            Origin {
                chat_id: Some("chat-alpha-1".into()),
                device_id: Some("dev-local".into()),
            },
        );
        assert!(inside.has("show_visual"));
        // The guidance names every color the runtime accepts.
        for name in openagents_visual::TOKEN_NAMES {
            assert!(SHOW_VISUAL_DESCRIPTION.contains(name), "{name}");
        }
        let shown = inside
            .call(
                "show_visual",
                json!({ "script": "return ui.text 'chart'", "title": "Chart" }),
            )
            .await
            .unwrap();
        assert_eq!(shown["visual"]["title"], "Chart");
        let writes = world.writes.lock().unwrap();
        let (method, params) = writes.last().unwrap();
        assert_eq!(method, methods::PUBLISH_VISUAL);
        assert_eq!(params["chatId"], "chat-alpha-1");
        assert_eq!(params["script"], "return ui.text 'chart'");
    }

    #[tokio::test]
    async fn voice_queues_a_spoken_request_while_the_agent_needs_input() {
        let world = Arc::new(World {
            voice_target: Mutex::new(json!({"revision":1,"target":{
                "agent_id":"a".repeat(64),"chat_id":"chat-alpha-1","host_id":"dev-local"
            }})),
            sessions: vec![Session {
                chat_id: "chat-alpha-1".into(),
                device_id: "dev-local".into(),
                status: SessionStatus::AwaitingInput,
                started_at: None,
                updated_at: chrono::Utc::now(),
                last_completed_turn: None,
                running_subagents: 0,
                background_tasks: 0,
                last_output_at: None,
            }],
            ..Default::default()
        });
        let tools = tools(
            world.clone(),
            Origin {
                chat_id: Some("voice-orchestrator-fixture".into()),
                device_id: Some("dev-local".into()),
            },
        );
        let sent = tools
            .call(
                "send_message",
                json!({"chat":"wrong-chat","text":"Keep my full request for later","wait":true}),
            )
            .await
            .unwrap();
        assert_eq!(sent["sent"]["delivery"], "queue");
        let writes = world.writes.lock().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, methods::QUEUE_MESSAGE);
        assert!(writes[0].1.to_string().contains("Spoken:"));
        assert!(
            writes[0]
                .1
                .to_string()
                .contains("Keep my full request for later")
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn machine_status_is_available_to_any_agent_and_rejects_write_arguments() {
        let world = Arc::new(World::default());
        for origin in [
            Origin::default(),
            Origin {
                chat_id: Some("ordinary-agent".into()),
                device_id: None,
            },
        ] {
            let tools = tools(world.clone(), origin);
            assert!(tools.has("machine_status"));
            assert!(
                tools
                    .call("machine_status", json!({"action":"prune"}))
                    .await
                    .is_err()
            );
        }
        assert!(world.writes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn notify_owner_uses_the_calling_chat_and_cannot_select_another_agent() {
        let world = Arc::new(World::default());
        let outside = tools(world.clone(), Origin::default());
        assert!(!outside.has("notify_owner"));
        let inside = tools(
            world.clone(),
            Origin {
                chat_id: Some("chat-alpha-1".into()),
                device_id: Some("dev-local".into()),
            },
        );
        assert!(inside.has("notify_owner"));
        let note = json!({"request_id":"a".repeat(64),"title":"I need your help","body":"The computer stopped answering."});
        inside.call("notify_owner", note.clone()).await.unwrap();
        let mut spoofed = note;
        spoofed["agent"] = json!("other");
        assert!(inside.call("notify_owner", spoofed).await.is_err());
        let writes = world.writes.lock().unwrap();
        let (method, params) = writes.last().unwrap();
        assert_eq!(method, methods::NOTIFY_OWNER);
        assert_eq!(params["originChatId"], "chat-alpha-1");
    }

    #[tokio::test]
    async fn an_agents_writes_name_its_chat_and_reach_its_log() {
        let world = Arc::new(World::default());
        let outside = tools(world.clone(), Origin::default());
        assert!(!outside.has("record_decision"));
        let inside = tools(
            world.clone(),
            Origin {
                chat_id: Some("chat-alpha-1".into()),
                device_id: Some("dev-local".into()),
            },
        );
        assert!(inside.has("record_decision"));
        inside
            .call("interrupt_chat", json!({ "chat": "Beta" }))
            .await
            .unwrap();
        inside
            .call(
                "record_decision",
                json!({ "saw": "Beta idle", "decided": "stop it", "did": "stopped Beta",
                        "working": [{ "issue": 12057, "chat": "chat-beta-1", "status": "handoff" }] }),
            )
            .await
            .unwrap();
        let writes = world.writes.lock().unwrap();
        let (method, params) = &writes[writes.len() - 2];
        assert_eq!(method, methods::QUEUE_COMMAND);
        assert_eq!(params["originChatId"], "chat-alpha-1");
        let (method, params) = writes.last().unwrap();
        assert_eq!(method, methods::RECORD_AGENT_DECISION);
        assert_eq!(params["originChatId"], "chat-alpha-1");
        assert_eq!(params["did"], "stopped Beta");
        assert_eq!(params["working"][0]["status"], "handoff");
    }

    #[tokio::test]
    async fn any_agent_can_judge_and_a_decision_carries_the_receipt() {
        let world = Arc::new(World::default());
        let fake = Arc::new(crate::judge::tests::Fake::default());
        let outside = tools(world.clone(), Origin::default()).with_decider(fake.clone());
        assert!(outside.has("judge"));
        let inside = tools(
            world.clone(),
            Origin {
                chat_id: Some("chat-alpha-1".into()),
                device_id: Some("dev-local".into()),
            },
        )
        .with_decider(fake.clone());
        let judged = inside
            .call(
                "judge",
                json!({ "set": "run-progress", "state": { "run": { "holds": "#1" }, "outputs": [] } }),
            )
            .await
            .unwrap();
        assert_eq!(judged["answers"][0]["name"], "progressed");
        inside
            .call(
                "record_decision",
                json!({
                    "saw": "Beta silent for 20 minutes",
                    "decided": "message it",
                    "did": "sent a nudge",
                    "judgments": [judged["receipt"]],
                }),
            )
            .await
            .unwrap();
        let writes = world.writes.lock().unwrap();
        let (method, params) = writes.last().unwrap();
        assert_eq!(method, methods::RECORD_AGENT_DECISION);
        assert_eq!(params["judgments"][0]["decision"], "dec_7");
        assert_eq!(
            params["judgments"][0]["questionSet"],
            "openagents.run-progress.v1"
        );
        let record: openagents_proto::agent_wakes::RecordDecision =
            serde_json::from_value(params.clone()).unwrap();
        assert_eq!(record.judgments[0].door.as_deref(), Some("clef"));
    }

    #[tokio::test]
    async fn changed_paths_reads_local_folders_and_reports_others_per_chat() {
        let world = Arc::new(World {
            beta_remote: true,
            ..World::default()
        });
        let tools = tools(world, Origin::default());
        let dir = std::env::temp_dir().join(format!("oa-changed-tool-{}", uuid::Uuid::new_v4()));
        let git = |args: &[&str]| {
            let ok = std::process::Command::new("git")
                .arg("-C")
                .arg(&dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@example.com"])
                .args(args)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?}");
        };
        std::fs::create_dir_all(&dir).unwrap();
        git(&["init", "-q", "-b", "main"]);
        git(&["commit", "-q", "--allow-empty", "-m", "base"]);
        std::fs::write(dir.join("shell.rs"), "1").unwrap();
        let out = tools
            .call(
                "changed_paths",
                json!({ "base": "main", "chats": [
                    { "chat": "chat-alpha-1", "dir": dir.to_str().unwrap() },
                    { "chat": "chat-beta-2" },
                    { "chat": "no-such-chat" },
                ]}),
            )
            .await
            .unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
        assert_eq!(out["chats"][0]["paths"], json!(["shell.rs"]));
        assert!(
            out["chats"][1]["error"]
                .as_str()
                .unwrap()
                .contains("another device")
        );
        assert!(out["chats"][2]["error"].is_string());
        assert_eq!(out["pushFreely"], json!(["chat-alpha-1"]));
        assert!(
            tools
                .call("changed_paths", json!({ "chats": [] }))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn list_and_read_chats() {
        let world = Arc::new(World::default());
        let tools = tools(world, Origin::default());
        let listed = tools.call("list_chats", json!({})).await.unwrap();
        assert_eq!(listed["total"], 2);
        // Newest activity first: Beta was created later.
        assert_eq!(listed["chats"][0]["title"], "Beta");
        assert_eq!(listed["chats"][0]["pinned"], false);
        assert_eq!(listed["chats"][0]["archived"], false);
        assert_eq!(listed["chats"][1]["project"]["name"], "comet");
        assert_eq!(listed["chats"][1]["status"], "idle");
        // Claim markers name this id, so an agent can map a claimed issue to its chat.
        assert_eq!(
            listed["chats"][1]["harnessSessionId"],
            "claude-session-alpha"
        );
        assert!(listed["chats"][0]["harnessSessionId"].is_null());

        let read = tools
            .call("read_chat", json!({ "chat": "alpha" }))
            .await
            .unwrap();
        assert_eq!(read["total"], 2);
        assert_eq!(read["messages"][1]["text"], "hello back");

        let read = tools
            .call("read_chat", json!({ "chat": "chat-alpha", "limit": 1 }))
            .await
            .unwrap();
        assert_eq!(read["returned"], 1);
        assert_eq!(read["olderRemaining"], 1);
        assert_eq!(read["messages"][0]["id"], "a1");
    }

    #[tokio::test]
    async fn pin_chat_preserves_order_and_reports_pin_and_archive_states() {
        let original = vec![
            "pin-before".to_owned(),
            "chat-alpha-1".to_owned(),
            "archived-chat".to_owned(),
        ];
        let world = Arc::new(World {
            pins: Mutex::new(original.clone()),
            extra_chats: vec![json!({
                "id": "archived-chat", "title": "Archived", "deviceId": "dev-local",
                "archived": true, "createdAt": "2026-08-01T00:00:00Z"
            })],
            ..Default::default()
        });
        let tools = tools(world.clone(), Origin::default());
        let def = tools
            .list()
            .into_iter()
            .find(|def| def.name == "pin_chat")
            .unwrap();
        assert_eq!(def.input_schema["properties"]["pinned"]["default"], true);
        assert_eq!(def.input_schema["required"], json!(["chat"]));

        // Repeating a pin must not move it behind the other existing pins.
        let pinned = tools
            .call("pin_chat", json!({ "chat": "Alpha" }))
            .await
            .unwrap();
        assert_eq!(pinned["chatId"], "chat-alpha-1");
        assert_eq!(pinned["pinned"], true);
        assert!(world.writes.lock().unwrap().is_empty());
        assert_eq!(*world.pins.lock().unwrap(), original);

        let pinned = tools
            .call("pin_chat", json!({ "chat": "chat-beta" }))
            .await
            .unwrap();
        assert_eq!(pinned["chatId"], "chat-beta-2");
        assert_eq!(pinned["pinned"], true);
        let mut appended = original.clone();
        appended.push("chat-beta-2".into());
        assert_eq!(*world.pins.lock().unwrap(), appended);
        tools
            .call("pin_chat", json!({ "chat": "chat-beta-2", "pinned": true }))
            .await
            .unwrap();
        assert_eq!(world.writes.lock().unwrap().len(), 1);
        let listed = tools
            .call("list_chats", json!({ "include_archived": true }))
            .await
            .unwrap();
        assert_eq!(listed["chats"][0]["id"], "chat-beta-2");
        assert_eq!(listed["chats"][0]["pinned"], true);
        assert_eq!(listed["chats"][0]["archived"], false);
        let archived = listed["chats"]
            .as_array()
            .unwrap()
            .iter()
            .find(|chat| chat["id"] == "archived-chat")
            .unwrap();
        assert_eq!(archived["pinned"], true);
        assert_eq!(archived["archived"], true);
        let shown = tools
            .call("get_chat", json!({ "chat": "Beta" }))
            .await
            .unwrap();
        assert_eq!(shown["pinned"], true);

        let unpinned = tools
            .call("pin_chat", json!({ "chat": "Beta", "pinned": false }))
            .await
            .unwrap();
        assert_eq!(unpinned["pinned"], false);
        assert_eq!(*world.pins.lock().unwrap(), original);
        let listed = tools.call("list_chats", json!({})).await.unwrap();
        assert_eq!(listed["total"], 2);
        assert_eq!(listed["chats"][0]["pinned"], false);
        assert_eq!(listed["chats"][1]["pinned"], true);
        let shown = tools
            .call("get_chat", json!({ "chat": "chat-beta" }))
            .await
            .unwrap();
        assert_eq!(shown["pinned"], false);
        tools
            .call("pin_chat", json!({ "chat": "Beta", "pinned": false }))
            .await
            .unwrap();
        assert_eq!(
            world.writes.lock().unwrap().as_slice(),
            &[
                (
                    methods::MUTATE.into(),
                    json!({ "op": "changeSidebarPin", "change": {
                        "action": "pin", "sessionId": "chat-beta-2", "after": null, "before": null
                    }})
                ),
                (
                    methods::MUTATE.into(),
                    json!({ "op": "changeSidebarPin", "change": {
                        "action": "unpin", "sessionId": "chat-beta-2"
                    }})
                ),
            ]
        );
    }

    #[tokio::test]
    async fn unknown_sidebar_pins_stay_unknown_and_cannot_be_changed() {
        let world = Arc::new(World {
            pins_unknown: true,
            ..Default::default()
        });
        let tools = tools(world.clone(), Origin::default());
        let listed = tools.call("list_chats", json!({})).await.unwrap();
        assert!(listed["chats"][0]["pinned"].is_null());
        let shown = tools
            .call("get_chat", json!({ "chat": "Alpha" }))
            .await
            .unwrap();
        assert!(shown["pinned"].is_null());
        for pinned in [true, false] {
            let error = tools
                .call("pin_chat", json!({ "chat": "Alpha", "pinned": pinned }))
                .await
                .unwrap_err();
            assert!(error.contains("still loading"), "{error}");
        }
        assert!(world.writes.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn pin_chat_reports_a_refused_pin_without_changing_the_list() {
        let world = Arc::new(World {
            pin_error: Some("You can pin up to 200 sessions"),
            ..Default::default()
        });
        let tools = tools(world.clone(), Origin::default());
        let error = tools
            .call("pin_chat", json!({ "chat": "Beta" }))
            .await
            .unwrap_err();
        assert!(error.contains("You can pin up to 200 sessions"), "{error}");
        let shown = tools
            .call("get_chat", json!({ "chat": "Beta" }))
            .await
            .unwrap();
        assert_eq!(shown["pinned"], false);
    }

    #[tokio::test]
    async fn send_attributes_and_refuses_self() {
        let world = Arc::new(World::default());
        let tools = tools(
            world.clone(),
            Origin {
                chat_id: Some("chat-beta-2".into()),
                device_id: Some("dev-local".into()),
            },
        );
        let err = tools
            .call("send_message", json!({ "chat": "beta", "text": "loop" }))
            .await
            .unwrap_err();
        assert!(err.contains("own chat"), "{err}");

        let sent = tools
            .call(
                "send_message",
                json!({ "chat": "alpha", "text": "please review" }),
            )
            .await
            .unwrap();
        assert_eq!(sent["sent"]["delivery"], "run");
        let writes = world.writes.lock().unwrap();
        let (method, params) = writes.last().expect("a queued command");
        assert_eq!(method, methods::QUEUE_COMMAND);
        assert_eq!(params["chatId"], "chat-alpha-1");
        assert_eq!(params["command"]["kind"], "run");
        let prompt = params["command"]["request"]["prompt"].as_str().unwrap();
        assert!(
            prompt.starts_with("[Message from OpenAgents chat Beta (chat-bet)"),
            "{prompt}"
        );
        assert!(prompt.ends_with("please review"));
        assert!(prompt.contains("tool, chat chat-beta-2.]"), "{prompt}");
        assert_eq!(params["command"]["request"]["cwd"], "/repo/comet");
        assert_eq!(params["command"]["request"]["harness"], "claude-code");
        assert_eq!(params["command"]["request"]["model"], "opus");
    }

    #[tokio::test]
    async fn voice_relay_has_no_reply_wrapper_with_multiple_orchestrators() {
        let first = "voice-orchestrator-11111111-1111-4111-8111-111111111111";
        let second = "voice-orchestrator-22222222-2222-4222-8222-222222222222";
        let world = Arc::new(World {
            voice_target: Mutex::new(json!({"revision":1,"target":{
                "agent_id":"a".repeat(64),"chat_id":"chat-alpha-1","host_id":"dev-local"
            }})),
            extra_chats: [first, second]
                .into_iter()
                .map(|id| {
                    json!({
                        "id": id, "deviceId": "dev-local", "title": "Voice session",
                        "archived": false, "createdAt": "2026-10-01T00:00:00Z"
                    })
                })
                .collect(),
            ..Default::default()
        });
        let orchestrator = tools(
            world.clone(),
            Origin {
                chat_id: Some(second.into()),
                device_id: None,
            },
        );
        let error = orchestrator
            .openagents
            .resolve_chat("voice-or")
            .await
            .unwrap_err();
        assert!(error.to_string().contains("2 chats start with"), "{error}");
        orchestrator
            .call(
                "send_message",
                json!({
                    "chat": "alpha", "text": "please review"
                }),
            )
            .await
            .unwrap();
        let writes = world.writes.lock().unwrap();
        assert_eq!(writes.last().unwrap().1["chatId"], "chat-alpha-1");
        let prompt = writes.last().unwrap().1["command"]["request"]["prompt"]
            .as_str()
            .unwrap();
        assert!(prompt.starts_with("Spoken:\n\nplease review"));
        assert!(!prompt.contains("Message from OpenAgents chat"));
        assert!(!prompt.contains(first) && !prompt.contains(second));
    }

    #[tokio::test]
    async fn auto_steers_busy_chats_even_at_turn_boundaries_or_after_long_quiet_tools() {
        let world = Arc::new(World::default());
        let tools = tools(world.clone(), Origin::default());
        let chats = tools.openagents.chats().await.unwrap();
        let session = Session {
            chat_id: chats[0].id.clone(),
            device_id: "dev-local".into(),
            status: SessionStatus::Working,
            started_at: None,
            updated_at: chrono::Utc::now() - chrono::Duration::minutes(10),
            last_completed_turn: None,
            running_subagents: 0,
            background_tasks: 0,
            last_output_at: None,
        };
        // No mid-turn capability is required for a live mailbox delivery.
        let sent = tools
            .deliver(
                &chats[0],
                None,
                &[],
                Some(&session),
                "follow up".into(),
                "auto",
            )
            .await
            .unwrap();
        assert_eq!(sent["delivery"], "steer");
        let writes = world.writes.lock().unwrap();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, methods::QUEUE_COMMAND);
        assert_eq!(writes[0].1["command"]["kind"], "steer");
    }

    #[tokio::test]
    async fn create_chat_writes_the_row_and_validates_model() {
        let world = Arc::new(World::default());
        let tools = tools(world.clone(), Origin::default());
        let err = tools
            .call(
                "create_chat",
                json!({ "project": "space-1", "model": "nope" }),
            )
            .await
            .unwrap_err();
        assert!(err.contains("not offered"), "{err}");

        let created = tools
            .call(
                "create_chat",
                json!({ "project": "space-1", "model": "sonnet", "title": "Review", "prompt": "go" }),
            )
            .await
            .unwrap();
        let chat_id = created["chatId"].as_str().unwrap().to_owned();
        let writes = world.writes.lock().unwrap();
        assert_eq!(writes.len(), 3, "createChat, renameChat, run");
        assert_eq!(writes[0].1["op"], "createChat");
        assert_eq!(writes[0].1["chatId"], chat_id);
        assert_eq!(writes[0].1["spaceId"], "space-1");
        assert!(
            writes[0].1.get("parentChatId").is_none(),
            "no origin, no parent"
        );
        assert_eq!(writes[0].1["config"]["harness"], "claude-code");
        assert_eq!(writes[0].1["config"]["model"], "sonnet");
        assert_eq!(writes[1].1["op"], "renameChat");
        assert_eq!(writes[2].0, methods::QUEUE_COMMAND);
        assert_eq!(writes[2].1["command"]["request"]["prompt"], "go");
    }

    #[tokio::test]
    async fn wait_on_an_idle_chat_returns_immediately() {
        let world = Arc::new(World::default());
        let tools = tools(world, Origin::default());
        let waited = tools
            .call(
                "wait_for_turn",
                json!({ "chat": "alpha", "timeout_secs": 5 }),
            )
            .await
            .unwrap();
        assert_eq!(waited["turn"]["outcome"], "completed");
        assert_eq!(waited["turn"]["replies"][0]["text"], "hello back");
    }

    #[tokio::test]
    async fn send_with_wait_keeps_waiting_until_a_turn_lands() {
        // The stub never grows a session row, so a send-then-wait must run
        // to its deadline rather than declaring the (unstarted) run done.
        let world = Arc::new(World::default());
        let tools = tools(world, Origin::default());
        let started = std::time::Instant::now();
        let sent = tools
            .call(
                "send_message",
                json!({ "chat": "alpha", "text": "hi", "wait": true, "timeout_secs": 1 }),
            )
            .await
            .unwrap();
        assert_eq!(sent["turn"]["outcome"], "timedOut");
        assert!(started.elapsed() >= Duration::from_millis(900));
    }

    #[test]
    fn blocking_waits_are_capped_at_two_minutes() {
        for def in catalog() {
            let properties = &def.input_schema["properties"];
            let timeout = properties
                .get("timeout_secs")
                .or_else(|| properties["requests"]["items"]["properties"].get("timeout_secs"));
            if let Some(timeout) = timeout {
                assert_eq!(timeout["maximum"], 120, "{}", def.name);
                assert_eq!(timeout["default"], 120, "{}", def.name);
            }
        }
        for name in [
            "wait_for_turn",
            "send_message",
            "send_messages",
            "create_chat",
            "create_chats",
        ] {
            let def = catalog().into_iter().find(|d| d.name == name).unwrap();
            assert!(
                def.input_schema.to_string().contains("\"maximum\":120"),
                "{name}"
            );
        }
        assert_eq!(wait_duration(None), (Duration::from_secs(120), None));
        assert_eq!(wait_duration(Some(30)), (Duration::from_secs(30), None));
        let (used, note) = wait_duration(Some(1500));
        assert_eq!(used, Duration::from_secs(120));
        let note = note.expect("a clamped wait says so");
        assert!(
            note.contains("capped at 120") && note.contains("1500"),
            "{note}"
        );
    }

    #[tokio::test]
    async fn a_new_message_for_the_waiting_chat_ends_the_wait_early() {
        for place in ["transcript", "queue"] {
            let world = Arc::new(World {
                new_message_in: Some(place),
                ..Default::default()
            });
            // Beta waits on Alpha, whose turn never lands in this world.
            let tools = tools(
                world,
                Origin {
                    chat_id: Some("chat-beta-2".into()),
                    device_id: Some("dev-local".into()),
                },
            );
            let started = std::time::Instant::now();
            let sent = tokio::time::timeout(
                Duration::from_secs(10),
                tools.call(
                    "send_message",
                    json!({ "chat": "alpha", "text": "go", "wait": true, "timeout_secs": 60 }),
                ),
            )
            .await
            .expect("the new message ends the wait")
            .unwrap();
            let turn = &sent["turn"];
            assert_eq!(turn["outcome"], "interrupted", "{place}: {turn}");
            assert_eq!(turn["interrupted"], "newMessage", "{place}");
            assert_eq!(turn["timedOut"], false, "{place}");
            assert_eq!(
                turn["newMessages"][0]["text"], "stop and look at this",
                "{place}"
            );
            assert_eq!(turn["newMessages"][0]["queued"], place == "queue");
            assert!(started.elapsed() < Duration::from_secs(10), "{place}");
            // Alpha's turn is still pending, so a later wait picks it up.
            assert!(
                tools
                    .pending_turns
                    .lock()
                    .await
                    .contains_key("chat-alpha-1")
            );
        }
    }

    #[tokio::test]
    async fn batches_dispatch_all_chats_before_waiting_and_keep_partial_results() {
        for (name, requests) in [
            (
                "create_chats",
                json!([
                    {"prompt": "first", "wait": true, "timeout_secs": 1},
                    {"harness": "not-a-harness"},
                    {"prompt": "second", "wait": true, "timeout_secs": 1},
                ]),
            ),
            (
                "send_messages",
                json!([
                    {"chat": "alpha", "text": "first", "wait": true, "timeout_secs": 1},
                    {"chat": "missing", "text": "invalid"},
                    {"chat": "beta", "text": "second", "wait": true, "timeout_secs": 1},
                ]),
            ),
        ] {
            let world = Arc::new(World {
                dispatch_barrier: Some(tokio::sync::Barrier::new(2)),
                ..Default::default()
            });
            let tools = tools(world.clone(), Origin::default());
            let reply = tokio::time::timeout(
                Duration::from_secs(3),
                crate::jsonrpc::handle_request(
                    &tools,
                    json!(42),
                    "tools/call",
                    json!({"name": name, "arguments": {"requests": requests}}),
                ),
            )
            .await
            .expect("both dispatches must proceed while other requests are waiting");
            assert_eq!(reply["result"]["isError"], false, "{reply}");
            let results = &reply["result"]["structuredContent"]["results"];
            for i in [0, 2] {
                assert_eq!(results[i]["index"], i);
                assert_eq!(results[i]["isError"], false, "{reply}");
                assert_eq!(results[i]["result"]["turn"]["outcome"], "timedOut");
            }
            assert_eq!(results[1]["index"], 1);
            assert_eq!(results[1]["isError"], true);
            let writes = world.writes.lock().unwrap();
            let dispatched: Vec<_> = writes
                .iter()
                .filter(|(method, _)| method == methods::QUEUE_COMMAND)
                .collect();
            assert_eq!(dispatched.len(), 2);
            assert_ne!(dispatched[0].1["chatId"], dispatched[1].1["chatId"]);
        }
    }

    #[tokio::test]
    async fn invalid_batch_sizes_have_no_side_effects() {
        let world = Arc::new(World::default());
        let tools = tools(world.clone(), Origin::default());
        for name in ["create_chats", "send_messages"] {
            for requests in [vec![], vec![json!({"prompt": "hello"}); MAX_BATCH + 1]] {
                assert!(
                    tools
                        .call(name, json!({"requests": requests}))
                        .await
                        .is_err()
                );
            }
        }
        assert!(world.writes.lock().unwrap().is_empty());
    }

    /// Every chat an agent creates is standalone: never a hidden child,
    /// whatever `kind` or `parent` an older caller still sends, and an old
    /// side chat may create chats too.
    #[tokio::test]
    async fn create_chat_always_makes_standalone_chats() {
        let world = Arc::new(World {
            beta_parent: Some("chat-alpha-1".into()),
            ..Default::default()
        });
        for (origin, args) in [
            ("chat-alpha-1", json!({})),
            ("chat-alpha-1", json!({"kind":"side"})),
            ("chat-alpha-1", json!({"kind":"chat"})),
            ("chat-alpha-1", json!({"parent":"Beta"})),
            ("chat-beta-2", json!({})),
        ] {
            let tools = tools(
                world.clone(),
                Origin {
                    chat_id: Some(origin.into()),
                    device_id: None,
                },
            );
            let created = tools.call("create_chat", args).await.unwrap();
            assert_eq!(created["kind"], "chat");
            assert_eq!(created["createdFrom"], origin);
            assert!(created.get("parentChatId").is_none());
            let writes = world.writes.lock().unwrap();
            let row = &writes.last().unwrap().1;
            assert_eq!(row["op"], "createChat");
            assert!(row.get("parentChatId").is_none());
        }
        let root = tools(world.clone(), Origin::default());
        let created = root.call("create_chat", json!({})).await.unwrap();
        assert_eq!(created["kind"], "chat");
        assert_eq!(created["createdFrom"], Value::Null);
        let schema = &root
            .list()
            .into_iter()
            .find(|tool| tool.name == "create_chat")
            .unwrap()
            .input_schema;
        assert!(schema["properties"].get("kind").is_none());
        assert!(schema["properties"].get("parent").is_none());
    }

    #[tokio::test]
    async fn project_and_device_resolution_scopes_names_and_exact_paths() {
        let world = Arc::new(World::default());
        let tools = tools(world.clone(), Origin::default());
        for args in [
            json!({"project":"comet"}),
            json!({"project":"/repo/comet"}),
            json!({"project":"space-1", "device":"Worker"}),
            json!({"project":"missing", "device":"Worker"}),
            json!({"device":"missing"}),
        ] {
            let error = tools.call("create_chat", args).await.unwrap_err();
            assert!(world.writes.lock().unwrap().is_empty(), "{error}");
        }
        let error = tools
            .call("create_chat", json!({"project":"/repo/comet"}))
            .await
            .unwrap_err();
        for candidate in ["space-1", "space-remote", "dev-local", "dev-remote"] {
            assert!(error.contains(candidate), "{error}");
        }
        for project in ["comet", "/repo/comet", "space-remote"] {
            let created = tools
                .call("create_chat", json!({"project":project,"device":"Worker"}))
                .await
                .unwrap();
            assert_eq!(created["deviceId"], "dev-remote");
            assert_eq!(created["project"]["id"], "space-remote");
            assert_eq!(created["harness"], "codex");
        }
        let created = tools
            .call("create_chat", json!({"project":"unique"}))
            .await
            .unwrap();
        assert_eq!(created["deviceId"], "dev-remote");
        let created = tools
            .call("create_chat", json!({"device":"dev-remote"}))
            .await
            .unwrap();
        assert!(created["project"].is_null());
        assert_eq!(created["deviceId"], "dev-remote");
        let created = tools.call("create_chat", json!({})).await.unwrap();
        assert_eq!(created["deviceId"], "dev-local");
    }

    #[tokio::test]
    async fn discovery_and_creation_use_the_selected_hosts_catalogs() {
        let world = Arc::new(World::default());
        let tools = tools(world.clone(), Origin::default());
        assert_eq!(
            tools.call("list_projects", json!({})).await.unwrap()["projects"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        let projects = tools
            .call("list_projects", json!({"device":"Worker"}))
            .await
            .unwrap();
        assert_eq!(projects["projects"].as_array().unwrap().len(), 2);
        assert!(
            projects["projects"]
                .as_array()
                .unwrap()
                .iter()
                .all(|p| p["deviceId"] == "dev-remote")
        );
        assert_eq!(
            tools.call("list_harnesses", json!({})).await.unwrap()["harnesses"][0]["id"],
            "claude-code"
        );
        assert_eq!(
            tools
                .call("list_harnesses", json!({"device":"Worker"}))
                .await
                .unwrap()["harnesses"][0]["id"],
            "codex"
        );
        assert_eq!(
            tools
                .call("list_models", json!({"harness":"codex", "device":"Worker"}))
                .await
                .unwrap()["models"][0]["id"],
            "remote-model"
        );
        assert_eq!(
            tools
                .call("list_models", json!({"harness":"claude-code"}))
                .await
                .unwrap()["models"][0]["id"],
            "opus"
        );
        for args in [
            json!({"device":"Worker", "harness":"claude-code"}),
            json!({"device":"Worker", "harness":"cursor"}),
            json!({"device":"Laptop", "harness":"codex"}),
            json!({"device":"Laptop", "harness":"mock"}),
            json!({"device":"Worker", "harness":"codex", "model":"opus"}),
        ] {
            assert!(tools.call("create_chat", args).await.is_err());
            assert!(world.writes.lock().unwrap().is_empty());
        }
        let created = tools
            .call(
                "create_chat",
                json!({"device":"Worker", "harness":"codex", "model":"remote-model"}),
            )
            .await
            .unwrap();
        assert_eq!(created["harness"], "codex");
        assert_eq!(created["model"], "remote-model");
        for method in [methods::LIST_HARNESSES, methods::LIST_MODELS] {
            let broken = Arc::new(World {
                catalog_error: Some(method),
                ..Default::default()
            });
            let tools = self::tools(broken.clone(), Origin::default());
            let error = tools
                .call(
                    "create_chat",
                    json!({"device":"Worker", "model":"remote-model"}),
                )
                .await
                .unwrap_err();
            assert!(
                error.contains("dev-remote") && error.contains("catalog unavailable"),
                "{error}"
            );
            assert!(broken.writes.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn remote_send_selects_the_remote_default_harness() {
        let world = Arc::new(World {
            beta_remote: true,
            ..Default::default()
        });
        let tools = tools(world.clone(), Origin::default());
        tools
            .call("send_message", json!({"chat":"Beta", "text":"go"}))
            .await
            .unwrap();
        assert_eq!(
            world.reads.lock().unwrap().last().unwrap().1["targetDeviceId"],
            "dev-remote"
        );
        assert_eq!(
            world.writes.lock().unwrap().last().unwrap().1["command"]["request"]["harness"],
            "codex"
        );
    }

    #[tokio::test]
    async fn mixed_batch_preserves_order_and_errors_per_request() {
        let world = Arc::new(World::default());
        let tools = tools(
            world.clone(),
            Origin {
                chat_id: Some("chat-alpha-1".into()),
                device_id: None,
            },
        );
        let result = tools
            .call(
                "create_chats",
                json!({"requests":[
                    {"kind":"chat", "device":"Worker", "project":"comet"},
                    {"harness":"bogus"},
                    {"kind":"side", "project":"space-1"},
                    {"sandbox":"bogus"}
                ]}),
            )
            .await
            .unwrap();
        let results = &result["results"];
        assert_eq!(results[0]["result"]["kind"], "chat");
        assert_eq!(results[0]["result"]["deviceId"], "dev-remote");
        assert_eq!(results[1]["isError"], true);
        assert_eq!(results[2]["result"]["kind"], "chat");
        assert_eq!(results[2]["result"]["createdFrom"], "chat-alpha-1");
        assert_eq!(results[3]["isError"], true);
        for i in 0..4 {
            assert_eq!(results[i]["index"], i);
        }
        assert_eq!(world.writes.lock().unwrap().len(), 2);
    }

    /// Sessions and transcripts are independently delivered. The command reply
    /// is deliberately slow, so the assistant starts before QueueCommand returns.
    struct DelayedTurn {
        world: World,
        sent: Mutex<Option<(i64, Instant, String)>>,
        reply_delay: Duration,
        never_reply: bool,
        /// Host clock offset: a remote engine stamps createdAt on its own clock.
        host_skew_millis: i64,
    }

    #[async_trait]
    impl RpcService for DelayedTurn {
        async fn handle(&self, method: &str, params: Value) -> Result<RpcReply, RpcError> {
            match method {
                methods::WATCH_CHATS => {
                    let reply = self.world.handle(method, params).await?;
                    let RpcReply::Stream(mut rows) = reply else {
                        unreachable!()
                    };
                    let mut rows = rows.next().await.unwrap();
                    if let Some((_, _, chat)) = self.sent.lock().unwrap().as_ref()
                        && chat != "chat-beta-2"
                    {
                        rows.as_array_mut().unwrap().push(json!({
                            "id": chat, "deviceId":"dev-remote", "archived":false,
                            "createdAt":chrono::Utc::now(), "config":{"harness":"codex", "sandbox":"workspace-write"}
                        }));
                    }
                    Ok(stream(rows))
                }
                methods::QUEUE_COMMAND => {
                    let reply = self.world.handle(method, params.clone()).await?;
                    *self.sent.lock().unwrap() = Some((
                        now_millis(),
                        Instant::now(),
                        params["chatId"].as_str().unwrap().into(),
                    ));
                    tokio::time::sleep(Duration::from_millis(150)).await;
                    Ok(reply)
                }
                methods::WATCH_SESSIONS => {
                    let sent = self.sent.lock().unwrap().clone();
                    let Some((_, _, chat)) = sent else {
                        return Ok(stream(json!([])));
                    };
                    Ok(RpcReply::Stream(
                        futures::stream::once(async { json!([]) })
                            .chain(futures::stream::once(async move {
                                tokio::time::sleep(Duration::from_millis(50)).await;
                                json!([{"chatId":chat, "deviceId":"dev-remote", "status":"idle",
                                "updatedAt":chrono::Utc::now(), "lastCompletedTurn":"new-turn"}])
                            }))
                            .boxed(),
                    ))
                }
                methods::WATCH_DOC_MESSAGES => {
                    let sent = self.sent.lock().unwrap().clone();
                    // Only the existing chat has history. Its previous reply
                    // shares the send's timestamp, so only its id tells them apart.
                    let mut messages = json!([]);
                    if params["chatId"] == "chat-beta-2" {
                        let created_at = sent.as_ref().map_or_else(now_millis, |(t, _, _)| *t);
                        messages.as_array_mut().unwrap().push(json!({
                            "id":"previous-reply", "role":"assistant", "createdAt":created_at,
                            "deviceId":"dev-remote", "status":"complete", "parts":[{"kind":"text", "id":"t", "text":"old reply"}]
                        }));
                    }
                    if let Some((timestamp, started, _)) = sent
                        && started.elapsed() >= self.reply_delay
                        && !self.never_reply
                    {
                        messages.as_array_mut().unwrap().push(json!({
                            "id":"new-reply", "role":"assistant", "createdAt":timestamp + self.host_skew_millis,
                            "deviceId":"dev-remote", "status":"complete", "parts":[{"kind":"text", "id":"t", "text":"new reply"}]
                        }));
                    }
                    Ok(stream(json!({"reset":messages})))
                }
                _ => self.world.handle(method, params).await,
            }
        }
    }

    #[tokio::test]
    async fn wait_allows_a_missing_session_and_a_late_remote_transcript() {
        for create in [true, false] {
            let service = Arc::new(DelayedTurn {
                world: World {
                    beta_remote: true,
                    ..Default::default()
                },
                sent: Mutex::new(None),
                reply_delay: Duration::from_millis(450),
                never_reply: false,
                host_skew_millis: 0,
            });
            let tools = Tools::new(Arc::new(OpenAgents::with_client(
                memory_client(service),
                Origin::default(),
            )));
            let (name, args) = if create {
                (
                    "create_chat",
                    json!({"kind":"chat", "device":"Worker", "prompt":"go", "wait":true, "timeout_secs":2}),
                )
            } else {
                (
                    "send_message",
                    json!({"chat":"Beta", "text":"go", "wait":true, "timeout_secs":2}),
                )
            };
            let result = tools.call(name, args).await.unwrap();
            assert_eq!(result["turn"]["outcome"], "completed", "{result}");
            assert_eq!(
                result["turn"]["replies"].as_array().unwrap().len(),
                1,
                "{result}"
            );
            assert_eq!(result["turn"]["replies"][0]["text"], "new reply");
        }
    }

    #[tokio::test]
    async fn standalone_wait_after_nonblocking_send_keeps_the_send_baseline() {
        for create in [true, false] {
            let service = Arc::new(DelayedTurn {
                world: World {
                    beta_remote: true,
                    ..Default::default()
                },
                sent: Mutex::new(None),
                reply_delay: Duration::from_millis(450),
                never_reply: false,
                host_skew_millis: 0,
            });
            let tools = Tools::new(Arc::new(OpenAgents::with_client(
                memory_client(service),
                Origin::default(),
            )));
            let sent = if create {
                tools
                    .call(
                        "create_chat",
                        json!({"kind":"chat", "device":"Worker", "prompt":"go", "wait":false}),
                    )
                    .await
                    .unwrap()
            } else {
                tools
                    .call(
                        "send_message",
                        json!({"chat":"Beta", "text":"go", "wait":false}),
                    )
                    .await
                    .unwrap()
            };
            let result = tools
                .call(
                    "wait_for_turn",
                    json!({"chat":sent["chatId"], "timeout_secs":2}),
                )
                .await
                .unwrap();
            assert_eq!(result["turn"]["outcome"], "completed", "{result}");
            assert_eq!(result["turn"]["replies"].as_array().unwrap().len(), 1);
            assert_eq!(result["turn"]["replies"][0]["text"], "new reply");
        }
    }

    /// A remote host stamps replies on its own clock. One running behind the
    /// caller must not hide the reply to this send.
    #[tokio::test]
    async fn wait_returns_the_reply_from_a_host_whose_clock_is_behind() {
        for create in [true, false] {
            let service = Arc::new(DelayedTurn {
                world: World {
                    beta_remote: true,
                    ..Default::default()
                },
                sent: Mutex::new(None),
                reply_delay: Duration::from_millis(100),
                never_reply: false,
                host_skew_millis: -30_000,
            });
            let tools = Tools::new(Arc::new(OpenAgents::with_client(
                memory_client(service),
                Origin::default(),
            )));
            let (name, args) = if create {
                (
                    "create_chat",
                    json!({"kind":"chat", "device":"Worker", "prompt":"go", "wait":true, "timeout_secs":2}),
                )
            } else {
                (
                    "send_message",
                    json!({"chat":"Beta", "text":"go", "wait":true, "timeout_secs":2}),
                )
            };
            let result = tools.call(name, args).await.unwrap();
            assert_eq!(result["turn"]["outcome"], "completed", "{result}");
            let replies = result["turn"]["replies"].as_array().unwrap();
            assert!(replies.iter().any(|r| r["text"] == "new reply"), "{result}");
        }
    }

    #[tokio::test]
    async fn completed_session_without_new_transcript_times_out_without_old_reply() {
        let service = Arc::new(DelayedTurn {
            world: World {
                beta_remote: true,
                ..Default::default()
            },
            sent: Mutex::new(None),
            reply_delay: Duration::ZERO,
            never_reply: true,
            host_skew_millis: 0,
        });
        let tools = Tools::new(Arc::new(OpenAgents::with_client(
            memory_client(service),
            Origin::default(),
        )));
        let result = tools
            .call(
                "send_message",
                json!({"chat":"Beta", "text":"go", "wait":true, "timeout_secs":1}),
            )
            .await
            .unwrap();
        assert_eq!(result["turn"]["outcome"], "timedOut");
        assert_eq!(result["turn"]["replies"], json!([]));
    }

    #[tokio::test]
    async fn remote_replies_do_not_require_synchronized_clocks() {
        for host_skew_millis in [-1_000, 1_000] {
            for wait in [true, false] {
                let service = Arc::new(DelayedTurn {
                    world: World {
                        beta_remote: true,
                        ..Default::default()
                    },
                    sent: Mutex::new(None),
                    reply_delay: Duration::from_millis(450),
                    never_reply: false,
                    host_skew_millis,
                });
                let tools = Tools::new(Arc::new(OpenAgents::with_client(
                    memory_client(service),
                    Origin::default(),
                )));
                let sent = tools
                    .call(
                        "send_message",
                        json!({
                            "chat":"Beta", "text":"go", "wait":wait, "timeout_secs":2
                        }),
                    )
                    .await
                    .unwrap();
                let result = if wait {
                    sent
                } else {
                    tools
                        .call(
                            "wait_for_turn",
                            json!({
                                "chat":sent["chatId"], "timeout_secs":2
                            }),
                        )
                        .await
                        .unwrap()
                };
                assert_eq!(
                    result["turn"]["outcome"], "completed",
                    "offset={host_skew_millis}, wait={wait}: {result}"
                );
                assert_eq!(
                    result["turn"]["replies"].as_array().unwrap().len(),
                    1,
                    "{result}"
                );
                assert_eq!(result["turn"]["replies"][0]["id"], "new-reply");
            }
        }
    }

    #[tokio::test]
    async fn initialize_and_list_over_jsonrpc() {
        let world = Arc::new(World::default());
        let tools = tools(world, Origin::default());
        let init = crate::jsonrpc::handle_request(
            &tools,
            json!(1),
            "initialize",
            json!({ "protocolVersion": "2025-03-26" }),
        )
        .await;
        assert_eq!(init["result"]["protocolVersion"], "2025-03-26");
        assert_eq!(init["result"]["serverInfo"]["name"], "openagents");
        let list =
            crate::jsonrpc::handle_request(&tools, json!(2), "tools/list", Value::Null).await;
        assert!(list["result"]["tools"].as_array().unwrap().len() >= 10);
        let bad = crate::jsonrpc::handle_request(
            &tools,
            json!(3),
            "tools/call",
            json!({ "name": "nope" }),
        )
        .await;
        assert_eq!(bad["error"]["code"], -32602);
        let whoami = crate::jsonrpc::handle_request(
            &tools,
            json!(4),
            "tools/call",
            json!({ "name": "whoami" }),
        )
        .await;
        assert_eq!(whoami["result"]["isError"], false);
        assert_eq!(
            whoami["result"]["structuredContent"]["localDeviceId"],
            "dev-local"
        );
    }
}
