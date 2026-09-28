//! The Gym's published results: the RESULTS board's panel state and its
//! loader.
//!
//! The publication is static files (`docs/verse/gym-leaderboard.md`):
//! `index.json`, `leaderboard.v1.json`, and one trace bundle per attempt.
//! The loader runs on its own thread while the player is inside the Gym,
//! like the Gym board's worker: it shows a verified cached copy at once,
//! reads the index, fetches the leaderboard when the index names a digest
//! it doesn't have, and fetches a bundle only when the player opens its
//! trace. It trusts only digests: a leaderboard whose recomputed digest
//! isn't the index's, or a bundle whose SHA-256 isn't its `TraceRef`'s, is
//! refused, and the cached copy stays. Leaving the Gym drops the worker.
//! No credential, cookie, or identity is sent.
//!
//! This loader is the Grid's until `gym-leaderboard`'s own client (#9846)
//! lands; the panel state and view model don't depend on which one loads.
//!
//! What the panel shows is `gym_leaderboard::view`: this module holds the
//! player's place ([`Nav`]) and the loaded data, and renders one screen at
//! a time when the host asks by revision.
use gym_leaderboard::contract::{Index, Leaderboard, TraceBundle, TraceRef};
use gym_leaderboard::view::{self, Filter, Freshness, Nav, Page, Source, Tab};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::time::{Duration, SystemTime};

/// Where the publication is read from by default: the public repository's
/// `main`, which the index names the latest publication of.
pub const DEFAULT_BASE_URL: &str = "https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/bench/terminal-bench/published/";

const INDEX_BYTES: usize = 64 * 1024;
const LEADERBOARD_BYTES: usize = gym_leaderboard::MAX_LEADERBOARD_BYTES;
const BUNDLE_BYTES: usize = 256 * 1024;
/// Opened bundles kept on disk, least recently used first out.
const BUNDLE_CACHE_BYTES: u64 = 16 * 1024 * 1024;

/// Where the loader reads from and caches to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// An `https://` base URL ending in `/`, or a local directory (tests
    /// and desktop checks). The files are read relative to it.
    pub base: String,
    /// The app's cache directory for verified copies. Without one, nothing
    /// is kept between visits.
    pub cache_directory: Option<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            base: DEFAULT_BASE_URL.into(),
            cache_directory: None,
        }
    }
}

/// A choice the player made in the panel.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(tag = "do", rename_all = "snake_case")]
pub enum Action {
    Board {
        id: String,
    },
    Filter {
        filter: Filter,
    },
    Caveats {
        open: bool,
    },
    Attempt {
        id: String,
    },
    Back,
    Trace,
    Tab {
        tab: Tab,
    },
    Page {
        page: usize,
    },
    Seek {
        fraction: f64,
    },
    Step {
        forward: bool,
    },
    Play {
        playing: bool,
    },
    Expand {
        index: Option<usize>,
    },
    /// Load again after a failure.
    Retry,
}

/// The panel as the host draws it: status, and one screen's page.
#[derive(Clone, Debug, Serialize)]
pub struct ResultsView {
    pub revision: u64,
    /// The player is inside the Gym, so the loader may run.
    pub active: bool,
    pub loading: bool,
    pub status: String,
    pub error: Option<String>,
    pub page: Option<Page>,
}

enum Update {
    Leaderboard {
        leaderboard: Box<Leaderboard>,
        source: Source,
    },
    /// The cached copy is the index's latest, published at this commit.
    Current(Option<String>),
    /// The index can't be read or verified; the message says why.
    Failed(String),
    /// The index couldn't be reached; the cached copy stays, offline.
    Offline,
    Bundle(String, Result<Box<TraceBundle>, String>),
}

struct Worker {
    commands: Sender<TraceRef>,
    updates: Receiver<Update>,
    cancel: Arc<AtomicBool>,
    /// Bumped by each bundle request, so an older one stops reading.
    generation: Arc<AtomicU64>,
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Release);
    }
}

/// The RESULTS board's panel: what's loaded, where the player is in it,
/// and the loader while the player is inside.
pub struct Results {
    config: Config,
    worker: Option<Worker>,
    leaderboard: Option<Box<Leaderboard>>,
    source: Option<Source>,
    bundle: Option<Box<TraceBundle>>,
    /// The bundle the open trace needs, by SHA-256.
    wanted: Option<TraceRef>,
    nav: Nav,
    loading: bool,
    error: Option<String>,
    active: bool,
    revision: u64,
}

impl Results {
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            worker: None,
            leaderboard: None,
            source: None,
            bundle: None,
            wanted: None,
            nav: Nav::default(),
            loading: false,
            error: None,
            active: false,
            revision: 1,
        }
    }

    /// Starts the loader on entering the Gym and drops it on leaving.
    /// Entering again reads the index again.
    pub fn set_active(&mut self, active: bool) {
        if active == self.active {
            return;
        }
        self.active = active;
        self.nav_pause();
        if active {
            self.start();
        } else {
            self.worker = None;
            self.loading = false;
        }
        self.changed();
    }

    fn start(&mut self) {
        let (commands, requests) = mpsc::channel::<TraceRef>();
        let (output, updates) = mpsc::sync_channel(8);
        let cancel = Arc::new(AtomicBool::new(false));
        let generation = Arc::new(AtomicU64::new(0));
        let config = self.config.clone();
        let (thread_cancel, thread_generation) = (cancel.clone(), generation.clone());
        let started = std::thread::Builder::new()
            .name("verse-gym-results".into())
            .spawn(move || {
                run(
                    &config,
                    &thread_cancel,
                    &thread_generation,
                    &requests,
                    &output,
                )
            });
        if started.is_err() {
            self.error = Some("The results loader couldn't start".into());
            return;
        }
        self.loading = true;
        self.error = None;
        self.worker = Some(Worker {
            commands,
            updates,
            cancel,
            generation,
        });
        // A trace open from an earlier visit still needs its bundle.
        if self.bundle.is_none()
            && let Some(wanted) = self.wanted.clone()
        {
            self.request_bundle(wanted);
        }
    }

    /// Applies what the loader finished. Call once per frame.
    pub fn poll(&mut self) {
        let mut updates = Vec::new();
        if let Some(worker) = &self.worker {
            while let Ok(update) = worker.updates.try_recv() {
                updates.push(update);
            }
        }
        for update in updates {
            self.apply(update);
        }
    }

    fn apply(&mut self, update: Update) {
        match update {
            Update::Leaderboard {
                leaderboard,
                source,
            } => {
                let current = source.freshness == Freshness::Current;
                if self.source.as_ref().map(|s| &s.digest) != Some(&source.digest) {
                    // A different publication: its boards and attempts may
                    // not be the ones on screen.
                    self.nav.reset();
                    self.bundle = None;
                    self.wanted = None;
                }
                self.leaderboard = Some(leaderboard);
                self.source = Some(source);
                if current {
                    self.loading = false;
                    self.error = None;
                }
            }
            Update::Current(commit) => {
                if let Some(source) = &mut self.source {
                    source.freshness = Freshness::Current;
                    source.age_seconds = None;
                    source.commit = commit;
                }
                self.loading = false;
            }
            Update::Offline => {
                if let Some(source) = &mut self.source {
                    source.freshness = Freshness::Offline;
                }
                self.loading = false;
            }
            Update::Failed(message) => {
                self.loading = false;
                self.error = Some(message);
                if let Some(source) = &mut self.source
                    && source.freshness == Freshness::Cached
                {
                    source.freshness = Freshness::Offline;
                }
            }
            Update::Bundle(sha, result) => {
                if self.wanted.as_ref().map(|w| &w.sha256) != Some(&sha) {
                    return;
                }
                match result {
                    Ok(bundle) => {
                        self.bundle = Some(bundle);
                        self.error = None;
                    }
                    Err(message) => self.error = Some(message),
                }
                self.loading = false;
            }
        }
        self.changed();
    }

    /// Advances trace playback. Returns whether the panel changed.
    pub fn tick(&mut self, dt: f64) -> bool {
        let Some(bundle) = &self.bundle else {
            return false;
        };
        let changed = self.nav.tick(bundle, dt);
        if changed {
            self.changed();
        }
        changed
    }

    /// Applies a choice. Choices never start a request except opening a
    /// trace (its bundle) and retrying.
    pub fn act(&mut self, action: Action) -> Result<(), String> {
        let result = self.apply_action(action);
        self.changed();
        result
    }

    fn apply_action(&mut self, action: Action) -> Result<(), String> {
        if let Action::Retry = action {
            if !self.active {
                return Err("Walk into the Gym to load its results".into());
            }
            self.worker = None;
            self.start();
            return Ok(());
        }
        let leaderboard = self
            .leaderboard
            .as_deref()
            .ok_or_else(|| "The results haven't loaded yet".to_owned())?;
        let bundle = self.bundle.as_deref();
        let need_bundle = || bundle.ok_or_else(|| "The trace is loading".to_owned());
        match action {
            Action::Board { id } => self.nav.select_board(leaderboard, &id),
            Action::Filter { filter } => self.nav.set_filter(filter),
            Action::Caveats { open } => self.nav.set_caveats_open(open),
            Action::Attempt { id } => self.nav.select_attempt(leaderboard, &id),
            Action::Back => {
                self.nav.back();
                if self.nav.trace().is_none() {
                    self.wanted = None;
                }
                Ok(())
            }
            Action::Trace => {
                let trace = self.nav.open_trace(leaderboard)?;
                if self.bundle.as_ref().is_none_or(|b| {
                    Some((b.board.as_str(), b.attempt.as_str())) != self.nav.trace()
                }) {
                    self.bundle = None;
                    self.request_bundle(trace);
                }
                Ok(())
            }
            Action::Tab { tab } => self.nav.set_tab(tab),
            Action::Page { page } => self.nav.set_page(need_bundle()?, page),
            Action::Seek { fraction } => self.nav.seek(need_bundle()?, fraction),
            Action::Step { forward } => self.nav.step(need_bundle()?, forward),
            Action::Play { playing } => self.nav.set_playing(need_bundle()?, playing),
            Action::Expand { index } => self.nav.expand(need_bundle()?, index),
            Action::Retry => unreachable!("handled above"),
        }
    }

    fn request_bundle(&mut self, trace: TraceRef) {
        self.wanted = Some(trace.clone());
        if let Some(worker) = &self.worker {
            worker.generation.fetch_add(1, Ordering::AcqRel);
            if worker.commands.send(trace).is_ok() {
                self.loading = true;
            }
        }
    }

    fn nav_pause(&mut self) {
        if let Some(bundle) = &self.bundle
            && self.nav.playing()
        {
            let _ = self.nav.set_playing(bundle, false);
        }
    }

    fn changed(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    #[must_use]
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Whether a trace is playing, so the frame loop keeps ticking it.
    #[must_use]
    pub fn playing(&self) -> bool {
        self.nav.playing()
    }

    /// The board, attempt, and bundle of the open trace, for a replay.
    #[must_use]
    pub fn open_trace(&self) -> Option<&TraceBundle> {
        self.nav.trace()?;
        self.bundle.as_deref()
    }

    /// The current screen, rendered.
    #[must_use]
    pub fn view(&self) -> ResultsView {
        let page = self.leaderboard.as_deref().and_then(|lb| {
            view::render(&self.nav, lb, self.source.as_ref(), self.bundle.as_deref()).ok()
        });
        let status = match (&self.leaderboard, self.loading, &self.source) {
            (None, true, _) => "Loading the published results…".to_owned(),
            (None, false, _) if self.error.is_some() => {
                "The published results aren't available".into()
            }
            (None, false, _) => "Walk into the Gym to load its results".into(),
            (Some(_), _, Some(source)) => source.footer(),
            (Some(_), _, None) => String::new(),
        };
        ResultsView {
            revision: self.revision,
            active: self.active,
            loading: self.loading,
            status,
            error: self.error.clone(),
            page,
        }
    }
}

fn run(
    config: &Config,
    cancel: &AtomicBool,
    generation: &AtomicU64,
    requests: &Receiver<TraceRef>,
    output: &SyncSender<Update>,
) {
    let origin = Origin::new(&config.base);
    let cache = config.cache_directory.as_deref().map(Cache::new);
    let cached = cache.as_ref().and_then(Cache::newest_leaderboard);
    let cached_digest = cached.as_ref().map(|(lb, _)| lb.digest.clone());
    if let Some((leaderboard, age)) = cached {
        let source = Source {
            digest: leaderboard.digest.clone(),
            commit: None,
            freshness: Freshness::Cached,
            age_seconds: Some(age),
        };
        if output
            .send(Update::Leaderboard {
                leaderboard: Box::new(leaderboard),
                source,
            })
            .is_err()
        {
            return;
        }
    }
    let never = AtomicU64::new(0);
    let latest = origin.as_ref().map_err(Clone::clone).and_then(|origin| {
        latest(
            origin,
            cache.as_ref(),
            cached_digest.as_deref(),
            cancel,
            &never,
        )
    });
    if cancel.load(Ordering::Acquire) {
        return;
    }
    let update = match latest {
        Ok(Latest::New(leaderboard, commit)) => Update::Leaderboard {
            source: Source {
                digest: leaderboard.digest.clone(),
                commit,
                freshness: Freshness::Current,
                age_seconds: None,
            },
            leaderboard,
        },
        Ok(Latest::Cached(commit)) => Update::Current(commit),
        Err(Failure::Offline) if cached_digest.is_some() => Update::Offline,
        Err(Failure::Offline) => {
            Update::Failed("The published results need a connection once.".into())
        }
        Err(Failure::Refused(message)) => Update::Failed(message),
    };
    if output.send(update).is_err() {
        return;
    }
    // Bundles, one at a time; a newer request supersedes an older one.
    while let Ok(mut trace) = requests.recv() {
        while let Ok(newer) = requests.try_recv() {
            trace = newer;
        }
        if cancel.load(Ordering::Acquire) {
            return;
        }
        let mine = generation.load(Ordering::Acquire);
        let result = match &origin {
            Ok(origin) => bundle(origin, cache.as_ref(), &trace, cancel, generation, mine),
            Err(Failure::Refused(message)) => Err(message.clone()),
            Err(Failure::Offline) => Err("The trace needs a connection once.".into()),
        };
        if cancel.load(Ordering::Acquire) || generation.load(Ordering::Acquire) != mine {
            continue;
        }
        if output
            .send(Update::Bundle(trace.sha256.clone(), result))
            .is_err()
        {
            return;
        }
    }
}

#[derive(Clone, Debug)]
enum Failure {
    /// The files couldn't be reached.
    Offline,
    /// The files were read but refused: a wrong digest, size, or shape.
    Refused(String),
}

/// The index's latest publication.
enum Latest {
    /// The cached copy is it; the index names this commit.
    Cached(Option<String>),
    /// A newer publication, verified, and its commit.
    New(Box<Leaderboard>, Option<String>),
}

fn latest(
    origin: &Origin,
    cache: Option<&Cache>,
    cached: Option<&str>,
    cancel: &AtomicBool,
    generation: &AtomicU64,
) -> Result<Latest, Failure> {
    let bytes = origin.read(
        gym_leaderboard::INDEX_FILE,
        INDEX_BYTES,
        cancel,
        generation,
        0,
    )?;
    let index: Index = serde_json::from_slice(&bytes)
        .map_err(|_| Failure::Refused("The results index doesn't parse".into()))?;
    let Some(publication) = index.publications.last() else {
        return Err(Failure::Refused(
            "The results index lists no publication".into(),
        ));
    };
    if cached == Some(publication.digest.as_str()) {
        return Ok(Latest::Cached(publication.commit.clone()));
    }
    let bytes = origin.read(
        gym_leaderboard::LEADERBOARD_FILE,
        LEADERBOARD_BYTES,
        cancel,
        generation,
        0,
    )?;
    let leaderboard = verified_leaderboard(&bytes, &publication.digest).ok_or_else(|| {
        Failure::Refused(if cached.is_some() {
            "Can't verify this publication; showing the last verified copy".into()
        } else {
            "Can't verify this publication".into()
        })
    })?;
    if let Some(cache) = cache {
        cache.store(&format!("leaderboard-{}.json", leaderboard.digest), &bytes);
    }
    Ok(Latest::New(
        Box::new(leaderboard),
        publication.commit.clone(),
    ))
}

/// A leaderboard whose recomputed digest is `digest`. The digest is over
/// the file's own `boards`, not a typed round trip, so fields this reader
/// doesn't know (an added optional field) still count.
fn verified_leaderboard(bytes: &[u8], digest: &str) -> Option<Leaderboard> {
    let value: serde_json::Value = serde_json::from_slice(bytes).ok()?;
    if value.get("schema")?.as_str()? != gym_leaderboard::contract::LEADERBOARD_SCHEMA
        || value.get("digest")?.as_str()? != digest
        || atif::digest(value.get("boards")?) != digest
    {
        return None;
    }
    serde_json::from_value(value).ok()
}

fn bundle(
    origin: &Origin,
    cache: Option<&Cache>,
    trace: &TraceRef,
    cancel: &AtomicBool,
    generation: &AtomicU64,
    mine: u64,
) -> Result<Box<TraceBundle>, String> {
    let name = format!("bundle-{}.json", trace.sha256);
    if let Some(bytes) = cache.and_then(|c| c.read(&name, BUNDLE_BYTES))
        && sha256_hex(&bytes) == trace.sha256
        && let Ok(bundle) = serde_json::from_slice(&bytes)
    {
        return Ok(Box::new(bundle));
    }
    if trace.path.contains("..") || trace.path.starts_with('/') {
        return Err("The trace's path isn't in the publication".into());
    }
    let bytes = origin
        .read(&trace.path, BUNDLE_BYTES, cancel, generation, mine)
        .map_err(|failure| match failure {
            Failure::Offline => "The trace needs a connection once.".to_owned(),
            Failure::Refused(message) => message,
        })?;
    if bytes.len() as u64 != trace.bytes || sha256_hex(&bytes) != trace.sha256 {
        return Err("Can't verify this trace".into());
    }
    let bundle: TraceBundle =
        serde_json::from_slice(&bytes).map_err(|_| "The trace doesn't parse".to_owned())?;
    if let Some(cache) = cache {
        cache.store(&name, &bytes);
        cache.prune_bundles(BUNDLE_CACHE_BYTES);
    }
    Ok(Box::new(bundle))
}

fn sha256_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut out = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

enum Origin {
    Http {
        base: String,
        client: reqwest::blocking::Client,
    },
    Directory(PathBuf),
}

impl Origin {
    fn new(base: &str) -> Result<Self, Failure> {
        if let Some(rest) = base.strip_prefix("https://") {
            if rest.is_empty() || !base.ends_with('/') {
                return Err(Failure::Refused("The results address must end in /".into()));
            }
            // No cookies, credentials, or identity: a plain GET.
            let client = reqwest::blocking::Client::builder()
                .https_only(true)
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(20))
                .build()
                .map_err(|_| Failure::Offline)?;
            Ok(Self::Http {
                base: base.to_owned(),
                client,
            })
        } else if base.starts_with('/') {
            Ok(Self::Directory(PathBuf::from(base)))
        } else {
            Err(Failure::Refused(
                "The results address must be https:// or a local directory".into(),
            ))
        }
    }

    /// Reads a file, refusing it past `limit` bytes while reading.
    fn read(
        &self,
        rel: &str,
        limit: usize,
        cancel: &AtomicBool,
        generation: &AtomicU64,
        mine: u64,
    ) -> Result<Vec<u8>, Failure> {
        let too_large = || Failure::Refused("A results file exceeds its size bound".into());
        let reader: Box<dyn Read> = match self {
            Self::Http { base, client } => {
                let response = client
                    .get(format!("{base}{rel}"))
                    .send()
                    .map_err(|_| Failure::Offline)?;
                if !response.status().is_success() {
                    return Err(if response.status().is_server_error() {
                        Failure::Offline
                    } else {
                        Failure::Refused("A results file isn't published".into())
                    });
                }
                if response.content_length().is_some_and(|n| n > limit as u64) {
                    return Err(too_large());
                }
                Box::new(response)
            }
            Self::Directory(dir) => {
                Box::new(std::fs::File::open(dir.join(rel)).map_err(|_| Failure::Offline)?)
            }
        };
        let mut reader = reader.take(limit as u64 + 1);
        let mut bytes = Vec::new();
        let mut block = [0; 32 * 1024];
        loop {
            if cancel.load(Ordering::Acquire) || generation.load(Ordering::Acquire) != mine {
                return Err(Failure::Refused("Canceled".into()));
            }
            let n = reader.read(&mut block).map_err(|_| Failure::Offline)?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&block[..n]);
            if bytes.len() > limit {
                return Err(too_large());
            }
        }
        Ok(bytes)
    }
}

/// Verified copies by digest in the app's cache directory.
struct Cache {
    dir: PathBuf,
}

impl Cache {
    fn new(dir: &Path) -> Self {
        Self {
            dir: dir.join("gym-results"),
        }
    }

    fn read(&self, name: &str, limit: usize) -> Option<Vec<u8>> {
        let path = self.dir.join(name);
        let file = std::fs::File::open(&path).ok()?;
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(limit as u64 + 1).read_to_end(&mut bytes).ok()?;
        if bytes.len() > limit {
            return None;
        }
        // Recently used: the bundle cap drops the oldest first.
        let _ = std::fs::File::options()
            .write(true)
            .open(&path)
            .and_then(|f| f.set_modified(SystemTime::now()));
        Some(bytes)
    }

    fn store(&self, name: &str, bytes: &[u8]) {
        if std::fs::create_dir_all(&self.dir).is_err() {
            return;
        }
        let part = self.dir.join(format!(".{name}.part"));
        if std::fs::write(&part, bytes).is_ok() {
            let _ = std::fs::rename(&part, self.dir.join(name));
        }
    }

    /// The most recently stored leaderboard that still verifies, and its
    /// age in seconds.
    fn newest_leaderboard(&self) -> Option<(Leaderboard, u64)> {
        let mut entries: Vec<(SystemTime, String)> = std::fs::read_dir(&self.dir)
            .ok()?
            .filter_map(Result::ok)
            .filter_map(|e| {
                let name = e.file_name().into_string().ok()?;
                name.starts_with("leaderboard-")
                    .then(|| Some((e.metadata().ok()?.modified().ok()?, name)))?
            })
            .collect();
        entries.sort();
        entries.into_iter().rev().find_map(|(modified, name)| {
            let digest = name.strip_prefix("leaderboard-")?.strip_suffix(".json")?;
            let bytes = self.read(&name, LEADERBOARD_BYTES)?;
            let leaderboard = verified_leaderboard(&bytes, digest)?;
            let age = SystemTime::now()
                .duration_since(modified)
                .map_or(0, |d| d.as_secs());
            Some((leaderboard, age))
        })
    }

    fn prune_bundles(&self, cap: u64) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut bundles: Vec<(SystemTime, u64, PathBuf)> = entries
            .filter_map(Result::ok)
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .is_some_and(|n| n.starts_with("bundle-"))
            })
            .filter_map(|e| {
                let metadata = e.metadata().ok()?;
                Some((metadata.modified().ok()?, metadata.len(), e.path()))
            })
            .collect();
        bundles.sort();
        let mut total: u64 = bundles.iter().map(|b| b.1).sum();
        for (_, size, path) in bundles {
            if total <= cap {
                break;
            }
            if std::fs::remove_file(&path).is_ok() {
                total -= size;
            }
        }
    }
}

#[cfg(test)]
#[path = "gym_results_tests.rs"]
mod tests;
