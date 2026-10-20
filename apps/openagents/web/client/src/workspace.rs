//! The account's workspace registry as the browser keeps it (#12265): every
//! computer's chats, the computers, their projects, and the sidebar's pins
//! and sections, read from the registry room on `coder-sync` and written
//! back to it, as the desktop and phone apps do.
//!
//! [`Replica`] is the protocol half and runs anywhere: the hello, the
//! server's frames applied to an [`openagents_doc::RegistryDoc`], and the
//! pushes of the browser's own writes. [`crate::registry`] carries it over a
//! WebSocket in the browser. The rest of this module derives the sidebar's
//! rows and builds a new session's chat row and first command, the same
//! values `openagents-client` writes on the desktop.

use std::collections::HashMap;

use chrono::{DateTime, TimeZone, Utc};
use openagents_doc::{RegistryDoc, RegistryRow, RowOp};
use openagents_proto::view::{Indicator, effective_indicator};
use openagents_proto::{
    Chat, ChatConfig, Device, RunRequest, SandboxLevel, Session, SidebarPinChange,
    SidebarPreferences, SidebarSectionChange, Space, WorktreeSpec,
};
use serde::Deserialize;
use serde_json::{Value, json};

/// A computer counts as online when it checked in this recently, as the
/// desktop's Devices page counts it.
pub const ONLINE_MS: i64 = 70_000;

/// What a write refused with, in words for the person.
pub type Refused = String;

#[derive(Deserialize)]
#[serde(tag = "t", rename_all = "lowercase")]
enum ServerFrame {
    State {
        seq: u64,
        full: bool,
        #[serde(rename = "gcFloor", default)]
        gc_floor: u64,
        rows: Vec<RegistryRow>,
        #[serde(default)]
        presence: HashMap<String, i64>,
    },
    Rows {
        seq: u64,
        rows: Vec<RegistryRow>,
    },
    Ack {
        batch: String,
        seq: u64,
    },
    Presence {
        device: String,
        at: i64,
    },
    #[serde(rename = "probe-ok")]
    ProbeOk {},
    Error {},
}

/// What a server frame did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    /// Nothing the sidebar shows changed.
    Nothing,
    /// Rows, acks, or presence changed: draw again.
    Changed,
    /// Frames were missed: reconnect, and the hello fills the gap.
    Resync,
}

/// The browser's replica of the account's registry.
pub struct Replica {
    doc: RegistryDoc,
    presence: HashMap<String, i64>,
    /// A `state` frame arrived on this connection.
    joined: bool,
    /// A `state` frame arrived at least once.
    synced: bool,
}

impl Replica {
    /// `device` names this browser tab's writes (their clocks' tie-break);
    /// it never becomes a computer in the list.
    pub fn new(device: impl Into<String>) -> Self {
        Self {
            doc: RegistryDoc::new(device),
            presence: HashMap::new(),
            joined: false,
            synced: false,
        }
    }

    pub fn doc(&self) -> &RegistryDoc {
        &self.doc
    }

    /// Whether the account's rows have arrived.
    pub fn synced(&self) -> bool {
        self.synced
    }

    /// The first frame on a new connection.
    pub fn hello(&mut self) -> String {
        self.joined = false;
        self.doc.mark_disconnected();
        let cursor = self.doc.cursor();
        json!({
            "t": "hello",
            "cursor": (self.synced && cursor > 0).then_some(cursor),
            "device": self.doc.device_id(),
        })
        .to_string()
    }

    /// The connection closed.
    pub fn disconnected(&mut self) {
        self.joined = false;
        self.doc.mark_disconnected();
    }

    /// Applies one server frame. Anything unreadable is left out.
    pub fn apply(&mut self, text: &str) -> Applied {
        let Ok(frame) = serde_json::from_str::<ServerFrame>(text) else {
            return Applied::Nothing;
        };
        match frame {
            ServerFrame::State {
                seq,
                full,
                gc_floor,
                rows,
                presence,
            } => {
                self.doc.apply_state(seq, full, gc_floor, rows);
                self.presence.extend(presence);
                self.joined = true;
                self.synced = true;
                Applied::Changed
            }
            ServerFrame::Rows { seq, rows } => {
                if self.doc.apply_rows(seq, rows) {
                    Applied::Changed
                } else {
                    Applied::Resync
                }
            }
            ServerFrame::Ack { batch, seq } => {
                self.doc.ack_batch(&batch, seq);
                Applied::Changed
            }
            ServerFrame::Presence { device, at } => {
                self.presence.insert(device, at);
                Applied::Changed
            }
            ServerFrame::ProbeOk {} | ServerFrame::Error {} => Applied::Nothing,
        }
    }

    /// The push frames for writes not yet sent on this connection.
    pub fn pushes(&mut self) -> Vec<String> {
        if !self.joined {
            return Vec::new();
        }
        self.doc
            .take_pushable()
            .into_iter()
            .map(|batch| push_frame(&batch.batch, &batch.ops))
            .collect()
    }

    /// Whether every write of this tab has been saved.
    pub fn saved(&self) -> bool {
        self.doc.pending_len() == 0
    }

    // ── reads ────────────────────────────────────────────────────────────

    pub fn devices(&self) -> Vec<Device> {
        self.doc.read_devices().unwrap_or_default()
    }

    pub fn spaces(&self) -> Vec<Space> {
        self.doc.read_spaces().unwrap_or_default()
    }

    pub fn chats(&self) -> Vec<Chat> {
        self.doc.read_chats().unwrap_or_default()
    }

    pub fn chat(&self, id: &str) -> Option<Chat> {
        self.doc.chat(id).ok().flatten()
    }

    pub fn sessions(&self) -> Vec<Session> {
        self.doc.read_sessions().unwrap_or_default()
    }

    /// Pins and sections, once the desktop has set them up.
    pub fn preferences(&self) -> Option<SidebarPreferences> {
        self.doc.sidebar_preferences()
    }

    /// Whether `device` checked in within [`ONLINE_MS`] of `now_ms`.
    pub fn online(&self, device: &Device, now_ms: i64) -> bool {
        let seen = self
            .presence
            .get(&device.id)
            .copied()
            .into_iter()
            .chain(device.last_seen_at.map(|at| at.timestamp_millis()))
            .max();
        seen.is_some_and(|at| now_ms - at <= ONLINE_MS)
    }

    /// The computers that can run a session: not phones.
    pub fn computers(&self) -> Vec<Device> {
        let mut devices: Vec<Device> = self
            .devices()
            .into_iter()
            .filter(|d| can_host(&d.platform))
            .collect();
        devices.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
        devices
    }

    /// The sidebar's rows: every listed chat with its computer, project,
    /// pin, section, and live status.
    pub fn rows(&self, now_ms: i64) -> Vec<ChatRow> {
        let devices = self.devices();
        let spaces = self.spaces();
        let sessions = self.sessions();
        let preferences = self.preferences().unwrap_or_default();
        let now = Utc
            .timestamp_millis_opt(now_ms)
            .single()
            .unwrap_or_else(Utc::now);
        self.chats()
            .into_iter()
            .filter(|chat| chat.is_top_level())
            .map(|chat| {
                let device = devices.iter().find(|d| d.id == chat.device_id);
                let space = chat
                    .space_id
                    .as_deref()
                    .and_then(|id| spaces.iter().find(|s| s.id == id));
                let session = sessions.iter().find(|s| s.chat_id == chat.id);
                let indicator = effective_indicator(session, now);
                ChatRow {
                    pin: preferences
                        .pinned_session_ids
                        .iter()
                        .position(|id| *id == chat.id),
                    section: preferences
                        .sections
                        .iter()
                        .find(|s| s.session_ids.contains(&chat.id))
                        .map(|s| s.id.clone()),
                    computer: device
                        .map(|d| d.name.clone())
                        .unwrap_or_else(|| "A computer".into()),
                    online: device.is_some_and(|d| self.online(d, now_ms)),
                    project: space.map(space_name),
                    updated_ms: chat
                        .last_message_at
                        .unwrap_or(chat.created_at)
                        .timestamp_millis(),
                    created_ms: chat.created_at.timestamp_millis(),
                    working: indicator == Indicator::Working,
                    needs_you: indicator == Indicator::AwaitingInput,
                    failed: indicator == Indicator::Errored && chat.unseen(),
                    unseen: chat.unseen(),
                    chat,
                }
            })
            .collect()
    }

    // ── writes ───────────────────────────────────────────────────────────

    fn found(
        &self,
        id: &str,
        wrote: Result<bool, openagents_doc::DocError>,
    ) -> Result<(), Refused> {
        match wrote {
            Ok(true) => Ok(()),
            Ok(false) => Err(format!("We couldn't find that chat ({id}).")),
            Err(_) => Err("We couldn't save that change. Try again.".into()),
        }
    }

    pub fn rename_device(&mut self, id: &str, name: &str) -> Result<(), Refused> {
        let name = name.trim();
        if name.is_empty() {
            return Err("Enter a computer name.".into());
        }
        match self.doc.rename_device(id, name) {
            Ok(true) => Ok(()),
            Ok(false) => Err("We couldn't find that computer.".into()),
            Err(_) => Err("We couldn't rename that computer. Try again.".into()),
        }
    }

    pub fn rename(&mut self, id: &str, title: &str) -> Result<(), Refused> {
        let wrote = self.doc.rename_chat(id, title.trim());
        self.found(id, wrote)
    }

    pub fn archive(&mut self, id: &str, archived: bool) -> Result<(), Refused> {
        let wrote = self.doc.set_chat_archived(id, archived);
        self.found(id, wrote)
    }

    /// Removes the chat from every device's list.
    pub fn delete(&mut self, id: &str) -> Result<(), Refused> {
        let wrote = self.doc.delete_chat(id);
        self.found(id, wrote)
    }

    /// Clears the chat's unseen mark everywhere.
    pub fn seen(&mut self, id: &str, now_ms: i64) {
        if let Some(at) = Utc.timestamp_millis_opt(now_ms).single() {
            let _ = self.doc.set_chat_seen(id, at);
        }
    }

    fn pins(&mut self, change: SidebarPinChange) -> Result<(), Refused> {
        if !self.doc.sidebar_pins_initialized() {
            return Err(
                "Pins aren't ready yet. Open the OpenAgents app once, then try again.".into(),
            );
        }
        self.doc
            .change_sidebar_pin(&change)
            .map_err(|_| "We couldn't save that change. Try again.".to_owned())
    }

    /// Pins the chat last.
    pub fn pin(&mut self, id: &str) -> Result<(), Refused> {
        let after = self
            .preferences()
            .and_then(|p| p.pinned_session_ids.last().cloned())
            .filter(|last| last != id);
        self.pins(SidebarPinChange::Pin {
            session_id: id.to_owned(),
            after,
            before: None,
        })
    }

    pub fn unpin(&mut self, id: &str) -> Result<(), Refused> {
        self.pins(SidebarPinChange::Unpin {
            session_id: id.to_owned(),
        })
    }

    /// Moves a pin one place up (`up`) or down among the pins.
    pub fn move_pin(&mut self, id: &str, up: bool) -> Result<(), Refused> {
        let pins = self.preferences().unwrap_or_default().pinned_session_ids;
        let Some(at) = pins.iter().position(|p| p == id) else {
            return Ok(());
        };
        let (after, before) = if up {
            if at == 0 {
                return Ok(());
            }
            (
                at.checked_sub(2).map(|i| pins[i].clone()),
                Some(pins[at - 1].clone()),
            )
        } else {
            if at + 1 >= pins.len() {
                return Ok(());
            }
            (Some(pins[at + 1].clone()), pins.get(at + 2).cloned())
        };
        self.pins(SidebarPinChange::Move {
            session_id: id.to_owned(),
            after,
            before,
        })
    }

    fn section(&mut self, change: SidebarSectionChange) -> Result<(), Refused> {
        self.pins(SidebarPinChange::Section { change })
    }

    /// Makes a section named `name` with id `id`.
    pub fn create_section(&mut self, id: &str, name: &str) -> Result<(), Refused> {
        self.section(SidebarSectionChange::Create {
            id: id.to_owned(),
            name: name.trim().to_owned(),
        })
    }

    pub fn rename_section(&mut self, id: &str, name: &str) -> Result<(), Refused> {
        self.section(SidebarSectionChange::Rename {
            id: id.to_owned(),
            name: name.trim().to_owned(),
        })
    }

    /// Deletes a section; its chats go back to the list.
    pub fn delete_section(&mut self, id: &str) -> Result<(), Refused> {
        self.section(SidebarSectionChange::Delete { id: id.to_owned() })
    }

    pub fn collapse_section(&mut self, id: &str, collapsed: bool) -> Result<(), Refused> {
        self.section(SidebarSectionChange::Collapse {
            id: id.to_owned(),
            collapsed,
        })
    }

    /// Files a chat in a section (`None`: back to the list).
    pub fn assign(&mut self, chat: &str, section: Option<&str>) -> Result<(), Refused> {
        self.section(SidebarSectionChange::Assign {
            session_id: chat.to_owned(),
            section_id: section.map(str::to_owned),
            after: None,
            before: None,
        })
    }

    /// Writes a new session's chat row, as the desktop's `create_session`
    /// does: on a project's computer in its folder (or `cwd`), or on a
    /// computer with no project in a scratch folder of its own.
    pub fn create_session(&mut self, new: &NewSession, now_ms: i64) -> Result<Chat, Refused> {
        if !chat_home::valid_id(&new.id) {
            return Err("Start a new chat.".into());
        }
        let now = Utc
            .timestamp_millis_opt(now_ms)
            .single()
            .unwrap_or_else(Utc::now);
        let devices = self.devices();
        let (device_id, space_id, cwd) = match &new.project {
            Some(space_id) => {
                let space = self
                    .spaces()
                    .into_iter()
                    .find(|s| &s.id == space_id)
                    .ok_or_else(|| "That project isn't on this computer anymore.".to_owned())?;
                let cwd = new.cwd.clone().unwrap_or_else(|| space.path.clone());
                (space.device_id, Some(space.id), cwd)
            }
            None => {
                let host = devices
                    .iter()
                    .find(|d| d.id == new.computer)
                    .ok_or_else(|| "That computer isn't in your account anymore.".to_owned())?;
                let cwd = if host.supports(openagents_proto::capabilities::PROJECTLESS_SCRATCH_V1) {
                    openagents_proto::projectless::scratch_cwd(&new.id, now)
                } else {
                    "~".to_owned()
                };
                (host.id.clone(), None, cwd)
            }
        };
        let host = devices.iter().find(|d| d.id == device_id);
        if !host.is_some_and(|d| can_host(&d.platform)) {
            return Err("That device can't run sessions.".into());
        }
        if self.chat(&new.id).is_some() {
            return Err("Start a new chat.".into());
        }
        let chat = Chat {
            links: Default::default(),
            id: new.id.clone(),
            device_id,
            title: new.title.clone().filter(|t| !t.trim().is_empty()),
            archived: false,
            cwd: Some(cwd),
            branch: new.branch.clone(),
            checkout_id: None,
            source_context: None,
            config: Some(new.config.clone()),
            last_message_preview: None,
            last_message_at: None,
            created_at: now,
            harness_session_id: None,
            harness_session_cwd: None,
            space_id,
            last_seen_at: None,
            room_gen: Some(2),
            parent_chat_id: None,
        };
        self.doc
            .upsert_chat(&chat)
            .map_err(|_| "We couldn't start that chat. Try again.".to_owned())?;
        Ok(chat)
    }
}

/// Whether a device of `platform` can run sessions: phones can't.
pub fn can_host(platform: &str) -> bool {
    !matches!(platform, "ios" | "android" | "ipados")
}

/// A project's name: its own, or its folder's.
pub fn space_name(space: &Space) -> String {
    space.display_name().to_owned()
}

fn push_frame(batch: &str, ops: &[RowOp]) -> String {
    json!({"t": "push", "batch": batch, "ops": ops}).to_string()
}

/// One computer chat in the sidebar.
#[derive(Clone, Debug)]
pub struct ChatRow {
    pub chat: Chat,
    /// The computer's name.
    pub computer: String,
    pub online: bool,
    pub project: Option<String>,
    /// Its place among the pins.
    pub pin: Option<usize>,
    pub section: Option<String>,
    pub updated_ms: i64,
    pub created_ms: i64,
    pub working: bool,
    pub needs_you: bool,
    pub failed: bool,
    pub unseen: bool,
}

/// A session to start, as the new-session pickers chose it.
#[derive(Clone, Debug, PartialEq)]
pub struct NewSession {
    /// The new chat's ID.
    pub id: String,
    /// The computer it runs on.
    pub computer: String,
    /// The project (space) on that computer, if any.
    pub project: Option<String>,
    /// A folder inside the project to run in, instead of its root.
    pub cwd: Option<String>,
    pub branch: Option<String>,
    pub config: ChatConfig,
    pub title: Option<String>,
}

/// `QueueCommand`'s parameters for a run of `prompt` in `chat`, as the
/// desktop builds them from the chat's settings. `worktree` asks the
/// computer for a fresh worktree to run in.
pub fn run_command(
    chat: &Chat,
    message_id: &str,
    prompt: &str,
    worktree: Option<WorktreeSpec>,
) -> Value {
    let config = chat.config.as_ref();
    let request = RunRequest {
        prompt: prompt.to_owned(),
        harness: config.map(|c| c.harness),
        model: config.and_then(|c| c.model.clone()),
        reasoning: config.and_then(|c| c.reasoning),
        model_options: config.map(|c| c.model_options.clone()).unwrap_or_default(),
        cwd: chat.cwd.clone().unwrap_or_else(|| "~".into()),
        sandbox: config.map_or(SandboxLevel::WorkspaceWrite, |c| c.sandbox),
        auto_approve: true,
        resume: None,
        attachments: Vec::new(),
        worktree,
        mcp: None,
    };
    json!({
        "chatId": chat.id,
        "command": {
            "kind": "run",
            "request": request,
            "messageId": message_id,
        },
    })
}

/// `QueueCommand`'s parameters to stop the chat's run.
pub fn interrupt_command(chat: &str) -> Value {
    json!({"chatId": chat, "command": {"kind": "interrupt"}})
}

/// `QueueMessage`'s parameters: a message the computer sends when the
/// running turn ends.
pub fn queue_message(chat: &str, text: &str) -> Value {
    json!({"chatId": chat, "text": text})
}

/// One harness as a computer's `ListHarnesses` reports it.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Harness {
    pub id: openagents_proto::HarnessId,
    pub name: String,
    #[serde(default)]
    pub reasoning_levels: Vec<openagents_proto::ReasoningLevel>,
    #[serde(default = "yes")]
    pub installed: bool,
    #[serde(default)]
    pub enabled: Option<bool>,
}

fn yes() -> bool {
    true
}

impl Harness {
    /// Whether the computer offers it for a new session: switched on in its
    /// settings, or, from an engine that predates the setting, installed.
    /// The test harness never counts.
    pub fn usable(&self) -> bool {
        self.id != openagents_proto::HarnessId::Mock && self.enabled.unwrap_or(self.installed)
    }
}

/// The sidebar's view choices, kept in this browser. The desktop keeps its
/// own in its settings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct View {
    pub organize: Organize,
    pub sort: Sort,
    pub compact: bool,
    pub archived: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Organize {
    #[default]
    ByComputer,
    ByProject,
    OneList,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Sort {
    #[default]
    Updated,
    Created,
}

impl View {
    /// The words this browser stores the view as.
    pub fn encode(self) -> String {
        format!(
            "{}.{}.{}.{}",
            match self.organize {
                Organize::ByComputer => "computer",
                Organize::ByProject => "project",
                Organize::OneList => "list",
            },
            match self.sort {
                Sort::Updated => "updated",
                Sort::Created => "created",
            },
            u8::from(self.compact),
            u8::from(self.archived),
        )
    }

    pub fn decode(text: &str) -> Self {
        let mut parts = text.split('.');
        let organize = match parts.next() {
            Some("project") => Organize::ByProject,
            Some("list") => Organize::OneList,
            _ => Organize::ByComputer,
        };
        let sort = match parts.next() {
            Some("created") => Sort::Created,
            _ => Sort::Updated,
        };
        Self {
            organize,
            sort,
            compact: parts.next() == Some("1"),
            archived: parts.next() == Some("1"),
        }
    }
}

/// Seconds since the epoch, for a sidebar row's age.
pub fn unix(at: DateTime<Utc>) -> u64 {
    at.timestamp().max(0) as u64
}

#[cfg(test)]
#[path = "workspace_tests.rs"]
mod tests;
