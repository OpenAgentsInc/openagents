//! Saving chats to the openagents.com account (#11046): `/sync`, an
//! upload after each save, a "working" heartbeat while a reply runs, and
//! deletes in both directions. The sending, screening, and setting live in
//! [`coder_sync`]; this is the terminal's side. Off by default, and shown
//! only when signed in.

use std::path::Path;
use std::time::Instant;

use coder_sync::{Event, Job, Settings, Worker};
use openagents_login::Saved;
use serde_json::Value;

use crate::{App, sessions};

/// What `/sync` says when it is off.
pub const OFF: &str = "Saving chats to your account is off. /sync on saves new and changed chats; /sync all adds your earlier chats too.";
/// What `/sync` says when it is on.
pub const ON: &str =
    "Chats save to your account. /sync off stops; /sync delete removes the ones already there.";
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
        }
        self.sync = Some(sync);
    }

    /// Stop sending (signed out).
    pub(crate) fn stop_sync(&mut self) {
        if let Some(sync) = &mut self.sync {
            sync.worker = None;
            sync.heartbeat = None;
        }
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
        if events.is_empty() {
            return;
        }
        for event in events {
            self.apply_sync(event);
        }
        self.store_sync();
    }

    fn apply_sync(&mut self, event: Event) {
        let open = self.session_id().map(str::to_owned);
        let dir = self.account_dir.clone();
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
            Event::Gone { session } => {
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
}
