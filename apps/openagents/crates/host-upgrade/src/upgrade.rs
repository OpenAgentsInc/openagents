//! The upgrader: fetch `main`, build, check, and hand the bundle to the app.
//! The steps that touch the machine are a [`Host`], so the order and the
//! failure handling are tested against a fixture host.

use std::path::{Path, PathBuf};

use crate::record::{LogEntry, Outcome, Pending, Phase};

/// What the upgrade does to this computer.
pub trait Host {
    /// The commit the installed app was built from, when known.
    fn installed_commit(&mut self) -> Option<String>;
    /// The commit the checkout is on now.
    fn checkout_commit(&mut self) -> Option<String>;
    /// Fetch `main`; its commit.
    fn fetch_main(&mut self) -> Result<String, String>;
    /// Put the checkout on `commit`.
    fn checkout(&mut self, commit: &str) -> Result<(), String>;
    /// Build upgrade `id`'s bundle from the checkout; where it is.
    fn build(&mut self, id: &str) -> Result<PathBuf, String>;
    /// Check the built bundle before anything runs it for real.
    fn verify(&mut self, staged: &Path, commit: &str) -> Result<(), String>;
    /// The installed bundle, and where it goes when replaced.
    fn bundle_paths(&mut self) -> (PathBuf, PathBuf);
    /// Whether the app is running.
    fn app_running(&mut self) -> bool;
    /// Hand the bundle to the running app, which swaps it in at a quiet
    /// moment.
    fn hand_over(&mut self, pending: &Pending) -> Result<(), String>;
    /// With no app running: swap the bundle in and open it.
    fn install_now(&mut self, pending: &Pending) -> Result<(), String>;
    /// Wait for the app to report the upgrade's outcome (it logs it when it
    /// reopens), rolling back if the new app never starts. `None` when the
    /// wait ends first.
    fn wait_for_outcome(&mut self, pending: &Pending) -> Option<LogEntry>;
    fn log(&mut self, entry: &LogEntry);
    fn now_ms(&mut self) -> u64;
}

/// One upgrade.
#[derive(Debug, Clone)]
pub struct Request {
    pub id: String,
    /// The agent (chat ID) that asked, or `owner`.
    pub requested_by: String,
    pub reason: String,
    /// Rebuild even when `main` is what's installed.
    pub force: bool,
}

impl Request {
    /// A new upgrade ID from the time.
    #[must_use]
    pub fn new_id(now_ms: u64) -> String {
        format!("u{now_ms}")
    }
}

/// The detail of an upgrade whose bundle is with the app, which installs it
/// at the next quiet moment and logs it then.
pub const WAITING: &str =
    "waiting for a quiet moment in the app; it logs the upgrade when it reopens";

/// Run one upgrade. Every outcome except a hand-over still waiting is
/// logged. A build or check failure puts the checkout back and never
/// touches the running app.
pub fn upgrade(host: &mut dyn Host, request: &Request) -> LogEntry {
    let from = host.installed_commit();
    let restore = host.checkout_commit();
    let to = match host.fetch_main() {
        Ok(to) => to,
        Err(e) => {
            return finish(
                host,
                request,
                Outcome::Failed,
                from.as_deref(),
                None,
                format!("couldn't fetch main: {e}"),
            );
        }
    };
    if !request.force && from.as_deref() == Some(to.as_str()) {
        return finish(
            host,
            request,
            Outcome::UpToDate,
            from.as_deref(),
            Some(&to),
            "main is what's running",
        );
    }
    if let Err(e) = host.checkout(&to) {
        return finish(
            host,
            request,
            Outcome::Failed,
            from.as_deref(),
            Some(&to),
            format!("couldn't check out main: {e}"),
        );
    }
    let staged = match host.build(&request.id) {
        Ok(staged) => staged,
        Err(e) => {
            put_back(host, restore.as_deref());
            return finish(
                host,
                request,
                Outcome::BuildFailed,
                from.as_deref(),
                Some(&to),
                format!("{e}; the running app wasn't touched"),
            );
        }
    };
    if let Err(e) = host.verify(&staged, &to) {
        put_back(host, restore.as_deref());
        return finish(
            host,
            request,
            Outcome::VerifyFailed,
            from.as_deref(),
            Some(&to),
            format!("{e}; the running app wasn't touched"),
        );
    }
    let (bundle, previous) = host.bundle_paths();
    let pending = Pending {
        id: request.id.clone(),
        from: from.clone(),
        to: to.clone(),
        staged,
        bundle,
        previous,
        requested_by: request.requested_by.clone(),
        reason: request.reason.clone(),
        requested_at_ms: host.now_ms(),
        phase: Phase::Ready,
    };
    let handed = if host.app_running() {
        host.hand_over(&pending)
    } else {
        host.install_now(&pending)
    };
    if let Err(e) = handed {
        return finish(
            host,
            request,
            Outcome::Failed,
            from.as_deref(),
            Some(&to),
            format!("couldn't hand the new app over: {e}"),
        );
    }
    host.wait_for_outcome(&pending).unwrap_or_else(|| {
        LogEntry::new(
            &request.id,
            pending.requested_at_ms,
            &request.requested_by,
            &request.reason,
            Outcome::Waiting,
            WAITING,
        )
        .commits(from.as_deref(), Some(&to))
    })
}

fn put_back(host: &mut dyn Host, commit: Option<&str>) {
    if let Some(commit) = commit {
        let _ = host.checkout(commit);
    }
}

fn finish(
    host: &mut dyn Host,
    request: &Request,
    outcome: Outcome,
    from: Option<&str>,
    to: Option<&str>,
    detail: impl Into<String>,
) -> LogEntry {
    let now = host.now_ms();
    let entry = LogEntry::new(
        &request.id,
        now,
        &request.requested_by,
        &request.reason,
        outcome,
        detail,
    )
    .commits(from, to);
    host.log(&entry);
    entry
}
