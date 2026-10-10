//! Scheduled prompts on the account (#11177): while the person's `/sync`
//! choice is on and this computer is signed in, Coder sends this
//! computer's scheduled prompts (`/schedule`, host background rules) to
//! openagents.com and applies the account's list for this computer back,
//! so a prompt made, paused, or deleted on the website or in the apps
//! becomes (or stops being) a background rule here. The computer stays the
//! one that runs them; the website only keeps the list.
//!
//! Each prompt carries the time of its last change. Coder remembers, per
//! rule, what it last sent or applied and when ([`Known`], in
//! `schedules.json` beside the account file): a rule that changed here is
//! sent as newer, a rule removed here is sent as a deletion, and a record
//! from the account is applied only when it is newer than what Coder
//! knows (a delete wins a tie), the same rule as memory notes
//! ([`crate::memory_sync`]).
//!
//! Coder looks for changes every [`coder_sync::schedules::LOOK_EVERY`] and
//! asks for the account's list at least every
//! [`coder_sync::schedules::PULL_EVERY`], one exchange at a time, off the
//! terminal's thread. With sync off nothing is sent.
#![cfg_attr(not(unix), allow(dead_code, unused_imports))]

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::App;
use crate::schedule::When;

/// How long Coder waits after an exchange that didn't go through.
const RETRY_AFTER: Duration = Duration::from_secs(60);
/// How long Coder waits when the website doesn't keep schedules yet.
const MISSING_AFTER: Duration = Duration::from_secs(600);
/// Where Coder remembers what it last sent or applied, beside the account.
const FILE: &str = "schedules.json";
/// The most deletions Coder remembers; the oldest go first.
const MAX_DELETIONS: usize = 500;

/// One scheduled prompt (or the fact that one was deleted) as it travels
/// to and from the account (`openagents-web` `account_schedules`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Record {
    pub id: String,
    pub computer: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<String>,
    /// 0 Sunday to 6 Saturday; every day when empty. Only with `time`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub days: Vec<u8>,
    /// `HH:MM`, local time on the computer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every_secs: Option<u64>,
    /// The Coder chat it posts into; a new Coder run when unset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<String>,
    #[serde(default)]
    pub paused: bool,
    #[serde(default)]
    pub updated: u64,
    #[serde(default)]
    pub deleted: bool,
}

/// What Coder last sent or applied for one rule.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Seen {
    pub updated: u64,
    /// The prompt as last sent or applied, in one canonical line.
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub deleted: bool,
}

/// Per rule id, what Coder last sent or applied.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Known {
    #[serde(default)]
    pub rules: BTreeMap<String, Seen>,
}

impl Known {
    pub(crate) fn load(dir: &std::path::Path) -> Self {
        std::fs::read_to_string(dir.join(FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn store(&self, dir: &std::path::Path) {
        let Ok(text) = serde_json::to_string_pretty(self) else {
            return;
        };
        let tmp = dir.join(format!(".{FILE}.{}.tmp", std::process::id()));
        if std::fs::write(&tmp, text).is_ok() {
            let _ = std::fs::rename(&tmp, dir.join(FILE));
        }
    }

    /// Forget the oldest deletions past [`MAX_DELETIONS`].
    fn prune(&mut self) {
        let mut deletions: Vec<(u64, String)> = self
            .rules
            .iter()
            .filter(|(_, seen)| seen.deleted)
            .map(|(id, seen)| (seen.updated, id.clone()))
            .collect();
        if deletions.len() <= MAX_DELETIONS {
            return;
        }
        deletions.sort();
        let over = deletions.len() - MAX_DELETIONS;
        for (_, id) in deletions.into_iter().take(over) {
            self.rules.remove(&id);
        }
    }
}

/// The record for a scheduled prompt.
#[cfg(unix)]
pub(crate) fn record(
    id: &str,
    computer: &str,
    prompt: &crate::schedule::rules::Prompt,
    updated: u64,
) -> Record {
    let (days, time, every_secs) = match &prompt.when {
        When::At { time, days } => (days.clone(), Some(time.clone()), None),
        When::Every { seconds } => (Vec::new(), None, Some(*seconds)),
    };
    Record {
        id: id.into(),
        computer: computer.into(),
        prompt: Some(prompt.prompt.clone()),
        days,
        time,
        every_secs,
        chat: prompt.chat.clone(),
        chat_title: None,
        workspace: prompt.workspace.clone(),
        paused: prompt.paused,
        updated,
        deleted: false,
    }
}

/// The scheduled prompt a record from the account describes, or `None`
/// when it can't be one here: no prompt, a time that isn't `HH:MM`, an
/// interval under a minute, a day past Saturday, or a chat id that isn't
/// one.
#[cfg(unix)]
pub(crate) fn prompt_of(record: &Record) -> Option<crate::schedule::rules::Prompt> {
    let prompt = record.prompt.as_deref()?.trim();
    if prompt.is_empty() || prompt.len() > 4000 {
        return None;
    }
    let when = match (&record.time, record.every_secs) {
        (Some(time), None) => {
            let time = background::compile::fields::time_of_day(time)?;
            if record.days.iter().any(|day| *day > 6) {
                return None;
            }
            let mut days = record.days.clone();
            days.sort_unstable();
            days.dedup();
            if days.len() == 7 {
                days.clear();
            }
            When::At { time, days }
        }
        (None, Some(seconds)) if seconds >= 60 => When::Every { seconds },
        _ => return None,
    };
    if record
        .chat
        .as_deref()
        .is_some_and(|chat| !background::rule::chat_id(chat))
    {
        return None;
    }
    let workspace = record
        .workspace
        .clone()
        .filter(|w| (w.starts_with('/') || w.starts_with("~/")) && !w.contains(".."));
    Some(crate::schedule::rules::Prompt {
        when,
        prompt: prompt.to_owned(),
        workspace,
        chat: record.chat.clone(),
        paused: record.paused,
    })
}

/// One canonical line for a prompt, to tell whether it changed.
#[cfg(unix)]
fn content(prompt: &crate::schedule::rules::Prompt) -> String {
    serde_json::to_string(&record("", "", prompt, 0)).unwrap_or_default()
}

/// This computer's records to send: each scheduled prompt, stamped newer
/// when it changed since Coder last sent or applied it, and a deletion for
/// each one that is gone. `known` learns the new stamps.
#[cfg(unix)]
pub(crate) fn outgoing(
    local: &[(String, crate::schedule::rules::Prompt)],
    known: &mut Known,
    computer: &str,
    now: u64,
) -> Vec<Record> {
    let mut records = Vec::new();
    for (id, prompt) in local {
        let line = content(prompt);
        let seen = known.rules.get(id);
        let updated = match seen {
            Some(seen) if !seen.deleted && seen.content == line => seen.updated,
            _ => {
                let updated = now.max(seen.map_or(0, |seen| seen.updated + 1));
                known.rules.insert(
                    id.clone(),
                    Seen {
                        updated,
                        content: line,
                        deleted: false,
                    },
                );
                updated
            }
        };
        records.push(record(id, computer, prompt, updated));
    }
    for (id, seen) in &mut known.rules {
        if !seen.deleted && !local.iter().any(|(have, _)| have == id) {
            seen.deleted = true;
            seen.content.clear();
            seen.updated = now.max(seen.updated + 1);
        }
        if seen.deleted {
            records.push(Record {
                id: id.clone(),
                computer: computer.into(),
                updated: seen.updated,
                deleted: true,
                ..Record::default()
            });
        }
    }
    known.prune();
    records
}

/// Whether `record` is newer than what Coder knows of its rule.
fn newer(record: &Record, seen: Option<&Seen>) -> bool {
    seen.is_none_or(|seen| {
        record.updated > seen.updated
            || (record.updated == seen.updated && record.deleted && !seen.deleted)
    })
}

/// Apply the account's records for this computer to its background
/// rules: a newer prompt is made or changed, a newer deletion removes the
/// rule. Returns how many rules changed here.
#[cfg(unix)]
pub(crate) fn incoming(
    layout: &background::Layout,
    remote: &[Record],
    known: &mut Known,
    computer: &str,
) -> usize {
    use crate::schedule::rules;
    let mut changed = 0;
    for record in remote {
        if record.computer != computer
            || !record.id.starts_with(rules::PREFIX)
            || !background::rule::id_like(&record.id)
            || !newer(record, known.rules.get(&record.id))
        {
            continue;
        }
        let have = background::store::load(layout, &record.id).ok();
        if record.deleted {
            if have.is_some() && rules::remove(layout, &record.id).is_ok() {
                changed += 1;
            }
            known.rules.insert(
                record.id.clone(),
                Seen {
                    updated: record.updated,
                    content: String::new(),
                    deleted: true,
                },
            );
            continue;
        }
        let Some(prompt) = prompt_of(record) else {
            continue;
        };
        let current = have.as_ref().and_then(|rule| rules::read(rule, 0));
        if current.as_ref() != Some(&prompt) {
            // A prompt made on the website keeps the thread it came from.
            let thread = match have.as_ref().map(|rule| &rule.origin) {
                Some(background::rule::Origin::Conversation { thread, .. }) => thread.clone(),
                _ => "openagents.com".to_owned(),
            };
            let rule = rules::build(&record.id, &prompt, &thread);
            if background::store::save(layout, &rule).is_err() {
                continue;
            }
            changed += 1;
        }
        known.rules.insert(
            record.id.clone(),
            Seen {
                updated: record.updated,
                content: content(&prompt),
                deleted: false,
            },
        );
    }
    known.prune();
    changed
}

/// Where the schedule exchange stands.
#[derive(Default)]
pub(crate) struct ScheduleSync {
    /// When Coder may next look for changes.
    next_look: Option<Instant>,
    /// When the account's list was last asked for.
    pulled: Option<Instant>,
    /// This computer's prompts, as of the last exchange.
    sent: Option<String>,
    /// The exchange running now.
    running: Option<mpsc::Receiver<Outcome>>,
}

/// How one exchange ended.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// The account has this computer's prompts; `changed` rules changed
    /// here, and `local` is this computer's list afterwards.
    Done { changed: usize, local: String },
    /// The website doesn't keep schedules (an older website).
    Missing,
    /// Not reached, refused, or signed out: try again later.
    Failed,
}

/// Whether an exchange is due: this computer's prompts changed since the
/// last one, or the account's list hasn't been asked for lately.
fn due(sent: Option<&str>, now: &str, pulled: Option<Instant>) -> bool {
    sent != Some(now) || pulled.is_none_or(|at| at.elapsed() >= coder_sync::schedules::PULL_EVERY)
}

/// This computer's scheduled prompts, by id.
#[cfg(unix)]
fn local(layout: &background::Layout) -> Vec<(String, crate::schedule::rules::Prompt)> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    crate::schedule::rules::list(layout)
        .into_iter()
        .filter_map(|rule| Some((rule.id.clone(), crate::schedule::rules::read(&rule, now)?)))
        .collect()
}

/// This computer's prompts in one line, to tell whether they changed.
#[cfg(unix)]
fn local_line(layout: &background::Layout) -> String {
    local(layout)
        .iter()
        .map(|(id, prompt)| format!("{id}={}", content(prompt)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One exchange: send this computer's prompts, apply the account's back.
#[cfg(unix)]
fn exchange(
    saved: &openagents_login::Saved,
    dir: &std::path::Path,
    layout: &background::Layout,
    computer: &str,
) -> Outcome {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut known = Known::load(dir);
    let records: Vec<serde_json::Value> = outgoing(&local(layout), &mut known, computer, now)
        .iter()
        .filter_map(|record| serde_json::to_value(record).ok())
        .collect();
    known.store(dir);
    match coder_sync::schedules::exchange_now(saved, computer, &records) {
        Ok(remote) => {
            let remote: Vec<Record> = remote
                .into_iter()
                .filter_map(|record| serde_json::from_value(record).ok())
                .collect();
            let changed = incoming(layout, &remote, &mut known, computer);
            known.store(dir);
            Outcome::Done {
                changed,
                local: local_line(layout),
            }
        }
        Err(coder_sync::Answer::Unknown) => Outcome::Missing,
        Err(_) => Outcome::Failed,
    }
}

impl App {
    /// Each tick: take the finished exchange, and start the next one when
    /// it is due, sync is on, and this computer is signed in.
    #[cfg(unix)]
    pub(crate) fn poll_schedules(&mut self) {
        let Some(dir) = self.account_dir.clone() else {
            return;
        };
        let Some(sync) = &mut self.sync else {
            return;
        };
        let state = &mut sync.schedules;
        if let Some(running) = &state.running {
            match running.try_recv() {
                Ok(outcome) => {
                    state.running = None;
                    match outcome {
                        Outcome::Done { changed, local } => {
                            state.sent = Some(local);
                            if changed > 0 && self.notice.is_none() {
                                self.notice = Some(if changed == 1 {
                                    "Scheduled prompts: 1 changed from your account.".into()
                                } else {
                                    format!(
                                        "Scheduled prompts: {changed} changed from your account."
                                    )
                                });
                            }
                        }
                        Outcome::Missing => {
                            state.next_look = Some(Instant::now() + MISSING_AFTER);
                        }
                        Outcome::Failed => {
                            state.next_look = Some(Instant::now() + RETRY_AFTER);
                        }
                    }
                }
                Err(mpsc::TryRecvError::Empty) => return,
                Err(mpsc::TryRecvError::Disconnected) => state.running = None,
            }
        }
        let Some(sync) = &mut self.sync else {
            return;
        };
        // Private by default: nothing is sent while sync is off.
        if !sync.settings.on || sync.worker.is_none() {
            return;
        }
        let computer = sync.computer.clone();
        let state = &mut sync.schedules;
        let now = Instant::now();
        if state.next_look.is_some_and(|at| at > now) {
            return;
        }
        state.next_look = Some(now + coder_sync::schedules::LOOK_EVERY);
        let Ok(layout) = background::Layout::from_env() else {
            return;
        };
        if !due(state.sent.as_deref(), &local_line(&layout), state.pulled) {
            return;
        }
        let Some(saved) = crate::account_sync::signed_in(&dir) else {
            return;
        };
        state.pulled = Some(now);
        let (send, receive) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = send.send(exchange(&saved, &dir, &layout, &computer));
        });
        state.running = Some(receive);
    }

    #[cfg(not(unix))]
    pub(crate) fn poll_schedules(&mut self) {}
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::schedule::rules::{self, Prompt};

    fn prompt(text: &str) -> Prompt {
        Prompt {
            when: When::At {
                time: "09:00".into(),
                days: vec![1, 2, 3, 4, 5],
            },
            prompt: text.into(),
            workspace: Some("/home/me/work".into()),
            chat: None,
            paused: false,
        }
    }

    #[test]
    fn an_exchange_is_due_when_prompts_changed_or_the_list_is_old() {
        let recent = Some(Instant::now());
        assert!(due(None, "a", recent), "never sent");
        assert!(due(Some("a"), "b", recent), "changed here");
        assert!(!due(Some("a"), "a", recent), "nothing new");
        assert!(due(Some("a"), "a", None), "never asked");
    }

    #[test]
    fn a_change_here_is_sent_newer_and_a_removal_as_a_deletion() {
        let mut known = Known::default();
        let local = vec![("prompt-triage".to_owned(), prompt("triage"))];
        let sent = outgoing(&local, &mut known, "mac", 100);
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].updated, 100);
        assert_eq!(sent[0].time.as_deref(), Some("09:00"));
        assert_eq!(sent[0].days, vec![1, 2, 3, 4, 5]);
        assert_eq!(sent[0].computer, "mac");
        // Unchanged: the same stamp, even later.
        assert_eq!(outgoing(&local, &mut known, "mac", 200)[0].updated, 100);
        // Changed here: newer.
        let mut paused = prompt("triage");
        paused.paused = true;
        let changed = vec![("prompt-triage".to_owned(), paused)];
        let sent = outgoing(&changed, &mut known, "mac", 300);
        assert!(sent[0].paused && sent[0].updated == 300);
        // Removed here: a deletion, kept until the account has it.
        let sent = outgoing(&[], &mut known, "mac", 400);
        assert_eq!(sent.len(), 1);
        assert!(sent[0].deleted && sent[0].updated == 400 && sent[0].prompt.is_none());
        assert_eq!(outgoing(&[], &mut known, "mac", 500)[0].updated, 400);
    }

    #[test]
    fn a_prompt_made_on_the_website_becomes_a_rule_here_and_its_delete_removes_it() {
        let home = tempfile::tempdir().unwrap();
        let layout = background::Layout::new(home.path(), None).unwrap();
        let mut known = Known::default();
        let made = Record {
            id: "prompt-0123456789ab".into(),
            computer: "mac".into(),
            prompt: Some("what changed overnight?".into()),
            days: vec![5, 1, 2, 3, 4],
            time: Some("9am".into()),
            chat: Some("2026-10-10-chat".into()),
            updated: 50,
            ..Record::default()
        };
        // Another computer's prompt is not this one's to run.
        let elsewhere = Record {
            id: "prompt-elsewhere".into(),
            computer: "other".into(),
            ..made.clone()
        };
        assert_eq!(
            incoming(&layout, &[made.clone(), elsewhere], &mut known, "mac"),
            1
        );
        let saved = rules::list(&layout);
        assert_eq!(saved.len(), 1);
        assert!(saved[0].validate().is_ok());
        let read = rules::read(&saved[0], 0).unwrap();
        assert_eq!(
            read.when,
            When::At {
                time: "09:00".into(),
                days: vec![1, 2, 3, 4, 5]
            }
        );
        assert_eq!(read.chat.as_deref(), Some("2026-10-10-chat"));
        // The same record again changes nothing, and is not sent as newer.
        assert_eq!(incoming(&layout, &[made.clone()], &mut known, "mac"), 0);
        let local = vec![(saved[0].id.clone(), read)];
        assert_eq!(outgoing(&local, &mut known, "mac", 999)[0].updated, 50);
        // Paused on the website: the rule is off here.
        let paused = Record {
            paused: true,
            updated: 60,
            ..made.clone()
        };
        assert_eq!(incoming(&layout, &[paused], &mut known, "mac"), 1);
        assert!(!rules::list(&layout)[0].enabled);
        // An older record doesn't undo it.
        assert_eq!(incoming(&layout, &[made.clone()], &mut known, "mac"), 0);
        assert!(!rules::list(&layout)[0].enabled);
        // Deleted on the website: the rule goes.
        let gone = Record {
            id: made.id.clone(),
            computer: "mac".into(),
            updated: 60,
            deleted: true,
            ..Record::default()
        };
        assert_eq!(incoming(&layout, &[gone], &mut known, "mac"), 1);
        assert!(rules::list(&layout).is_empty());
        // A record that can't be a prompt here is skipped.
        let bad = Record {
            id: "prompt-bad".into(),
            computer: "mac".into(),
            prompt: Some("x".into()),
            every_secs: Some(10),
            updated: 70,
            ..Record::default()
        };
        assert_eq!(incoming(&layout, &[bad], &mut known, "mac"), 0);
        assert!(rules::list(&layout).is_empty());
    }
}
