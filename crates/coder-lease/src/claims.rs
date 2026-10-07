//! Issue claims held by an agent session (#10764).
//!
//! A claim outlives the command that takes it, so it can't be a holder
//! lock: the kernel frees that when the command exits. Instead each claim
//! is a record, `claims/<owner>/<name>/issue-<n>.json` under the lease
//! root, that names the session and the agent process it runs in. The
//! claim counts as held while all of these are true:
//!
//! - it is younger than the maximum age ([`DEFAULT_CLAIM_AGE`], the
//!   claim window GitHub markers use);
//! - a lease in the table names its session, or its recorded agent process
//!   still runs and started when the record says it did, so a reused
//!   process identifier doesn't keep a claim.
//!
//! Another session's claim on a held issue is refused; the same session
//! claims again and renews it; a claim whose session ended is taken over.
//! Every change happens under an exclusive lock on `claims/.lock`.

use std::fs::File;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::Error;

/// The schema of a claim record, [`IssueClaim`].
pub const CLAIM_SCHEMA: &str = "openagents.lease.issue-claim.v1";

/// How long a claim lasts at most: six hours, the claim window of the
/// GitHub markers (`CLAIM_HOURS` in `scripts/project-sync.sh`).
pub const DEFAULT_CLAIM_AGE: Duration = Duration::from_secs(6 * 3_600);

/// One session's claim on one issue.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueClaim {
    /// [`CLAIM_SCHEMA`].
    pub schema: String,
    /// `owner/name`.
    pub repository: String,
    /// The issue number.
    pub issue: u64,
    /// The agent session that holds the claim.
    pub session: String,
    /// Its agent kind, or `none`.
    pub agent: String,
    /// The agent process the session runs in, when one was found.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// When that process started, as `ps` prints it, so a reused process
    /// identifier doesn't count as the session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid_started: Option<String>,
    /// When the session claimed or last renewed it, in Unix milliseconds.
    pub claimed_at_ms: u64,
}

/// The session that takes a claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Claimant {
    /// The session's identity.
    pub session: String,
    /// The agent kind, or `none`.
    pub agent: String,
    /// The process whose life is the session's.
    pub pid: Option<u32>,
}

impl Claimant {
    /// This process's session ([`crate::Session::detect`]) and the process
    /// it lives in: the one its name carries (`codex:4242`), else the
    /// nearest agent ancestor, else this process's parent.
    #[must_use]
    pub fn detect() -> Claimant {
        let tree = crate::ancestors();
        let session = crate::Session::detect(&|name| std::env::var(name).ok(), &|| tree.clone());
        let pid = crate::scratch::session_pid(&session.id)
            .or_else(|| crate::agent_ancestor(&tree).map(|(pid, _)| pid))
            .or_else(|| tree.first().map(|(pid, _)| *pid));
        Claimant {
            session: session.id,
            agent: session.agent,
            pid,
        }
    }
}

/// What a claim did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Claimed {
    /// Nobody held the issue.
    New,
    /// This session already held it; the claim is renewed.
    Renewed,
    /// Another session's claim ended, or was overridden, and this one
    /// took it over.
    TookOver(IssueClaim),
}

/// Another session holds the claim.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Held {
    /// The holder's record.
    pub claim: IssueClaim,
    /// How old the claim is, in milliseconds.
    pub age_ms: u64,
    /// Why its session counts as live.
    pub live: String,
}

impl Held {
    /// A sentence naming the holder and the claim's age.
    #[must_use]
    pub fn sentence(&self) -> String {
        format!(
            "#{} is claimed by session {} ({}), {} ago; {}",
            self.claim.issue,
            self.claim.session,
            self.claim.agent,
            age(self.age_ms / 1_000),
            self.live
        )
    }
}

/// Why a claim was not taken.
#[derive(Debug)]
pub enum Refused {
    /// Another live session holds it.
    Held(Held),
    /// The caller's own check refused it, such as a fresh GitHub marker
    /// from another session.
    Vetoed(String),
    /// The record could not be read or written.
    Broker(Error),
}

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refused::Held(held) => f.write_str(&held.sentence()),
            Refused::Vetoed(why) => f.write_str(why),
            Refused::Broker(error) => write!(f, "{error}"),
        }
    }
}

impl From<Error> for Refused {
    fn from(error: Error) -> Self {
        Refused::Broker(error)
    }
}

impl From<std::io::Error> for Refused {
    fn from(error: std::io::Error) -> Self {
        Refused::Broker(Error::Io(error))
    }
}

/// The record's file: `claims/<owner>/<name>/issue-<n>.json`.
///
/// # Errors
/// `repository` is not `owner/name` in safe characters.
pub fn path(root: &Path, repository: &str, issue: u64) -> Result<PathBuf, Error> {
    let safe = |part: &str| {
        !part.is_empty()
            && part != "."
            && part != ".."
            && part
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
    };
    match repository.split_once('/') {
        Some((owner, name)) if safe(owner) && safe(name) => Ok(root
            .join("claims")
            .join(owner.to_ascii_lowercase())
            .join(name.to_ascii_lowercase())
            .join(format!("issue-{issue}.json"))),
        _ => Err(Error::Invalid(format!(
            "`{repository}` is not a repository; name it owner/name"
        ))),
    }
}

/// The claim record of `issue`, when there is one.
///
/// # Errors
/// The repository is not `owner/name`, or the record can't be read.
pub fn read(root: &Path, repository: &str, issue: u64) -> Result<Option<IssueClaim>, Error> {
    let path = path(root, repository, issue)?;
    match std::fs::read(&path) {
        Ok(bytes) => {
            let claim: IssueClaim = serde_json::from_slice(&bytes)
                .map_err(|error| Error::Corrupt(format!("{}: {error}", path.display())))?;
            Ok((claim.schema == CLAIM_SCHEMA).then_some(claim))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// Why `claim` still holds its issue at `now_ms`, or `None` when it is
/// free: it is younger than `max_age` and a lease in the table at `root`
/// names its session, or its agent process runs and started when the
/// record says it did.
///
/// # Errors
/// The lease table can't be read.
pub fn live(
    root: &Path,
    claim: &IssueClaim,
    now_ms: u64,
    max_age: Duration,
) -> Result<Option<String>, Error> {
    let age_ms = now_ms.saturating_sub(claim.claimed_at_ms);
    if u128::from(age_ms) >= max_age.as_millis() {
        return Ok(None);
    }
    if crate::scratch::live_sessions(root)?.contains(&claim.session) {
        return Ok(Some(format!("session {} holds a lease", claim.session)));
    }
    let pid = claim
        .pid
        .or_else(|| crate::scratch::session_pid(&claim.session));
    if let Some(pid) = pid
        && crate::process_running(pid)
        && (claim.pid_started.is_none() || started(pid) == claim.pid_started)
    {
        return Ok(Some(format!("its agent process {pid} is still running")));
    }
    Ok(None)
}

/// Claims `issue` for `claimant` at `now_ms`. Refused when another session's
/// claim is live, unless `force`; when the issue is free, `veto` gets the
/// claim being replaced (if any) and may refuse with a sentence. The record
/// is written only when the claim is taken.
///
/// # Errors
/// [`Refused`].
#[allow(clippy::too_many_arguments)]
pub fn claim(
    root: &Path,
    repository: &str,
    issue: u64,
    claimant: &Claimant,
    now_ms: u64,
    max_age: Duration,
    force: bool,
    veto: &dyn Fn(Option<&IssueClaim>) -> Option<String>,
) -> Result<Claimed, Refused> {
    let path = path(root, repository, issue)?;
    let _lock = lock(root)?;
    let previous = read(root, repository, issue)?;
    let outcome = match previous {
        Some(previous) if previous.session == claimant.session => Claimed::Renewed,
        Some(previous) => {
            if !force && let Some(why) = live(root, &previous, now_ms, max_age)? {
                return Err(Refused::Held(Held {
                    age_ms: now_ms.saturating_sub(previous.claimed_at_ms),
                    claim: previous,
                    live: why,
                }));
            }
            Claimed::TookOver(previous)
        }
        None => Claimed::New,
    };
    if !force {
        let replaced = match &outcome {
            Claimed::TookOver(previous) => Some(previous),
            _ => None,
        };
        if let Some(why) = veto(replaced) {
            return Err(Refused::Vetoed(why));
        }
    }
    let record = IssueClaim {
        schema: CLAIM_SCHEMA.to_owned(),
        repository: repository.to_owned(),
        issue,
        session: claimant.session.clone(),
        agent: claimant.agent.clone(),
        pid: claimant.pid,
        pid_started: claimant.pid.and_then(started),
        claimed_at_ms: now_ms,
    };
    if let Some(parent) = path.parent() {
        private_dirs(parent)?;
    }
    let mut bytes = serde_json::to_vec_pretty(&record).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    crate::table::write_atomic(&path, &bytes)?;
    Ok(outcome)
}

/// Releases `issue`'s claim for `session`: removes the record when this
/// session holds it or its holder is no longer live, or always under
/// `force`. Returns the record removed.
///
/// # Errors
/// [`Refused::Held`] when another live session holds it, or the record
/// can't be read or removed.
pub fn release(
    root: &Path,
    repository: &str,
    issue: u64,
    session: &str,
    now_ms: u64,
    max_age: Duration,
    force: bool,
) -> Result<Option<IssueClaim>, Refused> {
    let path = path(root, repository, issue)?;
    let _lock = lock(root)?;
    let Some(previous) = read(root, repository, issue)? else {
        return Ok(None);
    };
    if previous.session != session
        && !force
        && let Some(why) = live(root, &previous, now_ms, max_age)?
    {
        return Err(Refused::Held(Held {
            age_ms: now_ms.saturating_sub(previous.claimed_at_ms),
            claim: previous,
            live: why,
        }));
    }
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(Some(previous)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

/// When process `pid` started, as `ps -o lstart=` prints it, or `None`
/// when `ps` can't tell.
#[must_use]
pub fn started(pid: u32) -> Option<String> {
    #[cfg(unix)]
    {
        let output = std::process::Command::new("ps")
            .args(["-o", "lstart=", "-p", &pid.to_string()])
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()?;
        let text = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (output.status.success() && !text.is_empty()).then_some(text)
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        None
    }
}

fn lock(root: &Path) -> Result<File, Error> {
    crate::root::refuse_real_home(root);
    let dir = root.join("claims");
    private_dirs(&dir)?;
    let file = crate::table::private_file(&dir.join(".lock"), false)?;
    file.lock()?;
    Ok(file)
}

fn private_dirs(dir: &Path) -> std::io::Result<()> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt as _;
        builder.mode(0o700);
    }
    builder.create(dir)
}

fn age(seconds: u64) -> String {
    match seconds {
        0..=119 => format!("{seconds} seconds"),
        120..=7_199 => format!("{} minutes", seconds / 60),
        _ => format!("{} hours", seconds / 3_600),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const REPO: &str = "OpenAgentsInc/openagents";
    const HOUR: u64 = 3_600_000;

    fn me(session: &str, pid: Option<u32>) -> Claimant {
        Claimant {
            session: session.into(),
            agent: "claude-code".into(),
            pid,
        }
    }

    fn none(_: Option<&IssueClaim>) -> Option<String> {
        None
    }

    fn gone() -> u32 {
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        pid
    }

    #[test]
    fn a_live_session_keeps_its_claim_from_another() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let alive = Some(std::process::id());
        let now = 10 * HOUR;
        let age = DEFAULT_CLAIM_AGE;
        let first = claim(root, REPO, 7, &me("a", alive), now, age, false, &none).unwrap();
        assert_eq!(first, Claimed::New);
        let saved = read(root, REPO, 7).unwrap().unwrap();
        assert_eq!(saved.session, "a");
        assert!(saved.pid_started.is_some());
        let Err(Refused::Held(held)) = claim(
            root,
            REPO,
            7,
            &me("b", None),
            now + 60_000,
            age,
            false,
            &none,
        ) else {
            panic!("another session's claim was taken");
        };
        assert!(held.sentence().contains("session a"));
        assert!(held.sentence().contains("60 seconds ago"));
        let again = claim(root, REPO, 7, &me("a", alive), now + 1, age, false, &none).unwrap();
        assert_eq!(again, Claimed::Renewed);
        // The release of another live session is refused, and its own works.
        assert!(release(root, REPO, 7, "b", now, age, false).is_err());
        assert!(
            release(root, REPO, 7, "a", now, age, false)
                .unwrap()
                .is_some()
        );
        assert!(read(root, REPO, 7).unwrap().is_none());
    }

    #[test]
    fn an_ended_or_expired_session_is_taken_over() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let now = 10 * HOUR;
        let age = DEFAULT_CLAIM_AGE;
        claim(
            root,
            REPO,
            8,
            &me("dead", Some(gone())),
            now,
            age,
            false,
            &none,
        )
        .unwrap();
        let took = claim(root, REPO, 8, &me("b", None), now + 1, age, false, &none).unwrap();
        assert!(matches!(took, Claimed::TookOver(previous) if previous.session == "dead"));
        // A live process's claim ends at the maximum age.
        claim(
            root,
            REPO,
            9,
            &me("a", Some(std::process::id())),
            now,
            age,
            false,
            &none,
        )
        .unwrap();
        let later = now + 6 * HOUR;
        assert!(matches!(
            claim(root, REPO, 9, &me("b", None), later, age, false, &none).unwrap(),
            Claimed::TookOver(_)
        ));
    }

    #[test]
    fn a_veto_refuses_and_force_overrides() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let age = DEFAULT_CLAIM_AGE;
        let veto = |_: Option<&IssueClaim>| Some("a fresh marker".to_owned());
        assert!(matches!(
            claim(root, REPO, 3, &me("a", None), HOUR, age, false, &veto),
            Err(Refused::Vetoed(_))
        ));
        assert!(read(root, REPO, 3).unwrap().is_none());
        claim(
            root,
            REPO,
            3,
            &me("a", Some(std::process::id())),
            HOUR,
            age,
            false,
            &none,
        )
        .unwrap();
        let forced = claim(root, REPO, 3, &me("b", None), HOUR, age, true, &veto).unwrap();
        assert!(matches!(forced, Claimed::TookOver(_)));
    }

    #[test]
    fn repositories_are_kept_apart_and_checked() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        assert!(path(root, "../x", 1).is_err());
        assert!(path(root, "owner", 1).is_err());
        let age = DEFAULT_CLAIM_AGE;
        let alive = Some(std::process::id());
        claim(root, "a/one", 1, &me("a", alive), HOUR, age, false, &none).unwrap();
        assert_eq!(
            claim(root, "a/two", 1, &me("b", None), HOUR, age, false, &none).unwrap(),
            Claimed::New
        );
    }
}
