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
use crate::share::{self, DecideLane, Settings, TextEngine, TextLane};
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
    pub serial: Option<crate::serial::Config>,
    pub kind: LaneKind,
    pub engine: TextEngine,
    pub chat_template: Option<PathBuf>,
    pub stop: Vec<String>,
    pub metadata_overrides: Vec<String>,
    pub gpu_layers: Option<u32>,
    pub port: u16,
    pub model_file: PathBuf,
    /// The device (`metal`, `cuda`, `cpu`), selected separately for text
    /// when the chosen model needs another backend.
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
            serial: None,
            kind: LaneKind::Decisions,
            engine: TextEngine::Openai,
            chat_template: None,
            stop: Vec::new(),
            metadata_overrides: Vec::new(),
            gpu_layers: None,
            port: lane.port,
            model_file: lane.model_file.clone(),
            device: Some(lane.device.clone()),
            model: None,
        });
    }
    if let Some(lane) = &settings.text {
        out.push(LaneSpec {
            serial: lane.serial.clone(),
            kind: LaneKind::Text,
            engine: lane.engine,
            chat_template: lane.chat_template.clone(),
            stop: lane.stop.clone(),
            metadata_overrides: lane.metadata_overrides.clone(),
            gpu_layers: lane.gpu_layers,
            port: lane.port,
            model_file: lane.model_file.clone(),
            device: lane
                .device
                .clone()
                .or_else(|| settings.decide.as_ref().map(|d| d.device.clone())),
            model: Some(lane.model.clone()),
        });
    }
    out
}

/// The local model server command for `lane`, listening on loopback.
#[must_use]
pub fn psionic_command(server: &Path, lane: &LaneSpec) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(server);
    command.arg("-m").arg(&lane.model_file).args([
        "--host",
        "127.0.0.1",
        "--port",
        &lane.port.to_string(),
    ]);
    if lane.engine == TextEngine::LlamaCpp {
        if lane.serial.is_some() && lane.device.as_deref() == Some("cuda") {
            command.args(["--device", "CUDA0"]);
        }
        let gpu_layers = if lane.device.as_deref() == Some("cpu") {
            "0".to_string()
        } else {
            lane.gpu_layers.map_or_else(
                || if lane.serial.is_some() { "all" } else { "auto" }.to_string(),
                |n| n.to_string(),
            )
        };
        command.args([
            "--ctx-size",
            "4096",
            "--parallel",
            "1",
            "--jinja",
            "--chat-template-kwargs",
            if lane.serial.is_some() {
                "{\"enable_thinking\":true}"
            } else {
                "{\"enable_thinking\":false}"
            },
            "--n-gpu-layers",
            &gpu_layers,
        ]);
        if lane.serial.is_some()
            && lane
                .model
                .as_deref()
                .is_some_and(|m| m.to_lowercase().contains("deepseek-r1"))
        {
            command.args(["--reasoning-format", "deepseek"]);
        }
        if let Some(template) = &lane.chat_template {
            command.arg("--chat-template-file").arg(template);
        }
        if !lane.metadata_overrides.is_empty() {
            command
                .arg("--override-kv")
                .arg(csv_arguments(&lane.metadata_overrides));
        }
        return command;
    }
    match lane.kind {
        LaneKind::Decisions => {
            command.args(["--decision-device", lane.device.as_deref().unwrap_or("cpu")]);
        }
        LaneKind::Text => {
            if let Some(device) = &lane.device {
                command.args(["--backend", device]);
            }
        }
    }
    if lane.serial.is_some() {
        command.arg("--single-generation");
    }
    if lane.engine != TextEngine::GptOss {
        command.args(["--mesh-coordination", "disabled"]);
    }
    command
}

// Current llama.cpp accepts one CSV value per option, rather than repeated flags.
fn csv_arguments(values: &[String]) -> String {
    values
        .iter()
        .map(|value| {
            if value.contains([',', '"', '\n', '\r']) {
                format!("\"{}\"", value.replace('"', "\"\""))
            } else {
                value.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// Environment variables an engine child keeps; everything else (API keys,
/// tokens) is dropped.
fn keeps(name: &str) -> bool {
    matches!(
        name,
        "PATH"
            | "HOME"
            | "USER"
            | "LANG"
            | "LC_ALL"
            | "TMPDIR"
            | "TZ"
            | "RUST_LOG"
            | "LLAMA_ARG_THREADS"
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
    /// The decision server at `url`, which loaded `model_file` (used to
    /// name the model by its tensors, #12234).
    ///
    /// # Errors
    ///
    /// When its client cannot be built.
    fn decider(&self, url: &str, model_file: &Path) -> Result<Arc<dyn Decider>, String>;
    /// The text engine at `url` serving `model`.
    ///
    /// # Errors
    ///
    /// When its client cannot be built.
    fn text(&self, url: &str, model: &str) -> Result<Arc<dyn Engine>, String>;
    /// Text completions with per-model answer boundaries.
    ///
    /// # Errors
    /// When the HTTP client cannot be built.
    fn text_with_stops(
        &self,
        url: &str,
        model: &str,
        _stop: &[String],
    ) -> Result<Arc<dyn Engine>, String> {
        self.text(url, model)
    }
    /// Text settings for a managed serial worker. Fixture factories may keep
    /// their fixed engine; the real factory forwards the explicit sampling.
    fn serial_text(&self, lane: &TextLane) -> Result<Arc<dyn Engine>, String> {
        self.text_with_stops(
            &format!("http://127.0.0.1:{}", lane.port),
            &lane.model,
            &lane.stop,
        )
    }
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

/// Supervised local model servers.
pub struct PsionicEngines;

impl Engines for PsionicEngines {
    fn command(&self, home: &Path, lane: &LaneSpec) -> Result<tokio::process::Command, String> {
        let mut server = lane
            .serial
            .as_ref()
            .map(|s| s.server.clone())
            .or_else(crate::setup::find_server)
            .ok_or_else(|| {
                format!(
                    "{} is not installed; build it with: {}",
                    crate::setup::SERVER,
                    crate::setup::BUILD_SERVER
                )
            })?;
        if lane.engine != TextEngine::Openai && lane.serial.is_none() {
            let (variable, name) = match lane.engine {
                TextEngine::GptOss => ("OPENAGENTS_PSIONIC_GPT_OSS_BIN", "psionic-gpt-oss-server"),
                TextEngine::LlamaCpp => ("OPENAGENTS_LLAMA_CPP_BIN", "llama-server"),
                TextEngine::Openai => unreachable!(),
            };
            server = std::env::var_os(variable)
                .map(PathBuf::from)
                .or_else(|| {
                    let sibling = server.with_file_name(name);
                    sibling.is_file().then_some(sibling)
                })
                .or_else(|| {
                    std::env::var_os("PATH").and_then(|path| {
                        std::env::split_paths(&path)
                            .map(|folder| folder.join(name))
                            .find(|candidate| candidate.is_file())
                    })
                })
                .ok_or_else(|| {
                    format!(
                        "{name} is missing; install it on PATH or set {variable} to its executable"
                    )
                })?;
            if !server.is_file() {
                return Err(format!(
                    "{name} is missing at {}; set {variable} to its executable",
                    server.display()
                ));
            }
        }
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

    fn decider(&self, url: &str, model_file: &Path) -> Result<Arc<dyn Decider>, String> {
        Ok(Arc::new(Clef::new(url, None)?.with_model_file(model_file)))
    }

    fn text(&self, url: &str, model: &str) -> Result<Arc<dyn Engine>, String> {
        Ok(Arc::new(Psionic::new(url, model)?))
    }

    fn text_with_stops(
        &self,
        url: &str,
        model: &str,
        stop: &[String],
    ) -> Result<Arc<dyn Engine>, String> {
        Ok(Arc::new(
            Psionic::new(url, model)?.with_stops(stop.to_vec()),
        ))
    }

    fn serial_text(&self, lane: &TextLane) -> Result<Arc<dyn Engine>, String> {
        let config = lane
            .serial
            .as_ref()
            .ok_or("missing serial worker settings")?;
        Ok(Arc::new(
            Psionic::new(&format!("http://127.0.0.1:{}", lane.port), &lane.model)?
                .with_stops(lane.stop.clone())
                .with_sampling(config.sampling),
        ))
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_busy: Option<bool>,
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
    if let Err(reason) = verify_profile(&settings).await {
        set(&live, |l| {
            l.state = Phase::Paused;
            l.why = Some(reason);
        });
        let _ = halt.await;
        return;
    }
    let serial = settings.text.as_ref().and_then(|lane| lane.serial.as_ref());
    let mut ownership = None;
    if let Some(config) = serial {
        let valid = config.validate().and_then(|()| {
            if settings.slots != 1 || settings.decide.is_some()
                || settings
                    .text
                    .as_ref()
                    .is_none_or(|t| !matches!(t.engine, TextEngine::Openai | TextEngine::LlamaCpp))
            {
                Err(
                    "managed serial recovery needs a text-only native Psionic or llama.cpp pylon with exactly one slot"
                        .to_string(),
                )
            } else {
                Ok(())
            }
        });
        if let Err(reason) = valid {
            set(&live, |l| {
                l.state = Phase::Paused;
                l.why = Some(reason.clone());
            });
            log(reason);
            let _ = halt.await;
            return;
        }
        match crate::serial::ownership(&home) {
            Ok(file) => ownership = Some(file),
            Err(reason) => {
                set(&live, |l| {
                    l.state = Phase::Paused;
                    l.why = Some(reason);
                });
                let _ = halt.await;
                return;
            }
        }
    }
    let control = serial.map(|_| crate::serial::Control::new());
    let specs = lanes(&settings);
    set(&live, |l| {
        l.state = Phase::Starting;
        l.why = Some("starting the engines".into());
        l.pylon = Some(settings.pylon.clone());
        l.serving_since = None;
        l.lanes = specs
            .iter()
            .map(|lane| LaneLive {
                generation_busy: lane.serial.as_ref().map(|_| false),
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
            if lane.serial.is_some() {
                let held = Arc::clone(&live);
                let record = move |pid, up, last_exit: Option<String>, restart| {
                    set(&held, |l| {
                        if let Some(lane) = l.lanes.get_mut(index) {
                            lane.pid = pid;
                            lane.up = up;
                            if last_exit.is_some() {
                                lane.last_exit = last_exit.clone();
                            }
                            if restart {
                                lane.restarts += 1;
                            }
                        }
                    });
                };
                return tokio::spawn(crate::serial::supervise(
                    lane.clone(),
                    home.clone(),
                    Arc::clone(&options),
                    Arc::clone(control.as_ref().expect("serial control")),
                    record,
                    Arc::clone(&log),
                    killed.clone(),
                ));
            }
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
            if lane.serial.is_some() && control.as_ref().is_none_or(|c| !c.is_ready())
                || !options.engines.ready(lane).await
            {
                ready = false;
                break;
            }
        }
        if ready {
            break;
        }
        if let Some(reason) = control.as_ref().and_then(|c| c.failure()) {
            set(&live, |l| {
                l.state = Phase::Paused;
                l.why = Some(reason);
            });
            let _ = (&mut halt).await;
            stop_children().await;
            return;
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
        control.clone(),
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
    let mut previous_down = None;
    loop {
        let down = read(&live, |l| l.lanes.iter().any(|lane| !lane.up)).unwrap_or(false);
        provider.hold(down).await;
        if control.is_some() && previous_down != Some(down) {
            provider.refresh_models().await;
            previous_down = Some(down);
        }
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
        let model = decider
            .as_ref()
            .map(|d| d.identity().advertised())
            .or_else(|| settings.text.as_ref().map(|lane| lane.model.clone()));
        set(&live, |l| {
            if let Some(control) = &control {
                for lane in &mut l.lanes {
                    if lane.generation_busy.is_some() {
                        lane.generation_busy = Some(control.busy());
                    }
                }
            }
            add(l, seen, counters);
            l.model = model;
            let (state, why) = if down {
                (
                    Phase::Paused,
                    Some(
                        control
                            .as_ref()
                            .and_then(|c| c.failure())
                            .unwrap_or_else(|| "an engine stopped; restarting it".into()),
                    ),
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
    if control.is_some() {
        provider.hold(true).await;
        stop_children().await;
        let _ = stop_provider.send(());
        let _ = serving.await;
    } else {
        let _ = stop_provider.send(());
        let _ = serving.await;
        stop_children().await;
    }
    let counters = provider.counters().await;
    set(&live, |l| add(l, seen, counters));
    drop(ownership);
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
    serial: Option<Arc<crate::serial::Control>>,
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
        .map(|lane| {
            if let Some(control) = &serial {
                return Ok(crate::serial::SerialEngine::wrap(
                    engines.serial_text(lane)?,
                    Arc::clone(control),
                ));
            }
            engines.text_with_stops(
                &format!("http://127.0.0.1:{}", lane.port),
                &lane.model,
                &lane.stop,
            )
        })
        .transpose()?;
    if let Some(lane) = &settings.decide {
        let decider =
            engines.decider(&format!("http://127.0.0.1:{}", lane.port), &lane.model_file)?;
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

/// Print a result: with `--json` it becomes the `openagents.pylon/1`
/// envelope's `data` (one document, never a second one), else the text.
fn emit(json_out: bool, value: &Value, text: &str) {
    if let Some(line) = printed(json_out, value, text) {
        println!("{line}");
    }
}

/// What [`emit`] prints itself: nothing when the envelope took the value.
fn printed(json_out: bool, value: &Value, text: &str) -> Option<String> {
    if crate::envelope::take(value) {
        return None;
    }
    Some(if json_out {
        value.to_string()
    } else {
        text.to_string()
    })
}

#[cfg(test)]
mod emit_tests {
    /// `status`, `up`, `down` and `id` print through the envelope: under
    /// `--json` their value becomes the envelope's data, so the command
    /// prints one document (it printed its own object and then an empty
    /// envelope before).
    #[test]
    fn json_results_go_into_the_envelope() {
        let value = serde_json::json!({"state": "off"});
        let mut inside = Some(String::new());
        let code = crate::envelope::run("status", true, || {
            inside = super::printed(true, &value, "off");
            Ok(())
        });
        assert_eq!(code, 0);
        assert_eq!(inside, None);
        // Outside a command (as in a library call) it prints the value.
        assert_eq!(
            super::printed(true, &value, "off").as_deref(),
            Some(r#"{"state":"off"}"#)
        );
        assert_eq!(super::printed(false, &value, "off").as_deref(), Some("off"));
    }
}

/// `up`'s choices.
#[derive(Debug, Clone, Default)]
pub struct Up {
    /// An explicit profile, or `None` inside `Some` to clear it.
    pub text_profile: Option<Option<crate::serial::Profile>>,
    pub managed_llama: Option<PathBuf>,
    pub managed_psionic: Option<PathBuf>,
    pub generation_seconds: Option<u64>,
    pub startup_seconds: Option<u64>,
    pub restart_limit: Option<u32>,
    pub yes: bool,
    pub foreground: bool,
    /// `false` for `--text off`.
    pub text: bool,
    pub text_model: Option<PathBuf>,
    /// The text backend; defaults to the decision lane's device.
    pub text_device: Option<String>,
    /// Which local server serves the text model.
    pub text_engine: Option<TextEngine>,
    /// A llama.cpp template path; `off` clears a saved override.
    pub text_chat_template: Option<PathBuf>,
    /// Answer boundaries for llama.cpp; a single `off` clears saved stops.
    pub text_stop: Option<Vec<String>>,
    /// llama.cpp metadata overrides; a single `off` clears saved values.
    pub text_override_kv: Option<Vec<String>>,
    /// llama.cpp GPU layer count, or auto.
    pub text_gpu_layers: Option<String>,
    pub device: Option<String>,
    pub port: Option<u16>,
    /// `--text-port`: the text engine's loopback port.
    pub text_port: Option<u16>,
    pub dedicated: bool,
    /// `--allow-slow`: serve decisions on the CPU anyway.
    pub allow_slow: bool,
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
        let text_profile = share::take_values(&mut words, "--text-profile")?
            .pop()
            .map(|value| match value.as_str() {
                crate::serial::Profile::MODEL => {
                    Ok(Some(crate::serial::Profile::DeepSeekR1DistillQwen15B))
                }
                "off" => Ok(None),
                _ => Err("--text-profile takes deepseek-r1-distill-qwen-1.5b or off".to_string()),
            })
            .transpose()?;
        let managed_llama = share::take_values(&mut words, "--managed-llama")?
            .pop()
            .map(PathBuf::from);
        let managed_psionic = share::take_values(&mut words, "--managed-psionic")?
            .pop()
            .map(PathBuf::from);
        let generation_seconds =
            share::take_number(&mut words, "--generation-seconds")?.map(u64::from);
        let startup_seconds = share::take_number(&mut words, "--startup-seconds")?.map(u64::from);
        let restart_limit = share::take_number(&mut words, "--restart-limit")?
            .map(|n| u32::try_from(n).map_err(|_| "--restart-limit is too large".to_string()))
            .transpose()?;
        let yes = share::take_flag(&mut words, "--yes");
        let foreground = share::take_flag(&mut words, "--foreground");
        let dedicated = share::take_flag(&mut words, "--dedicated");
        let allow_slow = share::take_flag(&mut words, "--allow-slow");
        let text = match share::take_values(&mut words, "--text")?.pop().as_deref() {
            None | Some("on") => true,
            Some("off") => false,
            Some(other) => return Err(format!("--text takes on or off, not `{other}`")),
        };
        let text_model = share::take_values(&mut words, "--text-model")?
            .pop()
            .map(PathBuf::from);
        let device = share::take_values(&mut words, "--device")?.pop();
        let text_chat_template = share::take_values(&mut words, "--text-chat-template")?
            .pop()
            .map(PathBuf::from);
        let stops = share::take_values(&mut words, "--text-stop")?;
        let text_stop = (!stops.is_empty()).then_some(stops);
        let overrides = share::take_values(&mut words, "--text-override-kv")?;
        let text_override_kv = (!overrides.is_empty()).then_some(overrides);
        let text_gpu_layers = share::take_values(&mut words, "--text-gpu-layers")?.pop();
        let text_device = share::take_values(&mut words, "--text-device")?.pop();
        let text_engine = share::take_values(&mut words, "--text-engine")?
            .pop()
            .map(|value| match value.as_str() {
                "openai" => Ok(TextEngine::Openai),
                "gpt-oss" => Ok(TextEngine::GptOss),
                "llama-cpp" => Ok(TextEngine::LlamaCpp),
                _ => Err("--text-engine takes openai, gpt-oss, or llama-cpp".to_string()),
            })
            .transpose()?;
        if let Some(value) = &text_device
            && !matches!(value.as_str(), "metal" | "cuda" | "cpu")
        {
            return Err("--text-device takes metal, cuda, or cpu".into());
        }
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
        let text_port = share::take_number(&mut words, "--text-port")?
            .map(|p| u16::try_from(p).map_err(|_| "--text-port takes a port number".to_string()))
            .transpose()?;
        Ok(Self {
            text_profile,
            managed_llama,
            managed_psionic,
            generation_seconds,
            startup_seconds,
            restart_limit,
            yes,
            foreground,
            text,
            text_model,
            text_device,
            text_engine,
            text_chat_template,
            text_stop,
            text_override_kv,
            text_gpu_layers,
            device,
            port,
            text_port,
            dedicated,
            allow_slow,
            serve: words,
        })
    }

    fn profile(&self, held: Option<&Settings>) -> Option<crate::serial::Profile> {
        self.text_profile.unwrap_or_else(|| {
            held.and_then(|settings| settings.text.as_ref())
                .filter(|lane| {
                    self.text
                        && self
                            .text_model
                            .as_ref()
                            .is_none_or(|file| file == &lane.model_file)
                        && self.text_engine.is_none_or(|engine| engine == lane.engine)
                })
                .and_then(|lane| lane.profile)
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
    if let Some(profile) = up.profile(Some(&settings)) {
        return profile_settings(settings, up, profile, relay);
    }
    if up.managed_llama.is_some()
        || up.managed_psionic.is_some()
        || up.generation_seconds.is_some()
        || up.startup_seconds.is_some()
        || up.restart_limit.is_some()
    {
        return Err(
            "managed recovery options need --text-profile deepseek-r1-distill-qwen-1.5b".into(),
        );
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
    let text_device = up
        .text_device
        .clone()
        .or_else(|| settings.text.as_ref().and_then(|t| t.device.clone()));
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
        } else if let Err(why) = text_room
            && text_device.as_deref() != Some("cpu")
        {
            notes.push(why);
            None
        } else {
            let held = settings.text.as_ref().map(|t| t.port);
            let text_port = up.text_port.or(held).unwrap_or(if port == TEXT_PORT {
                TEXT_PORT + 1
            } else {
                TEXT_PORT
            });
            if text_port == port {
                return Err(format!(
                    "the text engine and the decision engine cannot share port {port}; choose another with --text-port"
                ));
            }
            let model = if file.file_name().and_then(|n| n.to_str()) == Some(TEXT_MODEL_FILE) {
                TEXT_MODEL.to_string()
            } else {
                file.file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or(TEXT_MODEL)
                    .to_lowercase()
            };
            let engine = up
                .text_engine
                .or_else(|| settings.text.as_ref().map(|t| t.engine))
                .unwrap_or_default();
            let chat_template = match &up.text_chat_template {
                Some(path) if path == Path::new("off") => None,
                Some(path) => Some(path.clone()),
                None => settings.text.as_ref().and_then(|held| {
                    (held.model_file == file && held.engine == engine)
                        .then(|| held.chat_template.clone())
                        .flatten()
                }),
            };
            let stop = match &up.text_stop {
                Some(stops) if stops.as_slice() == ["off"] => Vec::new(),
                Some(stops) => stops.clone(),
                None => settings
                    .text
                    .as_ref()
                    .filter(|held| held.model_file == file && held.engine == engine)
                    .map(|held| held.stop.clone())
                    .unwrap_or_default(),
            };
            let metadata_overrides = match &up.text_override_kv {
                Some(values) if values.as_slice() == ["off"] => Vec::new(),
                Some(values) => values.clone(),
                None => settings
                    .text
                    .as_ref()
                    .filter(|held| held.model_file == file && held.engine == engine)
                    .map(|held| held.metadata_overrides.clone())
                    .unwrap_or_default(),
            };
            let gpu_layers = match up.text_gpu_layers.as_deref() {
                Some("auto") => None,
                Some(value) => Some(
                    value
                        .parse::<u32>()
                        .map_err(|_| "--text-gpu-layers takes a nonnegative count or auto")?,
                ),
                None => settings
                    .text
                    .as_ref()
                    .filter(|held| held.model_file == file && held.engine == engine)
                    .and_then(|held| held.gpu_layers),
            };
            if gpu_layers.is_some() && engine != TextEngine::LlamaCpp {
                return Err("--text-gpu-layers needs --text-engine llama-cpp".into());
            }
            if !metadata_overrides.is_empty() && engine != TextEngine::LlamaCpp {
                return Err("--text-override-kv needs --text-engine llama-cpp".into());
            }
            if !stop.is_empty() && engine != TextEngine::LlamaCpp {
                return Err("--text-stop needs --text-engine llama-cpp".into());
            }
            if let Some(path) = &chat_template {
                if engine != TextEngine::LlamaCpp {
                    return Err("--text-chat-template needs --text-engine llama-cpp".into());
                }
                if !path.is_file() {
                    return Err(format!(
                        "the chat template is missing at {}",
                        path.display()
                    ));
                }
            }
            Some(TextLane {
                profile: None,
                serial: None,
                port: text_port,
                model,
                model_file: file,
                device: text_device,
                engine,
                chat_template,
                stop,
                metadata_overrides,
                gpu_layers,
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

async fn verify_profile(settings: &Settings) -> Result<(), String> {
    if let Some(lane) = &settings.text
        && let Some(profile) = lane.profile
    {
        let path = lane.model_file.clone();
        tokio::task::spawn_blocking(move || profile.verify(&path))
            .await
            .map_err(|e| format!("model verification stopped: {e}"))??;
    }
    Ok(())
}

fn profile_settings(
    mut settings: Settings,
    up: &Up,
    profile: crate::serial::Profile,
    relay: &str,
) -> Result<(Settings, Vec<String>), String> {
    use crate::serial::{Config, Profile};
    if !up.text || up.port.is_some() || up.device.is_some() || up.allow_slow {
        return Err("the DeepSeek text profile does not serve decisions; use --text-device cuda and --text-port".into());
    }
    if up
        .text_engine
        .is_some_and(|engine| engine == TextEngine::GptOss)
        || up
            .text_device
            .as_deref()
            .is_some_and(|device| device != "cuda")
    {
        return Err("the DeepSeek text profile needs native Psionic or llama-cpp on cuda".into());
    }
    if up.text_chat_template.is_some()
        || up.text_stop.is_some()
        || up.text_override_kv.is_some()
        || up.text_gpu_layers.is_some()
    {
        return Err("the DeepSeek text profile uses its tested template, stop settings, and full GPU offload; drop the text overrides".into());
    }
    if up.serve.iter().any(|word| word == "--slots") && settings.slots != 1 {
        return Err("the DeepSeek text profile needs --slots 1".into());
    }
    if up.serve.iter().any(|word| word == "--max-tokens") && settings.max_tokens != 2048 {
        return Err("the DeepSeek text profile needs --max-tokens 2048".into());
    }
    if settings
        .allow
        .as_ref()
        .is_some_and(std::collections::BTreeSet::is_empty)
    {
        return Err("name who may send jobs with --allow NPUB, or pass --allow-any".into());
    }
    let held = settings
        .text
        .as_ref()
        .filter(|lane| lane.profile == Some(profile));
    let file = up
        .text_model
        .clone()
        .or_else(|| held.map(|lane| lane.model_file.clone()))
        .unwrap_or_else(|| crate::setup::models_dir().join(Profile::FILE));
    let file = file
        .canonicalize()
        .map_err(|e| format!("cannot open the DeepSeek model at {}: {e}", file.display()))?;
    if up.managed_llama.is_some() && up.managed_psionic.is_some() {
        return Err("choose --managed-psionic or --managed-llama".into());
    }
    let engine = up
        .text_engine
        .or_else(|| {
            if up.managed_psionic.is_some() {
                Some(TextEngine::Openai)
            } else if up.managed_llama.is_some() {
                Some(TextEngine::LlamaCpp)
            } else {
                held.map(|lane| lane.engine)
            }
        })
        .unwrap_or(TextEngine::LlamaCpp);
    if (engine == TextEngine::Openai && up.managed_llama.is_some())
        || (engine == TextEngine::LlamaCpp && up.managed_psionic.is_some())
    {
        return Err("the selected text engine does not match the managed executable option".into());
    }
    let server = if engine == TextEngine::Openai { up.managed_psionic.clone() } else { up.managed_llama.clone() }
        .or_else(|| held.filter(|lane| lane.engine == engine).and_then(|lane| lane.serial.as_ref()).map(|config| config.server.clone()))
        .or_else(|| if engine == TextEngine::Openai { crate::setup::find_server() } else { crate::serial::find_server() })
        .ok_or("the model server is missing; pass --managed-psionic /path/to/psionic-openai-server, or select llama-cpp with --managed-llama")?;
    let server = server
        .canonicalize()
        .map_err(|e| format!("cannot open the model server at {}: {e}", server.display()))?;
    let held_config = held.and_then(|lane| lane.serial.as_ref());
    let config = Config {
        server,
        sampling: crate::engine::Sampling::new(
            if engine == TextEngine::Openai {
                0.0
            } else {
                0.6
            },
            0.95,
            1,
        )?,
        generation_seconds: up
            .generation_seconds
            .or(held_config.map(|c| c.generation_seconds))
            .unwrap_or(75),
        startup_seconds: up
            .startup_seconds
            .or(held_config.map(|c| c.startup_seconds))
            .unwrap_or(180),
        stop_seconds: 5,
        max_restarts: up
            .restart_limit
            .or(held_config.map(|c| c.max_restarts))
            .unwrap_or(3),
    };
    config.validate()?;
    let port = up
        .text_port
        .or(held.map(|lane| lane.port))
        .unwrap_or(TEXT_PORT);
    if port == 0 {
        return Err("--text-port needs a port from 1 to 65535".into());
    }
    settings.text = Some(TextLane {
        profile: Some(profile),
        serial: Some(config),
        port,
        model: Profile::MODEL.into(),
        model_file: file,
        device: Some("cuda".into()),
        engine,
        chat_template: None,
        stop: Vec::new(),
        metadata_overrides: Vec::new(),
        gpu_layers: None,
    });
    settings.decide = None;
    settings.relay = relay.into();
    settings.slots = 1;
    settings.max_tokens = 2048;
    settings.model = Profile::MODEL.into();
    settings.engine = format!("http://127.0.0.1:{port}");
    settings.dedicated |= up.dedicated;
    settings.schema = if engine == TextEngine::Openai {
        share::NATIVE_PROFILE_SCHEMA
    } else {
        share::PROFILE_SCHEMA
    }
    .into();
    settings.on = true;
    settings.changed_at = now();
    Ok((settings, Vec::new()))
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

/// Whether `live` is another `up --foreground` that still runs: written
/// recently, not stopped, and its process alive. A supervisor that was
/// just stopped (by Ctrl-C, SIGTERM, or `down`) writes `off` as it ends, and
/// one that was killed leaves a dead process id; neither is running.
#[must_use]
pub fn foreground_running(live: &Live, at: u64) -> bool {
    live.fresh(at)
        && live.supervisor == "foreground"
        && live.state != Phase::Off
        && live.pid != std::process::id()
        && process_alive(live.pid)
}

/// Whether a recently reporting supervisor still owns this home, even if off.
#[must_use]
pub fn supervisor_running(live: &Live, at: u64) -> bool {
    live.fresh(at) && process_alive(live.pid)
}

/// Whether process `pid` exists (`kill -0`); `true` where that cannot be
/// asked.
fn process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .map_or(true, |s| s.success())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        true
    }
}

/// The memory a beacon's class names for `host` serving on `device`: the
/// GPU's own memory on NVIDIA, else the computer's (a Mac's unified
/// memory, or a CPU box's).
#[must_use]
pub fn class_memory_gb(host: &crate::setup::Host, device: &str) -> Option<u32> {
    let gb = match device {
        "cuda" => host.gpu_memory_gb?,
        _ => host.memory_gb?,
    };
    Some(gb.round().clamp(1.0, 4096.0) as u32)
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
    let held = share::load(&home)?;
    let profile = up.profile(held.as_ref());
    let (host, device, room) = if profile.is_some() {
        let host = crate::setup::host();
        if host.gpu != "nvidia" {
            return Err("the DeepSeek text profile needs an NVIDIA GPU".into());
        }
        (host, "cuda".to_string(), Ok(()))
    } else {
        let port = up
            .port
            .or_else(|| {
                held.as_ref()
                    .and_then(|s| s.decide.as_ref().map(|d| d.port))
            })
            .unwrap_or(crate::setup::DECIDE_PORT);
        let mut report =
            crate::setup::doctor(relay, &format!("http://127.0.0.1:{port}"), false).await;
        if up.allow_slow {
            report = report.allowing_slow();
        }
        if up.device.as_deref() == Some("cpu") && report.fit.useful {
            // A GPU machine asked to decide on the CPU: slow, so the same rule.
            let cpu = crate::setup::Fit {
                device: "cpu".into(),
                useful: false,
                note: "--device cpu decides on the CPU".into(),
            };
            crate::setup::may_serve_on(&cpu, up.allow_slow)?;
        }
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
        let room = text_fits(&report.host);
        (report.host, device, room)
    };
    let (mut settings, notes) =
        settings_for_up(held, &crate::cli::host_slug(), relay, &up, &device, room)?;
    verify_profile(&settings).await?;
    // The beacon's memory is this computer's, unless `--vram-gb` named it.
    if !up.serve.iter().any(|w| w == "--vram-gb")
        && let Some(gb) = class_memory_gb(&host, &device)
    {
        settings.vram_gb = gb;
    }
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
    if load_live(&home).is_some_and(|l| foreground_running(&l, at)) {
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
    let mut options = Options::foreground();
    if profile.is_some() {
        options.every = Duration::from_millis(250);
    }
    supervise(
        home,
        machine,
        options,
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
        let clef = Clef::new(&format!("http://127.0.0.1:{}", lane.port), None)?
            .with_model_file(&lane.model_file);
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
            .or_else(|| {
                settings
                    .and_then(|s| s.text.as_ref())
                    .map(|lane| lane.model.clone())
            })
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
        "device": settings.and_then(|s| s.decide.as_ref().map(|d| d.device.clone())
            .or_else(|| s.text.as_ref().and_then(|lane| lane.device.clone()))),
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
