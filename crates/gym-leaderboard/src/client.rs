//! Fetching, verifying, and caching the publication, off the frame loop.
//!
//! The publication is static files; a host only serves bytes, and the
//! client trusts digests. A refresh fetches `index.json` from the base
//! URL at [`Config::index_ref`] (`main`), takes the last publication's
//! digest and commit, and fetches `leaderboard.v1.json` at that commit;
//! when the files aren't at that commit (a publication records the commit
//! its evidence was read at, and lands in the next one), it falls back to
//! the index's ref. The leaderboard must hash to the index's digest
//! ([`crate::verify::leaderboard`]), and a bundle to its [`TraceRef`]; a
//! mismatch is refused and the cached copy kept.
//!
//! Files are cached by digest in a directory the caller gives (the app's
//! cache directory): `leaderboards/<digest>.json`, `bundles/<sha256>.json`,
//! and `state.json` with the current digest, its commit, the ref it was
//! read at, and when it was checked. Bundles and older leaderboards are
//! evicted least recently used first above [`Config::cache_bytes`]; the
//! current leaderboard never is.
//!
//! Requests carry no credential, cookie, or identity: no `Authorization`,
//! no cookie store, and no `User-Agent`. Each response is capped while it
//! is read (512 KiB for a leaderboard, 256 KiB for a bundle, 1 MiB for the
//! index), with connect and whole-request timeouts.
//!
//! After the leaderboard verifies, the client reads its signed results
//! publication at `signatures/<digest>.json`, at the index's ref, and
//! checks it against the pinned publishers ([`crate::signed`]): a missing
//! file is "not signed", a bad one is refused with its reason, and either
//! way the digest-verified numbers stay. The event is cached beside the
//! leaderboard and checked again from the cache.
//!
//! [`Fetcher`] does the work, blocking; [`Client`] runs it on its own
//! thread, one request at a time: a new request cancels the one in flight,
//! and the caller polls for [`Event`]s without blocking, like the Gym
//! worker in `crates/verse-gym/src/gym.rs`.

use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::contract::{INDEX_SCHEMA, Index, Leaderboard, TraceBundle, TraceRef};
use crate::signed::{self, MAX_SIGNATURE_BYTES, Publisher, Signature};
use crate::verify::{self, Refusal};
use crate::{INDEX_FILE, LEADERBOARD_FILE, MAX_BUNDLE_BYTES, MAX_LEADERBOARD_BYTES};

/// The default base URL. `{ref}` is replaced by a commit or branch.
pub const DEFAULT_BASE_URL: &str = "https://raw.githubusercontent.com/OpenAgentsInc/openagents/{ref}/bench/terminal-bench/published/";

/// The index's read bound.
pub const MAX_INDEX_BYTES: usize = 1024 * 1024;

/// The cache's default size cap.
pub const DEFAULT_CACHE_BYTES: u64 = 16 * 1024 * 1024;

const STATE_FILE: &str = "state.json";

/// Where and how to fetch.
#[derive(Clone, Debug)]
pub struct Config {
    /// The publication's base URL, ending in `/`. A `{ref}` in it is
    /// replaced by the commit or branch to read at; a mirror without one
    /// serves one ref.
    pub base_url: String,
    /// The ref the index is read at.
    pub index_ref: String,
    /// The cache directory; created when missing.
    pub cache_dir: PathBuf,
    pub cache_bytes: u64,
    pub connect_timeout: Duration,
    /// The whole request, headers and body.
    pub timeout: Duration,
    /// The publishers whose signed results publication is trusted; the
    /// build's pinned ones by default ([`signed::pinned`]).
    pub publishers: Vec<Publisher>,
}

impl Config {
    /// The defaults with the caller's cache directory.
    #[must_use]
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_url: DEFAULT_BASE_URL.into(),
            index_ref: "main".into(),
            cache_dir: cache_dir.into(),
            cache_bytes: DEFAULT_CACHE_BYTES,
            connect_timeout: Duration::from_secs(10),
            timeout: Duration::from_secs(30),
            publishers: signed::pinned(),
        }
    }

    fn url(&self, git_ref: &str, file: &str) -> String {
        format!("{}{file}", self.base_url.replace("{ref}", git_ref))
    }
}

/// Why a request didn't give a verified result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    /// The network or host failed: offline, refused, timed out, or an
    /// HTTP error status.
    Network(String),
    /// The bytes arrived but weren't accepted: "can't verify this
    /// publication".
    Refused(Refusal),
    /// The index lists no publication.
    EmptyIndex,
    /// No network and nothing cached: the results need a connection once.
    NeedsConnection(String),
    /// A newer request or [`Client::cancel`] stopped it.
    Cancelled,
    /// The cache directory couldn't be written or read.
    Cache(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(why) => write!(f, "network: {why}"),
            Self::Refused(why) => write!(f, "can't verify this publication: {why}"),
            Self::EmptyIndex => f.write_str("the index lists no publication"),
            Self::NeedsConnection(why) => {
                write!(f, "the results need a connection once ({why})")
            }
            Self::Cancelled => f.write_str("cancelled"),
            Self::Cache(why) => write!(f, "cache: {why}"),
        }
    }
}

impl std::error::Error for ClientError {}

/// Whether a leaderboard is the index's current one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Freshness {
    /// Just checked against the index.
    Current,
    /// From the cache, before or without a successful check; `problem`
    /// says why when a check failed (a refused or unreachable newer one).
    Cached { problem: Option<ClientError> },
    /// From the cache; the network was unreachable.
    Offline,
}

/// A verified leaderboard and where it stands.
#[derive(Clone, Debug)]
pub struct Loaded {
    pub leaderboard: Arc<Leaderboard>,
    pub digest: String,
    /// The commit its evidence was read at, per the index.
    pub commit: Option<String>,
    pub freshness: Freshness,
    /// Unix seconds when it was last checked against the index.
    pub checked_at: u64,
    /// Who signed it: verified, refused with a reason, not signed, or
    /// unchecked (no signed event cached and the host unreachable).
    pub signature: Signature,
}

impl Loaded {
    /// Seconds since it was last checked against the index.
    #[must_use]
    pub fn age_seconds(&self, now: u64) -> u64 {
        now.saturating_sub(self.checked_at)
    }
}

/// What the cache remembers about the current publication.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
struct State {
    digest: Option<String>,
    commit: Option<String>,
    /// The ref the leaderboard and its bundles were read at.
    read_at: Option<String>,
    checked_at: u64,
}

/// Fetches and caches, blocking. [`Client`] runs one on a thread.
pub struct Fetcher {
    config: Config,
    http: reqwest::blocking::Client,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Fetcher {
    /// A fetcher with no default headers, no cookie store, and the
    /// configured timeouts.
    pub fn new(config: Config) -> Result<Self, ClientError> {
        let http = reqwest::blocking::Client::builder()
            .connect_timeout(config.connect_timeout)
            .timeout(config.timeout)
            .build()
            .map_err(|e| ClientError::Network(e.to_string()))?;
        Ok(Self { config, http })
    }

    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    fn dir(&self, sub: &str) -> Result<PathBuf, ClientError> {
        let dir = self.config.cache_dir.join(sub);
        std::fs::create_dir_all(&dir).map_err(|e| ClientError::Cache(e.to_string()))?;
        Ok(dir)
    }

    fn state(&self) -> State {
        std::fs::read(self.config.cache_dir.join(STATE_FILE))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    fn save_state(&self, state: &State) -> Result<(), ClientError> {
        std::fs::create_dir_all(&self.config.cache_dir)
            .map_err(|e| ClientError::Cache(e.to_string()))?;
        let bytes = serde_json::to_vec(state).map_err(|e| ClientError::Cache(e.to_string()))?;
        write_atomic(&self.config.cache_dir.join(STATE_FILE), &bytes)
    }

    /// The cached current leaderboard, verified again from its bytes.
    #[must_use]
    pub fn cached(&self) -> Option<Loaded> {
        let state = self.state();
        let digest = state.digest.clone()?;
        let path = self
            .config
            .cache_dir
            .join("leaderboards")
            .join(format!("{digest}.json"));
        let bytes = std::fs::read(&path).ok()?;
        let leaderboard = verify::leaderboard(&bytes, Some(&digest)).ok()?;
        touch(&path);
        let signature = self.cached_signature(&leaderboard, &digest, state.commit.as_deref());
        Some(Loaded {
            leaderboard: Arc::new(leaderboard),
            digest,
            commit: state.commit,
            freshness: Freshness::Cached { problem: None },
            checked_at: state.checked_at,
            signature,
        })
    }

    /// Checks the index and returns the current leaderboard: fetched and
    /// verified when its digest is new, else the cached copy. On a network
    /// failure it returns the cache as [`Freshness::Offline`]; on a refused
    /// publication, the cache with the problem. Without a cache, those are
    /// errors.
    pub fn refresh(&self, cancel: &AtomicBool) -> Result<Loaded, ClientError> {
        let cached = self.cached();
        let fallback = |error: ClientError| -> Result<Loaded, ClientError> {
            match (cached.clone(), error) {
                (_, ClientError::Cancelled) => Err(ClientError::Cancelled),
                (Some(mut loaded), ClientError::Network(_)) => {
                    loaded.freshness = Freshness::Offline;
                    Ok(loaded)
                }
                (Some(mut loaded), problem) => {
                    loaded.freshness = Freshness::Cached {
                        problem: Some(problem),
                    };
                    Ok(loaded)
                }
                (None, ClientError::Network(why)) => Err(ClientError::NeedsConnection(why)),
                (None, error) => Err(error),
            }
        };
        let index_bytes = match self.get(
            &self.config.url(&self.config.index_ref, INDEX_FILE),
            MAX_INDEX_BYTES,
            cancel,
        ) {
            Ok(bytes) => bytes,
            Err(e) => return fallback(e),
        };
        let index: Index = match serde_json::from_slice(&index_bytes) {
            Ok(index) => index,
            Err(e) => return fallback(ClientError::Refused(Refusal::Malformed(e.to_string()))),
        };
        if index.schema != INDEX_SCHEMA {
            return fallback(ClientError::Refused(Refusal::Malformed(format!(
                "not {INDEX_SCHEMA}"
            ))));
        }
        let Some(last) = index.publications.last() else {
            return fallback(ClientError::EmptyIndex);
        };
        let now = now_secs();
        if let Some(mut loaded) = cached.clone().filter(|c| c.digest == last.digest) {
            let mut state = self.state();
            state.checked_at = now;
            state.commit.clone_from(&last.commit);
            self.save_state(&state)?;
            loaded.freshness = Freshness::Current;
            loaded.checked_at = now;
            loaded.commit.clone_from(&last.commit);
            loaded.signature = self.signature(
                &loaded.leaderboard,
                &loaded.digest,
                loaded.commit.as_deref(),
                cancel,
            )?;
            return Ok(loaded);
        }

        // At the evidence commit when the files are there, else at the
        // index's own ref; either way, only the digest decides.
        let mut refs: Vec<&str> = Vec::new();
        if self.config.base_url.contains("{ref}")
            && let Some(commit) = last.commit.as_deref()
        {
            refs.push(commit);
        }
        refs.push(&self.config.index_ref);
        let mut last_error = ClientError::EmptyIndex;
        for git_ref in refs {
            let bytes = match self.get(
                &self.config.url(git_ref, LEADERBOARD_FILE),
                MAX_LEADERBOARD_BYTES,
                cancel,
            ) {
                Ok(bytes) => bytes,
                Err(ClientError::Cancelled) => return Err(ClientError::Cancelled),
                Err(e) => {
                    last_error = e;
                    continue;
                }
            };
            match verify::leaderboard(&bytes, Some(&last.digest)) {
                Ok(leaderboard) => {
                    let path = self
                        .dir("leaderboards")?
                        .join(format!("{}.json", last.digest));
                    write_atomic(&path, &bytes)?;
                    self.save_state(&State {
                        digest: Some(last.digest.clone()),
                        commit: last.commit.clone(),
                        read_at: Some(git_ref.to_owned()),
                        checked_at: now,
                    })?;
                    self.evict()?;
                    let signature =
                        self.signature(&leaderboard, &last.digest, last.commit.as_deref(), cancel)?;
                    return Ok(Loaded {
                        leaderboard: Arc::new(leaderboard),
                        digest: last.digest.clone(),
                        commit: last.commit.clone(),
                        freshness: Freshness::Current,
                        checked_at: now,
                        signature,
                    });
                }
                Err(refusal) => last_error = ClientError::Refused(refusal),
            }
        }
        fallback(last_error)
    }

    /// The signed results publication for a verified leaderboard, fetched
    /// at the index's ref and checked. A missing file is
    /// [`Signature::Unsigned`]; an unreachable host falls back to the
    /// cached event, or [`Signature::Unchecked`] without one.
    fn signature(
        &self,
        leaderboard: &Leaderboard,
        digest: &str,
        commit: Option<&str>,
        cancel: &AtomicBool,
    ) -> Result<Signature, ClientError> {
        let file = signed::signature_path(digest);
        let url = self.config.url(&self.config.index_ref, &file);
        let path = self.dir("signatures")?.join(format!("{digest}.json"));
        match self.fetch(&url, MAX_SIGNATURE_BYTES, cancel, true) {
            Ok(Some(bytes)) => {
                write_atomic(&path, &bytes)?;
                Ok(self.check_signature(&bytes, leaderboard, digest, commit))
            }
            Ok(None) => {
                let _ = std::fs::remove_file(&path);
                Ok(Signature::Unsigned)
            }
            Err(ClientError::Cancelled) => Err(ClientError::Cancelled),
            Err(ClientError::Refused(refusal)) => Ok(Signature::Refused {
                reason: refusal.to_string(),
            }),
            Err(_) => Ok(self.cached_signature(leaderboard, digest, commit)),
        }
    }

    fn cached_signature(
        &self,
        leaderboard: &Leaderboard,
        digest: &str,
        commit: Option<&str>,
    ) -> Signature {
        let path = self
            .config
            .cache_dir
            .join("signatures")
            .join(format!("{digest}.json"));
        match std::fs::read(path) {
            Ok(bytes) => self.check_signature(&bytes, leaderboard, digest, commit),
            Err(_) => Signature::Unchecked,
        }
    }

    fn check_signature(
        &self,
        bytes: &[u8],
        leaderboard: &Leaderboard,
        digest: &str,
        commit: Option<&str>,
    ) -> Signature {
        let boards: Vec<String> = leaderboard.boards.iter().map(|b| b.id.clone()).collect();
        signed::check(bytes, digest, commit, &boards, &self.config.publishers)
    }

    /// A trace bundle: from the cache by its SHA-256, or fetched at the
    /// ref the current leaderboard was read at and checked against `trace`.
    pub fn bundle(
        &self,
        trace: &TraceRef,
        cancel: &AtomicBool,
    ) -> Result<Arc<TraceBundle>, ClientError> {
        let path = self.dir("bundles")?.join(format!("{}.json", trace.sha256));
        if let Ok(bytes) = std::fs::read(&path)
            && let Ok(bundle) = verify::bundle(&bytes, trace)
        {
            touch(&path);
            return Ok(Arc::new(bundle));
        }
        let state = self.state();
        let git_ref = state
            .read_at
            .unwrap_or_else(|| self.config.index_ref.clone());
        if trace.path.starts_with('/') || trace.path.split('/').any(|p| p == "..") {
            return Err(ClientError::Refused(Refusal::Malformed(format!(
                "trace path {}",
                trace.path
            ))));
        }
        let bytes = self.get(
            &self.config.url(&git_ref, &trace.path),
            MAX_BUNDLE_BYTES,
            cancel,
        )?;
        let bundle = verify::bundle(&bytes, trace).map_err(ClientError::Refused)?;
        write_atomic(&path, &bytes)?;
        self.evict()?;
        Ok(Arc::new(bundle))
    }

    /// One GET with no identity, capped while reading and cancellable
    /// between chunks.
    fn get(&self, url: &str, cap: usize, cancel: &AtomicBool) -> Result<Vec<u8>, ClientError> {
        self.fetch(url, cap, cancel, false)?
            .ok_or_else(|| ClientError::Network("HTTP 404 Not Found".into()))
    }

    /// As [`Self::get`]; with `missing_ok`, a `404` is `Ok(None)`.
    fn fetch(
        &self,
        url: &str,
        cap: usize,
        cancel: &AtomicBool,
        missing_ok: bool,
    ) -> Result<Option<Vec<u8>>, ClientError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(ClientError::Cancelled);
        }
        let mut response = self
            .http
            .get(url)
            .send()
            .map_err(|e| ClientError::Network(e.without_url().to_string()))?;
        if missing_ok && response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }
        if !response.status().is_success() {
            return Err(ClientError::Network(format!("HTTP {}", response.status())));
        }
        if let Some(length) = response.content_length()
            && length > cap as u64
        {
            return Err(ClientError::Refused(Refusal::TooLarge {
                bytes: usize::try_from(length).unwrap_or(usize::MAX),
                bound: cap,
            }));
        }
        let mut body = Vec::new();
        let mut chunk = [0_u8; 16 * 1024];
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(ClientError::Cancelled);
            }
            let n = response
                .read(&mut chunk)
                .map_err(|e| ClientError::Network(e.to_string()))?;
            if n == 0 {
                return Ok(Some(body));
            }
            if body.len() + n > cap {
                return Err(ClientError::Refused(Refusal::TooLarge {
                    bytes: body.len() + n,
                    bound: cap,
                }));
            }
            body.extend_from_slice(&chunk[..n]);
        }
    }

    /// Removes bundles and older leaderboards, least recently used first,
    /// until the cache fits its cap. The current leaderboard stays.
    fn evict(&self) -> Result<(), ClientError> {
        let current = self.state().digest.map(|d| format!("{d}.json"));
        let mut files = Vec::new();
        let mut total = 0_u64;
        for sub in ["leaderboards", "bundles"] {
            let Ok(entries) = std::fs::read_dir(self.config.cache_dir.join(sub)) else {
                continue;
            };
            for entry in entries.flatten() {
                let Ok(meta) = entry.metadata() else { continue };
                total += meta.len();
                let name = entry.file_name().to_string_lossy().into_owned();
                if sub == "leaderboards" && Some(&name) == current.as_ref() {
                    continue;
                }
                let used = meta.modified().unwrap_or(UNIX_EPOCH);
                files.push((used, name, entry.path(), meta.len()));
            }
        }
        files.sort();
        for (_, _, path, len) in files {
            if total <= self.config.cache_bytes {
                break;
            }
            std::fs::remove_file(&path).map_err(|e| ClientError::Cache(e.to_string()))?;
            total -= len;
        }
        Ok(())
    }
}

/// Marks a cached file used now, for least-recently-used eviction.
fn touch(path: &Path) {
    if let Ok(file) = std::fs::File::options().append(true).open(path) {
        let _ = file.set_modified(SystemTime::now());
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), ClientError> {
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, bytes)
        .and_then(|()| std::fs::rename(&tmp, path))
        .map_err(|e| ClientError::Cache(e.to_string()))
}

/// What the caller asks the worker for.
#[derive(Clone, Debug)]
pub enum Request {
    /// Show the cached leaderboard at once, then check the index.
    Refresh,
    /// Open one attempt's trace.
    Bundle(TraceRef),
}

/// What the worker reports. Each carries the request's sequence number.
#[derive(Clone, Debug)]
pub enum Event {
    Leaderboard {
        seq: u64,
        loaded: Loaded,
    },
    Bundle {
        seq: u64,
        trace: TraceRef,
        bundle: Arc<TraceBundle>,
    },
    Failed {
        seq: u64,
        error: ClientError,
    },
}

/// The fetcher on its own thread: at most one request in flight, a new
/// one cancelling the old, results polled without blocking.
pub struct Client {
    requests: Option<Sender<(u64, Request, Arc<AtomicBool>)>>,
    events: Receiver<Event>,
    in_flight: Arc<std::sync::Mutex<Arc<AtomicBool>>>,
    seq: AtomicU64,
    worker: Option<JoinHandle<()>>,
}

impl Client {
    /// Starts the worker thread.
    pub fn start(config: Config) -> Result<Self, ClientError> {
        let (requests, inbox) = mpsc::channel::<(u64, Request, Arc<AtomicBool>)>();
        let (outbox, events) = mpsc::channel();
        let worker = std::thread::Builder::new()
            .name("gym-leaderboard".into())
            .spawn(move || {
                // Built on the worker: reqwest's blocking client owns a
                // runtime that must not start on the caller's thread.
                let fetcher = match Fetcher::new(config) {
                    Ok(f) => f,
                    Err(error) => {
                        let _ = outbox.send(Event::Failed { seq: 0, error });
                        return;
                    }
                };
                while let Ok((seq, request, cancel)) = inbox.recv() {
                    if cancel.load(Ordering::Relaxed) {
                        continue;
                    }
                    let event = match request {
                        Request::Refresh => {
                            if let Some(loaded) = fetcher.cached() {
                                let _ = outbox.send(Event::Leaderboard { seq, loaded });
                            }
                            match fetcher.refresh(&cancel) {
                                Ok(loaded) => Event::Leaderboard { seq, loaded },
                                Err(error) => Event::Failed { seq, error },
                            }
                        }
                        Request::Bundle(trace) => match fetcher.bundle(&trace, &cancel) {
                            Ok(bundle) => Event::Bundle { seq, trace, bundle },
                            Err(error) => Event::Failed { seq, error },
                        },
                    };
                    if outbox.send(event).is_err() {
                        return;
                    }
                }
            })
            .map_err(|e| ClientError::Network(e.to_string()))?;
        Ok(Self {
            requests: Some(requests),
            events,
            in_flight: Arc::new(std::sync::Mutex::new(Arc::new(AtomicBool::new(false)))),
            seq: AtomicU64::new(0),
            worker: Some(worker),
        })
    }

    /// Queues a request, cancelling the one in flight. Returns its
    /// sequence number; events for older numbers are stale.
    pub fn request(&self, request: Request) -> u64 {
        let seq = self.seq.fetch_add(1, Ordering::Relaxed) + 1;
        let cancel = Arc::new(AtomicBool::new(false));
        if let Ok(mut current) = self.in_flight.lock() {
            current.store(true, Ordering::Relaxed);
            *current = cancel.clone();
        }
        if let Some(requests) = &self.requests {
            let _ = requests.send((seq, request, cancel));
        }
        seq
    }

    /// Cancels the request in flight, as leaving the Gym does.
    pub fn cancel(&self) {
        if let Ok(current) = self.in_flight.lock() {
            current.store(true, Ordering::Relaxed);
        }
    }

    /// The next event, if one is ready; never blocks.
    pub fn poll(&self) -> Option<Event> {
        self.events.try_recv().ok()
    }

    /// Waits up to `timeout` for the next event.
    pub fn wait(&self, timeout: Duration) -> Option<Event> {
        self.events.recv_timeout(timeout).ok()
    }
}

impl Drop for Client {
    /// Cancels the request in flight and lets the worker finish on its
    /// own: a caller on the frame loop never waits for the network.
    fn drop(&mut self) {
        self.cancel();
        self.requests = None;
        drop(self.worker.take());
    }
}
