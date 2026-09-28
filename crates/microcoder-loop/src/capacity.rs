//! Model-provider capacity: which providers a host can reach, and which of
//! them refused work for a usage or rate limit, until when.
//!
//! A provider's usage-limit refusal is durable state, not a per-call error.
//! The Codex backend answers HTTP 429 `usage_limit_reached` with the reset
//! time, and every request before then fails the same way. This module
//! records each refusal in `capacity.json` in the task store directory,
//! one entry per provider, and answers whether a provider has capacity now.
//! The auto-start policy reads it to choose a route before it starts a
//! task, Coder's delegate door reads it to choose a provider before a
//! turn, and the Microcoder loop's failover ([`crate::failover`]) writes it
//! when a generation is refused, so a later task or turn does not repeat a
//! request that cannot succeed. `coder::task::capacity` re-exports this
//! module, so the task store and the loop read one book.
//!
//! The book holds no credential, prompt, or response text: the provider,
//! the kind of limit, when it was observed, and when it resets. A refusal
//! whose reset the provider did not report holds for
//! [`UNKNOWN_RESET_HOLD`] seconds. A missing or unreadable book means no
//! recorded refusal: the next request finds out again.
//!
//! [`probe`] reports whether a provider has a usable local login, without
//! a network request and without reading any secret into a log.

use std::io::{Read, Seek, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// The book in the task store directory.
pub const FILE: &str = "capacity.json";
/// The book's format.
pub const SCHEMA: &str = "openagents.coder.provider-capacity.v1";
/// How long a refusal with no reported reset holds, in seconds.
pub const UNKNOWN_RESET_HOLD: u64 = 30 * 60;
/// The latest reset the book accepts: 31 days after the refusal. A later
/// reported time is held to this bound.
pub const MAX_HOLD: u64 = 31 * 24 * 60 * 60;
/// The result ending a repository run records when no admitted provider
/// had capacity. The task owner keeps it as the run's `ending`.
pub const NO_CAPACITY_ENDING: &str = "no_capacity";

/// A model provider a repository run can generate through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Provider {
    /// The operator's Codex login (ChatGPT backend).
    Codex,
    /// The operator's Claude Code login, through the `claude` binary.
    Claude,
}

impl Provider {
    /// Every provider, in a fixed order.
    pub const ALL: [Provider; 2] = [Provider::Codex, Provider::Claude];

    /// The provider an adapter configuration names, or `None` for one
    /// without durable capacity, such as `synthetic` fixtures. The names
    /// are the configuration's closed set, not free text.
    #[must_use]
    pub fn from_config(name: &str) -> Option<Provider> {
        match name {
            "codex" => Some(Provider::Codex),
            "claude" => Some(Provider::Claude),
            _ => None,
        }
    }

    /// The name an adapter configuration uses.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Provider::Codex => "codex",
            Provider::Claude => "claude",
        }
    }

    /// The generation endpoint a repository grant names for this provider.
    #[must_use]
    pub const fn endpoint(self) -> &'static str {
        match self {
            Provider::Codex => codex_transport::codex::BASE_URL,
            Provider::Claude => "https://api.anthropic.com",
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What kind of limit refused the work.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// The login's plan used its allowance for a window, such as Codex's
    /// `usage_limit_reached`.
    UsageLimit,
    /// The provider refused with HTTP 429 and no typed quota detail.
    RateLimit,
}

/// One provider's refusal, as the book keeps it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Refusal {
    pub provider: Provider,
    pub kind: Kind,
    /// When the refusal was observed, in Unix seconds.
    pub observed_at: u64,
    /// When the provider said the limit resets, in Unix seconds, if it did.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
    /// Until when the provider counts as having no capacity: `resets_at`,
    /// or [`UNKNOWN_RESET_HOLD`] after the refusal when the provider did
    /// not say. Never more than [`MAX_HOLD`] after the refusal.
    pub until: u64,
    /// The plan the provider named, such as `pro`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
    /// The allowance window in minutes, when the provider named one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub window_minutes: Option<u64>,
}

impl Refusal {
    /// A refusal observed at `now`, holding until the reported reset.
    #[must_use]
    pub fn new(provider: Provider, kind: Kind, now: u64, resets_at: Option<u64>) -> Refusal {
        let until = resets_at
            .filter(|at| *at > now)
            .unwrap_or(now + UNKNOWN_RESET_HOLD)
            .min(now + MAX_HOLD);
        Refusal {
            provider,
            kind,
            observed_at: now,
            resets_at,
            until,
            plan: None,
            window_minutes: None,
        }
    }

    /// The refusal a Codex error response carries: a typed usage limit
    /// (`usage_limit_reached`) with its reset, else `None`. Ordinary 429s
    /// are retried by the transport and are not recorded.
    #[must_use]
    pub fn codex(status: u16, body: &str, now: u64) -> Option<Refusal> {
        let limit = codex_transport::codex::UsageLimit::parse(status, body)?;
        let resets_at = limit
            .resets_at
            .or_else(|| limit.resets_in_seconds.map(|s| now.saturating_add(s)));
        let mut refusal = Refusal::new(Provider::Codex, Kind::UsageLimit, now, resets_at);
        refusal.plan = limit.plan_type;
        refusal.window_minutes = limit.window_minutes;
        Some(refusal)
    }

    /// The refusal a Claude Code result carries: an error result whose API
    /// status is 429, or whose stream's last `rate_limit_event` is
    /// `rejected`. The rejected event's `resetsAt` is the reset, and its
    /// window (five-hour or seven-day) makes it a usage limit. Without a
    /// rejected event the reset is unknown and it holds for
    /// [`UNKNOWN_RESET_HOLD`], unless [`Refusal::with_probed_reset`] finds one.
    #[must_use]
    pub fn claude(
        is_error: bool,
        api_error_status: Option<u16>,
        rate_limit: Option<&crate::claude::RateLimit>,
        now: u64,
    ) -> Option<Refusal> {
        let rejected = rate_limit.filter(|limit| limit.rejected());
        if !is_error || (api_error_status != Some(429) && rejected.is_none()) {
            return None;
        }
        let window = rejected.and_then(|limit| limit.window);
        let minutes = window.and_then(crate::claude::LimitWindow::minutes);
        let kind = if minutes.is_some() {
            Kind::UsageLimit
        } else {
            Kind::RateLimit
        };
        let mut refusal = Refusal::new(
            Provider::Claude,
            kind,
            now,
            rejected.and_then(|limit| limit.resets_at),
        );
        refusal.window_minutes = minutes;
        Some(refusal)
    }

    /// This refusal with the reset a usage probe read, when the provider
    /// did not report one and `reading` is fresh at the refusal: the reset
    /// of the window the reading shows at its limit (the latest, when
    /// several are). Otherwise the refusal is unchanged.
    #[must_use]
    pub fn with_probed_reset(mut self, book: &crate::usage::Book) -> Refusal {
        if self.resets_at.is_some() {
            return self;
        }
        let Some(reset) = book
            .reading(self.provider, self.observed_at)
            .and_then(crate::usage::Reading::limiting_reset)
        else {
            return self;
        };
        let probed = Refusal::new(
            self.provider,
            Kind::UsageLimit,
            self.observed_at,
            Some(reset),
        );
        if probed.resets_at.is_some_and(|at| at > self.observed_at) {
            self.kind = probed.kind;
            self.resets_at = probed.resets_at;
            self.until = probed.until;
        }
        self
    }

    /// Whether the refusal still holds at `now`.
    #[must_use]
    pub fn holds(&self, now: u64) -> bool {
        now < self.until
    }
}

/// Every recorded refusal, at most one per provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub schema: String,
    pub refusals: Vec<Refusal>,
}

impl Default for Book {
    fn default() -> Self {
        Book {
            schema: SCHEMA.into(),
            refusals: Vec::new(),
        }
    }
}

impl Book {
    /// The book in `dir`. Missing, unreadable, or malformed is empty.
    #[must_use]
    pub fn load(dir: &Path) -> Book {
        std::fs::read(dir.join(FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Book>(&bytes).ok())
            .filter(|book| book.schema == SCHEMA)
            .unwrap_or_default()
    }

    /// The refusal that keeps `provider` from work at `now`, if one holds.
    #[must_use]
    pub fn blocking(&self, provider: Provider, now: u64) -> Option<&Refusal> {
        self.refusals
            .iter()
            .find(|refusal| refusal.provider == provider && refusal.holds(now))
    }

    /// Whether `provider` has capacity at `now` as far as the book knows.
    #[must_use]
    pub fn has_capacity(&self, provider: Provider, now: u64) -> bool {
        self.blocking(provider, now).is_none()
    }

    /// The earliest time one of `providers` has capacity again, if every
    /// one of them is blocked at `now`.
    #[must_use]
    pub fn earliest_reset(&self, providers: &[Provider], now: u64) -> Option<u64> {
        providers
            .iter()
            .map(|provider| self.blocking(*provider, now).map(|r| r.until))
            .collect::<Option<Vec<u64>>>()
            .and_then(|until| until.into_iter().min())
    }

    fn merge(&mut self, refusal: Refusal, now: u64) {
        self.refusals
            .retain(|kept| kept.provider != refusal.provider && kept.holds(now));
        self.refusals.push(refusal);
        self.refusals.sort_by_key(|refusal| refusal.provider);
    }
}

/// Record `refusal` in the book in `dir`, replacing the provider's earlier
/// entry and dropping entries that no longer hold. The file is `0600`,
/// written under an exclusive lock so concurrent runs do not lose entries.
///
/// # Errors
/// Reports a failed read or write.
pub fn record(dir: &Path, refusal: Refusal) -> Result<Book, String> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let now = refusal.observed_at;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)
        .map_err(|_| format!("cannot create {}", dir.display()))?;
    let path = dir.join(FILE);
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(&path)
        .map_err(|_| format!("cannot open {}", path.display()))?;
    file.lock()
        .map_err(|_| format!("cannot lock {}", path.display()))?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|_| format!("cannot read {}", path.display()))?;
    let mut book = serde_json::from_slice::<Book>(&bytes)
        .ok()
        .filter(|book| book.schema == SCHEMA)
        .unwrap_or_default();
    book.merge(refusal, now);
    let bytes = serde_json::to_vec_pretty(&book).map_err(|e| e.to_string())?;
    file.set_len(0)
        .and_then(|()| file.rewind())
        .and_then(|()| file.write_all(&bytes))
        .and_then(|()| file.sync_all())
        .map_err(|_| format!("cannot write {}", path.display()))?;
    Ok(book)
}

/// Whether a provider can be used from this host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Connection {
    /// A usable local login was found.
    Connected,
    /// No usable login: why, without secret material.
    Missing(String),
}

impl Connection {
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self, Connection::Connected)
    }
}

/// Whether `provider` has a usable login on this host, decided locally:
///
/// - **Codex**: `$CODEX_HOME/auth.json` or `~/.codex/auth.json` holds a
///   ChatGPT login whose access token is not about to expire, as
///   `codex_transport::codex::Login::load` requires before a request.
/// - **Claude**: a `claude` binary (`CLAUDE_BIN`, `PATH`, or
///   `~/.local/bin/claude`) and a Claude Code sign-in: the account record in
///   `~/.claude.json`, or `~/.claude/.credentials.json`. The credential
///   itself, in the macOS keychain or that file, is not read.
///
/// A connected provider can still refuse: the login may have been revoked.
/// That refusal then ends the generation as any other error does.
#[must_use]
pub fn probe(provider: Provider) -> Connection {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    match provider {
        Provider::Codex => {
            let Some(path) = codex_transport::codex::Login::default_path() else {
                return Connection::Missing("HOME is not set".into());
            };
            match codex_transport::codex::Login::load(&path) {
                Ok(_) => Connection::Connected,
                Err(error) => Connection::Missing(error.to_string()),
            }
        }
        Provider::Claude => {
            if claude_binary(home.as_deref()).is_none() {
                return Connection::Missing(
                    "no claude binary in CLAUDE_BIN, PATH, or ~/.local/bin".into(),
                );
            }
            let Some(home) = home else {
                return Connection::Missing("HOME is not set".into());
            };
            if claude_signed_in(&home) {
                Connection::Connected
            } else {
                Connection::Missing("Claude Code is not signed in; run `claude` and log in".into())
            }
        }
    }
}

fn claude_binary(home: Option<&Path>) -> Option<PathBuf> {
    if let Some(named) = std::env::var_os("CLAUDE_BIN").filter(|v| !v.is_empty()) {
        return Some(PathBuf::from(named)).filter(|path| path.is_file());
    }
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|dir| dir.join("claude"))
                .find(|path| path.is_file())
        })
        .or_else(|| {
            home.map(|home| home.join(".local/bin/claude"))
                .filter(|p| p.is_file())
        })
}

/// Claude Code keeps the signed-in account's metadata (not the credential)
/// in `~/.claude.json` as `oauthAccount`.
fn claude_signed_in(home: &Path) -> bool {
    #[derive(Deserialize)]
    struct State {
        #[serde(default, rename = "oauthAccount")]
        oauth_account: Option<serde::de::IgnoredAny>,
    }
    if home.join(".claude/.credentials.json").is_file() {
        return true;
    }
    std::fs::read(home.join(".claude.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<State>(&bytes).ok())
        .is_some_and(|state| state.oauth_account.is_some())
}

/// `YYYY-MM-DD HH:MM UTC` for Unix seconds, for host-built text.
#[must_use]
pub fn utc(seconds: u64) -> String {
    format!(
        "{} {:02}:{:02} UTC",
        nostr::git_sign::utc_date(seconds),
        seconds % 86_400 / 3_600,
        seconds % 3_600 / 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const BODY: &str = r#"{"error":{"type":"usage_limit_reached","message":"The usage limit has been reached","plan_type":"pro","resets_at":1791050823,"eligible_promo":null,"limit_window_minutes":10080,"resets_in_seconds":478613}}"#;

    #[test]
    fn a_codex_usage_limit_records_its_reset_and_blocks_until_then() {
        let dir = tempfile::tempdir().unwrap();
        let now = 1_790_572_210;
        let refusal = Refusal::codex(429, BODY, now).unwrap();
        assert_eq!(refusal.kind, Kind::UsageLimit);
        assert_eq!(refusal.resets_at, Some(1_791_050_823));
        assert_eq!(refusal.until, 1_791_050_823);
        assert_eq!(refusal.plan.as_deref(), Some("pro"));
        record(dir.path(), refusal).unwrap();
        let book = Book::load(dir.path());
        assert!(!book.has_capacity(Provider::Codex, now + 60));
        assert!(book.has_capacity(Provider::Claude, now + 60));
        assert!(book.has_capacity(Provider::Codex, 1_791_050_823));
        assert_eq!(
            book.earliest_reset(&[Provider::Codex], now),
            Some(1_791_050_823)
        );
        assert_eq!(
            book.earliest_reset(&[Provider::Codex, Provider::Claude], now),
            None
        );
        // Private, and it holds nothing from the body but typed fields.
        use std::os::unix::fs::PermissionsExt;
        let meta = std::fs::metadata(dir.path().join(FILE)).unwrap();
        assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert!(!text.contains("message"));
    }

    #[test]
    fn a_reset_in_seconds_or_none_still_bounds_the_hold() {
        let body = r#"{"error":{"type":"usage_limit_reached","resets_in_seconds":600}}"#;
        let refusal = Refusal::codex(429, body, 1_000).unwrap();
        assert_eq!(refusal.until, 1_600);
        let body = r#"{"error":{"type":"usage_limit_reached"}}"#;
        let refusal = Refusal::codex(429, body, 1_000).unwrap();
        assert_eq!(refusal.until, 1_000 + UNKNOWN_RESET_HOLD);
        let body = r#"{"error":{"type":"usage_limit_reached","resets_at":99999999999}}"#;
        let refusal = Refusal::codex(429, body, 1_000).unwrap();
        assert_eq!(refusal.until, 1_000 + MAX_HOLD);
        // An ordinary rate limit is not recorded.
        let body = r#"{"error":{"type":"rate_limit_exceeded"}}"#;
        assert_eq!(Refusal::codex(429, body, 1_000), None);
        // Claude: only an error result with API status 429 or a rejected
        // limit event; without a reported reset it holds 30 minutes.
        let claude = Refusal::claude(true, Some(429), None, 1_000).unwrap();
        assert_eq!(
            (claude.kind, claude.until),
            (Kind::RateLimit, 1_000 + UNKNOWN_RESET_HOLD)
        );
        assert_eq!(Refusal::claude(true, Some(529), None, 1_000), None);
        assert_eq!(Refusal::claude(false, Some(429), None, 1_000), None);
    }

    #[test]
    fn a_claude_refusal_records_the_reset_its_stream_reported() {
        let stdout = include_str!("../fixtures/claude/session-limit.stream.jsonl");
        let report = crate::claude::Report::parse(stdout).unwrap();
        let now = 1_790_162_000;
        let refusal = Refusal::claude(
            report.is_error,
            report.api_error_status,
            report.rate_limit.as_ref(),
            now,
        )
        .unwrap();
        assert_eq!(refusal.kind, Kind::UsageLimit);
        assert_eq!(refusal.resets_at, Some(1_790_164_200));
        assert_eq!(refusal.until, 1_790_164_200);
        assert_eq!(refusal.window_minutes, Some(300));
        // A probe reading doesn't override a reported reset.
        let probed = refusal.clone().with_probed_reset(&probe_book(now));
        assert_eq!(probed, refusal);
        // A rejected event alone is a refusal, even without status 429.
        let rejected = report.rate_limit.as_ref();
        assert!(Refusal::claude(true, None, rejected, now).is_some());
        assert!(Refusal::claude(false, None, rejected, now).is_none());
    }

    /// A usage book holding the recorded Claude probe, observed at `now`,
    /// with its five-hour window at its limit.
    fn probe_book(now: u64) -> crate::usage::Book {
        let body = include_str!("../fixtures/usage/claude-oauth-usage.json");
        let mut reading = crate::usage::parse_claude(body.as_bytes(), now).unwrap();
        reading.windows[0].used_fraction = 1.0;
        reading.windows[0].resets_at = Some(now + 3_600);
        crate::usage::Book {
            entries: vec![crate::usage::Entry {
                provider: Provider::Claude,
                attempted_at: now,
                next_probe_at: now + 60,
                reading: Some(reading),
                failure: None,
            }],
            ..crate::usage::Book::default()
        }
    }

    #[test]
    fn without_a_reported_reset_a_fresh_probe_reading_supplies_it() {
        let now = 1_790_572_210;
        let refusal = Refusal::claude(true, Some(429), None, now + 120).unwrap();
        let probed = refusal.clone().with_probed_reset(&probe_book(now));
        assert_eq!(probed.kind, Kind::UsageLimit);
        assert_eq!(probed.resets_at, Some(now + 3_600));
        assert_eq!(probed.until, now + 3_600);
        // A stale reading, or one with no window at its limit, leaves the
        // 30-minute hold.
        let stale = refusal
            .clone()
            .with_probed_reset(&probe_book(now - crate::usage::STALE_AFTER));
        assert_eq!(stale.until, now + 120 + UNKNOWN_RESET_HOLD);
        let mut calm = probe_book(now);
        calm.entries[0].reading.as_mut().unwrap().windows[0].used_fraction = 0.5;
        let unchanged = refusal.clone().with_probed_reset(&calm);
        assert_eq!(unchanged, refusal);
        assert_eq!(
            refusal
                .with_probed_reset(&crate::usage::Book::default())
                .until,
            now + 120 + UNKNOWN_RESET_HOLD
        );
    }

    #[test]
    fn recording_replaces_a_providers_entry_and_drops_expired_ones() {
        let dir = tempfile::tempdir().unwrap();
        record(
            dir.path(),
            Refusal::new(Provider::Claude, Kind::RateLimit, 100, Some(200)),
        )
        .unwrap();
        record(
            dir.path(),
            Refusal::new(Provider::Codex, Kind::UsageLimit, 150, Some(900)),
        )
        .unwrap();
        record(
            dir.path(),
            Refusal::new(Provider::Codex, Kind::UsageLimit, 160, Some(1_000)),
        )
        .unwrap();
        let book = Book::load(dir.path());
        assert_eq!(book.refusals.len(), 2);
        assert_eq!(book.blocking(Provider::Codex, 170).unwrap().until, 1_000);
        // Claude's entry expired by the next write and is dropped.
        let book = record(
            dir.path(),
            Refusal::new(Provider::Codex, Kind::UsageLimit, 300, Some(1_000)),
        )
        .unwrap();
        assert_eq!(book.refusals.len(), 1);
        // A malformed book reads as empty and is replaced on the next record.
        std::fs::write(dir.path().join(FILE), b"{").unwrap();
        assert!(Book::load(dir.path()).refusals.is_empty());
        record(
            dir.path(),
            Refusal::new(Provider::Claude, Kind::RateLimit, 1, None),
        )
        .unwrap();
        assert_eq!(Book::load(dir.path()).refusals.len(), 1);
    }

    #[test]
    fn utc_text_names_the_date_and_minute() {
        assert_eq!(utc(1_791_050_823), "2026-10-03 18:07 UTC");
    }
}
