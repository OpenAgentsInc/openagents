//! Shared compute as a setting of the Coder host: `openagents host share
//! on|off|status` records the choice in `share.json` in the pylon home,
//! and the running host's [`supervise`] task starts the provider when it
//! is on and stops it (with an offline beacon) when it is turned off.
//!
//! Sharing is off by default and takes an explicit owner action. Pool jobs
//! run under the lease broker at `background` priority ([`crate::lease`]),
//! so the owner's own work comes first.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use nostr::pylon::Tier;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::oneshot;

use crate::engine::Psionic;
use crate::identity::{Identity, hex_pubkey, load_owner, npub};
use crate::lease::{Leases, Machine};
use crate::provider::{Config, Provider};
use crate::{DEFAULT_RELAY, now};

/// The settings file's schema.
pub const SCHEMA: &str = "openagents.pylon.share.v1";

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
        Ok(bytes) => serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|e| format!("{}: {e}", path(home).display())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
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
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?,
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

fn take_flag(words: &mut Vec<String>, name: &str) -> bool {
    let before = words.len();
    words.retain(|w| w != name);
    words.len() != before
}

fn take_values(words: &mut Vec<String>, name: &str) -> Result<Vec<String>, String> {
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

fn take_number(words: &mut Vec<String>, name: &str) -> Result<Option<u32>, String> {
    take_values(words, name)?
        .pop()
        .map(|v| v.parse().map_err(|_| format!("{name} takes a number")))
        .transpose()
}

fn turn_on(words: &[String], home: &Path, host: &str) -> Result<Settings, String> {
    let mut words = words.to_vec();
    let mut settings = load(home)?.unwrap_or_else(|| Settings::off(host));
    for (flag, field) in [
        ("--engine", &mut settings.engine),
        ("--model", &mut settings.model),
        ("--label", &mut settings.label),
        ("--pool", &mut settings.pool),
        ("--relay", &mut settings.relay),
    ] {
        if let Some(value) = take_values(&mut words, flag)?.pop() {
            *field = value;
        }
    }
    if let Some(slug) = take_values(&mut words, "--pylon")?.pop() {
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
        if let Some(value) = take_number(&mut words, flag)? {
            *field = value;
        }
    }
    if take_flag(&mut words, "--allow-any") {
        settings.allow = None;
    }
    let mut added = BTreeSet::new();
    for value in take_values(&mut words, "--allow")? {
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
/// restarts the provider under the new settings. Logs one line per
/// change through `log`.
pub async fn supervise(
    home: PathBuf,
    machine: Arc<dyn Machine>,
    every: Duration,
    log: impl Fn(String) + Send + Sync + 'static,
    stop: impl std::future::Future<Output = ()>,
) {
    let mut running: Option<(Settings, oneshot::Sender<()>, tokio::task::JoinHandle<()>)> = None;
    tokio::pin!(stop);
    loop {
        let wanted = match load(&home) {
            Ok(settings) => settings.filter(|s| s.on),
            Err(e) => {
                log(format!("shared compute settings: {e}"));
                None
            }
        };
        let same = match (&running, &wanted) {
            (Some((held, _, task)), Some(want)) => held == want && !task.is_finished(),
            (None, None) => true,
            _ => false,
        };
        if !same {
            if let Some((_, halt, task)) = running.take() {
                let _ = halt.send(());
                let _ = task.await;
                log("shared compute off".into());
            }
            if let Some(settings) = wanted {
                match start(&home, &settings, Arc::clone(&machine)) {
                    Ok((halt, task, npub)) => {
                        log(format!(
                            "shared compute on: pylon {} ({npub}) on {}",
                            settings.pylon, settings.relay
                        ));
                        running = Some((settings, halt, task));
                    }
                    Err(e) => log(format!("shared compute could not start: {e}")),
                }
            }
        }
        tokio::select! {
            () = &mut stop => break,
            () = tokio::time::sleep(every) => {}
        }
    }
    if let Some((_, halt, task)) = running {
        let _ = halt.send(());
        let _ = task.await;
    }
}

type Started = (oneshot::Sender<()>, tokio::task::JoinHandle<()>, String);

fn start(home: &Path, settings: &Settings, machine: Arc<dyn Machine>) -> Result<Started, String> {
    let mut config = settings.config(home);
    config.owner = load_owner(home)?;
    let engine = Arc::new(Psionic::new(&settings.engine, &settings.model)?);
    let identity = Identity::load_or_create(&home.join("provider.key"))?;
    let npub = identity.npub();
    let provider = Provider::on(config, identity, engine, machine)?;
    let (halt, halted) = oneshot::channel::<()>();
    let task = tokio::spawn(async move {
        if let Err(e) = provider
            .run(async {
                let _ = halted.await;
            })
            .await
        {
            eprintln!("pylon: {e}");
        }
    });
    Ok((halt, task, npub))
}

/// The lease broker as the host's machine, or `None` with the reason when
/// the lease root can't be found.
///
/// # Errors
///
/// When the broker cannot be opened.
pub fn host_machine() -> Result<Arc<dyn Machine>, String> {
    Ok(Arc::new(Leases::from_env()?))
}
