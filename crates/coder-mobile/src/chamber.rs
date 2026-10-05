//! The phone in the shared chamber: the RITUAL arch's client connects on
//! its own thread, the shared [`chamber_session::Session`] plays it, and
//! the Grid's engine surface draws it, signed by the phone's world identity
//! unless the chamber's configuration names a key file. Suspend stops the
//! worker; resume connects again to the same instance.
//!
//! A visit opened from a configuration file writes `chamber-frames.json`
//! beside it: the frame intervals and actor count the phone saw while
//! joined, the retained measurement for the chamber's frame-time receipts.
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use verse::imported::chamber_session::{self, Session};
use verse::ritual::{self, Opened};
use verse_world::controls::Held;

/// Where the chamber stands between the arch and the return.
pub(crate) enum Stage {
    /// Connecting on a thread; the frame shows the Grid meanwhile.
    Connecting(std::thread::JoinHandle<Result<Opened, String>>),
    /// Playing.
    Joined(Session),
    /// Stopped by a suspend; `resume` connects again.
    Suspended,
    /// The connection or the worker failed; the player returns to the Grid.
    Failed(String),
}

/// Opens a connection to the chamber; it runs on the connect thread.
pub(crate) type Connector = Arc<dyn Fn() -> Result<Opened, String> + Send + Sync>;

pub(crate) struct Play {
    connector: Connector,
    pub stage: Stage,
    /// The content the joined session plays; absent while connecting.
    pub content: Option<Content>,
    pub started: Instant,
    /// Frames drawn through the engine while joined.
    pub frames: u64,
    /// Whether the content changed since the host last opened a surface.
    pub content_revision: u64,
    /// Frame intervals while joined.
    pub timing: Timing,
    /// Where the timing is written, beside the configuration.
    receipt: Option<PathBuf>,
}

/// Frames the phone drew while joined: their intervals and how many actors
/// the chamber showed.
#[derive(Default)]
pub(crate) struct Timing {
    last: Option<Instant>,
    /// Intervals in milliseconds, up to [`Timing::LIMIT`].
    intervals: Vec<f32>,
    pub frames: u64,
    pub actors: usize,
    pub snapshots: u64,
}

impl Timing {
    /// About ten minutes at 60 Hz.
    const LIMIT: usize = 36_000;

    fn frame(&mut self, session: &Session) {
        let now = Instant::now();
        if let Some(last) = self.last
            && self.intervals.len() < Self::LIMIT
        {
            self.intervals
                .push(now.duration_since(last).as_secs_f32() * 1000.0);
        }
        self.last = Some(now);
        self.frames += 1;
        self.snapshots = session.snapshots;
        if let Some(state) = session.view().replica().latest() {
            self.actors = self.actors.max(state.presentation.actors.len());
        }
    }

    /// A gap in play: the next frame starts a new interval.
    fn pause(&mut self) {
        self.last = None;
    }

    /// The receipt: interval percentiles in milliseconds and the counts.
    pub fn summary(&self) -> serde_json::Value {
        let mut sorted = self.intervals.clone();
        sorted.sort_by(f32::total_cmp);
        let at = |q: f32| {
            (!sorted.is_empty()).then(|| sorted[((sorted.len() - 1) as f32 * q).round() as usize])
        };
        let mean = (!sorted.is_empty()).then(|| sorted.iter().sum::<f32>() / sorted.len() as f32);
        serde_json::json!({
            "schema": "openagents.verse.phone-chamber-frames.v1",
            "frames": self.frames,
            "intervals": sorted.len(),
            "interval_ms": {
                "mean": mean, "p50": at(0.5), "p95": at(0.95), "p99": at(0.99),
                "max": sorted.last(),
            },
            "over_33ms": sorted.iter().filter(|ms| **ms > 33.4).count(),
            "actors_max": self.actors,
            "snapshots": self.snapshots,
        })
    }
}

/// What the joined session draws with.
pub(crate) struct Content {
    pub pack: verse_engine::assets::Pack,
    pub atlas: verse::ui::Atlas,
    pub scene: verse_engine::director::Scene,
    pub dir: PathBuf,
}

impl Play {
    /// Starts connecting to the chamber `config` names as `profile`.
    pub fn open(config: PathBuf, secret: secp256k1::SecretKey) -> Self {
        let receipt = config.with_file_name("chamber-frames.json");
        let mut play = Self::open_with(Arc::new(move || ritual::connect_as(&config, secret)));
        play.receipt = Some(receipt);
        play
    }

    /// Starts connecting through `connector`, which each resume calls again.
    pub fn open_with(connector: Connector) -> Self {
        let mut play = Self {
            connector,
            stage: Stage::Suspended,
            content: None,
            started: Instant::now(),
            frames: 0,
            content_revision: 0,
            timing: Timing::default(),
            receipt: None,
        };
        play.connect();
        play
    }

    fn connect(&mut self) {
        let connector = self.connector.clone();
        self.stage = match std::thread::Builder::new()
            .name("chamber-connect".into())
            .spawn(move || connector())
        {
            Ok(handle) => Stage::Connecting(handle),
            Err(error) => Stage::Failed(format!("Cannot start the chamber connection: {error}")),
        };
    }

    /// The session, while joined.
    pub fn session(&self) -> Option<&Session> {
        match &self.stage {
            Stage::Joined(session) => Some(session),
            _ => None,
        }
    }
    pub fn session_mut(&mut self) -> Option<&mut Session> {
        match &mut self.stage {
            Stage::Joined(session) => Some(session),
            _ => None,
        }
    }
    pub fn joined(&self) -> bool {
        matches!(self.stage, Stage::Joined(_))
    }
    pub fn failed(&self) -> Option<&str> {
        match &self.stage {
            Stage::Failed(message) => Some(message),
            _ => None,
        }
    }

    /// Writes the timing beside the configuration, when there is one.
    fn record(&self) {
        if let Some(path) = &self.receipt
            && self.timing.frames > 0
            && let Ok(bytes) = serde_json::to_vec_pretty(&self.timing.summary())
        {
            // The measurement is best effort; play goes on without it.
            let _ = std::fs::write(path, bytes);
        }
    }

    /// The surface goes inactive: stop the worker, keep the content.
    pub fn suspend(&mut self) {
        self.timing.pause();
        self.record();
        match std::mem::replace(&mut self.stage, Stage::Suspended) {
            Stage::Joined(session) => {
                session.stop();
            }
            Stage::Connecting(handle) => {
                // A connection in flight finishes on its thread and drops.
                drop(handle);
            }
            Stage::Failed(message) => self.stage = Stage::Failed(message),
            Stage::Suspended => {}
        }
    }

    /// The surface is active again: connect to the same instance.
    pub fn resume(&mut self) {
        if matches!(self.stage, Stage::Suspended) {
            self.connect();
        }
    }

    /// Advances one frame: finishes a connection that completed, then
    /// steps the session with `held`. Returns whether the session is joined.
    ///
    /// # Errors
    /// The joined session's connection stopped; the stage becomes `Failed`.
    pub fn step(&mut self, held: Held) -> bool {
        if let Stage::Connecting(handle) = &self.stage {
            if !handle.is_finished() {
                return false;
            }
            let Stage::Connecting(handle) = std::mem::replace(&mut self.stage, Stage::Suspended)
            else {
                unreachable!()
            };
            match handle.join() {
                Ok(Ok(opened)) => {
                    match Session::start(opened.client, opened.runtime, &opened.scene) {
                        Ok(session) => {
                            self.content = Some(Content {
                                pack: opened.pack,
                                atlas: opened.atlas,
                                scene: opened.scene,
                                dir: opened.dir,
                            });
                            self.content_revision += 1;
                            self.stage = Stage::Joined(session);
                        }
                        Err(message) => self.stage = Stage::Failed(message),
                    }
                }
                Ok(Err(message)) => self.stage = Stage::Failed(message),
                Err(_) => self.stage = Stage::Failed("The chamber connection panicked".into()),
            }
        }
        let Stage::Joined(session) = &mut self.stage else {
            return false;
        };
        let Some(content) = &self.content else {
            return false;
        };
        if !session.alive() {
            let stopped = match std::mem::replace(&mut self.stage, Stage::Suspended) {
                Stage::Joined(session) => session.stop(),
                _ => unreachable!(),
            };
            self.stage = Stage::Failed(match stopped {
                chamber_session::Stopped::Closed => "The chamber connection closed".into(),
                chamber_session::Stopped::Failed(message) => message,
            });
            return false;
        }
        if let Err(message) = session.step(&content.scene, held) {
            self.stage = Stage::Failed(message);
            return false;
        }
        self.timing.frame(session);
        if self.timing.frames % 600 == 0 {
            self.record();
        }
        true
    }

    /// Assembles the joined session's frame for a viewport of `size`.
    pub fn frame(&self, size: [u32; 2]) -> Result<Option<chamber_session::Frame>, String> {
        let (Stage::Joined(session), Some(content)) = (&self.stage, &self.content) else {
            return Ok(None);
        };
        session
            .frame(&content.pack, &content.atlas, &content.scene, size)
            .map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use verse::imported::chamber_loopback::Loopback;

    fn key() -> secp256k1::SecretKey {
        secp256k1::SecretKey::from_byte_array([7; 32]).unwrap()
    }

    fn settle(play: &mut Play) {
        for _ in 0..200 {
            if !matches!(play.stage, Stage::Connecting(_)) {
                return;
            }
            play.step(Held::default());
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn missing_config_fails_without_blocking_the_frame() {
        let mut play = Play::open(PathBuf::from("/nonexistent/ritual.json"), key());
        assert!(!play.joined());
        settle(&mut play);
        let message = play.failed().expect("the connection fails");
        assert!(!message.is_empty());
        assert!(!play.step(Held::default()));
        assert!(play.frame([320, 240]).unwrap().is_none());
    }

    #[test]
    fn suspend_drops_a_connection_in_flight_and_resume_connects_again() {
        let mut play = Play::open(PathBuf::from("/nonexistent/ritual.json"), key());
        play.suspend();
        assert!(matches!(play.stage, Stage::Suspended));
        assert!(!play.step(Held::default()));
        play.resume();
        assert!(matches!(play.stage, Stage::Connecting(_)));
        settle(&mut play);
        assert!(play.failed().is_some());
        // A failure stays a failure through suspend and resume.
        play.suspend();
        assert!(play.failed().is_some());
        play.resume();
        assert!(play.failed().is_some());
    }

    fn joined(play: &mut Play) -> bool {
        for _ in 0..500 {
            play.step(Held::default());
            if play.session().is_some_and(|s| s.hud().is_some()) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    fn loopback(dead: bool) -> (Arc<Loopback>, tempfile::TempDir, Play) {
        let host = Arc::new(Loopback::start(dead).unwrap());
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();
        let reach = host.clone();
        let play = Play::open_with(Arc::new(move || reach.open(&path)));
        (host, dir, play)
    }

    #[test]
    fn suspend_stops_the_worker_and_resume_rejoins_the_same_character() {
        let (_host, _dir, mut play) = loopback(false);
        assert!(joined(&mut play));
        let life = play.session().unwrap().owned_life();
        assert!(life.is_some());
        assert!(play.frame([320, 240]).unwrap().is_some());
        play.suspend();
        assert!(matches!(play.stage, Stage::Suspended));
        assert!(!play.step(Held::default()));
        assert!(play.frame([320, 240]).unwrap().is_none());
        play.resume();
        assert!(matches!(play.stage, Stage::Connecting(_)));
        assert!(joined(&mut play));
        assert_eq!(play.session().unwrap().owned_life(), life);
        assert_eq!(play.content_revision, 2);
        let summary = play.timing.summary();
        assert!(summary["frames"].as_u64().unwrap() > 1);
        assert!(summary["actors_max"].as_u64().unwrap() >= 14);
        assert!(summary["interval_ms"]["p95"].as_f64().unwrap() > 0.0);
    }

    #[test]
    fn a_host_that_goes_away_fails_the_play() {
        let (host, _dir, mut play) = loopback(false);
        assert!(joined(&mut play));
        host.sever();
        for _ in 0..500 {
            if play.failed().is_some() {
                break;
            }
            play.step(Held::default());
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(play.failed().is_some_and(|m| !m.is_empty()));
        assert!(!play.step(Held::default()));
    }

    #[test]
    fn a_dead_player_respawns_from_the_phone() {
        let (_host, _dir, mut play) = loopback(true);
        assert!(joined(&mut play));
        assert!(play.session().unwrap().dead());
        play.session_mut().unwrap().respawn();
        for _ in 0..500 {
            if play.session().is_some_and(|s| !s.dead()) {
                break;
            }
            play.step(Held::default());
            std::thread::sleep(Duration::from_millis(10));
        }
        let session = play.session().unwrap();
        assert!(!session.dead());
        assert_eq!(session.life_changes, 1);
    }
}
