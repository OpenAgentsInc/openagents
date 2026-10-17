//! One environment worker across processes.
//!
//! A Cloud Run candidate starts before it receives traffic. It may read
//! environments, and it must not resume a setup, call a model, cancel a
//! computer, or delete a home. The process whose revision has traffic
//! takes this lease and does that work. A crash, a restart, or a traffic
//! rollback bumps the token, so a process still holding the old token
//! stops.

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// How long a claim stays valid without a renewal.
pub(crate) const LEASE_TTL: Duration = Duration::from_secs(15);

/// What a person sees when this process is not the one doing environment work.
pub(crate) const REFUSAL: &str =
    "This site isn't taking environment work right now. Try again in a minute.";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Claim {
    pub holder: String,
    pub token: u64,
    pub expires_ms: u64,
}

pub(crate) struct Lease {
    _file: File,
    path: PathBuf,
    pub claim: Claim,
}

impl Lease {
    pub(crate) fn token(&self) -> u64 {
        self.claim.token
    }
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum FenceError {
    Busy { holder: String },
    Expired,
    Invalid(String),
}

/// The file-backed lease under `state/environment-studio`.
pub(crate) struct Fence {
    dir: PathBuf,
}

impl Fence {
    pub(crate) fn under(state: &Path) -> Self {
        Self {
            dir: state.join("environment-studio"),
        }
    }

    fn lock_path(&self) -> PathBuf {
        self.dir.join("execution.lock")
    }

    fn claim_path(&self) -> PathBuf {
        self.dir.join("execution.json")
    }

    pub(crate) fn acquire(
        &self,
        holder: &str,
        now_ms: u64,
        ttl: Duration,
    ) -> Result<Lease, FenceError> {
        check_holder(holder)?;
        prepare(&self.dir)?;
        let file = open_lock(&self.lock_path())?;
        if file.try_lock_exclusive().is_err() {
            let holder = self
                .read()
                .map(|claim| claim.holder)
                .unwrap_or_else(|| "someone else".into());
            return Err(FenceError::Busy { holder });
        }
        let token = read_claim(&self.claim_path())
            .map(|claim| claim.token)
            .unwrap_or(0)
            .saturating_add(1);
        let claim = Claim {
            holder: holder.to_owned(),
            token,
            expires_ms: now_ms.saturating_add(ttl_ms(ttl)),
        };
        write_claim(&self.claim_path(), &claim)?;
        Ok(Lease {
            _file: file,
            path: self.claim_path(),
            claim,
        })
    }

    pub(crate) fn renew(lease: &mut Lease, now_ms: u64, ttl: Duration) -> Result<(), FenceError> {
        let current = read_claim(&lease.path).ok_or(FenceError::Expired)?;
        if current.token != lease.claim.token || current.holder != lease.claim.holder {
            return Err(FenceError::Expired);
        }
        lease.claim.expires_ms = now_ms.saturating_add(ttl_ms(ttl));
        write_claim(&lease.path, &lease.claim)
    }

    pub(crate) fn release(lease: Lease) -> Result<(), FenceError> {
        let mut claim = lease.claim.clone();
        claim.expires_ms = 0;
        write_claim(&lease.path, &claim)
    }

    pub(crate) fn read(&self) -> Option<Claim> {
        read_claim(&self.claim_path())
    }

    /// Whether `token` is the current unexpired claim for `holder`.
    pub(crate) fn accepts(&self, holder: &str, token: u64, now_ms: u64) -> bool {
        self.read().is_some_and(|claim| {
            claim.holder == holder && claim.token == token && claim.expires_ms > now_ms
        })
    }
}

/// Whether this process may change environments.
#[derive(Clone)]
pub(crate) enum Admission {
    /// Local development and tests: this process may work.
    Open,
    /// Work only while [`Admission`] says this revision has traffic,
    /// and only as the lease holder.
    Serving { holder: String, check: Check },
}

#[derive(Clone)]
pub(crate) enum Check {
    #[cfg_attr(not(test), allow(dead_code))]
    Flag(Arc<dyn Fn() -> bool + Send + Sync>),
    /// Cloud Run: `K_REVISION` has a non-zero traffic share.
    CloudRun,
}

impl Admission {
    pub(crate) fn open() -> Self {
        Self::Open
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) fn serving(
        holder: impl Into<String>,
        serving: Arc<dyn Fn() -> bool + Send + Sync>,
    ) -> Self {
        Self::Serving {
            holder: holder.into(),
            check: Check::Flag(serving),
        }
    }

    /// On Cloud Run, wait until this revision has traffic. Anywhere else,
    /// work immediately.
    pub(crate) fn from_process_env() -> Self {
        match (std::env::var("K_REVISION"), std::env::var("K_SERVICE")) {
            (Ok(revision), Ok(service)) if !revision.is_empty() && !service.is_empty() => {
                Self::Serving {
                    holder: format!("{revision}:{}", std::process::id()),
                    check: Check::CloudRun,
                }
            }
            _ => Self::Open,
        }
    }

    pub(crate) fn is_open(&self) -> bool {
        matches!(self, Self::Open)
    }

    pub(crate) fn permit(&self) -> Arc<Permit> {
        match self {
            Self::Open => Permit::open(),
            Self::Serving { .. } => Permit::held_back(),
        }
    }
}

/// Published to the setup agent. Zero means do not change shared state.
pub(crate) struct Permit {
    token: AtomicU64,
}

impl Permit {
    pub(crate) fn open() -> Arc<Self> {
        Arc::new(Self {
            token: AtomicU64::new(1),
        })
    }

    pub(crate) fn held_back() -> Arc<Self> {
        Arc::new(Self {
            token: AtomicU64::new(0),
        })
    }

    pub(crate) fn allows(&self) -> bool {
        self.token.load(Ordering::Acquire) != 0
    }

    pub(crate) fn allow(&self, token: u64) {
        self.token.store(token.max(1), Ordering::Release);
    }

    pub(crate) fn revoke(&self) {
        self.token.store(0, Ordering::Release);
    }
}

pub(crate) async fn claim(
    admission: &Admission,
    fence: &Fence,
    lease: &mut Option<Lease>,
    permit: &Permit,
    client: Option<&reqwest::Client>,
    cache: &mut Option<(Instant, bool)>,
) -> bool {
    let Admission::Serving { holder, check } = admission else {
        return true;
    };
    let serving = match check {
        Check::Flag(flag) => flag(),
        Check::CloudRun => cached_traffic(client, cache).await,
    };
    if !serving {
        drop_lease(lease, permit);
        return false;
    }
    if let Some(current) = lease {
        if Fence::renew(current, crate::now_ms(), LEASE_TTL).is_ok()
            && fence.accepts(holder, current.token(), crate::now_ms())
        {
            permit.allow(current.token());
            return true;
        }
        drop_lease(lease, permit);
    }
    match fence.acquire(holder, crate::now_ms(), LEASE_TTL) {
        Ok(next) => {
            permit.allow(next.token());
            *lease = Some(next);
            true
        }
        Err(_) => {
            permit.revoke();
            false
        }
    }
}

fn drop_lease(lease: &mut Option<Lease>, permit: &Permit) {
    if let Some(current) = lease.take() {
        let _ = Fence::release(current);
    }
    permit.revoke();
}

async fn cached_traffic(
    client: Option<&reqwest::Client>,
    cache: &mut Option<(Instant, bool)>,
) -> bool {
    if let Some((at, value)) = *cache
        && at.elapsed() < Duration::from_secs(2)
    {
        return value;
    }
    let value = match client {
        Some(client) => revision_has_traffic(client).await,
        None => false,
    };
    *cache = Some((Instant::now(), value));
    value
}

async fn revision_has_traffic(client: &reqwest::Client) -> bool {
    let revision = std::env::var("K_REVISION").unwrap_or_default();
    let service = std::env::var("K_SERVICE").unwrap_or_default();
    if revision.is_empty() || service.is_empty() {
        return false;
    }
    let token = match metadata(client, "instance/service-accounts/default/token").await {
        Some(body) => serde_json::from_str::<serde_json::Value>(&body)
            .ok()
            .and_then(|value| value["access_token"].as_str().map(str::to_owned)),
        None => None,
    };
    let Some(token) = token else {
        return false;
    };
    let Some(project) = metadata(client, "project/project-id").await else {
        return false;
    };
    let Some(region) = metadata(client, "instance/region").await else {
        return false;
    };
    let Some(region) = region.rsplit('/').next() else {
        return false;
    };
    let url = format!(
        "https://run.googleapis.com/v2/projects/{}/locations/{region}/services/{service}",
        project.trim()
    );
    let Ok(response) = client.get(url).bearer_auth(token).send().await else {
        return false;
    };
    if !response.status().is_success() {
        return false;
    }
    let Ok(body) = response.text().await else {
        return false;
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&body) else {
        return false;
    };
    value["traffic"].as_array().is_some_and(|shares| {
        shares.iter().any(|share| {
            share["revision"].as_str() == Some(revision.as_str())
                && share["percent"].as_u64().unwrap_or(0) > 0
        })
    })
}

async fn metadata(client: &reqwest::Client, path: &str) -> Option<String> {
    let response = client
        .get(format!(
            "http://metadata.google.internal/computeMetadata/v1/{path}"
        ))
        .header("Metadata-Flavor", "Google")
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    response.text().await.ok()
}

fn ttl_ms(ttl: Duration) -> u64 {
    ttl.as_millis().min(u128::from(u64::MAX)) as u64
}

fn check_holder(holder: &str) -> Result<(), FenceError> {
    if holder.is_empty() || holder.len() > 200 || holder.bytes().any(|byte| byte.is_ascii_control())
    {
        return Err(FenceError::Invalid(
            "The environment worker name is invalid.".into(),
        ));
    }
    Ok(())
}

fn prepare(dir: &Path) -> Result<(), FenceError> {
    fs::create_dir_all(dir).map_err(|error| FenceError::Invalid(error.to_string()))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| FenceError::Invalid(error.to_string()))?;
    }
    Ok(())
}

fn open_lock(path: &Path) -> Result<File, FenceError> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)
        .map_err(|error| FenceError::Invalid(error.to_string()))
}

fn read_claim(path: &Path) -> Option<Claim> {
    let mut file = File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    if bytes.is_empty() || bytes.len() > 4096 {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn write_claim(path: &Path, claim: &Claim) -> Result<(), FenceError> {
    let bytes =
        serde_json::to_vec(claim).map_err(|error| FenceError::Invalid(error.to_string()))?;
    let temporary = path.with_extension("json.writing");
    {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&temporary)
            .map_err(|error| FenceError::Invalid(error.to_string()))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|error| FenceError::Invalid(error.to_string()))?;
    }
    fs::rename(&temporary, path).map_err(|error| FenceError::Invalid(error.to_string()))
}

/// Holds or watches the lease. The studio tests dispatch here so the
/// two-process cases share one test-binary entry.
#[cfg(test)]
pub(crate) fn lease_child(dir: &Path, role: &str) {
    match role {
        "lease-hold" => {
            let fence = Fence::under(dir);
            let lease = fence
                .acquire("holder-a", crate::now_ms(), Duration::from_secs(30))
                .expect("the first holder takes the lease");
            fs::write(dir.join("held-token"), lease.token().to_string()).unwrap();
            std::thread::sleep(Duration::from_secs(30));
            let _ = Fence::release(lease);
        }
        "lease-watch" => {
            let holder = std::env::var("OA_ENV_FENCE_HOLDER").unwrap();
            let fence = Fence::under(dir);
            let log_path = dir.join(format!("watch-{holder}.log"));
            let mut lease = None;
            let mut logged_busy = false;
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs(12) {
                let serving = fs::read_to_string(dir.join("admission"))
                    .unwrap_or_default()
                    .lines()
                    .any(|line| line == holder);
                if serving {
                    if lease.is_none() {
                        match fence.acquire(&holder, crate::now_ms(), LEASE_TTL) {
                            Ok(next) => {
                                logged_busy = false;
                                let _ = fs::OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(&log_path)
                                    .and_then(|mut file| {
                                        use std::io::Write;
                                        writeln!(file, "acquired {}", next.token())
                                    });
                                lease = Some(next);
                            }
                            Err(FenceError::Busy { .. }) if !logged_busy => {
                                logged_busy = true;
                                let _ = fs::OpenOptions::new()
                                    .create(true)
                                    .append(true)
                                    .open(&log_path)
                                    .and_then(|mut file| {
                                        use std::io::Write;
                                        writeln!(file, "busy")
                                    });
                            }
                            Err(_) => {}
                        }
                    } else if let Some(current) = lease.as_mut() {
                        logged_busy = false;
                        if Fence::renew(current, crate::now_ms(), LEASE_TTL).is_err() {
                            lease = None;
                        }
                    }
                } else if let Some(current) = lease.take() {
                    let _ = Fence::release(current);
                    let _ = fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&log_path)
                        .and_then(|mut file| {
                            use std::io::Write;
                            writeln!(file, "released")
                        });
                }
                std::thread::sleep(Duration::from_millis(40));
            }
        }
        _ => {}
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::process::{Command, Stdio};
    use std::time::Duration;

    #[test]
    fn a_new_holder_replaces_an_expired_token() {
        let dir = tempfile::tempdir().unwrap();
        let fence = Fence::under(dir.path());
        let now = 1_000;
        let first = fence.acquire("a", now, Duration::from_secs(30)).unwrap();
        let old = first.token();
        drop(first);
        let second = fence.acquire("b", now, Duration::from_secs(30)).unwrap();
        assert_ne!(second.token(), old);
        assert!(!fence.accepts("a", old, now));
        assert!(fence.accepts("b", second.token(), now));
        Fence::release(second).unwrap();
        assert!(!fence.accepts("b", 2, now + 1));
    }

    pub(crate) fn spawn(dir: &Path, role: &str, extra: &[(&str, &str)]) -> std::process::Child {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args([
                "--exact",
                "studio::tests::environment_fence_child",
                "--nocapture",
            ])
            .env("OA_ENV_FENCE_DIR", dir)
            .env("OA_ENV_FENCE_ROLE", role)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (key, value) in extra {
            command.env(key, value);
        }
        command.spawn().expect("the fence child starts")
    }

    pub(crate) fn wait_for(path: &Path, needle: &str) -> String {
        wait_count(path, needle, 1)
    }

    pub(crate) fn wait_count(path: &Path, prefix: &str, count: usize) -> String {
        let start = Instant::now();
        loop {
            let text = fs::read_to_string(path).unwrap_or_default();
            let seen = text.lines().filter(|line| line.starts_with(prefix)).count();
            if seen >= count {
                return text;
            }
            if start.elapsed() > Duration::from_secs(8) {
                panic!(
                    "timed out waiting for {count} {prefix} lines in {}:\n{text}",
                    path.display()
                );
            }
            std::thread::sleep(Duration::from_millis(40));
        }
    }

    #[test]
    fn a_killed_holder_cannot_keep_the_lease() {
        let dir = tempfile::tempdir().unwrap();
        let mut child = spawn(dir.path(), "lease-hold", &[]);
        let token = wait_for(&dir.path().join("held-token"), "1")
            .trim()
            .parse::<u64>()
            .unwrap();
        let fence = Fence::under(dir.path());
        assert!(matches!(
            fence.acquire("other", crate::now_ms(), LEASE_TTL),
            Err(FenceError::Busy { .. })
        ));
        child.kill().unwrap();
        child.wait().unwrap();
        let next = fence
            .acquire("other", crate::now_ms(), LEASE_TTL)
            .expect("the lock is free after the holder exits");
        assert_ne!(next.token(), token);
        assert!(!fence.accepts("holder-a", token, crate::now_ms()));
    }

    #[test]
    fn traffic_moves_and_rolls_back_to_one_holder() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("admission"), "a\n").unwrap();
        let mut first = spawn(dir.path(), "lease-watch", &[("OA_ENV_FENCE_HOLDER", "a")]);
        let mut second = spawn(dir.path(), "lease-watch", &[("OA_ENV_FENCE_HOLDER", "b")]);
        let a = dir.path().join("watch-a.log");
        let b = dir.path().join("watch-b.log");
        let first_take = wait_count(&a, "acquired ", 1);
        assert!(
            !fs::read_to_string(&b)
                .unwrap_or_default()
                .contains("acquired "),
            "the candidate took the lease while another revision had traffic"
        );
        let fence = Fence::under(dir.path());
        assert_eq!(fence.read().unwrap().holder, "a");
        fs::write(dir.path().join("admission"), "b\n").unwrap();
        wait_for(&a, "released");
        let second_take = wait_count(&b, "acquired ", 1);
        assert_eq!(fence.read().unwrap().holder, "b");
        fs::write(dir.path().join("admission"), "a\n").unwrap();
        wait_for(&b, "released");
        let rollback = wait_count(&a, "acquired ", 2);
        assert_eq!(fence.read().unwrap().holder, "a");
        let token = |log: &str| -> u64 {
            log.lines()
                .filter_map(|line| line.strip_prefix("acquired "))
                .last()
                .unwrap()
                .trim()
                .parse()
                .unwrap()
        };
        let t1 = token(&first_take);
        let t2 = token(&second_take);
        let t3 = token(&rollback);
        assert!(t2 > t1 && t3 > t2, "{t1} {t2} {t3}");
        assert!(!fence.accepts("a", t1, crate::now_ms()));
        assert!(!fence.accepts("b", t2, crate::now_ms()));
        fs::write(dir.path().join("admission"), "").unwrap();
        first.kill().ok();
        second.kill().ok();
        first.wait().ok();
        second.wait().ok();
    }
}
