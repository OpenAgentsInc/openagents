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
                Err(error) => return self.resume_error(error),
            }
        };
        let target = lease.as_ref().or(self.history.active.as_ref()).unwrap();
        let document = match target.read() {
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
        self.main_draft = Draft::default();
        self.scroll = u16::MAX;
        self.main_scroll = u16::MAX;
        self.resume_picker = None;
        self.model_picker = None;
        self.screen = Screen::Conversation;
        self.slash_hidden = false;
        self.slash_selected = 0;
        if let Some(lease) = lease {
            self.history.active = Some(lease);
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

    fn resume_error(&mut self, error: String) -> bool {
        if let Some(picker) = &mut self.resume_picker {
            picker.error = Some(error.clone());
        }
        self.notice = Some(error);
        false
    }
}
