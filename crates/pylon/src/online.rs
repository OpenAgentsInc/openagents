//! Pylon 1.0's front door: `openagents pylon up`, `down`, `status` (this
//! pylon), and `id`, and the supervisor that keeps a pylon serving.
//!
//! `up` reads this computer (`doctor`), fetches the pinned decision model
//! when it is missing, and records the wanted state in `share.json`
//! ([`crate::share`]) with a decision lane and, when memory allows, a text
//! lane. The resident host service (`openagents host serve`) runs
//! [`supervise`] over that file; without it, or with `--foreground`, `up`
//! runs the same loop in its own process.
//!
//! The supervisor owns the Psionic engine children: it starts one
//! `psionic-openai-server` per lane on loopback, waits for them to answer,
//! self-checks the decision lane when this model has no passing
//! self-check, and then runs the provider. A crashed child is restarted
//! with backoff (1 s doubling to 60 s), and the pylon's beacon says
//! `draining` while one is down. What it is doing is written to
//! `live.json` beside the settings every few seconds, which `status` and
//! `down` read.
//!
//! `down` records `on: false`; the supervisor then lets admitted jobs
//! finish (at most [`crate::provider::DRAIN`]), publishes an offline
//! beacon, and stops the children. With no supervisor running, `down`
//! publishes the offline beacon itself.

use std::io::{BufRead, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use nostr::pylon::{Check, Status, owned_beacon_event};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{oneshot, watch};

use crate::decide::{Clef, Decider};
use crate::engine::{Engine, Pending, Psionic};
use crate::identity::{Identity, load_owner, npub};
use crate::lease::{Dedicated, Machine};
use crate::provider::{Counters, Provider};
use crate::share::{self, DecideLane, Settings, TextLane};
use crate::{home, now};

/// The schema of `live.json`.
pub const LIVE_SCHEMA: &str = "openagents.pylon.live.v1";
/// The file the supervisor writes what it is doing to.
pub const LIVE_FILE: &str = "live.json";
/// The text lane's loopback port.
pub const TEXT_PORT: u16 = 18097;
/// The text lane's default model, as the beacon names it.
pub const TEXT_MODEL: &str = "qwen3.5-0.8b-q8_0";
/// The text lane's default model file in `~/.openagents/models`.
pub const TEXT_MODEL_FILE: &str = "Qwen3.5-0.8B-Q8_0.gguf";
/// A `live.json` older than this has no supervisor behind it.
pub const STALE_SECS: u64 = 20;
/// Seconds in a day: "today" is the UTC day.
const DAY: u64 = 86_400;

/// A lane the supervisor runs an engine for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LaneKind {
    Decisions,
    Text,
}

/// One engine child: which lane, where it listens, what it loads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneSpec {
    pub kind: LaneKind,
    pub port: u16,
    pub model_file: PathBuf,
    /// The decision device (`metal`, `cuda`, `cpu`); decisions only.
    pub device: Option<String>,
    /// The served model's name; text only.
    pub model: Option<String>,
}

impl LaneSpec {
    /// The engine's loopback URL.
    #[must_use]
    pub fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    /// The lane's name in plain words.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self.kind {
            LaneKind::Decisions => "decisions",
            LaneKind::Text => "text",
        }
    }
}

/// The engines `settings` asks the supervisor to run.
#[must_use]
pub fn lanes(settings: &Settings) -> Vec<LaneSpec> {
    let mut out = Vec::new();
    if let Some(lane) = &settings.decide {
        out.push(LaneSpec {
            kind: LaneKind::Decisions,
            port: lane.port,
            model_file: lane.model_file.clone(),
            device: Some(lane.device.clone()),
            model: None,
        });
    }
    if let Some(lane) = &settings.text {
        out.push(LaneSpec {
            kind: LaneKind::Text,
            port: lane.port,
            model_file: lane.model_file.clone(),
            device: None,
            model: Some(lane.model.clone()),
        });
    }
    out
}

/// The `psionic-openai-server` command line for `lane`: loopback only, no
/// mesh.
#[must_use]
pub fn psionic_command(server: &Path, lane: &LaneSpec) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(server);
    command.arg("-m").arg(&lane.model_file).args([
        "--host",
        "127.0.0.1",
        "--port",
        &lane.port.to_string(),
    ]);
    if lane.kind == LaneKind::Decisions {
        command.args(["--decision-device", lane.device.as_deref().unwrap_or("cpu")]);
    }
    command.args(["--mesh-coordination", "disabled"]);
    command
}

/// Environment variables an engine child keeps; everything else (API keys,
/// tokens) is dropped.
fn keeps(name: &str) -> bool {
    matches!(
        name,
        "PATH" | "HOME" | "USER" | "LANG" | "LC_ALL" | "TMPDIR" | "TZ" | "RUST_LOG"
    ) || ["LD_LIBRARY_PATH", "DYLD_", "CUDA_", "NVIDIA_", "PSIONIC_"]
        .iter()
        .any(|prefix| name.starts_with(prefix))
}

/// What the supervisor runs and asks: the real Psionic engines in
/// production, fake child commands and fixed engines in tests.
pub trait Engines: Send + Sync {
    /// The command that runs `lane`'s engine, its output going to a log in
    /// `home`.
    ///
    /// # Errors
    ///
    /// When the engine is not installed or the log cannot be opened.
    fn command(&self, home: &Path, lane: &LaneSpec) -> Result<tokio::process::Command, String>;
    /// The decision server at `url`.
    ///
    /// # Errors
    ///
    /// When its client cannot be built.
    fn decider(&self, url: &str) -> Result<Arc<dyn Decider>, String>;
    /// The text engine at `url` serving `model`.
    ///
    /// # Errors
    ///
    /// When its client cannot be built.
    fn text(&self, url: &str, model: &str) -> Result<Arc<dyn Engine>, String>;
    /// Whether `lane`'s engine answers yet (its warm-up is done).
    fn ready<'a>(&'a self, lane: &'a LaneSpec) -> Pending<'a, bool>;
    /// Self-check the decision lane unless this model already passed one;
    /// `Err` says why it must not serve.
    fn selfcheck<'a>(
        &'a self,
        home: &'a Path,
        lane: &'a LaneSpec,
    ) -> Pending<'a, Result<(), String>>;
}

/// The production engines: `psionic-openai-server` children.
pub struct PsionicEngines;

impl Engines for PsionicEngines {
    fn command(&self, home: &Path, lane: &LaneSpec) -> Result<tokio::process::Command, String> {
        let server = crate::setup::find_server().ok_or_else(|| {
            format!(
                "{} is not installed; build it with: {}",
                crate::setup::SERVER,
                crate::setup::BUILD_SERVER
            )
        })?;
        std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
        let log = home.join(format!("psionic-{}.log", lane.name()));
        let out = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .map_err(|e| format!("{}: {e}", log.display()))?;
        let err = out.try_clone().map_err(|e| e.to_string())?;
        let mut command = psionic_command(&server, lane);
        command.env_clear();
        command.envs(std::env::vars_os().filter(|(k, _)| k.to_str().is_some_and(keeps)));
        command.stdout(out).stderr(err);
        Ok(command)
    }

    fn decider(&self, url: &str) -> Result<Arc<dyn Decider>, String> {
        Ok(Arc::new(Clef::new(url, None)?))
    }

    fn text(&self, url: &str, model: &str) -> Result<Arc<dyn Engine>, String> {
        Ok(Arc::new(Psionic::new(url, model)?))
    }

    fn ready<'a>(&'a self, lane: &'a LaneSpec) -> Pending<'a, bool> {
        Box::pin(async move {
            match lane.kind {
                LaneKind::Decisions => crate::setup::serving(&lane.url()).await.is_some(),
                LaneKind::Text => {
                    match Psionic::new(&lane.url(), lane.model.as_deref().unwrap_or(TEXT_MODEL)) {
                        Ok(engine) => engine.healthy().await,
                        Err(_) => false,
                    }
                }
            }
        })
    }

    fn selfcheck<'a>(
        &'a self,
        home: &'a Path,
        lane: &'a LaneSpec,
    ) -> Pending<'a, Result<(), String>> {
        Box::pin(async move {
            let url = lane.url();
            let model_id = crate::setup::serving(&url)
                .await
                .and_then(|s| s["model_id"].as_str().map(str::to_string))
                .unwrap_or_default();
            if crate::calibrate::shipped_reference(&model_id).is_none() {
                return Ok(());
            }
            if let Ok(Some(_)) = crate::calibrate::may_serve(home, &model_id) {
                return Ok(());
            }
            let file = lane
                .model_file
                .is_file()
                .then_some(lane.model_file.as_path());
            let check = crate::cli::run_selfcheck(&url, 64, true, file).await?;
            if check.pass {
                Ok(())
            } else {
                Err(format!(
                    "self-check failed: {:.1}% of rounds pass, worst item {:.3}",
                    check.round_pass * 100.0,
                    check.max
                ))
            }
        })
    }
}

/// How a supervisor runs.
#[derive(Clone)]
pub struct Options {
    pub engines: Arc<dyn Engines>,
    /// How often it looks at the settings and writes `live.json`.
    pub every: Duration,
    /// The first restart wait; it doubles up to `max_backoff`.
    pub backoff: Duration,
    pub max_backoff: Duration,
    /// `host` (the resident host service) or `foreground` (`up` in a
    /// terminal).
    pub kind: &'static str,
    /// Return once the pylon was turned off (`up --foreground` ends on
    /// `down`).
    pub exit_when_off: bool,
}

impl Options {
    /// The host service's supervisor.
    #[must_use]
    pub fn host(every: Duration) -> Self {
        Self {
            engines: Arc::new(PsionicEngines),
            every,
            backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(60),
            kind: "host",
            exit_when_off: false,
        }
    }

    /// `up --foreground`'s supervisor.
    #[must_use]
    pub fn foreground() -> Self {
        Self {
            kind: "foreground",
            exit_when_off: true,
            ..Self::host(Duration::from_secs(5))
        }
    }
}

/// What a pylon is doing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    Serving,
    Paused,
    Starting,
    #[default]
    Off,
}

impl Phase {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Serving => "serving",
            Self::Paused => "paused",
            Self::Starting => "starting",
            Self::Off => "off",
        }
    }
}

/// One engine child as the supervisor last saw it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaneLive {
    pub lane: LaneKind,
    pub port: u16,
    /// The child's process id while it runs.
    pub pid: Option<u32>,
    pub up: bool,
    pub restarts: u32,
    /// How the child last ended, or why it could not start.
    pub last_exit: Option<String>,
}

/// `live.json`: what the supervisor is doing, rewritten every few seconds.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Live {
    pub schema: String,
    /// When it was written, Unix seconds.
    pub at: u64,
    /// `host` or `foreground`.
    pub supervisor: String,
    pub pid: u32,
    pub state: Phase,
    /// Why it is paused, starting, or off, in plain words.
    pub why: Option<String>,
    pub pylon: Option<String>,
    pub npub: Option<String>,
    /// The served decision model, as the beacon names it.
    pub model: Option<String>,
    pub lanes: Vec<LaneLive>,
    /// The UTC day the counts are for (days since 1970).
    pub day: u64,
    pub jobs_today: u64,
    pub refused_today: u64,
    pub failed_today: u64,
    /// When it last started serving.
    pub serving_since: Option<u64>,
}

impl Live {
    /// Whether a supervisor wrote this recently.
    #[must_use]
    pub fn fresh(&self, at: u64) -> bool {
        at.saturating_sub(self.at) <= STALE_SECS
    }
}

/// `live.json` in `home`, if there is one.
#[must_use]
pub fn load_live(home: &Path) -> Option<Live> {
    serde_json::from_slice(&std::fs::read(home.join(LIVE_FILE)).ok()?).ok()
}

fn save_live(home: &Path, live: &Live) -> Result<(), String> {
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let tmp = home.join(format!("{LIVE_FILE}.tmp"));
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(live).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, home.join(LIVE_FILE)).map_err(|e| e.to_string())
}

/// A log line sink.
pub type Log = Arc<dyn Fn(String) + Send + Sync>;

type Shared = Arc<Mutex<Live>>;

fn set(live: &Shared, change: impl FnOnce(&mut Live)) {
    if let Ok(mut held) = live.lock() {
        change(&mut held);
    }
}

fn read<T>(live: &Shared, look: impl FnOnce(&Live) -> T) -> Option<T> {
    live.lock().ok().map(|held| look(&held))
}

fn write(home: &Path, live: &Shared, log: &Log) {
    let snapshot = {
        let Ok(mut held) = live.lock() else { return };
        let at = now();
        held.at = at;
        if held.day != at / DAY {
            held.day = at / DAY;
            held.jobs_today = 0;
            held.refused_today = 0;
            held.failed_today = 0;
        }
        held.clone()
    };
    if let Err(e) = save_live(home, &snapshot) {
        log(format!("pylon state: {e}"));
    }
}

struct Running {
    settings: Settings,
    halt: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<()>,
}

/// Keep the pylon `home`'s settings ask for running until `stop` resolves:
/// its engine children (restarted with backoff when one crashes), a
/// self-check, and the provider, on `machine` (the lease table, unless
/// the settings say the box is dedicated). Looks at the settings every
/// `options.every`; a change restarts the pylon under the new settings,
/// and `on: false` stops it. Logs one line per change through `log`.
pub async fn supervise(
    home: PathBuf,
    machine: Arc<dyn Machine>,
    options: Options,
    log: Log,
    stop: impl std::future::Future<Output = ()>,
) {
    let today = now() / DAY;
    let mut first = Live {
        schema: LIVE_SCHEMA.into(),
        supervisor: options.kind.into(),
        pid: std::process::id(),
        day: today,
        ..Live::default()
    };
    if let Some(held) = load_live(&home)
        && held.day == today
    {
        first.jobs_today = held.jobs_today;
        first.refused_today = held.refused_today;
        first.failed_today = held.failed_today;
    }
    let live: Shared = Arc::new(Mutex::new(first));
    let options = Arc::new(options);
    let mut running: Option<Running> = None;
    let mut served = false;
    tokio::pin!(stop);
    loop {
        let settings = match share::load(&home) {
            Ok(settings) => settings,
            Err(e) => {
                log(format!("pylon settings: {e}"));
                None
            }
        };
        let wanted = settings.clone().filter(|s| s.on);
        let same = match (&running, &wanted) {
            (Some(held), Some(want)) => &held.settings == want && !held.task.is_finished(),
            (None, None) => true,
            _ => false,
        };
        if !same {
            if let Some(held) = running.take() {
                let _ = held.halt.send(());
                let _ = held.task.await;
                log("pylon off".into());
            }
            if let Some(settings) = wanted {
                let names: Vec<&str> = lanes(&settings).iter().map(LaneSpec::name).collect();
                log(format!(
                    "pylon on: {} on {}{}",
                    settings.pylon,
                    settings.relay,
                    if names.is_empty() {
                        format!(", text from {}", settings.engine)
                    } else {
                        format!(", engines: {}", names.join(", "))
                    }
                ));
                let (halt, halted) = oneshot::channel();
                let task = tokio::spawn(unit(
                    home.clone(),
                    settings.clone(),
                    Arc::clone(&machine),
                    Arc::clone(&options),
                    Arc::clone(&live),
                    Arc::clone(&log),
                    halted,
                ));
                running = Some(Running {
                    settings,
                    halt,
                    task,
                });
                served = true;
            }
        }
        if running.is_none() {
            set(&live, |l| {
                l.state = Phase::Off;
                l.why = Some(match &settings {
                    None => "not set up".into(),
                    Some(_) => "turned off".into(),
                });
                l.lanes.clear();
                l.serving_since = None;
            });
        }
        write(&home, &live, &log);
        if options.exit_when_off && served && running.is_none() {
            break;
        }
        tokio::select! {
            () = &mut stop => break,
            () = tokio::time::sleep(options.every) => {}
        }
    }
    if let Some(held) = running.take() {
        let _ = held.halt.send(());
        let _ = held.task.await;
    }
    set(&live, |l| {
        l.state = Phase::Off;
        l.why = Some("stopped".into());
        l.lanes.clear();
        l.serving_since = None;
    });
    write(&home, &live, &log);
}

/// One pylon under one set of settings, until `halt` resolves.
async fn unit(
    home: PathBuf,
    settings: Settings,
    machine: Arc<dyn Machine>,
    options: Arc<Options>,
    live: Shared,
    log: Log,
    mut halt: oneshot::Receiver<()>,
) {
    let specs = lanes(&settings);
    set(&live, |l| {
        l.state = Phase::Starting;
        l.why = Some("starting the engines".into());
        l.pylon = Some(settings.pylon.clone());
        l.serving_since = None;
        l.lanes = specs
            .iter()
            .map(|lane| LaneLive {
                lane: lane.kind,
                port: lane.port,
                pid: None,
                up: false,
                restarts: 0,
                last_exit: None,
            })
            .collect();
    });
    let (kill, killed) = watch::channel(false);
    let children: Vec<_> = specs
        .iter()
        .enumerate()
        .map(|(index, lane)| {
            tokio::spawn(child(
                index,
                lane.clone(),
                home.clone(),
                Arc::clone(&options),
                Arc::clone(&live),
                Arc::clone(&log),
                killed.clone(),
            ))
        })
        .collect();
    let stop_children = || async {
        let _ = kill.send(true);
        for task in children {
            let _ = task.await;
        }
    };
    let wait = options.every.min(Duration::from_secs(1));

    // Wait for every engine to answer: Psionic warms up before it does.
    let started = Instant::now();
    loop {
        let mut ready = true;
        for lane in &specs {
            if !options.engines.ready(lane).await {
                ready = false;
                break;
            }
        }
        if ready {
            break;
        }
        if started.elapsed() > Duration::from_secs(900) {
            set(&live, |l| {
                l.why = Some(format!(
                    "the engine has not started after 15 minutes; see {}",
                    home.join("psionic-decisions.log").display()
                ));
            });
        } else {
            set(&live, |l| l.why = Some("the engine is warming up".into()));
        }
        tokio::select! {
            _ = &mut halt => {
                stop_children().await;
                return;
            }
            () = tokio::time::sleep(wait) => {}
        }
    }

    // Refuse to serve decisions this computer answers differently.
    if let Some(lane) = specs.iter().find(|l| l.kind == LaneKind::Decisions) {
        set(&live, |l| {
            l.why = Some("checking this computer's answers".into())
        });
        if let Err(why) = options.engines.selfcheck(&home, lane).await {
            log(format!("pylon not serving: {why}"));
            set(&live, |l| {
                l.state = Phase::Paused;
                l.why = Some(why);
            });
            let _ = (&mut halt).await;
            stop_children().await;
            return;
        }
    }

    let machine: Arc<dyn Machine> = if settings.dedicated {
        Arc::new(Dedicated)
    } else {
        machine
    };
    let (provider, decider) = match build(
        &home,
        &settings,
        Arc::clone(&machine),
        options.engines.as_ref(),
    ) {
        Ok(built) => built,
        Err(why) => {
            log(format!("pylon could not start: {why}"));
            set(&live, |l| {
                l.state = Phase::Paused;
                l.why = Some(format!("could not start: {why}"));
            });
            let _ = (&mut halt).await;
            stop_children().await;
            return;
        }
    };
    set(&live, |l| {
        l.npub = Some(npub(provider.pubkey()));
        l.serving_since = Some(now());
    });
    let (stop_provider, stopped) = oneshot::channel::<()>();
    let serving = tokio::spawn(Arc::clone(&provider).run(async {
        let _ = stopped.await;
    }));
    let mut seen = Counters::default();
    let mut reachable = true;
    let mut probed: Option<Instant> = None;
    loop {
        let down = read(&live, |l| l.lanes.iter().any(|lane| !lane.up)).unwrap_or(false);
        provider.hold(down).await;
        let busy = if settings.dedicated {
            false
        } else {
            let machine = Arc::clone(&machine);
            tokio::task::spawn_blocking(move || machine.owner_busy())
                .await
                .unwrap_or(false)
        };
        if probed.is_none_or(|at| at.elapsed() >= Duration::from_secs(60)) {
            reachable = crate::setup::relay_reachable(&settings.relay).await;
            probed = Some(Instant::now());
        }
        let counters = provider.counters().await;
        let model = decider.as_ref().map(|d| d.identity().advertised());
        set(&live, |l| {
            add(l, seen, counters);
            l.model = model;
            let (state, why) = if down {
                (
                    Phase::Paused,
                    Some("an engine stopped; restarting it".into()),
                )
            } else if !reachable {
                (
                    Phase::Paused,
                    Some("no network: the relay cannot be reached".into()),
                )
            } else if busy {
                (
                    Phase::Paused,
                    Some("owner busy: this computer's own work comes first".into()),
                )
            } else {
                (Phase::Serving, None)
            };
            l.state = state;
            l.why = why;
        });
        seen = counters;
        tokio::select! {
            _ = &mut halt => break,
            () = tokio::time::sleep(options.every) => {}
        }
    }
    set(&live, |l| {
        l.state = Phase::Paused;
        l.why = Some("finishing jobs before it stops".into());
    });
    let _ = stop_provider.send(());
    let _ = serving.await;
    let counters = provider.counters().await;
    set(&live, |l| add(l, seen, counters));
    stop_children().await;
}

fn add(live: &mut Live, seen: Counters, now: Counters) {
    live.jobs_today += now.served.saturating_sub(seen.served);
    live.refused_today += now.refused.saturating_sub(seen.refused);
    live.failed_today += now.failed.saturating_sub(seen.failed);
}

/// The provider for `settings`: decisions (and text from the text lane)
/// when there is a decision lane, else text from the text lane or
/// `engine`.
type Built = (Arc<Provider>, Option<Arc<dyn Decider>>);

fn build(
    home: &Path,
    settings: &Settings,
    machine: Arc<dyn Machine>,
    engines: &dyn Engines,
) -> Result<Built, String> {
    let mut config = settings.config(home);
    config.owner = load_owner(home)?;
    let identity = Identity::load_or_create(&home.join("provider.key"))?;
    let machine = if settings.dedicated {
        Arc::new(Dedicated)
    } else {
        machine
    };
    let text = settings
        .text
        .as_ref()
        .map(|lane| engines.text(&format!("http://127.0.0.1:{}", lane.port), &lane.model))
        .transpose()?;
    if let Some(lane) = &settings.decide {
        let decider = engines.decider(&format!("http://127.0.0.1:{}", lane.port))?;
        let provider = Provider::deciding(config, identity, text, Arc::clone(&decider), machine)?;
        Ok((provider, Some(decider)))
    } else {
        let engine = match text {
            Some(engine) => engine,
            None => engines.text(&settings.engine, &settings.model)?,
        };
        Ok((Provider::on(config, identity, engine, machine)?, None))
    }
}

/// One engine child: start it, wait for it to end, and start it again
/// after a wait that doubles from `options.backoff` to
/// `options.max_backoff` (back to the first after a minute up), until
/// `kill` turns true.
async fn child(
    index: usize,
    lane: LaneSpec,
    home: PathBuf,
    options: Arc<Options>,
    live: Shared,
    log: Log,
    mut kill: watch::Receiver<bool>,
) {
    let mut wait = options.backoff;
    let record = |change: &dyn Fn(&mut LaneLive)| {
        set(&live, |l| {
            if let Some(held) = l.lanes.get_mut(index) {
                change(held);
            }
        });
    };
    loop {
        if *kill.borrow() {
            return;
        }
        let started = Instant::now();
        let spawned = options
            .engines
            .command(&home, &lane)
            .and_then(|mut command| {
                command
                    .kill_on_drop(true)
                    .spawn()
                    .map_err(|e| format!("{} did not start: {e}", crate::setup::SERVER))
            });
        match spawned {
            Ok(mut process) => {
                let pid = process.id();
                record(&|held| {
                    held.pid = pid;
                    held.up = true;
                });
                log(format!(
                    "{} engine started on port {}{}",
                    lane.name(),
                    lane.port,
                    pid.map(|p| format!(" (process {p})")).unwrap_or_default()
                ));
                tokio::select! {
                    status = process.wait() => {
                        let ended = match status {
                            Ok(status) => format!("exited ({status})"),
                            Err(e) => format!("lost: {e}"),
                        };
                        log(format!("{} engine {ended}; restarting in {}s", lane.name(), wait.as_secs_f32()));
                        record(&|held| {
                            held.pid = None;
                            held.up = false;
                            held.restarts += 1;
                            held.last_exit = Some(ended.clone());
                        });
                    }
                    _ = kill.changed() => {
                        let _ = process.kill().await;
                        record(&|held| {
                            held.pid = None;
                            held.up = false;
                        });
                        return;
                    }
                }
            }
            Err(why) => {
                log(format!("{} engine: {why}", lane.name()));
                record(&|held| {
                    held.pid = None;
                    held.up = false;
                    held.restarts += 1;
                    held.last_exit = Some(why.clone());
                });
            }
        }
        if started.elapsed() >= options.max_backoff {
            wait = options.backoff;
        }
        tokio::select! {
            () = tokio::time::sleep(wait) => {}
            _ = kill.changed() => return,
        }
        wait = (wait * 2).min(options.max_backoff);
    }
}

fn emit(json_out: bool, value: &Value, text: &str) {
    if json_out {
        println!("{value}");
    } else {
        println!("{text}");
    }
}

/// `up`'s choices.
#[derive(Debug, Clone, Default)]
pub struct Up {
    pub yes: bool,
    pub foreground: bool,
    /// `false` for `--text off`.
    pub text: bool,
    pub text_model: Option<PathBuf>,
    pub device: Option<String>,
    pub port: Option<u16>,
    pub dedicated: bool,
    /// `serve`'s options, applied to the settings.
    pub serve: Vec<String>,
}

impl Up {
    /// Parse `up`'s words.
    ///
    /// # Errors
    ///
    /// On a value that does not parse.
    pub fn parse(words: &[String]) -> Result<Self, String> {
        let mut words = words.to_vec();
        let yes = share::take_flag(&mut words, "--yes");
        let foreground = share::take_flag(&mut words, "--foreground");
        let dedicated = share::take_flag(&mut words, "--dedicated");
        let text = match share::take_values(&mut words, "--text")?.pop().as_deref() {
            None | Some("on") => true,
            Some("off") => false,
            Some(other) => return Err(format!("--text takes on or off, not `{other}`")),
        };
        let text_model = share::take_values(&mut words, "--text-model")?
            .pop()
            .map(PathBuf::from);
        let device = share::take_values(&mut words, "--device")?.pop();
        if let Some(device) = &device
            && !matches!(device.as_str(), "metal" | "cuda" | "cpu")
        {
            return Err(format!(
                "--device takes metal, cuda, or cpu, not `{device}`"
            ));
        }
        let port = share::take_number(&mut words, "--port")?
            .map(|p| u16::try_from(p).map_err(|_| "--port takes a port number".to_string()))
            .transpose()?;
        Ok(Self {
            yes,
            foreground,
            text,
            text_model,
            device,
            port,
            dedicated,
            serve: words,
        })
    }
}

/// Whether this computer has room for a text engine beside the decision
/// engine.
///
/// # Errors
///
/// Why not, in plain words.
pub fn text_fits(host: &crate::setup::Host) -> Result<(), String> {
    let room = match host.gpu.as_str() {
        "apple" => host.memory_gb.unwrap_or(0.0) >= 24.0,
        "nvidia" => host.gpu_memory_gb.unwrap_or(0.0) >= 12.0,
        _ => false,
    };
    if room {
        Ok(())
    } else {
        Err("text answers are off: this computer has room for the decision engine only".into())
    }
}

/// The settings `up` writes: the held ones (or the defaults for `host`)
/// with `up`'s choices, a decision lane on `device`, a text lane when
/// `text_room` allows and its model is on disk, and `on: true`. Returns
/// notes on what was left out.
///
/// # Errors
///
/// On a bad serve option or clashing ports.
pub fn settings_for_up(
    held: Option<Settings>,
    host: &str,
    relay: &str,
    up: &Up,
    device: &str,
    text_room: Result<(), String>,
) -> Result<(Settings, Vec<String>), String> {
    let mut settings = held.unwrap_or_else(|| Settings::off(host));
    let mut words = up.serve.clone();
    share::apply(&mut settings, &mut words)?;
    if let Some(extra) = words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    // A decision pylon admits any key by default; rate limits still apply.
    if settings
        .allow
        .as_ref()
        .is_some_and(std::collections::BTreeSet::is_empty)
    {
        settings.allow = None;
    }
    settings.relay = relay.into();
    if up.dedicated {
        settings.dedicated = true;
    }
    let port = up
        .port
        .or_else(|| settings.decide.as_ref().map(|d| d.port))
        .unwrap_or(crate::setup::DECIDE_PORT);
    settings.decide = Some(DecideLane {
        port,
        device: device.into(),
        model_file: crate::setup::model_path(),
    });
    let mut notes = Vec::new();
    settings.text = if up.text {
        let file = up
            .text_model
            .clone()
            .or_else(|| settings.text.as_ref().map(|t| t.model_file.clone()))
            .unwrap_or_else(|| crate::setup::models_dir().join(TEXT_MODEL_FILE));
        if !file.is_file() {
            notes.push(format!(
                "text answers are off: no text model at {}",
                file.display()
            ));
            None
        } else if let Err(why) = text_room {
            notes.push(why);
            None
        } else {
            let text_port = if port == TEXT_PORT {
                TEXT_PORT + 1
            } else {
                TEXT_PORT
            };
            let model = if file.file_name().and_then(|n| n.to_str()) == Some(TEXT_MODEL_FILE) {
                TEXT_MODEL.to_string()
            } else {
                file.file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or(TEXT_MODEL)
                    .to_lowercase()
            };
            Some(TextLane {
                port: text_port,
                model,
                model_file: file,
            })
        }
    } else {
        notes.push("text answers are off (--text off)".into());
        None
    };
    settings.on = true;
    settings.changed_at = now();
    Ok((settings, notes))
}

/// Download the pinned decision model, asking first on a terminal.
async fn fetch_model(json_out: bool, yes: bool) -> Result<(), String> {
    let path = crate::setup::model_path();
    let gb = crate::setup::CLEF_FLASH_BYTES as f64 / 1e9;
    if !yes {
        if !std::io::stdin().is_terminal() || json_out {
            return Err(format!(
                "the decision model ({gb:.1} GB) is not downloaded; run `openagents pylon up --yes` to download it"
            ));
        }
        eprint!(
            "Download the decision model ({gb:.1} GB) to {}? [y/N] ",
            path.display()
        );
        let _ = std::io::stderr().flush();
        let mut answer = String::new();
        std::io::stdin()
            .lock()
            .read_line(&mut answer)
            .map_err(|e| e.to_string())?;
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            return Err("not downloaded; nothing changed".into());
        }
    }
    if !json_out {
        eprintln!(
            "downloading Clef-Flash Q4_K_M ({gb:.1} GB) to {}",
            path.display()
        );
    }
    let mut shown = 0u64;
    crate::setup::fetch(
        crate::setup::CLEF_FLASH_URL,
        &path,
        crate::setup::CLEF_FLASH_BYTES,
        crate::setup::CLEF_FLASH_SHA256,
        |got| {
            if !json_out && got >= shown + 500_000_000 {
                shown = got;
                eprintln!("{:.1} of {gb:.1} GB", got as f64 / 1e9);
            }
        },
    )
    .await?;
    if !crate::setup::model_state(&path, crate::setup::CLEF_FLASH_MODEL_ID, false).verified {
        return Err("the download is not the pinned model".into());
    }
    Ok(())
}

/// `openagents pylon up`.
///
/// # Errors
///
/// When something blocks serving, the download is declined, or the
/// settings cannot be written.
pub async fn up(json_out: bool, words: &[String], relay: &str) -> Result<(), String> {
    let up = Up::parse(words)?;
    let home = home();
    let port = up
        .port
        .or_else(|| {
            share::load(&home)
                .ok()
                .flatten()
                .and_then(|s| s.decide.map(|d| d.port))
        })
        .unwrap_or(crate::setup::DECIDE_PORT);
    let report = crate::setup::doctor(relay, &format!("http://127.0.0.1:{port}"), false).await;
    // `up` fetches the model and runs the self-check itself.
    let blocks: Vec<&crate::setup::Block> = report
        .blocks
        .iter()
        .filter(|b| b.fix != "openagents pylon setup" && b.fix != "openagents pylon selfcheck")
        .collect();
    if !blocks.is_empty() {
        let lines: Vec<String> = blocks
            .iter()
            .map(|b| format!("{} (fix: {})", b.what, b.fix))
            .collect();
        return Err(format!(
            "this computer cannot serve yet: {}",
            lines.join("; ")
        ));
    }
    if !report.model.verified {
        fetch_model(json_out, up.yes).await?;
    }
    let device = up
        .device
        .clone()
        .unwrap_or_else(|| report.fit.device.clone());
    let (settings, notes) = settings_for_up(
        share::load(&home)?,
        &crate::cli::host_slug(),
        relay,
        &up,
        &device,
        text_fits(&report.host),
    )?;
    share::save(&home, &settings)?;
    let identity = Identity::load_or_create(&home.join("provider.key"))?;
    let at = now();
    let live = load_live(&home).filter(|l| l.fresh(at) && l.supervisor == "host");
    let summary = json!({
        "on": true,
        "pylon": format!("30200:{}:{}", identity.pubkey(), settings.pylon),
        "npub": identity.npub(),
        "device": device,
        "decide": settings.decide,
        "text": settings.text,
        "notes": notes,
    });
    if live.is_some() && !up.foreground {
        // The host service picks the settings up within seconds.
        let started = Instant::now();
        let mut seen = None;
        while started.elapsed() < Duration::from_secs(30) {
            tokio::time::sleep(Duration::from_secs(1)).await;
            if let Some(l) = load_live(&home)
                && l.at > at
                && l.state != Phase::Off
            {
                seen = Some(l);
                break;
            }
        }
        let mut value = summary;
        value["supervisor"] = json!("host");
        value["state"] = json!(seen.as_ref().map(|l| l.state.as_str()));
        let mut text = format!(
            "{} is on. This computer's host service runs it and brings it back after a restart.\n",
            settings.pylon
        );
        for note in &notes {
            text.push_str(&format!("note: {note}\n"));
        }
        text.push_str(match &seen {
            Some(l) if l.state == Phase::Serving => "serving now",
            Some(_) => "starting: the engine warms up before it serves",
            None => "the host service has not picked it up yet",
        });
        text.push_str("\nnext: openagents pylon status");
        emit(json_out, &value, &text);
        return Ok(());
    }
    if load_live(&home).is_some_and(|l| l.fresh(at) && l.supervisor == "foreground") {
        let mut value = summary;
        value["supervisor"] = json!("foreground");
        emit(
            json_out,
            &value,
            "This pylon already runs in another terminal; it picks up these settings within seconds.\nnext: openagents pylon status",
        );
        return Ok(());
    }
    if live.is_some() {
        return Err(
            "the host service already runs this pylon; see `openagents pylon status`".into(),
        );
    }
    let machine: Arc<dyn Machine> = if settings.dedicated {
        Arc::new(Dedicated)
    } else {
        share::host_machine()?
    };
    let mut value = summary;
    value["supervisor"] = json!("foreground");
    let mut text = format!(
        "{} is on, running in this terminal (Ctrl-C stops it; `openagents pylon down` turns it off).\n",
        settings.pylon
    );
    for note in &notes {
        text.push_str(&format!("note: {note}\n"));
    }
    emit(json_out, &value, text.trim_end());
    supervise(
        home,
        machine,
        Options::foreground(),
        Arc::new(|line| eprintln!("pylon: {line}")),
        // Stop on Ctrl-C or SIGTERM alike, so `kill` or a service manager
        // still stops the engines and publishes the offline beacon.
        async {
            #[cfg(unix)]
            {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {}
                    () = async { match term.as_mut() { Some(t) => { t.recv().await; } None => std::future::pending().await } } => {}
                }
            }
            #[cfg(not(unix))]
            {
                let _ = tokio::signal::ctrl_c().await;
            }
        },
    )
    .await;
    Ok(())
}

/// `down` at `home`: record `on: false`, then wait up to `wait` for a
/// running supervisor to finish jobs and stop the engines, or publish the
/// offline beacon here when none runs.
///
/// # Errors
///
/// When the settings cannot be written, or the supervisor is still
/// finishing jobs after `wait`.
pub async fn down_at(home: &Path, wait: Duration) -> Result<(Value, String), String> {
    let Some(mut settings) = share::load(home)? else {
        return Ok((
            json!({"on": false, "was_on": false}),
            "This computer is not a pylon; nothing to stop.".into(),
        ));
    };
    let was_on = settings.on;
    settings.on = false;
    settings.changed_at = now();
    share::save(home, &settings)?;
    let running = load_live(home).is_some_and(|l| l.fresh(now()) && l.state != Phase::Off);
    let mut warnings = Vec::new();
    if running {
        let started = Instant::now();
        loop {
            tokio::time::sleep(Duration::from_millis(200)).await;
            match load_live(home) {
                Some(l) if l.fresh(now()) && l.state != Phase::Off => {}
                _ => break,
            }
            if started.elapsed() > wait {
                return Err(
                    "turned off, but the pylon is still finishing jobs; `openagents pylon status` shows when it stops"
                        .into(),
                );
            }
        }
    } else if let Err(e) = publish_offline(home, &settings).await {
        warnings.push(format!("the offline notice did not reach the relay: {e}"));
    }
    let jobs = load_live(home)
        .filter(|l| l.day == now() / DAY)
        .map_or(0, |l| l.jobs_today);
    let mut text = format!(
        "{} is off{}. It stays off until `openagents pylon up`.",
        settings.pylon,
        if running {
            "; its jobs finished and its engines stopped"
        } else {
            ""
        }
    );
    for warning in &warnings {
        text.push_str(&format!("\nwarning: {warning}"));
    }
    Ok((
        json!({"on": false, "was_on": was_on, "stopped_running": running,
               "jobs_today": jobs, "warnings": warnings}),
        text,
    ))
}

/// Publish an offline beacon for `settings`' pylon.
async fn publish_offline(home: &Path, settings: &Settings) -> Result<(), String> {
    let identity = Identity::load_or_create(&home.join("provider.key"))?;
    let mut config = settings.config(home);
    config.owner = load_owner(home)?;
    let owner = config.owner.clone();
    let provider = if let Some(lane) = &settings.decide {
        let clef = Clef::new(&format!("http://127.0.0.1:{}", lane.port), None)?;
        Provider::deciding(
            config,
            identity.clone(),
            None,
            Arc::new(clef),
            Arc::new(Dedicated),
        )?
    } else {
        Provider::on(
            config,
            identity.clone(),
            Arc::new(Psionic::new(&settings.engine, &settings.model)?),
            Arc::new(Dedicated),
        )?
    };
    let beacon = provider.beacon(Some(Status::Offline)).await;
    let event = owned_beacon_event(identity.signer(), &beacon, owner.as_ref())?;
    let mut conn =
        crate::relay::connect(&settings.relay, &identity, Duration::from_secs(10)).await?;
    let published = crate::relay::publish(&mut conn, &event).await;
    let _ = conn.close().await;
    published
}

/// `openagents pylon down`.
///
/// # Errors
///
/// As [`down_at`], and on an unexpected argument.
pub async fn down(json_out: bool, words: &[String]) -> Result<(), String> {
    if let Some(extra) = words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let (value, text) = down_at(&home(), crate::provider::DRAIN + Duration::from_secs(30)).await?;
    emit(json_out, &value, &text);
    Ok(())
}

fn ago(secs: u64) -> String {
    match secs {
        0..=89 => format!("{secs} s ago"),
        90..=5_399 => format!("{} min ago", (secs + 30) / 60),
        5_400..=129_599 => format!("{} h ago", (secs + 1_800) / 3_600),
        _ => format!("{} days ago", (secs + 43_200) / 86_400),
    }
}

/// What `status` says about this pylon. `checks` are the trusted
/// checkers' verdicts about it, newest last, or why they could not be
/// read.
///
/// # Errors
///
/// When the pylon key cannot be read or made.
pub fn report(
    home: &Path,
    host: &str,
    settings: Option<&Settings>,
    live: Option<&Live>,
    checks: Result<Vec<Check>, String>,
    at: u64,
) -> Result<(Value, String), String> {
    let identity = Identity::load_or_create(&home.join("provider.key"))?;
    let owner = load_owner(home)?.map(|o| npub(&o.owner_pubkey));
    let slug = settings.map_or_else(|| host.to_string(), |s| s.pylon.clone());
    let fresh = live.filter(|l| l.fresh(at));
    let (state, why): (&str, Option<String>) = match (settings, fresh) {
        (None, _) => (
            "off",
            Some("not set up yet; `openagents pylon up` makes this computer a pylon".into()),
        ),
        (Some(s), _) if !s.on => (
            "off",
            Some("turned off; `openagents pylon up` turns it back on".into()),
        ),
        (Some(_), Some(l)) => (l.state.as_str(), l.why.clone()),
        (Some(_), None) => (
            "paused",
            Some(
                "not running: start the host service (`openagents host serve`) or run `openagents pylon up --foreground`"
                    .into(),
            ),
        ),
    };
    let today = live.filter(|l| l.day == at / DAY);
    let (jobs, refused, failed) = today.map_or((0, 0, 0), |l| {
        (l.jobs_today, l.refused_today, l.failed_today)
    });
    let selfcheck = crate::calibrate::last(home);
    let model = fresh.and_then(|l| l.model.clone()).or_else(|| {
        settings
            .and_then(|s| s.decide.as_ref())
            .map(|_| "clef-flash".to_string())
    });
    let lanes = fresh.map(|l| l.lanes.clone()).unwrap_or_default();
    let (checks, checks_error) = match checks {
        Ok(list) => (list, None),
        Err(e) => (Vec::new(), Some(e)),
    };
    let recent: Vec<&Check> = checks.iter().rev().take(5).collect();
    let value = json!({
        "state": state,
        "why": why,
        "pylon": slug,
        "address": format!("30200:{}:{}", identity.pubkey(), slug),
        "npub": identity.npub(),
        "owner": owner,
        "supervisor": fresh.map(|l| l.supervisor.clone()),
        "model": model,
        "model_id": selfcheck.as_ref().map(|c| c.model_id.clone()),
        "device": settings.and_then(|s| s.decide.as_ref()).map(|d| d.device.clone()),
        "engines": lanes,
        "jobs_today": jobs,
        "refused_today": refused,
        "failed_today": failed,
        "selfcheck": selfcheck.as_ref().map(|c| json!({
            "pass": c.pass, "at": c.at, "round_pass": c.round_pass, "model_id": c.model_id,
        })),
        "checks": recent.iter().map(|c| json!({
            "verdict": format!("{:?}", c.verdict).to_lowercase(),
            "at": c.created_at,
            "method": c.method,
            "checker": npub(&c.checker),
        })).collect::<Vec<_>>(),
        "checks_error": checks_error.clone(),
        "relay": settings.map(|s| s.relay.clone()),
    });
    let mut text = match &why {
        Some(why) => format!("{state}: {why}\n"),
        None => format!("{state}\n"),
    };
    text.push_str(&format!("pylon    {slug} ({})\n", identity.npub()));
    text.push_str(&format!(
        "owner    {}\n",
        owner
            .as_deref()
            .unwrap_or("not linked (openagents pylon link)")
    ));
    if let Some(model) = &model {
        text.push_str(&format!(
            "model    {model}{}\n",
            settings
                .and_then(|s| s.decide.as_ref())
                .map(|d| format!(" on {}", d.device))
                .unwrap_or_default()
        ));
    }
    for (i, lane) in lanes.iter().enumerate() {
        text.push_str(&format!(
            "{}{} on port {}: {}{}\n",
            if i == 0 { "engines  " } else { "         " },
            match lane.lane {
                LaneKind::Decisions => "decisions",
                LaneKind::Text => "text",
            },
            lane.port,
            if lane.up { "running" } else { "down" },
            match (lane.restarts, &lane.last_exit) {
                (0, _) => String::new(),
                (n, Some(last)) => format!(" (restarted {n} times; last {last})"),
                (n, None) => format!(" (restarted {n} times)"),
            }
        ));
    }
    text.push_str(&format!(
        "jobs     {jobs} today ({refused} refused, {failed} failed)\n"
    ));
    if let Some(check) = &selfcheck {
        text.push_str(&format!(
            "self-check {} ({:.1}% of rounds), {}\n",
            if check.pass { "passed" } else { "failed" },
            check.round_pass * 100.0,
            ago(at.saturating_sub(check.at))
        ));
    }
    if let Some(e) = &checks_error {
        text.push_str(&format!("checks   could not be read: {e}\n"));
    } else if recent.is_empty() {
        text.push_str("checks   none in the last day\n");
    } else {
        let words: Vec<String> = recent
            .iter()
            .map(|c| {
                format!(
                    "{} {}",
                    format!("{:?}", c.verdict).to_lowercase(),
                    ago(at.saturating_sub(c.created_at))
                )
            })
            .collect();
        text.push_str(&format!("checks   {}\n", words.join(", ")));
    }
    Ok((value, text.trim_end().to_string()))
}

/// The trusted checkers' verdicts about `pubkey` over the last day, oldest
/// first.
async fn checks_about(home: &Path, relay: &str, pubkey: &str) -> Result<Vec<Check>, String> {
    let reader = Identity::load_or_create(&home.join("buyer.key"))?;
    let checkers = crate::check::trusted(home);
    let mut conn = crate::relay::connect(relay, &reader, Duration::from_secs(15)).await?;
    let since = now().saturating_sub(crate::check::CHECK_WINDOW_SECS);
    let (labels, receipts) = crate::check::fetch(&mut conn, &checkers, since).await?;
    let _ = conn.close().await;
    let verdicts = crate::check::Verdicts::new(&labels, &receipts, &checkers);
    Ok(verdicts
        .checks
        .into_iter()
        .filter(|c| c.provider == pubkey)
        .collect())
}

/// `openagents pylon status` with no `--field`: this pylon.
///
/// # Errors
///
/// On an unexpected argument, or when the settings or key cannot be read.
pub async fn status(json_out: bool, words: &[String], relay: &str) -> Result<(), String> {
    if let Some(extra) = words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let home = home();
    let settings = share::load(&home)?;
    let identity = Identity::load_or_create(&home.join("provider.key"))?;
    let relay = settings.as_ref().map_or(relay, |s| s.relay.as_str());
    let checks = match tokio::time::timeout(
        Duration::from_secs(10),
        checks_about(&home, relay, identity.pubkey()),
    )
    .await
    {
        Ok(result) => result,
        Err(_) => Err("the relay did not answer in time".into()),
    };
    let (value, text) = report(
        &home,
        &crate::cli::host_slug(),
        settings.as_ref(),
        load_live(&home).as_ref(),
        checks,
        now(),
    )?;
    emit(json_out, &value, &text);
    Ok(())
}

/// What `id` says: the pylon key, its name, owner link, relays, and beacon
/// address, with this computer's other role keys.
///
/// # Errors
///
/// When a key cannot be read or made.
pub fn id_at(home: &Path, host: &str) -> Result<(Value, String), String> {
    let key = |name: &str| Identity::load_or_create(&home.join(format!("{name}.key")));
    let (provider, buyer, aggregator, checker) = (
        key("provider")?,
        key("buyer")?,
        key("aggregator")?,
        key("checker")?,
    );
    let settings = share::load(home)?.unwrap_or_else(|| Settings::off(host));
    let owner = load_owner(home)?.map(|o| npub(&o.owner_pubkey));
    let address = format!("30200:{}:{}", provider.pubkey(), settings.pylon);
    let value = json!({
        "npub": provider.npub(),
        "pubkey": provider.pubkey(),
        "pylon": settings.pylon,
        "label": settings.label,
        "owner": owner,
        "relays": [settings.relay],
        "address": address,
        "home": home,
        "provider": provider.npub(),
        "buyer": buyer.npub(),
        "aggregator": aggregator.npub(),
        "checker": checker.npub(),
    });
    let text = format!(
        "pylon      {}\nname       {} ({})\nowner      {}\nrelays     {}\naddress    {address}\nbuyer      {}\naggregator {}\nchecker    {}\nkeys in {}",
        provider.npub(),
        settings.pylon,
        settings.label,
        owner
            .as_deref()
            .unwrap_or("not linked (openagents pylon link --owner-secret FILE)"),
        settings.relay,
        buyer.npub(),
        aggregator.npub(),
        checker.npub(),
        home.display()
    );
    Ok((value, text))
}

/// `openagents pylon id` (and `whoami`).
///
/// # Errors
///
/// As [`id_at`], and on an unexpected argument.
pub fn id(json_out: bool, words: &[String]) -> Result<(), String> {
    if let Some(extra) = words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let (value, text) = id_at(&home(), &crate::cli::host_slug())?;
    emit(json_out, &value, &text);
    Ok(())
}
