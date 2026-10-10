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
//! the answer syncs back like any other. Messages added to a chat on the
//! website while this computer was offline (a run on a Cloud computer,
//! #11050) come with the same take and join the chat here, in order,
//! before any reply is answered (#11052). Screenshots and files asked for
//! there come with the take too and run through this computer's own host
//! (`crate::web_asks`, #11185).

use std::collections::{BTreeSet, VecDeque};
use std::path::Path;
use std::sync::mpsc;
use std::time::Instant;

use coder_sync::{Choice, Event, Job, Settings, Worker};
use openagents_login::Saved;
use serde_json::Value;

use crate::{App, sessions};

/// What `/sync` says when it is off.
pub const OFF: &str = "Saving chats to your account is off. /sync on saves new and changed chats; /sync all adds your earlier chats too.";
/// What `/sync` says when it is on.
pub const ON: &str = "Chats and memory save to your account, and you can reply to them on openagents.com while Coder is open here. /sync off stops; /sync delete removes the chats already there.";
/// What Coder asks, once, when signed in and nobody chose yet (#11089).
pub const QUESTION: &str = "Where should this computer's chats live? /sync all syncs all your chats to your account; /sync off keeps them on this computer.";
/// What Coder says when it answers a reply typed on the website.
pub const ANSWERING: &str = "Answering your reply from openagents.com.";
/// The most earlier chats `/sync all` sends (the newest).
pub(crate) const EARLIER: usize = 150;

/// The session prefix of a chat started on openagents.com for Coder on
/// this computer (the website's `coder_sync::web_session`): Coder takes
/// its first message though the chat isn't here yet, and opens a new
/// conversation under the id to answer it.
pub const WEB_STARTED: &str = "web-";

/// Whether this computer takes the waiting chat `session`: its own (sent
/// from here, or open here), or one started on the website for it that
/// isn't saved here yet. The website lists only chats waiting for this
/// computer's name, and hands each message out once.
fn takes(session: &str, sent_here: bool, open_here: bool, saved_here: bool) -> bool {
    sent_here || open_here || (session.starts_with(WEB_STARTED) && !saved_here)
}

/// The terminal's sync state.
pub(crate) struct SyncState {
    pub(crate) settings: Settings,
    pub(crate) worker: Option<Worker>,
    screen: secret_screen::Screen,
    computer: String,
    /// The last heartbeat sent for the open chat: working, and when.
    heartbeat: Option<(String, bool, Instant)>,
    told_full: bool,
    /// Checking in for replies typed on the website.
    listening: bool,
    /// Chats whose replies were asked for and haven't arrived.
    taking: BTreeSet<String>,
    /// What was taken from the website, waiting for Coder to be free.
    inbox: VecDeque<Inbound>,
    /// The website's choice for this computer, asked once at start
    /// (#11089); `None` inside when it has none.
    asked: Option<mpsc::Receiver<Option<Choice>>>,
    /// When this computer last told the website its choice: the website's
    /// answers just before that landed are older, so they wait.
    told: Option<Instant>,
    /// What runs here, as last told to the phone (#11165).
    pub(crate) board: crate::supervise::Board,
    /// Memory notes on the account (#11182).
    pub(crate) memory: crate::memory_sync::MemorySync,
    /// Coding runs for the account's own API key (#11080), taken while
    /// sync is on.
    own_runs: Option<crate::own_runs::Host>,
}

/// What one take brought for one chat.
#[derive(Debug, Default)]
struct Inbound {
    session: String,
    /// Messages added on the website while this computer was offline (a
    /// run on a Cloud computer, #11050), oldest first: they join the chat
    /// here, in order, before any reply is answered (#11052).
    added: Vec<coder_sync::Added>,
    /// Replies typed on the website, to answer.
    replies: Vec<String>,
}

/// A message from the website as a chat entry here.
fn entry(message: &coder_sync::Added) -> crate::live::Entry {
    if message.user {
        crate::live::Entry::User(message.text.clone())
    } else {
        crate::live::Entry::Assistant {
            text: message.text.clone(),
            model: None,
            elapsed_ms: None,
        }
    }
}

fn added_notice(count: usize) -> String {
    if count == 1 {
        "Added a message from a Cloud computer on openagents.com.".into()
    } else {
        format!("Added {count} messages from a Cloud computer on openagents.com.")
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// The live sign-in in `dir`.
pub(crate) fn signed_in(dir: &Path) -> Option<Saved> {
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
            asked: None,
            told: None,
            board: crate::supervise::Board::default(),
            memory: crate::memory_sync::MemorySync::default(),
            own_runs: None,
        });
        sync.settings = settings;
        // Ask the website once where this computer's chats live: a choice
        // made there (Settings, or the Connect page) applies here.
        if sync.asked.is_none()
            && let Some(saved) = signed_in(&dir)
        {
            let (send, asked) = mpsc::channel();
            let computer = sync.computer.clone();
            std::thread::spawn(move || {
                let _ = send.send(coder_sync::choice_now(&saved, &computer));
            });
            sync.asked = Some(asked);
        }
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
            sync.own_runs = None;
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

    /// Handle the oldest take from the website, when Coder is free: add the
    /// messages from a Cloud computer to its chat (in place when the chat
    /// isn't open and nothing is to be answered), then open the chat, show
    /// the oldest reply as the person's message, and run the turn.
    pub(crate) fn answer_web_reply(&mut self) {
        let Some((session, answers)) = self
            .sync
            .as_ref()
            .and_then(|sync| sync.inbox.front())
            .map(|inbound| (inbound.session.clone(), !inbound.replies.is_empty()))
        else {
            return;
        };
        // Messages for a chat that isn't open, with nothing to answer, go
        // straight to its saved copy: nothing here changes.
        let in_place = !answers && self.session_id() != Some(session.as_str());
        if !in_place && !self.free_for(&session) {
            return;
        }
        let Some(Inbound {
            session,
            added,
            replies: texts,
        }) = self.sync.as_mut().and_then(|sync| sync.inbox.pop_front())
        else {
            return;
        };
        let open = self.session_id() == Some(session.as_str());
        if !open && texts.is_empty() {
            self.notice = Some(match self.add_to_saved(&session, &added) {
                Ok(()) => added_notice(added.len()),
                Err(_) => {
                    "Messages from openagents.com couldn't be added to their chat here.".into()
                }
            });
            return;
        }
        // A chat started on the website for this computer opens as a new
        // conversation under its id; any other is resumed.
        let opened = open
            || if session.starts_with(WEB_STARTED) && !self.saved_here(&session) {
                self.open_new_session(&session)
            } else {
                self.resume(Some(session.as_str()))
            };
        if !opened {
            self.notice = Some(
                "A reply from openagents.com couldn't be answered: its chat isn't on this computer."
                    .into(),
            );
            return;
        }
        if !added.is_empty() {
            for message in &added {
                self.live.entries.push(entry(message));
            }
            self.history.dirty = true;
            self.notice = Some(added_notice(added.len()));
            if texts.is_empty() {
                self.persist_session(true);
                return;
            }
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

    /// Add messages from the website to a saved chat that isn't open here,
    /// after its last step, and send the chat again.
    fn add_to_saved(&mut self, session: &str, added: &[coder_sync::Added]) -> Result<(), String> {
        let dir = self.account_dir.clone().ok_or("No chats are saved here.")?;
        let lease = sessions::Store::under(dir).lease(session)?;
        let mut document = lease.read()?;
        for message in added {
            let source = if message.user {
                atif::Source::User
            } else {
                atif::Source::Agent
            };
            atif::append(&mut document, &atif::Step::said(source, &message.text))?;
        }
        if !atif::validate(&document).is_empty() {
            return Err("The chat couldn't take the messages.".into());
        }
        document["extra"]["updated_ms"] = serde_json::json!(atif::now_ms());
        lease.save(&document)?;
        drop(lease);
        self.sync_saved(&document);
        Ok(())
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
        self.poll_memory();
        let busy = self.live.busy;
        let open = self.session_id().map(str::to_owned);
        let account_dir = self.account_dir.clone();
        let cwd = self.cwd.clone();
        let asked = self
            .sync
            .as_mut()
            .and_then(|sync| sync.asked.as_ref()?.try_recv().ok());
        if let Some(web) = asked {
            self.apply_web_choice(web);
        }
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
        // Coding runs for the account's own API key (#11080): taken while
        // this computer checks in.
        if !sync.listening {
            sync.own_runs = None;
        } else if sync.own_runs.is_none()
            && let Some(saved) = account_dir.as_deref().and_then(signed_in)
        {
            sync.own_runs = Some(crate::own_runs::Host::start(
                saved,
                sync.computer.clone(),
                cwd,
            ));
        }
        let own_events = sync
            .own_runs
            .as_ref()
            .map(crate::own_runs::Host::drain)
            .unwrap_or_default();
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
        if let Some(event) = own_events.last() {
            self.notice = Some(crate::own_runs::notice(event));
        }
        self.report_activity();
        self.answer_web_reply();
    }

    /// Tell the website what runs here when it changed, while sync is on
    /// (#11165); the sender repeats it on its own schedule.
    fn report_activity(&mut self) {
        let on = self
            .sync
            .as_ref()
            .is_some_and(|sync| sync.settings.on && sync.worker.is_some());
        let items = if on {
            Some(self.activity_items())
        } else {
            None
        };
        let Some(sync) = &mut self.sync else {
            return;
        };
        let Some(worker) = &sync.worker else {
            return;
        };
        // Token counts alone wait for the next scheduled report.
        if crate::supervise::settled(items.as_ref())
            == crate::supervise::settled(sync.board.sent.as_ref())
        {
            return;
        }
        sync.board.sent.clone_from(&items);
        worker.send(Job::Activity {
            computer: sync.computer.clone(),
            items,
        });
    }

    /// A message sent from the phone for a chat here (#11165): answered
    /// like a reply typed on openagents.com.
    pub(crate) fn queue_phone_message(&mut self, session: String, text: String) {
        if let Some(sync) = &mut self.sync {
            sync.inbox.push_back(Inbound {
                session,
                added: Vec::new(),
                replies: vec![text],
            });
        }
    }

    /// The website's choice for this computer (#11089): one made there
    /// applies here; with none there, this computer's own goes there, and
    /// with neither, Coder asks.
    pub(crate) fn apply_web_choice(&mut self, web: Option<Choice>) {
        let Some(sync) = &self.sync else {
            return;
        };
        let here = Choice::of(&sync.settings);
        let fresh = sync
            .told
            .is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(30));
        match (web, here) {
            (Some(web), here) if Some(web) != here && !fresh => {
                let (argument, notice) = match web {
                    Choice::All => (
                        "all",
                        "Syncing all your chats, as chosen on openagents.com.",
                    ),
                    Choice::Local => (
                        "off",
                        "Chats stay on this computer, as chosen on openagents.com.",
                    ),
                };
                self.set_sync(argument, false);
                self.notice = Some(notice.into());
            }
            (None, Some(here)) => self.tell_web(here),
            (None, None) if self.account.is_some() && self.notice.is_none() => {
                self.notice = Some(QUESTION.into());
            }
            _ => {}
        }
    }

    /// Tell the website this computer's choice, off the terminal's thread.
    fn tell_web(&mut self, choice: Choice) {
        let Some(saved) = self.account_dir.as_deref().and_then(signed_in) else {
            return;
        };
        if let Some(sync) = &mut self.sync {
            sync.told = Some(Instant::now());
        }
        let computer = self
            .sync
            .as_ref()
            .map_or_else(openagents_login::computer_name, |sync| {
                sync.computer.clone()
            });
        std::thread::spawn(move || {
            let _ = coder_sync::choose_now(&saved, &computer, choice);
        });
    }

    fn apply_sync(&mut self, event: Event) {
        if let Event::Chosen { choice } = event {
            self.apply_web_choice(Some(choice));
            return;
        }
        if let Event::Commands { commands } = &event {
            for command in commands {
                self.apply_command(command);
            }
            return;
        }
        let open = self.session_id().map(str::to_owned);
        let dir = self.account_dir.clone();
        if let Event::Waiting { sessions } = &event {
            let free: Vec<bool> = sessions.iter().map(|s| self.free_for(s)).collect();
            let saved: BTreeSet<String> = sessions
                .iter()
                .filter(|s| self.saved_here(s))
                .cloned()
                .collect();
            let Some(sync) = &mut self.sync else {
                return;
            };
            for (session, free) in sessions.iter().zip(free) {
                // Only this computer's own chats, and chats started on the
                // website for it, one take at a time.
                let ours = takes(
                    session,
                    sync.settings.sent.contains_key(session),
                    open.as_deref() == Some(session.as_str()),
                    saved.contains(session),
                );
                if !free
                    || !ours
                    || sync.settings.kept_here.contains(session)
                    || sync.taking.contains(session)
                    || sync.inbox.iter().any(|waiting| waiting.session == *session)
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
            Event::Replies {
                session,
                replies,
                added,
            } => {
                sync.taking.remove(&session);
                // The website screened them; screen again before keeping.
                let added: Vec<coder_sync::Added> = added
                    .into_iter()
                    .map(|mut message| {
                        if sync.screen.check(&message.text).is_err() {
                            message.text = coder_sync::LEFT_OUT.into();
                        }
                        message
                    })
                    .collect();
                if !replies.is_empty() || !added.is_empty() {
                    sync.inbox.push_back(Inbound {
                        session,
                        added,
                        replies: replies.into_iter().map(|r| r.text).collect(),
                    });
                }
            }
            // A screenshot or file asked for on the website (#11185): run
            // through this computer's own host in the background; what came
            // of it goes back with the next round.
            Event::Asks { session, asks } => {
                if let Some(worker) = &sync.worker {
                    crate::web_asks::run(session, asks, sync.computer.clone(), worker.sender());
                }
            }
            Event::Waiting { .. } | Event::Chosen { .. } | Event::Commands { .. } => {}
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
        self.set_sync(argument, true);
    }

    /// [`Self::sync_command`]; `tell` sends the choice to the website (not
    /// when it came from there).
    fn set_sync(&mut self, argument: &str, tell: bool) {
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
                self.notice = Some(
                    if !sync.settings.chosen {
                        QUESTION
                    } else if sync.settings.on {
                        ON
                    } else {
                        OFF
                    }
                    .into(),
                );
                return;
            }
            "on" => {
                sync.settings.on = true;
                sync.settings.chosen = true;
                self.notice = Some(
                    "New and changed chats now save to your account. /sync all adds your earlier chats too."
                        .into(),
                );
            }
            "all" => {
                sync.settings.on = true;
                sync.settings.chosen = true;
                self.notice = Some("Saving your chats to your account…".into());
            }
            "off" => {
                sync.settings.on = false;
                sync.settings.chosen = true;
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
        if tell
            && matches!(argument.trim(), "on" | "all" | "off")
            && let Some(choice) = self.sync.as_ref().and_then(|s| Choice::of(&s.settings))
        {
            self.tell_web(choice);
        }
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
        assert_eq!(app.notice.as_deref(), Some(QUESTION));
        app.sync_command("on");
        assert!(Settings::load(dir.path()).on);
        assert!(Settings::load(dir.path()).chosen);
        assert_eq!(
            app.notice.as_deref(),
            Some("Sign in again with /login to save chats to your account.")
        );
        app.sync_command("off");
        assert!(!Settings::load(dir.path()).on);
        app.sync_command("");
        assert_eq!(app.notice.as_deref(), Some(OFF));
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
            app.sync.as_mut().unwrap().inbox.push_back(Inbound {
                session: "web".into(),
                added: Vec::new(),
                replies: vec![text.into()],
            });
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
        app.sync.as_mut().unwrap().inbox.push_back(Inbound {
            session: "elsewhere".into(),
            added: Vec::new(),
            replies: vec!["Hi".into()],
        });
        app.answer_web_reply();
        assert!(app.request.is_none());
        assert!(
            app.notice
                .as_deref()
                .is_some_and(|notice| notice.contains("isn't on this computer"))
        );
    }

    #[test]
    fn a_chat_started_on_the_website_opens_as_a_new_conversation_and_runs() {
        // Taken: this computer's own chats, and website-started ones not
        // saved here yet.
        assert!(takes("web-1", false, false, false));
        assert!(!takes("web-1", false, false, true));
        assert!(!takes("someone-elses", false, false, false));
        assert!(takes("mine", true, false, true) && takes("open", false, true, true));

        let dir = tempfile::tempdir().unwrap();
        let store = sessions::Store::under(dir.path());
        let mut app = App {
            account_dir: Some(dir.path().to_path_buf()),
            account: Some("Octo".into()),
            ..App::default()
        };
        app.attach_session_store(sessions::Store::under(dir.path()));
        app.start_sync();
        let session = "web-12345678-1234-4234-8234-123456789abc";
        assert!(!app.saved_here(session));
        app.sync.as_mut().unwrap().inbox.push_back(Inbound {
            session: session.into(),
            added: Vec::new(),
            replies: vec!["Fix the login".into()],
        });
        app.answer_web_reply();
        assert_eq!(app.session_id(), Some(session), "{:?}", app.notice);
        assert_eq!(texts(&app.live), ["you: Fix the login"]);
        assert!(app.live.busy && app.request.is_some());
        assert_eq!(app.notice.as_deref(), Some(ANSWERING));
        // It saves under the website's id, so the answer syncs to that chat.
        app.history.dirty = true;
        assert!(app.persist_session(true));
        assert!(app.saved_here(session));
        assert_eq!(store.recent(10).unwrap()[0].id, session);
    }

    fn added(user: bool, text: &str) -> coder_sync::Added {
        coder_sync::Added {
            user,
            text: text.into(),
        }
    }

    fn texts(chat: &crate::live::Chat) -> Vec<String> {
        chat.entries
            .iter()
            .filter_map(|entry| match entry {
                crate::live::Entry::User(text) => Some(format!("you: {text}")),
                crate::live::Entry::Assistant { text, .. } => Some(format!("coder: {text}")),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn messages_from_a_cloud_computer_join_the_chat_in_order() {
        let dir = tempfile::tempdir().unwrap();
        let store = sessions::Store::under(dir.path());
        for id in ["closed", "open"] {
            let mut chat = crate::live::Chat::default();
            chat.entries.push(crate::live::Entry::User("Fix it".into()));
            let document = crate::trajectory::document(&chat, id, "test/local", dir.path());
            store.save(id, &document).unwrap();
        }
        let mut app = App {
            account_dir: Some(dir.path().to_path_buf()),
            account: Some("Octo".into()),
            ..App::default()
        };
        app.attach_session_store(sessions::Store::under(dir.path()));
        app.start_sync();

        // A chat that isn't open, with nothing to answer: added to its saved
        // copy in place; the open chat and the draft are untouched.
        assert!(app.resume(Some("open")));
        app.draft.text = "half typed".into();
        app.apply_sync(Event::Replies {
            session: "closed".into(),
            replies: Vec::new(),
            added: vec![
                added(true, "Go on"),
                added(false, "Fixed on a Cloud computer."),
            ],
        });
        app.answer_web_reply();
        assert_eq!(app.session_id(), Some("open"));
        assert_eq!(app.draft.text, "half typed");
        let saved = crate::trajectory::from_document(&store.read("closed").unwrap()).unwrap();
        assert_eq!(
            texts(&saved),
            [
                "you: Fix it",
                "you: Go on",
                "coder: Fixed on a Cloud computer."
            ]
        );
        assert_eq!(
            app.notice.as_deref(),
            Some("Added 2 messages from a Cloud computer on openagents.com.")
        );

        // The open chat: added before the reply that came with them, which
        // then runs.
        app.draft = crate::Draft::default();
        app.apply_sync(Event::Replies {
            session: "open".into(),
            replies: vec![coder_sync::Reply {
                id: "r1".into(),
                text: "Now the docs".into(),
            }],
            added: vec![added(false, "Tests pass on the Cloud computer.")],
        });
        app.answer_web_reply();
        assert_eq!(
            texts(&app.live)[1..],
            [
                "coder: Tests pass on the Cloud computer.",
                "you: Now the docs"
            ]
        );
        assert!(app.live.busy && app.request.is_some());
    }
}
