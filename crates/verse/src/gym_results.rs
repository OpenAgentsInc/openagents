//! The Gym's published results: the RESULTS board's panel state.
//!
//! The publication is static files (`docs/verse/gym-leaderboard.md`).
//! Loading, verification, and caching are `gym_leaderboard::client`: its
//! worker thread shows a verified cached copy at once, checks the index,
//! accepts a leaderboard only when its digest recomputed over the served
//! JSON matches, fetches a bundle only when the player opens its trace and
//! checks it against its `TraceRef`, and sends no credential, cookie, or
//! identity. This module starts that client while the player is inside the
//! Gym and drops it on leaving, holds the player's place ([`Nav`]) and the
//! loaded data, and renders one screen at a time from
//! `gym_leaderboard::view` when the host asks by revision.
use gym_leaderboard::client::{self, Client, ClientError, Event, Request};
use gym_leaderboard::contract::{Leaderboard, TraceBundle, TraceRef};
use gym_leaderboard::view::{self, Filter, Freshness, Nav, Page, Source, Tab};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Where the publication is read from by default: the public repository,
/// with `{ref}` replaced by `main` for the index and by a commit for the
/// files it names.
pub const DEFAULT_BASE_URL: &str = client::DEFAULT_BASE_URL;

/// Where the panel reads from and caches to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Config {
    /// The publication's base URL, ending in `/`, with an optional `{ref}`.
    pub base: String,
    /// The app's cache directory for verified copies. Without one, a
    /// per-process directory under the system's temporary directory is
    /// used, and nothing is promised between launches.
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
    /// The player is inside the Gym, so the client may run.
    pub active: bool,
    pub loading: bool,
    pub status: String,
    pub error: Option<String>,
    /// Where the open trace's replay ghost stands in the Gym, in words.
    pub replay: Option<String>,
    /// The player is past the boards list, so Back applies (also while an
    /// opened trace is still loading and there's no page yet).
    pub can_back: bool,
    pub page: Option<Page>,
}

/// The RESULTS board's panel: what's loaded, where the player is in it,
/// and the client while the player is inside.
pub struct Results {
    config: Config,
    client: Option<Client>,
    /// The sequence numbers of the refresh and bundle requests in flight.
    refresh: Option<u64>,
    fetching: Option<u64>,
    leaderboard: Option<Arc<Leaderboard>>,
    source: Option<Source>,
    bundle: Option<Arc<TraceBundle>>,
    /// The open bundle as a replay in the Gym.
    replay: Option<crate::gym_replay::TraceReplay>,
    /// The bundle the open trace needs.
    wanted: Option<TraceRef>,
    nav: Nav,
    error: Option<String>,
    active: bool,
    revision: u64,
}

impl Results {
    #[must_use]
    pub fn new(config: Config) -> Self {
        Self {
            config,
            client: None,
            refresh: None,
            fetching: None,
            leaderboard: None,
            source: None,
            bundle: None,
            replay: None,
            wanted: None,
            nav: Nav::default(),
            error: None,
            active: false,
            revision: 1,
        }
    }

    /// Starts the client on entering the Gym and drops it on leaving,
    /// which cancels its request. Entering again checks the index again.
    pub fn set_active(&mut self, active: bool) {
        if active == self.active {
            return;
        }
        self.active = active;
        self.pause();
        if active {
            self.start();
        } else {
            self.client = None;
            self.refresh = None;
            self.fetching = None;
        }
        self.changed();
    }

    fn start(&mut self) {
        let cache = self.config.cache_directory.clone().unwrap_or_else(|| {
            std::env::temp_dir().join(format!("verse-gym-results-{}", std::process::id()))
        });
        let mut config = client::Config::new(cache.join("gym-results"));
        config.base_url.clone_from(&self.config.base);
        match Client::start(config) {
            Ok(client) => {
                self.refresh = Some(client.request(Request::Refresh));
                self.error = None;
                self.client = Some(client);
                // A trace open from an earlier visit still needs its bundle.
                if self.bundle.is_none()
                    && let Some(wanted) = self.wanted.clone()
                {
                    self.request_bundle(wanted);
                }
            }
            Err(error) => self.error = Some(message(&error)),
        }
    }

    /// Applies what the client finished. Call once per frame.
    pub fn poll(&mut self) {
        let mut events = Vec::new();
        if let Some(client) = &self.client {
            while let Some(event) = client.poll() {
                events.push(event);
            }
        }
        for event in events {
            self.apply(event);
        }
    }

    fn apply(&mut self, event: Event) {
        match event {
            Event::Leaderboard { seq, loaded } => {
                if Some(seq) != self.refresh {
                    return;
                }
                let (freshness, done, problem) = match &loaded.freshness {
                    client::Freshness::Current => (Freshness::Current, true, None),
                    // The cached copy, shown while the index is checked.
                    client::Freshness::Cached { problem: None } => (Freshness::Cached, false, None),
                    client::Freshness::Cached { problem: Some(p) } => {
                        (Freshness::Cached, true, Some(message(p)))
                    }
                    client::Freshness::Offline => (Freshness::Offline, true, None),
                };
                if self.source.as_ref().map(|s| &s.digest) != Some(&loaded.digest) {
                    // A different publication: its boards and attempts may
                    // not be the ones on screen.
                    self.nav.reset();
                    self.bundle = None;
                    self.wanted = None;
                }
                self.source = Some(Source {
                    digest: loaded.digest.clone(),
                    commit: loaded.commit.clone(),
                    freshness,
                    age_seconds: (freshness != Freshness::Current)
                        .then(|| loaded.age_seconds(now_secs())),
                });
                self.leaderboard = Some(loaded.leaderboard);
                if done {
                    self.refresh = None;
                    self.error = problem;
                }
            }
            Event::Bundle { seq, trace, bundle } => {
                if Some(seq) != self.fetching
                    || self.wanted.as_ref().map(|w| &w.sha256) != Some(&trace.sha256)
                {
                    return;
                }
                self.fetching = None;
                self.replay = Some(crate::gym_replay::TraceReplay::of(&bundle));
                self.bundle = Some(bundle);
                self.error = None;
            }
            Event::Failed { seq, error } => {
                if Some(seq) == self.fetching {
                    self.fetching = None;
                    if error != ClientError::Cancelled {
                        self.error = Some(match error {
                            ClientError::Refused(_) => "Can't verify this trace".into(),
                            ClientError::Network(_) | ClientError::NeedsConnection(_) => {
                                "The trace needs a connection once.".into()
                            }
                            other => message(&other),
                        });
                    }
                } else if Some(seq) == self.refresh || seq == 0 {
                    self.refresh = None;
                    if error != ClientError::Cancelled {
                        self.error = Some(message(&error));
                    }
                } else {
                    return;
                }
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
            self.client = None;
            self.start();
            return Ok(());
        }
        let leaderboard = self
            .leaderboard
            .clone()
            .ok_or_else(|| "The results haven't loaded yet".to_owned())?;
        let bundle = self.bundle.clone();
        let need_bundle = || {
            bundle
                .as_deref()
                .ok_or_else(|| "The trace is loading".to_owned())
        };
        match action {
            Action::Board { id } => self.nav.select_board(&leaderboard, &id),
            Action::Filter { filter } => self.nav.set_filter(filter),
            Action::Caveats { open } => self.nav.set_caveats_open(open),
            Action::Attempt { id } => self.nav.select_attempt(&leaderboard, &id),
            Action::Back => {
                self.nav.back();
                if self.nav.trace().is_none() {
                    self.wanted = None;
                }
                Ok(())
            }
            Action::Trace => {
                let trace = self.nav.open_trace(&leaderboard)?;
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
        if let Some(client) = &self.client {
            // One request in flight: a bundle supersedes an index check
            // still running, which the next entry repeats.
            self.refresh = None;
            self.fetching = Some(client.request(Request::Bundle(trace)));
        }
    }

    fn pause(&mut self) {
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

    /// The bundle of the open trace, for a replay.
    #[must_use]
    pub fn open_trace(&self) -> Option<&TraceBundle> {
        self.nav.trace()?;
        self.bundle.as_deref()
    }

    /// Where the open trace's ghost stands for the viewer's current row.
    #[must_use]
    pub fn replay_place(&self) -> Option<crate::replay::Place> {
        self.open_trace()?;
        self.replay
            .as_ref()?
            .visit(self.nav.current_row())
            .map(|v| v.place)
    }

    fn loading(&self) -> bool {
        self.refresh.is_some() || self.fetching.is_some()
    }

    /// The current screen, rendered.
    #[must_use]
    pub fn view(&self) -> ResultsView {
        let page = self.leaderboard.as_deref().and_then(|lb| {
            view::render(&self.nav, lb, self.source.as_ref(), self.bundle.as_deref()).ok()
        });
        let loading = self.loading();
        let status = match (&self.leaderboard, &self.source) {
            (Some(_), _) if page.is_none() && self.nav.trace().is_some() => {
                if loading {
                    "Loading the trace…".to_owned()
                } else {
                    "The trace isn't available".to_owned()
                }
            }
            (Some(_), Some(source)) => source.footer(),
            (Some(_), None) => String::new(),
            (None, _) if loading => "Loading the published results…".to_owned(),
            (None, _) if self.error.is_some() => "The published results aren't available".into(),
            (None, _) => "Walk into the Gym to load its results".into(),
        };
        ResultsView {
            revision: self.revision,
            active: self.active,
            loading,
            status,
            error: self.error.clone(),
            replay: self
                .replay_place()
                .map(|p| format!("In the Gym, the replay is at the {}", p.name())),
            can_back: self.nav.board().is_some(),
            page,
        }
    }
}

/// A client error in the panel's words.
fn message(error: &ClientError) -> String {
    match error {
        ClientError::Refused(_) => "Can't verify this publication".into(),
        ClientError::NeedsConnection(_) => "The published results need a connection once.".into(),
        ClientError::Network(_) => "The results couldn't be reached".into(),
        ClientError::EmptyIndex => "The results index lists no publication".into(),
        ClientError::Cancelled => "Canceled".into(),
        ClientError::Cache(_) => "The results cache is unavailable".into(),
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
#[path = "gym_results_tests.rs"]
mod tests;
