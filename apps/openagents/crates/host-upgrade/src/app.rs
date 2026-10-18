//! The app's half: notice a handed-over bundle, wait for a quiet moment,
//! swap it in, and, once reopened, say so and log the upgrade.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use crate::layout::Layout;
use crate::quiet::{Activity, QuietWait, Step};
use crate::record::{self, LogEntry, Outcome, Pending, Phase};

/// What the app does next.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    /// Show this notice: the restart is coming.
    Notice(String),
    /// The owner came back: the restart waits.
    Cancel(String),
    /// Restart now: [`begin_restart`], relaunch, quit.
    Restart(Pending),
}

/// Watches `<data>/host-upgrade` for a bundle handed to this app.
#[derive(Debug, Clone)]
pub struct Watcher {
    state_dir: PathBuf,
    bundle: PathBuf,
    timings: (Duration, Duration),
    wait: QuietWait,
    current: Option<String>,
}

impl Watcher {
    /// A watcher for the app at `bundle` with data in `data_dir`.
    #[must_use]
    pub fn new(data_dir: &Path, bundle: PathBuf) -> Watcher {
        Watcher::with_timings(data_dir, bundle, crate::quiet::IDLE, crate::quiet::NOTICE)
    }

    /// A watcher that counts `idle` without input as quiet and keeps the
    /// notice up for `notice`.
    #[must_use]
    pub fn with_timings(
        data_dir: &Path,
        bundle: PathBuf,
        idle: Duration,
        notice: Duration,
    ) -> Watcher {
        Watcher {
            state_dir: crate::layout::state_dir(data_dir),
            bundle,
            timings: (idle, notice),
            wait: QuietWait::new(idle, notice),
            current: None,
        }
    }

    #[must_use]
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// One look, at `now`, with the owner's `activity`.
    pub fn poll(&mut self, now: Instant, activity: &Activity) -> Action {
        let pending = record::read_pending(&self.state_dir)
            .filter(|p| p.phase == Phase::Ready && p.bundle == self.bundle);
        let Some(pending) = pending else {
            let noticing = self.wait.noticing();
            self.current = None;
            self.wait = QuietWait::new(self.timings.0, self.timings.1);
            // The upgrader took the bundle back (a newer one is building).
            return if noticing {
                Action::Cancel("The upgrade was replaced by a newer one.".into())
            } else {
                Action::None
            };
        };
        if self.current.as_deref() != Some(pending.id.as_str()) {
            self.current = Some(pending.id.clone());
            self.wait = QuietWait::new(self.timings.0, self.timings.1);
        }
        match self.wait.step(now, activity) {
            Step::Wait => Action::None,
            Step::Notice => Action::Notice(format!(
                "OpenAgents restarts in {} seconds to install an upgrade ({}). Your chats keep running.",
                self.timings.1.as_secs(),
                crate::short(&pending.to)
            )),
            Step::Cancel => Action::Cancel(
                "The upgrade waits until you've stopped typing or the call ends.".into(),
            ),
            Step::Restart => Action::Restart(pending),
        }
    }
}

/// Swap the handed-over bundle in and record that the app is restarting
/// onto `chat`, with the queues of `running` chats to run on afterwards.
/// The caller then relaunches and quits.
///
/// # Errors
/// The swap failed; the app is unchanged. The upgrade is logged as failed.
pub fn begin_restart(
    state_dir: &Path,
    pending: &Pending,
    chat: Option<String>,
    running: &[String],
    now_ms: u64,
) -> Result<Pending, String> {
    if let Err(e) = crate::swap::swap_in(&pending.staged, &pending.bundle, &pending.previous) {
        abandon(state_dir, pending, &e, now_ms);
        return Err(e);
    }
    let _ = record::write_resume_queues(state_dir, running);
    let restarting = Pending {
        phase: Phase::Restarting {
            at_ms: now_ms,
            chat,
        },
        ..pending.clone()
    };
    // Without this record the reopened app can't log the upgrade, but the
    // swap has happened, so the restart goes ahead; the upgrader still sees
    // the new app start.
    let _ = record::write_pending(state_dir, &restarting);
    Ok(restarting)
}

/// Log `pending` as failed for `why` and drop it.
pub fn abandon(state_dir: &Path, pending: &Pending, why: &str, now_ms: u64) {
    let entry = LogEntry::new(
        &pending.id,
        now_ms,
        &pending.requested_by,
        &pending.reason,
        Outcome::Failed,
        format!("{why}; the running app wasn't touched"),
    )
    .commits(pending.from.as_deref(), Some(&pending.to));
    let _ = record::log(state_dir, &entry);
    record::clear_pending(state_dir);
}

/// At start: the upgrade that reopened this app (at `bundle`), now logged
/// as installed, and the chat to open. `None` when this start isn't one.
#[must_use]
pub fn reopened(data_dir: &Path, bundle: &Path, now_ms: u64) -> Option<(LogEntry, Option<String>)> {
    let state_dir = crate::layout::state_dir(data_dir);
    let pending = record::read_pending(&state_dir)?;
    let Phase::Restarting { chat, .. } = &pending.phase else {
        return None;
    };
    if pending.bundle != bundle {
        return None;
    }
    // A rolled-back app is the old commit; the upgrader logs that.
    if Layout::bundle_commit(bundle).as_deref() != Some(pending.to.as_str()) {
        return None;
    }
    let entry = LogEntry::new(
        &pending.id,
        now_ms,
        &pending.requested_by,
        &pending.reason,
        Outcome::Installed,
        "reopened on the new app",
    )
    .commits(pending.from.as_deref(), Some(&pending.to));
    let _ = record::log(&state_dir, &entry);
    record::clear_pending(&state_dir);
    Some((entry, chat.clone()))
}

/// The line the reopened app shows.
#[must_use]
pub fn reopened_line(entry: &LogEntry) -> String {
    match (&entry.from, &entry.to) {
        (Some(from), Some(to)) => format!(
            "Reopened after an upgrade from {} to {}. Your chats kept running.",
            crate::short(from),
            crate::short(to)
        ),
        (None, Some(to)) => format!(
            "Reopened after an upgrade to {}. Your chats kept running.",
            crate::short(to)
        ),
        _ => "Reopened after an upgrade. Your chats kept running.".into(),
    }
}
