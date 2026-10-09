//! Saving chats to the openagents.com account (#11046): `/sync`, an
//! upload after each save, a "working" heartbeat while a reply runs, and
//! deletes in both directions. The sending, screening, and setting live in
//! [`coder_sync`]; this is the terminal's side. Off by default, and shown
//! only when signed in.
//!
//! While sync is on, this computer also checks in with the website, which
//! then offers a reply box on its chats (#11048). A reply typed there is
//! taken when Coder is free (the chat open here, or any chat when nothing
//! is being typed), shown as the person's message, and answered as usual;
//! the answer syncs back like any other.

use std::collections::{BTreeSet, VecDeque};
use std::path::Path;
use std::time::Instant;

use coder_sync::{Event, Job, Settings, Worker};
use openagents_login::Saved;
use serde_json::Value;

use crate::{App, sessions};

/// What `/sync` says when it is off.
pub const OFF: &str = "Saving chats to your account is off. /sync on saves new and changed chats; /sync all adds your earlier chats too.";
/// What `/sync` says when it is on.
pub const ON: &str = "Chats save to your account, and you can reply to them on openagents.com while Coder is open here. /sync off stops; /sync delete removes the ones already there.";
/// What Coder says when it answers a reply typed on the website.
pub const ANSWERING: &str = "Answering your reply from openagents.com.";
/// The most earlier chats `/sync all` sends (the newest).
const EARLIER: usize = 150;

/// The terminal's sync state.
pub(crate) struct SyncState {
    pub(crate) settings: Settings,
    worker: Option<Worker>,
    screen: secret_screen::Screen,
    computer: String,
    /// The last heartbeat sent for the open chat: working, and when.
    heartbeat: Option<(String, bool, Instant)>,
    told_full: bool,
    /// Checking in for replies typed on the website.
    listening: bool,
    /// Chats whose replies were asked for and haven't arrived.
    taking: BTreeSet<String>,
    /// Replies taken from the website, waiting for Coder to be free.
    inbox: VecDeque<(String, Vec<String>)>,
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The live sign-in in `dir`.
fn signed_in(dir: &Path) -> Option<Saved> {
    Saved::load(dir).filter(|saved| !saved.expired(now()))
}

/// After `coder sessions delete ID`: delete the chat from the website too,
/// now if it can be reached, else the next time Coder runs.
pub fn forget_deleted(dir: &Path, session: &str) {
    let mut settings = Settings::load(dir);
    if !settings.sent.contains_key(session) {
        return;
    }
    settings.sent.remove(session);
    let done = signed_in(dir).is_some_and(|saved| coder_sync::delete_now(&saved, session));
    if !done {
        settings.to_delete.insert(session.to_owned());
    }
    let _ = settings.store(dir);
}

impl App {
    /// Read the setting, and start sending when it is on (or deletes are
    /// waiting) and this computer is signed in.
    pub fn start_sync(&mut self) {
        let Some(dir) = self.account_dir.clone() else {
            return;
        };
        let settings = Settings::load(&dir);
        let mut sync = self.sync.take().unwrap_or_else(|| SyncState {
            settings: settings.clone(),
            worker: None,
            screen: secret_screen::Screen::host(),
            computer: openagents_login::computer_name(),
            heartbeat: None,
            told_full: false,
            listening: false,
            taking: BTreeSet::new(),
            inbox: VecDeque::new(),
        });
        sync.settings = settings;
        if (sync.settings.on || !sync.settings.to_delete.is_empty())
            && sync.worker.is_none()
            && let Some(saved) = signed_in(&dir)
        {
            let worker = Worker::start(saved);
            for session in &sync.settings.to_delete {
                worker.send(Job::Delete {
                    session: session.clone(),
                });
            }
            sync.worker = Some(worker);
            sync.listening = false;
            sync.taking.clear();
        }
        self.sync = Some(sync);
    }

    /// Stop sending (signed out).
    pub(crate) fn stop_sync(&mut self) {
        if let Some(sync) = &mut self.sync {
            sync.worker = None;
            sync.heartbeat = None;
            sync.listening = false;
            sync.taking.clear();
        }
    }

    /// Whether nothing runs here, so a reply from the website can start.
    fn idle_for_replies(&self) -> bool {
        !self.live.busy
            && !self.following()
            && self.request.is_none()
            && self.brainstorm_job.is_none()
            && !self.checking_key
            && !self.checking_jev
            && !self.delegations.iter().any(|child| child.running)
    }

    /// Whether a reply for `session` can start now: the chat is open here,
    /// or nothing is being typed and no picker is open, so switching to it
    /// loses nothing.
    fn free_for(&self, session: &str) -> bool {
        self.idle_for_replies()
            && (self.session_id() == Some(session)
                || (self.draft.text.trim().is_empty() && self.resume_picker.is_none()))
    }

    /// Answer the oldest reply taken from the website, when Coder is free:
    /// open its chat, show it as the person's message, and run the turn.
    pub(crate) fn answer_web_reply(&mut self) {
        let Some(session) = self
            .sync
            .as_ref()
            .and_then(|sync| sync.inbox.front())
            .map(|(session, _)| session.clone())
        else {
            return;
        };
        if !self.free_for(&session) {
            return;
        }
        let Some((session, texts)) = self.sync.as_mut().and_then(|sync| sync.inbox.pop_front())
        else {
            return;
        };
        if self.session_id() != Some(session.as_str()) && !self.resume(Some(session.as_str())) {
            self.notice = Some(
                "A reply from openagents.com couldn't be answered: its chat isn't on this computer."
                    .into(),
            );
            return;
        }
        let Some((last, earlier)) = texts.split_last() else {
            return;
        };
        self.select_agent(None);
        let typed = std::mem::take(&mut self.draft);
        for text in earlier {
            self.live
                .entries
                .push(crate::live::Entry::User(text.clone()));
        }
        self.draft.text = last.clone();
        self.draft.cursor = self.draft.text.len();
        self.submit_live();
        self.draft = typed;
        self.notice = Some(ANSWERING.into());
    }

    fn store_sync(&mut self) {
        if let (Some(dir), Some(sync)) = (&self.account_dir, &self.sync)
            && let Err(error) = sync.settings.store(dir)
        {
            self.notice = Some(error);
        }
    }

    /// A chat was saved here: send it when saving to the account is on.
    pub(crate) fn sync_saved(&mut self, document: &Value) {
        let Some(sync) = &mut self.sync else {
            return;
        };
        let (Some(worker), Some(session)) = (
            &sync.worker,
            document.get("session_id").and_then(Value::as_str),
        ) else {
            return;
        };
        if !sync.settings.on {
            return;
        }
        let upload = coder_sync::upload(document, &sync.computer, &sync.screen);
        if sync.settings.sends(session, &upload.digest) {
            worker.send(Job::Upload {
                session: session.to_owned(),
                upload,
            });
        }
    }

    /// Each tick: apply what the sender reported, and keep the open chat's
    /// "working" heartbeat current.
    pub(crate) fn poll_sync(&mut self) {
        let busy = self.live.busy;
        let open = self.session_id().map(str::to_owned);
        let Some(sync) = &mut self.sync else {
            return;
        };
        let Some(worker) = &sync.worker else {
            return;
        };
        let events = worker.drain();
        // Check in for replies typed on the website while sync is on.
        if sync.listening != sync.settings.on {
            sync.listening = sync.settings.on;
            worker.send(Job::Listen {
                computer: sync.listening.then(|| sync.computer.clone()),
            });
        }
        if let Some(session) = open.filter(|session| sync.settings.sent.contains_key(session)) {
            let due = match &sync.heartbeat {
                Some((last, working, at)) => {
                    *last != session
                        || *working != busy
                        || (busy && at.elapsed() >= coder_sync::HEARTBEAT)
                }
                None => busy,
            };
            if due && sync.settings.on {
                worker.send(Job::Status {
                    session: session.clone(),
                    working: busy,
                });
                sync.heartbeat = Some((session, busy, Instant::now()));
            }
        }
        if !events.is_empty() {
            for event in events {
                self.apply_sync(event);
            }
            self.store_sync();
        }
        self.answer_web_reply();
    }

    fn apply_sync(&mut self, event: Event) {
        let open = self.session_id().map(str::to_owned);
        let dir = self.account_dir.clone();
        if let Event::Waiting { sessions } = &event {
            let free: Vec<bool> = sessions.iter().map(|s| self.free_for(s)).collect();
            let Some(sync) = &mut self.sync else {
                return;
            };
            for (session, free) in sessions.iter().zip(free) {
                // Only this computer's own chats, one take at a time.
                let ours = sync.settings.sent.contains_key(session)
                    || open.as_deref() == Some(session.as_str());
                if !free
                    || !ours
                    || sync.settings.kept_here.contains(session)
                    || sync.taking.contains(session)
                    || sync.inbox.iter().any(|(waiting, _)| waiting == session)
                {
                    continue;
                }
                if let Some(worker) = &sync.worker {
                    sync.taking.insert(session.clone());
                    worker.send(Job::Take {
                        session: session.clone(),
                    });
                }
            }
            return;
        }
        let Some(sync) = &mut self.sync else {
            return;
        };
        match event {
            Event::Saved { session, digest } => {
                sync.settings.sent.insert(session, digest);
            }
            Event::Removed { session } => {
                sync.settings.to_delete.remove(&session);
                sync.settings.sent.remove(&session);
            }
            Event::Replies { session, replies } => {
                sync.taking.remove(&session);
                if !replies.is_empty() {
                    sync.inbox
                        .push_back((session, replies.into_iter().map(|r| r.text).collect()));
                }
            }
            Event::Waiting { .. } => {}
            Event::Gone { session } => {
                sync.taking.remove(&session);
                sync.settings.sent.remove(&session);
                // Deleted on the website: delete it here too, unless it is
                // the chat open now, which stays here and isn't sent again.
                let removed = open.as_deref() != Some(session.as_str())
                    && dir
                        .as_deref()
                        .is_some_and(|dir| sessions::Store::under(dir).delete(&session).is_ok());
                if !removed {
                    sync.settings.kept_here.insert(session.clone());
                }
                if let Some(worker) = &sync.worker {
                    worker.send(Job::Delete { session });
                }
            }
            Event::Full => {
                if !sync.told_full {
                    sync.told_full = true;
                    self.notice = Some(
                        "Your account has no room for more chats. Delete some on openagents.com."
                            .into(),
                    );
                }
            }
            Event::SignedOut => {
                sync.worker = None;
                sync.listening = false;
                sync.taking.clear();
                self.notice = Some(
                    "Your sign-in ended. Run /login to keep saving chats to your account.".into(),
                );
            }
            Event::Refused { message, .. } => {
                self.notice = Some(format!("A chat wasn't saved to your account. {message}"));
            }
        }
    }

    /// `/sync`, `/sync on`, `/sync all`, `/sync off`, `/sync delete`.
    pub(crate) fn sync_command(&mut self, argument: &str) {
        if self.account.is_none() {
            self.notice = Some("Sign in first with /login.".into());
            return;
        }
        if self.sync.is_none() {
            self.start_sync();
        }
        let Some(sync) = &mut self.sync else {
            return;
        };
        match argument.trim() {
            "" => {
                self.notice = Some(if sync.settings.on { ON } else { OFF }.into());
                return;
            }
            "on" => {
                sync.settings.on = true;
                self.notice = Some(
                    "New and changed chats now save to your account. /sync all adds your earlier chats too."
                        .into(),
                );
            }
            "all" => {
                sync.settings.on = true;
                self.notice = Some("Saving your chats to your account…".into());
            }
            "off" => {
                sync.settings.on = false;
                sync.heartbeat = None;
                self.notice = Some(
                    "Chats no longer save to your account. /sync delete removes the ones already there."
                        .into(),
                );
            }
            "delete" => {
                let sent: Vec<String> = sync.settings.sent.keys().cloned().collect();
                sync.settings.to_delete.extend(sent);
                sync.settings.sent.clear();
                self.notice = Some(if sync.settings.to_delete.is_empty() {
                    "Your account has no chats from this computer.".into()
                } else {
                    "Removing this computer's chats from your account…".into()
                });
            }
            _ => {
                self.notice = Some("Use /sync on, /sync all, /sync off, or /sync delete.".into());
                return;
            }
        }
        self.store_sync();
        // Start the sender if it isn't running; queue what is waiting.
        self.start_sync();
        let Some(sync) = &mut self.sync else {
            return;
        };
        let Some(worker) = &sync.worker else {
            if sync.settings.on || !sync.settings.to_delete.is_empty() {
                self.notice =
                    Some("Sign in again with /login to save chats to your account.".into());
            }
            return;
        };
        for session in &sync.settings.to_delete {
            worker.send(Job::Delete {
                session: session.clone(),
            });
        }
        if argument.trim() == "all"
            && let Some(dir) = self.account_dir.clone()
        {
            // Read and send the earlier chats off the terminal's thread.
            let jobs = worker.sender();
            let settings = sync.settings.clone();
            let computer = sync.computer.clone();
            let screen = sync.screen.clone();
            std::thread::spawn(move || {
                let store = sessions::Store::under(&dir);
                for summary in store.recent(EARLIER).unwrap_or_default() {
                    let Ok(document) = store.read(&summary.id) else {
                        continue;
                    };
                    let upload = coder_sync::upload(&document, &computer, &screen);
                    if settings.sends(&summary.id, &upload.digest)
                        && jobs
                            .send(Job::Upload {
                                session: summary.id,
                                upload,
                            })
                            .is_err()
                    {
                        return;
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sync_needs_a_sign_in_and_starts_off() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App {
            account_dir: Some(dir.path().to_path_buf()),
            ..App::default()
        };
        app.sync_command("on");
        assert_eq!(app.notice.as_deref(), Some("Sign in first with /login."));
        assert!(!Settings::load(dir.path()).on);

        // Signed in by name only (no token here): the setting is kept, and
        // the person is told to sign in again.
        app.account = Some("Octo".into());
        app.sync_command("");
        assert_eq!(app.notice.as_deref(), Some(OFF));
        app.sync_command("on");
        assert!(Settings::load(dir.path()).on);
        assert_eq!(
            app.notice.as_deref(),
            Some("Sign in again with /login to save chats to your account.")
        );
        app.sync_command("off");
        assert!(!Settings::load(dir.path()).on);
        app.sync_command("sideways");
        assert_eq!(
            app.notice.as_deref(),
            Some("Use /sync on, /sync all, /sync off, or /sync delete.")
        );
    }

    #[test]
    fn a_web_delete_removes_the_chat_here_but_keeps_the_open_one() {
        let dir = tempfile::tempdir().unwrap();
        let store = sessions::Store::under(dir.path());
        let mut chat = crate::live::Chat::default();
        chat.entries.push(crate::live::Entry::User("Hello".into()));
        let document = crate::trajectory::document(&chat, "old", "test/local", dir.path());
        store.save("old", &document).unwrap();
        let mut app = App {
            account_dir: Some(dir.path().to_path_buf()),
            account: Some("Octo".into()),
            ..App::default()
        };
        app.start_sync();
        app.sync
            .as_mut()
            .unwrap()
            .settings
            .sent
            .insert("old".into(), "d".into());
        app.apply_sync(Event::Gone {
            session: "old".into(),
        });
        assert!(store.read("old").is_err());
        let settings = &app.sync.as_ref().unwrap().settings;
        assert!(settings.sent.is_empty() && settings.kept_here.is_empty());

        // A chat this terminal doesn't have stays marked, never sent again.
        app.apply_sync(Event::Gone {
            session: "elsewhere".into(),
        });
        assert!(
            app.sync
                .as_ref()
                .unwrap()
                .settings
                .kept_here
                .contains("elsewhere")
        );
    }

    #[test]
    fn a_reply_from_the_website_opens_its_chat_and_runs_without_losing_a_draft() {
        let dir = tempfile::tempdir().unwrap();
        let store = sessions::Store::under(dir.path());
        let mut chat = crate::live::Chat::default();
        chat.entries
            .push(crate::live::Entry::User("Fix the build".into()));
        let document = crate::trajectory::document(&chat, "web", "test/local", dir.path());
        store.save("web", &document).unwrap();
        let mut app = App {
            account_dir: Some(dir.path().to_path_buf()),
            account: Some("Octo".into()),
            ..App::default()
        };
        app.attach_session_store(sessions::Store::under(dir.path()));
        app.start_sync();
        let reply = |app: &mut App, text: &str| {
            app.sync
                .as_mut()
                .unwrap()
                .inbox
                .push_back(("web".into(), vec![text.into()]));
        };

        // Something typed in another chat: the reply waits.
        app.draft.text = "half typed".into();
        reply(&mut app, "Now the tests");
        app.answer_web_reply();
        assert!(app.request.is_none());
        assert_eq!(app.sync.as_ref().unwrap().inbox.len(), 1);

        // Nothing typed: its chat opens and the turn runs.
        app.draft = crate::Draft::default();
        app.answer_web_reply();
        assert_eq!(app.session_id(), Some("web"));
        assert!(matches!(
            app.live.entries.last(),
            Some(crate::live::Entry::User(text)) if text == "Now the tests"
        ));
        assert!(app.live.busy && app.request.is_some());
        assert_eq!(app.notice.as_deref(), Some(ANSWERING));

        // While it runs, the next waits; then it runs in the open chat and
        // keeps what is being typed.
        reply(&mut app, "And the docs");
        app.answer_web_reply();
        assert_eq!(app.sync.as_ref().unwrap().inbox.len(), 1);
        app.live.busy = false;
        app.request = None;
        app.draft.text = "my own words".into();
        app.answer_web_reply();
        assert!(matches!(
            app.live.entries.last(),
            Some(crate::live::Entry::User(text)) if text == "And the docs"
        ));
        assert_eq!(app.draft.text, "my own words");

        // A chat this computer doesn't have: said plainly, nothing runs.
        app.live.busy = false;
        app.request = None;
        app.draft = crate::Draft::default();
        app.sync
            .as_mut()
            .unwrap()
            .inbox
            .push_back(("elsewhere".into(), vec!["Hi".into()]));
        app.answer_web_reply();
        assert!(app.request.is_none());
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("isn't on this computer"))
        );
    }
}
