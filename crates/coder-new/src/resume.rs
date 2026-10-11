//! Recent-session selection and terminal persistence over the shared ATIF store.

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use serde_json::json;

use crate::{App, Draft, Mode, Screen, sessions, trajectory};

#[derive(Default)]
pub(crate) struct History {
    store: Option<sessions::Store>,
    active: Option<sessions::Lease>,
    pub(crate) dirty: bool,
    last_attempt: Option<u64>,
    save_failed: bool,
    choices: Vec<sessions::Summary>,
    following: Option<Following>,
}

/// A session another process holds, which this terminal watches
/// (`coder --follow ID`, #10752): an agent's Coder session in its pane.
#[derive(Debug)]
struct Following {
    id: String,
    /// The session file's last change this terminal loaded.
    seen: Option<std::time::SystemTime>,
    /// The person pressed a key: take the session over as soon as its
    /// holder lets go.
    takeover: bool,
    /// Keep the newest work in view; scrolling up lets go of it, and
    /// End takes it up again.
    stick: bool,
}

/// A stable newest-first snapshot of saved conversations.
pub struct Picker {
    pub sessions: Vec<sessions::Summary>,
    pub selected: usize,
    pub page: usize,
    pub error: Option<String>,
}

pub enum Action {
    Continue,
    Close,
    Select(String),
}

impl Picker {
    pub fn handle(&mut self, key: KeyEvent) -> Action {
        if key.kind == KeyEventKind::Release
            || key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return Action::Continue;
        }
        let last = self.sessions.len().saturating_sub(1);
        match key.code {
            KeyCode::Esc => return Action::Close,
            KeyCode::Enter if key.kind == KeyEventKind::Press => {
                if let Some(session) = self.sessions.get(self.selected) {
                    return Action::Select(session.id.clone());
                }
            }
            KeyCode::Up | KeyCode::BackTab | KeyCode::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Tab | KeyCode::Char('j') => {
                self.selected = self.selected.saturating_add(1).min(last);
            }
            KeyCode::PageUp => self.selected = self.selected.saturating_sub(self.page.max(1)),
            KeyCode::PageDown => {
                self.selected = self.selected.saturating_add(self.page.max(1)).min(last);
            }
            KeyCode::Home => self.selected = 0,
            KeyCode::End => self.selected = last,
            _ => {}
        }
        Action::Continue
    }
}

impl App {
    /// Attach an explicit storage root. CLI callers retain their own session lease.
    pub fn attach_session_store(&mut self, store: sessions::Store) {
        self.history.store = Some(store);
    }

    pub fn session_id(&self) -> Option<&str> {
        self.history.active.as_ref().map(sessions::Lease::id)
    }

    pub(crate) fn ensure_session(&mut self) -> bool {
        let Some(store) = &self.history.store else {
            return true;
        };
        if self.history.active.is_some() {
            return true;
        }
        // A new terminal conversation must never overwrite an existing session.
        for _ in 0..8 {
            let id = atif::log::session_id(atif::now_ms());
            match store.lease(&id).and_then(|lease| {
                if lease.exists()? {
                    Ok(None)
                } else {
                    Ok(Some(lease))
                }
            }) {
                Ok(Some(lease)) => {
                    self.history.active = Some(lease);
                    return true;
                }
                Ok(None) => {}
                Err(error) => return self.resume_error(error),
            }
        }
        self.resume_error("Cannot allocate a new conversation ID.".into())
    }

    /// Save changed live transcripts, with checkpoints during streaming replies.
    pub fn persist_session(&mut self, force: bool) -> bool {
        // A followed session belongs to its holder until the takeover.
        if self.history.following.is_some() {
            return true;
        }
        if self.mode != Mode::Live || !self.history.dirty || self.history.store.is_none() {
            return true;
        }
        if self.live.entries.is_empty() && self.delegations.is_empty() {
            return true;
        }
        if !force
            && self
                .history
                .last_attempt
                .is_some_and(|last| self.elapsed_seconds.saturating_sub(last) < 5)
            && (self.live.busy || self.history.save_failed)
        {
            return true;
        }
        if !self.ensure_session() {
            return false;
        }
        let cwd = self.cwd.as_deref().unwrap_or(std::path::Path::new("."));
        let mut document = trajectory::main_document(self, cwd);
        document["extra"]["updated_ms"] = json!(atif::now_ms());
        self.history.last_attempt = Some(self.elapsed_seconds);
        match self.history.active.as_ref().unwrap().save(&document) {
            Ok(()) => {
                self.history.dirty = false;
                self.history.save_failed = false;
                self.sync_saved(&document);
                if self
                    .notice
                    .as_deref()
                    .is_some_and(|notice| notice.starts_with("Cannot save conversation:"))
                {
                    self.notice = None;
                }
                true
            }
            Err(error) => {
                self.history.save_failed = true;
                self.resume_error(format!("Cannot save conversation: {error}"))
            }
        }
    }

    /// Open the picker, choose a displayed number, or resume an exact session ID.
    pub fn resume(&mut self, selection: Option<&str>) -> bool {
        if self.live.busy
            || self.checking_key
            || self.checking_jev
            || self.brainstorm_job.is_some()
            || self.request.is_some()
            || self.delegations.iter().any(|child| child.running)
        {
            return self.resume_error(
                "Stop the current work with Esc before resuming a conversation.".into(),
            );
        }
        let Some(store) = self.history.store.clone() else {
            return self.resume_error("Conversation storage is unavailable.".into());
        };
        if !self.persist_session(true) {
            return false;
        }
        let Some(selection) = selection else {
            return match store.recent(100) {
                Ok(sessions) => {
                    self.history.choices = sessions.clone();
                    self.resume_picker = Some(Picker {
                        sessions,
                        selected: 0,
                        page: 10,
                        error: None,
                    });
                    self.model_picker = None;
                    self.screen = Screen::Conversation;
                    self.notice = None;
                    true
                }
                Err(error) => self.resume_error(error),
            };
        };
        let id = if let Ok(number) = selection.parse::<usize>() {
            let Some(session) = number
                .checked_sub(1)
                .and_then(|index| self.history.choices.get(index))
            else {
                return self.resume_error(
                    "Choose a number from the last /resume list, or use a session ID.".into(),
                );
            };
            session.id.clone()
        } else {
            selection.to_owned()
        };
        let same = self.session_id() == Some(id.as_str());
        if same && self.mode == Mode::Live {
            self.select_agent(None);
            self.scroll_main_to_end();
            self.resume_picker = None;
            self.notice = Some(format!("Conversation {id} is already open."));
            return true;
        }
        let lease = if same {
            None
        } else {
            match store.lease(&id) {
                Ok(lease) => Some(lease),
                Err(error) if error == "Another process is using this chat session." => None,
                Err(error) => return self.resume_error(error),
            }
        };
        let held = !same && lease.is_none();
        let document = match store.read(&id) {
            Ok(document) => document,
            Err(error) => return self.resume_error(error),
        };
        let mut restored = App::default();
        if let Err(error) = trajectory::restore_app(&mut restored, &document) {
            return self.resume_error(error);
        }
        // Decode every child before replacing the current conversation or its lease.
        self.set_mode(Mode::Live);
        self.cancel_request();
        self.select_agent(None);
        self.live = restored.live;
        self.delegations = restored.delegations;
        self.draft = Draft::default();
        self.composer = Default::default();
        self.main_composer = Default::default();
        self.composer_history.reset();
        self.main_draft = Draft::default();
        self.scroll = u16::MAX;
        self.main_scroll = u16::MAX;
        self.resume_picker = None;
        self.model_picker = None;
        self.screen = Screen::Conversation;
        self.slash_hidden = false;
        self.slash_selected = 0;
        if !same {
            self.history.active = lease;
        }
        self.history.following = held.then(|| Following {
            id: id.clone(),
            seen: None,
            takeover: true,
            stick: true,
        });
        if held {
            // Ask an idle terminal to hand over; keep showing updates until it does.
            if let Ok(path) = store.path(&id) {
                let _ = std::fs::write(path.with_extension("reclaim"), "reclaim\n");
            }
        }
        self.history.dirty = false;
        self.history.last_attempt = None;
        self.history.save_failed = false;
        let saved_cwd = document
            .pointer("/extra/repository")
            .and_then(serde_json::Value::as_str);
        self.notice = Some(match (saved_cwd, &self.cwd) {
            (Some(saved), Some(current)) if std::path::Path::new(saved) != current => {
                format!(
                    "Resumed {id}. Saved in {saved}; continuing in {}.",
                    current.display()
                )
            }
            _ => format!("Resumed {id}."),
        });
        true
    }

    /// Whether the conversation `id` is saved on this computer.
    pub(crate) fn saved_here(&self, id: &str) -> bool {
        self.history
            .store
            .as_ref()
            .and_then(|store| store.path(id).ok())
            .is_some_and(|path| path.exists())
    }

    /// Open a new, empty conversation saved under `id` (a chat started on
    /// openagents.com for this computer, `account_sync::WEB_STARTED`), as
    /// `/resume` would open a saved one; the current one is saved first.
    /// An `id` already saved here is resumed instead.
    pub(crate) fn open_new_session(&mut self, id: &str) -> bool {
        let Some(store) = self.history.store.clone() else {
            return self.resume_error("Conversation storage is unavailable.".into());
        };
        let lease = match store.lease(id) {
            Ok(lease) => lease,
            Err(error) => return self.resume_error(error),
        };
        match lease.exists() {
            Ok(false) => {}
            Ok(true) => {
                drop(lease);
                return self.resume(Some(id));
            }
            Err(error) => return self.resume_error(error),
        }
        if !self.persist_session(true) {
            return false;
        }
        let fresh = App::default();
        self.set_mode(Mode::Live);
        self.cancel_request();
        self.select_agent(None);
        self.live = fresh.live;
        self.delegations = fresh.delegations;
        self.draft = Draft::default();
        self.composer = Default::default();
        self.main_composer = Default::default();
        self.composer_history.reset();
        self.main_draft = Draft::default();
        self.scroll = u16::MAX;
        self.main_scroll = u16::MAX;
        self.resume_picker = None;
        self.model_picker = None;
        self.screen = Screen::Conversation;
        self.slash_hidden = false;
        self.slash_selected = 0;
        self.history.active = Some(lease);
        self.history.dirty = false;
        self.history.last_attempt = None;
        self.history.save_failed = false;
        true
    }

    /// Watch session `id`, which another process may hold, and take it
    /// over when the person presses a key and the holder lets go.
    pub fn follow(&mut self, id: &str) -> bool {
        let Some(store) = self.history.store.clone() else {
            return self.resume_error("Conversation storage is unavailable.".into());
        };
        if let Err(error) = store.path(id) {
            return self.resume_error(error);
        }
        self.set_mode(Mode::Live);
        self.history.following = Some(Following {
            id: id.to_owned(),
            seen: None,
            takeover: false,
            stick: true,
        });
        self.notice = Some(format!(
            "Following {id}. Press any key to take over the conversation."
        ));
        self.follow_tick();
        true
    }

    /// Whether this terminal watches a session it does not hold.
    #[must_use]
    pub fn following(&self) -> bool {
        self.history.following.is_some()
    }

    /// Reload the followed session when it changed, and take it over when
    /// the person asked and its holder has let go.
    pub fn follow_tick(&mut self) {
        if self.history.following.is_none() {
            self.answer_reclaim();
            return;
        }
        let Some(store) = self.history.store.clone() else {
            return;
        };
        let Some(following) = &self.history.following else {
            return;
        };
        let id = following.id.clone();
        if following.takeover
            && let Ok(lease) = store.lease(&id)
        {
            let document = if lease.exists().unwrap_or(false) {
                lease.read().ok()
            } else {
                None
            };
            if let Some(document) = document {
                self.load_followed(&document);
            }
            let _ = std::fs::remove_file(lease.path().with_extension("reclaim"));
            self.history.active = Some(lease);
            self.history.following = None;
            self.history.dirty = false;
            self.notice = Some(format!("Conversation {id} is yours now."));
            return;
        }
        let Ok(path) = store.path(&id) else {
            return;
        };
        let modified = std::fs::metadata(&path)
            .and_then(|metadata| metadata.modified())
            .ok();
        if modified.is_none() || modified == following.seen {
            return;
        }
        if let Ok(document) = store.read(&id) {
            self.load_followed(&document);
            if let Some(following) = &mut self.history.following {
                following.seen = modified;
            }
        }
    }

    /// An agent asked for the session this terminal holds, because the
    /// owner asked her for something (#10752). Idle, this terminal saves
    /// the conversation, lets go, and follows her new turn; the unsent
    /// draft stays in the composer. Mid-reply, it says so in the request
    /// and lets go when the reply ends.
    fn answer_reclaim(&mut self) {
        let Some(lease) = &self.history.active else {
            return;
        };
        let marker = lease.path().with_extension("reclaim");
        let Ok(asked) = std::fs::read_to_string(&marker) else {
            return;
        };
        let id = lease.id().to_owned();
        if self.live.busy {
            if asked.trim() != "busy" {
                let _ = std::fs::write(&marker, "busy\n");
            }
            return;
        }
        if !self.persist_session(true) {
            return;
        }
        self.history.active = None;
        let _ = std::fs::remove_file(&marker);
        self.history.following = Some(Following {
            id,
            seen: None,
            takeover: false,
            stick: true,
        });
        self.notice = Some("Your agent took the conversation back to answer you.".into());
        self.follow_tick();
    }

    fn load_followed(&mut self, document: &serde_json::Value) {
        let mut restored = App::default();
        if trajectory::restore_app(&mut restored, document).is_err() {
            return;
        }
        let at_end = self.history.following.as_ref().is_none_or(|f| f.stick);
        self.live = restored.live;
        self.delegations = restored.delegations;
        // A follower at the end watches the delegation running now, such as
        // an agent's Codex child chat, and the main chat otherwise.
        if at_end {
            self.selected_agent = running_delegation(document)
                .and_then(|id| self.delegations.iter().position(|child| child.id == id));
        } else if self
            .selected_agent
            .is_some_and(|index| index >= self.delegations.len())
        {
            self.selected_agent = None;
        }
        if at_end || self.live.entries.is_empty() {
            self.scroll = u16::MAX;
            self.main_scroll = u16::MAX;
        }
        self.screen = Screen::Conversation;
        self.history.dirty = false;
    }

    pub(crate) fn follow_stick(&mut self, stick: bool) {
        if let Some(following) = &mut self.history.following {
            following.stick = stick;
        }
    }

    /// A key while following asks to take the session over; Ctrl+C quits.
    pub(crate) fn follow_key(&mut self, key: KeyEvent) -> bool {
        if key.kind == KeyEventKind::Release {
            return true;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c' | 'd'))
        {
            return false;
        }
        match key.code {
            KeyCode::PageUp | KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(3);
                self.follow_stick(false);
                return true;
            }
            KeyCode::PageDown | KeyCode::Down => {
                self.scroll = self.scroll.saturating_add(3);
                return true;
            }
            KeyCode::End => {
                self.scroll = u16::MAX;
                self.follow_stick(true);
                return true;
            }
            _ => {}
        }
        if let Some(following) = &mut self.history.following
            && !following.takeover
        {
            following.takeover = true;
            self.notice =
                Some("Taking over: the conversation is yours as soon as its agent stops.".into());
        }
        self.follow_tick();
        true
    }

    fn resume_error(&mut self, error: String) -> bool {
        if let Some(picker) = &mut self.resume_picker {
            picker.error = Some(error.clone());
        }
        self.notice = Some(error);
        false
    }
}

/// The delegation a saved chat records as running, if one is: the last
/// `delegate` call, when it is still marked running.
fn running_delegation(document: &serde_json::Value) -> Option<String> {
    document
        .get("steps")?
        .as_array()?
        .iter()
        .rev()
        .filter_map(|step| step.pointer("/tool_calls/0"))
        .find(|call| {
            call.pointer("/extra/schema")
                .and_then(serde_json::Value::as_str)
                == Some("openagents.delegation.v1")
        })
        .filter(|call| {
            call.pointer("/extra/running")
                .and_then(serde_json::Value::as_bool)
                == Some(true)
        })
        .and_then(|call| call.get("tool_call_id"))
        .and_then(serde_json::Value::as_str)
        .map(str::to_owned)
}

#[cfg(test)]
mod follow_tests {
    use super::running_delegation;
    use serde_json::json;

    #[test]
    fn a_follower_finds_the_delegation_that_runs_now() {
        let call = |id: &str, running: bool| {
            json!({"tool_calls":[{"tool_call_id":id,"function_name":"delegate",
                "extra":{"schema":"openagents.delegation.v1","running":running}}]})
        };
        let running = json!({"steps":[call("s-delegate-1", false), {"message":"hi"},
            call("s-delegate-2", true)]});
        assert_eq!(
            running_delegation(&running).as_deref(),
            Some("s-delegate-2")
        );
        let done = json!({"steps":[call("s-delegate-1", true), call("s-delegate-2", false)]});
        assert_eq!(running_delegation(&done), None);
        assert_eq!(running_delegation(&json!({"steps":[]})), None);
    }
}
