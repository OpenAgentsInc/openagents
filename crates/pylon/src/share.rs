//! Shared compute as a setting of the Coder host: `openagents host share
//! on|off|status` records the choice in `share.json` in the pylon home,
//! and the running host's [`supervise`] task starts the provider when it
//! is on and stops it (with an offline beacon) when it is turned off.
//!
//! Sharing is off by default and takes an explicit owner action. Pool jobs
//! run under the lease broker at `background` priority ([`crate::lease`]),
//! so the owner's own work comes first.
//!
//! Pylon 1.0's `openagents pylon up` writes the same file with a decision
//! lane (and, when memory allows, a text lane): the supervisor then owns
//! the Psionic engine children too, restarting a crashed one with backoff
//! ([`crate::online`]).

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use crate::identity::{Identity, hex_pubkey, load_owner, npub};
use crate::lease::{Leases, Machine};
use crate::provider::Config;
use crate::{DEFAULT_RELAY, now};
use nostr::pylon::{Family, Tier};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The settings file's schema. Version 1 files (no lanes) still load.
pub const SCHEMA: &str = "openagents.pylon.share.v2";

/// The schema of the files written before engine lanes.
pub const SCHEMA_V1: &str = "openagents.pylon.share.v1";
/// Older readers must reject managed recovery rather than ignore its policy.
pub const SERIAL_SCHEMA: &str = "openagents.pylon.share.v3";
/// Profile-aware hosts verify the pinned artifact before starting a worker.
pub const PROFILE_SCHEMA: &str = "openagents.pylon.share.v4";
/// Native profiles require a host that supervises native serial workers.
pub const NATIVE_PROFILE_SCHEMA: &str = "openagents.pylon.share.v5";
/// A model group (#12402) needs a host that runs stage groups; older hosts
/// refuse the file rather than serve without its stages.
pub const GROUP_SCHEMA: &str = "openagents.pylon.share.v6";

/// One stage of a group lane: how the head starts it and which layers it
/// runs. The head appends `--stage NAME --generation N --front ADDR
/// --token T` to `args`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupStage {
    pub name: String,
    pub program: PathBuf,
    #[serde(default)]
    pub args: Vec<String>,
    /// First layer.
    pub from: u32,
    /// One past the last layer.
    pub to: u32,
}

/// The model group this pylon heads (#12402): the stage workers it
/// supervises through [`crate::stage_group`], and what its head beacon
/// declares.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupLane {
    /// The group's slug.
    pub id: String,
    /// The placement it was planned from, such as `four-single`.
    pub placement: String,
    /// The served model's name, before `@`.
    pub model: String,
    /// The exact artifact, `sha256:<64 hex>`.
    pub artifact: String,
    /// SHA-256 of the group manifest file the stages were planned from.
    pub manifest: String,
    /// The weights digest every stage must announce.
    pub weights: String,
    /// The runtime every stage must announce.
    pub runtime: String,
    pub protocol: u32,
    pub deadline_secs: u64,
    pub max_output_tokens: u32,
    /// The front's sampling settings, sent to the last stage each step.
    pub sampling: crate::engine::Sampling,
    pub stages: Vec<GroupStage>,
    /// Where stage state travels: through the head (`front`, W10) or
    /// straight from stage to stage (`direct`, W12). Left out when `front`,
    /// so W15's hosts still read the file.
    #[serde(default, skip_serializing_if = "is_default")]
    pub transport: crate::stage_group::Transport,
    /// The direct links' bounds.
    #[serde(default, skip_serializing_if = "is_default")]
    pub links: crate::stage_group::LinkPolicy,
    pub step_timeout_ms: u64,
    pub health_timeout_ms: u64,
    pub startup_timeout_ms: u64,
    pub stop_timeout_ms: u64,
    pub max_restarts: u32,
    pub backoff_ms: u64,
    pub max_backoff_ms: u64,
}

fn is_default<T: Default + PartialEq>(value: &T) -> bool {
    *value == T::default()
}

impl GroupLane {
    /// The stage group configuration this lane describes.
    #[must_use]
    pub fn stage_config(&self) -> crate::stage_group::StageGroupConfig {
        let ms = Duration::from_millis;
        crate::stage_group::StageGroupConfig {
            model: format!("{}@{}", self.model, self.artifact),
            weights: self.weights.clone(),
            runtime: self.runtime.clone(),
            protocol: self.protocol,
            stages: self
                .stages
                .iter()
                .map(|s| crate::stage_group::StageSpec {
                    name: s.name.clone(),
                    program: s.program.clone(),
                    args: s.args.clone(),
                })
                .collect(),
            transport: self.transport,
            links: self.links,
            step_timeout: ms(self.step_timeout_ms),
            health_timeout: ms(self.health_timeout_ms),
            startup_timeout: ms(self.startup_timeout_ms),
            stop_timeout: ms(self.stop_timeout_ms),
            max_restarts: self.max_restarts,
            backoff: ms(self.backoff_ms),
            max_backoff: ms(self.max_backoff_ms),
        }
    }

    /// What the head beacon declares.
    #[must_use]
    pub fn spec(&self) -> crate::group::GroupSpec {
        crate::group::GroupSpec {
            id: self.id.clone(),
            model: self.model.clone(),
            artifact: self.artifact.clone(),
            deadline_secs: self.deadline_secs,
            max_output_tokens: self.max_output_tokens,
        }
    }

    /// Refuse a lane the head can't publish or serve.
    ///
    /// # Errors
    ///
    /// Says what is wrong.
    pub fn validate(&self) -> Result<(), String> {
        self.validate_lane()?;
        self.stage_config().validate()
    }

    /// As [`Self::validate`], for a lane another machine heads, where the
    /// stage programs live.
    ///
    /// # Errors
    ///
    /// Says which setting is wrong.
    pub fn validate_elsewhere(&self) -> Result<(), String> {
        self.validate_lane()?;
        self.stage_config().validate_settings()
    }

    fn validate_lane(&self) -> Result<(), String> {
        let ok_slug = |v: &str| {
            !v.is_empty()
                && v.len() <= 48
                && v.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        };
        if !ok_slug(&self.id) {
            return Err("a group id is 1 to 48 of a-z, 0-9, and -".into());
        }
        let hex = |v: &str| v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit());
        if !self.artifact.strip_prefix("sha256:").is_some_and(hex) {
            return Err("a group artifact is sha256:<64 hex>".into());
        }
        // Past 90 seconds, the head takes only `pylon.deadline.v1` jobs and
        // refuses ordinary ones at once (W14, #12401).
        if !(1..=nostr::pylon::deadline::MAX_SECS).contains(&self.deadline_secs) {
            return Err("a group deadline is 1 to 3600 seconds".into());
        }
        if !(1..=4_096).contains(&self.max_output_tokens) {
            return Err("a group answers with 1 to 4096 tokens".into());
        }
        let mut next = 0;
        for stage in &self.stages {
            if !ok_slug(&stage.name) || stage.from != next || stage.to <= stage.from {
                return Err(format!(
                    "stage {} must follow layer {next} with a slug name",
                    stage.name
                ));
            }
            next = stage.to;
        }
        Ok(())
    }
}

/// The decision lane: a Psionic server the supervisor runs on loopback for
/// NIP-DEC decisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecideLane {
    /// Its loopback port (default 18096).
    pub port: u16,
    /// `metal`, `cuda`, or `cpu`.
    pub device: String,
    /// The GGUF it loads.
    pub model_file: PathBuf,
}

/// The local server that runs a text model.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextEngine {
    /// The general Psionic model server.
    #[default]
    Openai,
    /// Psionic's GPT-OSS server.
    GptOss,
    /// A local llama.cpp server using its OpenAI-compatible API.
    LlamaCpp,
}

/// The text lane: a second model server on another loopback port for
/// NIP-CJ text jobs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextLane {
    /// The pinned text-only `up` profile, if explicitly selected.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<crate::serial::Profile>,
    /// Opt-in ownership, serial admission, and recovery of this llama.cpp worker.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial: Option<crate::serial::Config>,
    /// Its loopback port (default 18097).
    pub port: u16,
    /// The model name the beacon advertises.
    pub model: String,
    /// The GGUF it loads.
    pub model_file: PathBuf,
    /// Override the decision lane's device for this text model.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<String>,
    /// Which local server loads this model.
    #[serde(default)]
    pub engine: TextEngine,
    /// Optional llama.cpp chat template for models with incorrect GGUF metadata.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat_template: Option<PathBuf>,
    /// Strings that end a llama.cpp answer before another transcript turn.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
    /// llama.cpp metadata fixes for GGUFs with incorrect tokenizer settings.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub metadata_overrides: Vec<String>,
    /// Explicit GPU layer count; None lets llama.cpp choose automatically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gpu_layers: Option<u32>,
}

/// What the owner chose for this computer's shared compute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Settings {
    pub schema: String,
    /// Whether this computer serves the pool.
    pub on: bool,
    /// When the owner last changed it, Unix seconds.
    pub changed_at: u64,
    pub relay: String,
    /// The Psionic server on loopback.
    pub engine: String,
    pub model: String,
    /// The beacon's slug.
    pub pylon: String,
    pub label: String,
    pub slots: u32,
    /// Hex keys that may send jobs; `None` admits any key.
    pub allow: Option<BTreeSet<String>>,
    pub rate_per_minute: u32,
    pub max_tokens: u32,
    pub vram_gb: u32,
    pub pool: String,
    /// The decision lane `pylon up` turns on; `None` serves text only from
    /// `engine` (the `host share` path).
    #[serde(default)]
    pub decide: Option<DecideLane>,
    /// A text lane the supervisor runs itself; `None` uses `engine` when
    /// there is no decision lane, and no text otherwise.
    #[serde(default)]
    pub text: Option<TextLane>,
    /// Ignore this computer's lease table (a box that runs no owner work).
    #[serde(default)]
    pub dedicated: bool,
    /// A model group this pylon heads instead of serving a model itself
    /// (#12402); needs version 6 settings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupLane>,
}

impl Settings {
    /// Sharing off, with the defaults `serve` uses.
    #[must_use]
    pub fn off(pylon: &str) -> Self {
        Self {
            schema: SCHEMA.into(),
            on: false,
            changed_at: 0,
            relay: DEFAULT_RELAY.into(),
            engine: "http://127.0.0.1:18080".into(),
            model: "qwen3.5-0.8b-q8_0".into(),
            pylon: pylon.into(),
            label: pylon.into(),
            slots: 2,
            allow: Some(BTreeSet::new()),
            rate_per_minute: 10,
            max_tokens: 512,
            vram_gb: 16,
            pool: "everglade".into(),
            decide: None,
            text: None,
            dedicated: false,
            group: None,
        }
    }

    /// The provider's configuration under these settings.
    #[must_use]
    pub fn config(&self, home: &Path) -> Config {
        let mut config = Config::new(&self.relay, &self.pylon, home.to_path_buf());
        config.label.clone_from(&self.label);
        config.slots = self.slots.clamp(1, 64);
        config.allow.clone_from(&self.allow);
        config.rate_per_minute = self.rate_per_minute.clamp(1, 600);
        config.max_tokens = self.max_tokens.clamp(1, 4_096);
        // The beacon's family follows the decision lane's device: a Mac's
        // Metal is unified memory and a CPU box is a CPU, not a GPU.
        if let Some(device) = self
            .decide
            .as_ref()
            .map(|lane| lane.device.as_str())
            .or_else(|| self.text.as_ref().and_then(|lane| lane.device.as_deref()))
        {
            config.class.family = match device {
                "metal" => Family::UnifiedMemory,
                "cpu" => Family::Cpu,
                _ => Family::Gpu,
            };
        }
        config.class.tier = Tier::for_gpu(self.vram_gb);
        config.class.memory_gb = [512, 256, 128, 64, 32, 16, 8]
            .into_iter()
            .find(|&gb| gb <= self.vram_gb)
            .unwrap_or(8);
        config.pools = vec![self.pool.clone()];
        config
    }
}

/// The settings file in `home`.
#[must_use]
pub fn path(home: &Path) -> PathBuf {
    home.join("share.json")
}

/// The stored settings, or `None` when sharing was never set up.
///
/// # Errors
///
/// When the file exists and does not parse.
pub fn load(home: &Path) -> Result<Option<Settings>, String> {
    match std::fs::read(path(home)) {
        Ok(bytes) => {
            let mut settings: Settings = serde_json::from_slice(&bytes)
                .map_err(|e| format!("{}: {e}", path(home).display()))?;
            if settings
                .text
                .as_ref()
                .is_some_and(|lane| lane.profile.is_some() && lane.serial.is_none())
            {
                return Err("a text profile needs managed serial recovery settings".into());
            }
            if settings.schema != SCHEMA
                && settings.schema != SCHEMA_V1
                && settings.schema != SERIAL_SCHEMA
                && settings.schema != PROFILE_SCHEMA
                && settings.schema != NATIVE_PROFILE_SCHEMA
                && settings.schema != GROUP_SCHEMA
            {
                return Err(format!(
                    "{} is a newer settings file ({}); update openagents",
                    path(home).display(),
                    settings.schema
                ));
            }
            if settings
                .text
                .as_ref()
                .is_some_and(|lane| lane.serial.is_some())
                && !matches!(
                    settings.schema.as_str(),
                    SERIAL_SCHEMA | PROFILE_SCHEMA | NATIVE_PROFILE_SCHEMA
                )
            {
                return Err("managed serial recovery needs version 3 settings".into());
            }
            if settings
                .text
                .as_ref()
                .is_some_and(|lane| lane.profile.is_some())
                && !matches!(
                    settings.schema.as_str(),
                    PROFILE_SCHEMA | NATIVE_PROFILE_SCHEMA
                )
            {
                return Err("pinned text profiles need version 4 settings".into());
            }
            if settings
                .text
                .as_ref()
                .is_some_and(|lane| lane.profile.is_some() && lane.engine == TextEngine::Openai)
                && settings.schema != NATIVE_PROFILE_SCHEMA
            {
                return Err("native text profiles need version 5 settings".into());
            }
            if settings.group.is_some() && settings.schema != GROUP_SCHEMA {
                return Err("model groups need version 6 settings".into());
            }
            if settings.group.is_some()
                && (settings.decide.is_some() || settings.text.is_some() || settings.slots != 1)
            {
                return Err("a group head serves only its group, with one slot".into());
            }
            settings.schema = settings_schema(&settings).into();
            Ok(Some(settings))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

fn settings_schema(settings: &Settings) -> &'static str {
    if settings.group.is_some() {
        return GROUP_SCHEMA;
    }
    match settings.text.as_ref() {
        Some(lane) if lane.profile.is_some() && lane.engine == TextEngine::Openai => {
            NATIVE_PROFILE_SCHEMA
        }
        Some(lane) if lane.profile.is_some() => PROFILE_SCHEMA,
        Some(lane) if lane.serial.is_some() => SERIAL_SCHEMA,
        _ => SCHEMA,
    }
}

/// Store `settings`, replacing the file atomically.
///
/// # Errors
///
/// When the file cannot be written.
pub fn save(home: &Path, settings: &Settings) -> Result<(), String> {
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let tmp = home.join("share.json.tmp");
    let mut settings = settings.clone();
    settings.schema = settings_schema(&settings).into();
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(&settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path(home)).map_err(|e| e.to_string())
}

/// Usage text for `openagents host share`.
pub const USAGE: &str = "\
usage: openagents host share on|off|status [options]

Share this computer's model with the pool as a NIP-PYLON pylon. Off by
default. The running host starts the pylon within seconds of `on` and stops
it, with an offline beacon, on `off`. Pool jobs take a `pylon` lease at
background priority, so this computer's own work goes first. Free jobs only.

  on                        Turn sharing on, keeping earlier choices.
      --engine URL          The Psionic server (default http://127.0.0.1:18080).
      --model NAME          The served model (default qwen3.5-0.8b-q8_0).
      --pylon SLUG          The beacon's name (default: the host name).
      --label TEXT          Display label (default: the slug).
      --slots N             Concurrent jobs (default 2).
      --allow NPUB          A key that may send jobs; repeat or comma-separate.
      --allow-any           Admit any key (rate limits still apply).
      --rate N              Jobs per buyer per minute (default 10).
      --max-tokens N        Output bound per job (default 512).
      --vram-gb N           GPU memory for the beacon's class (default 16).
      --pool SLUG           Pool to ask to join (default everglade).
      --relay URL           The relay (default wss://relay.openagents.com).
  off                       Turn sharing off.
  status                    Print the setting, the pylon key, and its owner link.
  --json                    One JSON document on standard output.";

/// Run `openagents host share WORDS` with the pylon home `home` and the
/// host name `host`. Returns the exit code.
#[must_use]
pub fn command(words: &[String], home: &Path, host: &str) -> u8 {
    let mut words = words.to_vec();
    let json_out = take_flag(&mut words, "--json");
    let result = match words.first().map(String::as_str) {
        Some("on") => turn_on(&words[1..], home, host),
        Some("off") => load(home).and_then(|held| {
            let mut settings = held.unwrap_or_else(|| Settings::off(host));
            settings.on = false;
            settings.changed_at = now();
            save(home, &settings).map(|()| settings)
        }),
        Some("status") => load(home).map(|held| held.unwrap_or_else(|| Settings::off(host))),
        Some("help" | "--help") | None => {
            println!("{USAGE}");
            return if words.is_empty() { 2 } else { 0 };
        }
        Some(other) => Err(format!("unknown share command `{other}`\n\n{USAGE}")),
    };
    match result.and_then(|settings| describe(home, &settings)) {
        Ok((value, text)) => {
            if json_out {
                println!("{value}");
            } else {
                println!("{text}");
            }
            0
        }
        Err(e) => {
            if json_out {
                println!("{}", json!({"ok": false, "error": e}));
            } else {
                eprintln!("openagents host share: {e}");
            }
            1
        }
    }
}

pub(crate) fn take_flag(words: &mut Vec<String>, name: &str) -> bool {
    let before = words.len();
    words.retain(|w| w != name);
    words.len() != before
}

pub(crate) fn take_values(words: &mut Vec<String>, name: &str) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    while let Some(i) = words.iter().position(|w| w == name) {
        if i + 1 >= words.len() {
            return Err(format!("{name} needs a value"));
        }
        out.push(words.remove(i + 1));
        words.remove(i);
    }
    Ok(out)
}

pub(crate) fn take_number(words: &mut Vec<String>, name: &str) -> Result<Option<u32>, String> {
    take_values(words, name)?
        .pop()
        .map(|v| v.parse().map_err(|_| format!("{name} takes a number")))
        .transpose()
}

fn turn_on(words: &[String], home: &Path, host: &str) -> Result<Settings, String> {
    let mut words = words.to_vec();
    let mut settings = load(home)?.unwrap_or_else(|| Settings::off(host));
    apply(&mut settings, &mut words)?;
    if let Some(extra) = words.first() {
        return Err(format!("unexpected argument `{extra}`"));
    }
    if settings.allow.as_ref().is_some_and(BTreeSet::is_empty) {
        return Err("name who may send jobs with --allow NPUB, or pass --allow-any".into());
    }
    settings.on = true;
    settings.changed_at = now();
    save(home, &settings)?;
    Ok(settings)
}

/// Take `serve`'s options (`--engine`, `--model`, `--pylon`, `--label`,
/// `--pool`, `--relay`, `--slots`, `--rate`, `--max-tokens`, `--vram-gb`,
/// `--allow`, `--allow-any`) out of `words` into `settings`.
pub(crate) fn apply(settings: &mut Settings, words: &mut Vec<String>) -> Result<(), String> {
    for (flag, field) in [
        ("--engine", &mut settings.engine),
        ("--model", &mut settings.model),
        ("--label", &mut settings.label),
        ("--pool", &mut settings.pool),
        ("--relay", &mut settings.relay),
    ] {
        if let Some(value) = take_values(words, flag)?.pop() {
            *field = value;
        }
    }
    if let Some(slug) = take_values(words, "--pylon")?.pop() {
        if settings.label == settings.pylon {
            settings.label.clone_from(&slug);
        }
        settings.pylon = slug;
    }
    for (flag, field) in [
        ("--slots", &mut settings.slots),
        ("--rate", &mut settings.rate_per_minute),
        ("--max-tokens", &mut settings.max_tokens),
        ("--vram-gb", &mut settings.vram_gb),
    ] {
        if let Some(value) = take_number(words, flag)? {
            *field = value;
        }
    }
    if take_flag(words, "--allow-any") {
        settings.allow = None;
    }
    let mut added = BTreeSet::new();
    for value in take_values(words, "--allow")? {
        for item in value.split(',').filter(|s| !s.is_empty()) {
            added.insert(hex_pubkey(item.trim()).ok_or_else(|| format!("`{item}` is not a key"))?);
        }
    }
    if !added.is_empty() {
        settings
            .allow
            .get_or_insert_with(BTreeSet::new)
            .extend(added);
    }
    Ok(())
}

fn describe(home: &Path, settings: &Settings) -> Result<(Value, String), String> {
    let provider = Identity::load_or_create(&home.join("provider.key"))?;
    let owner = load_owner(home)?.map(|o| npub(&o.owner_pubkey));
    let allow = settings
        .allow
        .as_ref()
        .map(|keys| keys.iter().map(|k| npub(k)).collect::<Vec<_>>());
    let value = json!({
        "on": settings.on,
        "pylon": format!("30200:{}:{}", provider.pubkey(), settings.pylon),
        "npub": provider.npub(),
        "owner": owner,
        "relay": settings.relay,
        "engine": settings.engine,
        "model": settings.model,
        "slots": settings.slots,
        "pool": settings.pool,
        "allow": allow,
        "decide": settings.decide,
        "text": settings.text,
        "settings": path(home),
    });
    let text = format!(
        "sharing {}\npylon   {} ({})\nowner   {}\nmodel   {} at {}\nslots   {}, pool {}, relay {}\nadmits  {}",
        if settings.on { "on" } else { "off" },
        settings.pylon,
        provider.npub(),
        owner
            .as_deref()
            .unwrap_or("not linked (openagents pylon link)"),
        settings.model,
        settings.engine,
        settings.slots,
        settings.pool,
        settings.relay,
        allow.map_or_else(|| "any key".into(), |keys| keys.join(", ")),
    );
    Ok((value, text))
}

/// Serve the pool while `home`'s settings say sharing is on, on `machine`,
/// until `stop` resolves. Looks at the settings every `every`; a change
/// restarts the pylon under the new settings. With a decision or text lane
/// it also runs the Psionic engines and restarts a crashed one with
/// backoff ([`crate::online::supervise`]). Logs one line per change
/// through `log`.
pub async fn supervise(
    home: PathBuf,
    machine: Arc<dyn Machine>,
    every: Duration,
    log: impl Fn(String) + Send + Sync + 'static,
    stop: impl std::future::Future<Output = ()>,
) {
    crate::online::supervise(
        home,
        machine,
        crate::online::Options::host(every),
        Arc::new(log),
        stop,
    )
    .await;
}

/// The lease broker as the host's machine, or `None` with the reason when
/// the lease root can't be found.
///
/// # Errors
///
/// When the broker cannot be opened.
pub fn host_machine() -> Result<Arc<dyn Machine>, String> {
    // The user's own pause choices (on battery, hot) drain it too.
    Ok(Arc::new(crate::lease::Paused::new(
        Arc::new(Leases::from_env()?),
        &crate::home(),
    )))
}
