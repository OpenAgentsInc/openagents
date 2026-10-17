//! "Run this on your own computer" (#11982): after the first good answer
//! in a cloud chat ([`crate::cloud_chat`]), one card offers the person's
//! own computer, with the install command, the download page, and how a
//! computer joins the account.
//!
//! The card belongs to one chat: the first one that got a good answer. It
//! shows there until the person closes it or a computer of theirs joins,
//! and never anywhere else. That's once per account, or once per install
//! while signed out. An account with no record of its own takes the
//! install's, so signing in after closing the card doesn't bring it back.
//! The record is a small file beside the account data.
use crate::cloud_chat::{Role, Snapshot};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// The install command for macOS and Linux (`/download` on openagents.com).
pub const INSTALL_COMMAND: &str = "curl -fsSL https://openagents.com/cli/install.sh | bash";
/// Every download: Coder for each platform and the apps.
pub const DOWNLOAD_URL: &str = "https://openagents.com/download";
/// Where a `coder login` code is approved.
pub const APPROVE_URL: &str = "https://openagents.com/device";

/// What the card says. Every instruction comes with its command or link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Card {
    pub title: String,
    pub body: String,
    /// Copyable.
    pub command: String,
    pub download_label: String,
    pub download_url: String,
    pub connect: String,
    pub approve_url: String,
}

impl Card {
    pub fn new() -> Self {
        Self {
            title: "Run this on your own computer".into(),
            body: "Coder works on the code on your computer. Install it on a Mac or Linux \
                   computer with one command:"
                .into(),
            command: INSTALL_COMMAND.into(),
            download_label: "Windows and other downloads".into(),
            download_url: DOWNLOAD_URL.into(),
            connect: "Then run coder login, approve it at openagents.com/device, and type \
                      /sync on. Its chats and agents show up here."
                .into(),
            approve_url: APPROVE_URL.into(),
        }
    }
}

impl Default for Card {
    fn default() -> Self {
        Self::new()
    }
}

/// Whose record the card uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope<'a> {
    /// Signed in: the account's user id.
    Account(&'a str),
    /// Signed out: this install.
    Install,
}

impl Scope<'_> {
    fn key(self) -> String {
        match self {
            Scope::Account(id) => format!("account:{id}"),
            Scope::Install => INSTALL.into(),
        }
    }
}

const INSTALL: &str = "install";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    /// The chat the card appeared in.
    #[serde(default)]
    chat: Option<String>,
    /// Closed, or a computer joined: the card is done.
    #[serde(default)]
    done: bool,
}

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
    scopes: BTreeMap<String, Record>,
}

/// The open chat has a finished answer from OpenAgents, with no error.
pub fn answered(snapshot: &Snapshot) -> bool {
    snapshot.chat.is_some()
        && !snapshot.answering
        && snapshot.error.is_none()
        && snapshot
            .lines
            .iter()
            .any(|line| line.role == Role::Assistant && !line.text.trim().is_empty())
}

/// The card's record for every account and the install.
pub struct Offer {
    path: PathBuf,
    saved: Mutex<Saved>,
}

impl Offer {
    /// `dir` is the account data folder; the record is one file in it.
    pub fn open(dir: &Path) -> Self {
        let path = dir.join("own-computer-card.json");
        let saved = std::fs::read(&path)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default();
        Self {
            path,
            saved: Mutex::new(saved),
        }
    }

    /// Whether the card shows under `snapshot`'s chat now. `computers` is
    /// how many of the person's own computers have joined (Our cloud
    /// doesn't count). The first chat with a good answer gets the card.
    pub fn shows(&self, scope: Scope, snapshot: &Snapshot, computers: usize) -> bool {
        let mut saved = self.saved.lock().unwrap_or_else(|e| e.into_inner());
        let key = scope.key();
        let mut record = saved
            .scopes
            .get(&key)
            .or_else(|| saved.scopes.get(INSTALL))
            .cloned()
            .unwrap_or_default();
        let before = saved.scopes.get(&key).cloned();
        if computers > 0 {
            record.done = true;
        }
        let shows = if record.done {
            false
        } else if let Some(chat) = &record.chat {
            snapshot.chat.as_ref() == Some(chat)
        } else if answered(snapshot) {
            record.chat = snapshot.chat.clone();
            true
        } else {
            false
        };
        if before.as_ref() != Some(&record) && (record.done || record.chat.is_some()) {
            saved.scopes.insert(key, record);
            self.save(&saved);
        }
        shows
    }

    /// The person closed the card.
    pub fn close(&self, scope: Scope) {
        let mut saved = self.saved.lock().unwrap_or_else(|e| e.into_inner());
        let record = saved.scopes.entry(scope.key()).or_default();
        if !record.done {
            record.done = true;
            self.save(&saved);
        }
    }

    fn save(&self, saved: &Saved) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let Ok(bytes) = serde_json::to_vec(saved) else {
            return;
        };
        let partial = self.path.with_extension("partial");
        if std::fs::write(&partial, bytes).is_ok() {
            let _ = std::fs::rename(&partial, &self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cloud_chat::Line;

    fn chat(id: &str, answer: &str) -> Snapshot {
        Snapshot {
            chat: Some(id.into()),
            lines: vec![
                Line {
                    role: Role::User,
                    text: "What's a closure?".into(),
                },
                Line {
                    role: Role::Assistant,
                    text: answer.into(),
                },
            ],
            ..Snapshot::default()
        }
    }

    #[test]
    fn waits_for_a_finished_good_answer() {
        let dir = tempfile::tempdir().unwrap();
        let offer = Offer::open(dir.path());
        assert!(!offer.shows(Scope::Install, &Snapshot::default(), 0));
        let mut writing = chat("c1", "A closure is");
        writing.answering = true;
        assert!(!offer.shows(Scope::Install, &writing, 0));
        let mut failed = chat("c1", "");
        failed.error = Some("Too many messages right now.".into());
        assert!(!offer.shows(Scope::Install, &failed, 0));
        assert!(!offer.shows(Scope::Install, &chat("c1", "  "), 0));
        assert!(offer.shows(Scope::Install, &chat("c1", "A closure is a function."), 0));
    }

    #[test]
    fn stays_in_its_chat_and_survives_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let offer = Offer::open(dir.path());
        assert!(offer.shows(Scope::Account("u1"), &chat("c1", "Yes."), 0));
        assert!(!offer.shows(Scope::Account("u1"), &chat("c2", "Yes."), 0));
        let reopened = Offer::open(dir.path());
        assert!(reopened.shows(Scope::Account("u1"), &chat("c1", "Yes."), 0));
        assert!(!reopened.shows(Scope::Account("u1"), &chat("c2", "Yes."), 0));
        // Another account gets its own card.
        assert!(reopened.shows(Scope::Account("u2"), &chat("c9", "Yes."), 0));
    }

    #[test]
    fn closing_it_or_a_computer_joining_ends_it() {
        let dir = tempfile::tempdir().unwrap();
        let offer = Offer::open(dir.path());
        assert!(offer.shows(Scope::Account("u1"), &chat("c1", "Yes."), 0));
        offer.close(Scope::Account("u1"));
        assert!(!offer.shows(Scope::Account("u1"), &chat("c1", "Yes."), 0));
        assert!(!Offer::open(dir.path()).shows(Scope::Account("u1"), &chat("c1", "Yes."), 0));

        assert!(offer.shows(Scope::Account("u2"), &chat("c2", "Yes."), 0));
        assert!(!offer.shows(Scope::Account("u2"), &chat("c2", "Yes."), 1));
        // The computer leaving later doesn't bring it back.
        assert!(!offer.shows(Scope::Account("u2"), &chat("c2", "Yes."), 0));
        // An account that already has a computer never sees it.
        assert!(!offer.shows(Scope::Account("u3"), &chat("c3", "Yes."), 2));
        assert!(!offer.shows(Scope::Account("u3"), &chat("c3", "Yes."), 0));
    }

    #[test]
    fn signing_in_keeps_the_install_record() {
        let dir = tempfile::tempdir().unwrap();
        let offer = Offer::open(dir.path());
        assert!(offer.shows(Scope::Install, &chat("v1", "Yes."), 0));
        // The visitor chat is claimed and keeps its id.
        assert!(offer.shows(Scope::Account("u1"), &chat("v1", "Yes."), 0));
        assert!(!offer.shows(Scope::Account("u1"), &chat("c2", "Yes."), 0));
        offer.close(Scope::Account("u1"));
        assert!(!offer.shows(Scope::Account("u1"), &chat("v1", "Yes."), 0));

        let other = tempfile::tempdir().unwrap();
        let offer = Offer::open(other.path());
        assert!(offer.shows(Scope::Install, &chat("v1", "Yes."), 0));
        offer.close(Scope::Install);
        assert!(!offer.shows(Scope::Account("u1"), &chat("c1", "Yes."), 0));
    }

    #[test]
    fn the_card_gives_the_command_and_links() {
        let card = Card::new();
        assert_eq!(card.command, INSTALL_COMMAND);
        // The same command and page the website's /download gives.
        let page = include_str!("../../../../../crates/openagents-web/src/pages/download.rs");
        assert!(page.contains(INSTALL_COMMAND));
        assert!(page.contains(".route(\"/download\""));
        assert!(card.download_url.starts_with("https://openagents.com/"));
        assert!(card.connect.contains("coder login"));
        assert!(card.connect.contains("openagents.com/device"));
        assert!(card.connect.contains("/sync on"));
        for text in [&card.title, &card.body, &card.connect, &card.download_label] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }
}
