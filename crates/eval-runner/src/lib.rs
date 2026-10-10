//! The hosted eval runner: runs extension test sets for chat on our
//! computers, so a first run needs no computer of the trainer's own
//! (`docs/extensions/evaluation.md`, "Where runs execute";
//! `docs/deployment/eval-runner.md`).
//!
//! It is a NIP-CJ execution worker whose one target is the `ext-eval`
//! program (`nostr::eval_ext::hosted`). A phone sends a signed `25920`
//! after a tap on **Start the test**; the runner verifies the signature and
//! binding, admits only catalog tools and chat-made tools whose parts are
//! skills and catalog tools ([`catalog`]), holds the request to the hosted
//! bounds (with no usage limit; [`quota`] keeps an off-by-default
//! emergency brake), records every job in the usage log ([`usage`]),
//! refuses everything else with a typed reason, and runs the suite
//! through `crates/ext-eval` with the fixed hosted grant: read and sandbox
//! write, never `exec` or `network`. It streams progress, seals the report to the requester as a
//! `3188`, and publishes only when the requester sends a publish request
//! naming that report: the suite's release (once) and a `3189` signed by
//! the runner's key, naming the requester and carrying their signed
//! request inline, so the referee can credit them.
//!
//! [`runner::Runner`] is the service; the relay and the blob store sit
//! behind [`wire::Wire`] and [`wire::Blobs`], so tests run it in memory.

pub mod catalog;
pub mod config;
pub mod defaults;
pub mod quota;
pub mod runner;
pub mod store;
pub mod usage;
pub mod wire;

/// A refusal the runner answers with: a typed code and a plain message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    /// `not_admitted`, `over_quota`, `too_large`, or an execution-layer
    /// code such as `malformed`.
    pub code: String,
    /// What to tell the person, in plain words.
    pub message: String,
}

impl Refusal {
    /// A refusal with `code` and `message`.
    #[must_use]
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            code: code.to_string(),
            message: message.into(),
        }
    }

    /// `not_admitted`.
    #[must_use]
    pub fn not_admitted(message: impl Into<String>) -> Self {
        Self::new(nostr::eval_ext::hosted::NOT_ADMITTED, message)
    }

    /// `over_quota`.
    #[must_use]
    pub fn over_quota(message: impl Into<String>) -> Self {
        Self::new(nostr::eval_ext::hosted::OVER_QUOTA, message)
    }

    /// `too_large`.
    #[must_use]
    pub fn too_large(message: impl Into<String>) -> Self {
        Self::new(nostr::eval_ext::hosted::TOO_LARGE, message)
    }

    /// The refusal for a contract error: `too_large` past a bound, the
    /// error's own code otherwise.
    #[must_use]
    pub fn contract(error: &nostr::contracts::ContractError) -> Self {
        match error.code {
            nostr::contracts::RefusalCode::LimitExceeded => Self::too_large(error.to_string()),
            nostr::contracts::RefusalCode::NotAdmitted => Self::not_admitted(error.to_string()),
            _ => Self::new("malformed", error.to_string()),
        }
    }
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

/// Unix seconds now.
#[must_use]
pub fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |span| span.as_secs())
}

/// Writes `bytes` to `path` through a temporary file and a rename, so a
/// crash leaves the old file or the new one, never half of either.
///
/// # Errors
///
/// Returns the I/O error.
pub fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    let parent = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => std::path::Path::new("."),
    };
    std::fs::create_dir_all(parent)?;
    // A uniquely named temporary file (owner-only, as `tempfile` creates
    // them) in the same directory, so concurrent writers of one path never
    // share or truncate each other's half-written file.
    let mut file = tempfile::NamedTempFile::new_in(parent)?;
    file.write_all(bytes)?;
    file.as_file().sync_all()?;
    file.persist(path).map_err(|error| error.error)?;
    Ok(())
}
