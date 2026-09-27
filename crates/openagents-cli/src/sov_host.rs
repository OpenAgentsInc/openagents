//! NIP-SOV, step two of its implementation order: a bounded local lifecycle
//! under a no-spend, explicitly trusted local custody profile. This machine
//! is the authority, the controller, the custodian, and the runtime host at
//! once, and the records say so.
//!
//! - `admit` authenticates the exact profile bytes with the authority key
//!   held in a local key profile. The record is a signed, unpublished
//!   `3188` event kept beside the profile: durable local host provenance.
//! - `spawn` binds `(agent, activation)` to the admitted profile digest, a
//!   finite budget, and controller generation 1, then starts the lifecycle
//!   in its own process. The agent presence in Verse is the visible effect.
//! - Each tick publishes the agent's NIP-MV state and writes a checkpoint
//!   revision; the plan ends at its budget, on `stop`, or when the relay
//!   refuses. Exhaustion is recorded as `budget`, never as success.
//!
//! Treasury is null, so nothing here can spend. No guardian policy is
//! supported: a profile that names one refuses.

use std::path::{Path, PathBuf};
use std::time::Duration;

use glam::{Quat, Vec3};
use nostr::domain::{Event, Tag};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::sov::{ArtifactRef, PROFILE_VERSION, Profile};
use crate::{Args, Output};

/// The private-artifact kind SOV records use (NIP-SOV, encoding).
pub const RECORD_KIND: u16 = 3188;
pub const ADMISSION_VERSION: &str = "openagents.sovereign-admission.v1";
pub const ACTIVATION_VERSION: &str = "openagents.sovereign-activation.v1";
pub const CHECKPOINT_VERSION: &str = "openagents.sovereign-checkpoint.v1";
/// The one custody adapter this host enforces: the agent key in a local
/// key profile, used only by this process, exportable by whoever can read
/// the file. The profile must say so.
pub const LOCAL_CUSTODY_ADAPTER: &str = "openagents.local-key.v1";
const MAX_BUDGET_SECONDS: u64 = 86_400;
const MAX_TICKS: u64 = 100_000;
const MIN_TICK: Duration = Duration::from_secs(1);
const RELAY_WAIT: Duration = Duration::from_secs(8);

/// Where the lifecycle records for NAME live.
pub struct Paths {
    pub profile: PathBuf,
    pub admission: PathBuf,
    pub activation: PathBuf,
    pub checkpoints: PathBuf,
    pub stop: PathBuf,
    pub log: PathBuf,
}

impl Paths {
    pub fn for_name(home: &Path, name: &str) -> Self {
        Self {
            profile: home.join(format!("{name}.profile.json")),
            admission: home.join(format!("{name}.admission.json")),
            activation: home.join(format!("{name}.activation.json")),
            checkpoints: home.join(format!("{name}.checkpoints")),
            stop: home.join(format!("{name}.stop")),
            log: home.join(format!("{name}.log")),
        }
    }
}

/// The exact profile bytes and their ArtifactRef.
pub fn profile_ref(bytes: &[u8]) -> ArtifactRef {
    let digest = Sha256::digest(bytes);
    ArtifactRef {
        digest: format!("sha256:{}", hex(&digest)),
        size: bytes.len() as u64,
        media_type: "application/json".to_owned(),
        schema: Some(PROFILE_VERSION.to_owned()),
        event: None,
        sources: None,
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The body of an admission record: the authority's statement that it
/// admitted these exact profile bytes for this agent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Admission {
    pub v: String,
    #[serde(default)]
    pub requires: Vec<String>,
    pub agent: String,
    pub authority: String,
    pub revision: u64,
    pub profile: ArtifactRef,
    pub custody_adapter: String,
    pub admitted_at: u64,
}

/// Builds and signs an admission of `bytes` by `signer`, which must hold
/// the profile's authority key.
pub fn admit(
    profile: &Profile,
    bytes: &[u8],
    signer: &nostr::domain::RelaySigner,
    now: u64,
) -> Result<Event, String> {
    profile.validate()?;
    if signer.pubkey() != profile.authority {
        return Err(format!(
            "the key {}… is not the profile's authority {}…; admission needs the authority's own key",
            &signer.pubkey()[..8],
            &profile.authority[..8]
        ));
    }
    let adapter = custody_adapter(profile)?;
    if profile.guardian_policy.is_some() {
        return Err("this host supports no guardian policy; the profile names one".into());
    }
    if profile.treasury.is_some() {
        return Err("this host is no-spend; the profile names a treasury".into());
    }
    let body = Admission {
        v: ADMISSION_VERSION.to_owned(),
        requires: Vec::new(),
        agent: profile.agent.clone(),
        authority: profile.authority.clone(),
        revision: profile.revision,
        profile: profile_ref(bytes),
        custody_adapter: adapter.to_owned(),
        admitted_at: now,
    };
    let content = serde_json::to_string(&body).map_err(|e| e.to_string())?;
    Ok(signer.sign(
        now,
        RECORD_KIND,
        vec![Tag::new(vec!["p".to_owned(), profile.agent.clone()])],
        content,
    ))
}

/// The custody adapter id the profile pins, if this host enforces it.
fn custody_adapter(profile: &Profile) -> Result<&str, String> {
    let id = profile.custody.adapter["id"].as_str().unwrap_or("");
    if id == LOCAL_CUSTODY_ADAPTER {
        Ok(id)
    } else {
        Err(format!(
            "custody adapter `{id}` is not supported; this host enforces only {LOCAL_CUSTODY_ADAPTER}"
        ))
    }
}

/// Reads and checks a stored admission against the current profile bytes:
/// signature, kind, agent, authority, and exact digest.
pub fn verify_admission(
    event: &Event,
    profile: &Profile,
    bytes: &[u8],
) -> Result<Admission, String> {
    event
        .validate_crypto()
        .map_err(|e| format!("admission signature does not verify: {e}"))?;
    if event.kind != RECORD_KIND {
        return Err(format!(
            "admission is kind {}, not {RECORD_KIND}",
            event.kind
        ));
    }
    let body: Admission = serde_json::from_str(&event.content)
        .map_err(|e| format!("admission does not parse: {e}"))?;
    if body.v != ADMISSION_VERSION {
        return Err(format!("admission version `{}` is unknown", body.v));
    }
    if !body.requires.is_empty() {
        return Err("admission requires features this host lacks".into());
    }
    if event.pubkey != profile.authority || body.authority != profile.authority {
        return Err("admission was not signed by the profile's authority".into());
    }
    if body.agent != profile.agent {
        return Err("admission names another agent".into());
    }
    let current = profile_ref(bytes);
    if body.profile.digest != current.digest || body.profile.size != current.size {
        return Err("the profile changed after admission; admit the new revision".to_owned());
    }
    Ok(body)
}

/// The finite budget a plan runs under.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Budget {
    pub seconds: u64,
    pub ticks: u64,
    pub tick_seconds: u64,
}

impl Budget {
    pub fn parse(args: &Args) -> Result<Self, String> {
        let seconds: u64 = args.number("seconds", 0)?;
        if seconds == 0 || seconds > MAX_BUDGET_SECONDS {
            return Err(format!(
                "--seconds N is required, 1 to {MAX_BUDGET_SECONDS}: the plan's finite lifetime"
            ));
        }
        let tick_seconds: u64 = args.number("tick", 5)?;
        if tick_seconds < MIN_TICK.as_secs() || tick_seconds > seconds {
            return Err("--tick is 1 to --seconds".into());
        }
        let ticks: u64 = args.number("ticks", seconds / tick_seconds)?;
        if ticks == 0 || ticks > MAX_TICKS {
            return Err(format!("--ticks is 1 to {MAX_TICKS}"));
        }
        Ok(Self {
            seconds,
            ticks,
            tick_seconds,
        })
    }
}

/// `openagents.sovereign-activation.v1`, with the local host's bindings.
/// The AUTO plan is the budget below; COORD and ENV are this process and
/// this machine, recorded as such rather than as separately admitted refs.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Activation {
    pub v: String,
    #[serde(default)]
    pub requires: Vec<String>,
    pub activation: String,
    pub agent: String,
    pub profile: ArtifactRef,
    pub admission: String,
    pub plan: Budget,
    pub controller: String,
    pub generation: u64,
    pub claim: String,
    pub environment: Value,
    pub checkpoint: Option<String>,
    pub admitted_at: u64,
    pub relay: String,
    pub world: String,
    pub entity: String,
    pub display_name: Option<String>,
    pub pid: Option<u32>,
    pub ended: Option<Ended>,
}

/// How a lifecycle ended: its budget, a stop, or a refusal.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ended {
    pub at: u64,
    pub reason: String,
    pub ticks: u64,
}

/// `openagents.sovereign-checkpoint.v1`: a projection of one tick.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    pub v: String,
    #[serde(default)]
    pub requires: Vec<String>,
    pub activation: String,
    pub revision: u64,
    pub previous: Option<String>,
    pub controller: String,
    pub generation: u64,
    pub record: Value,
    pub state: Value,
    pub obligations: Vec<Value>,
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path, what: &str) -> Result<T, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("no {what} at {}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{what} does not parse: {e}"))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, text).map_err(|e| format!("cannot write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("cannot write {}: {e}", path.display()))
}

/// The profile, its exact bytes, and its verified admission.
pub fn admitted(paths: &Paths) -> Result<(Profile, Vec<u8>, Admission, Event), String> {
    let bytes = std::fs::read(&paths.profile)
        .map_err(|e| format!("no profile at {}: {e}", paths.profile.display()))?;
    let profile: Profile =
        serde_json::from_slice(&bytes).map_err(|e| format!("profile does not parse: {e}"))?;
    profile.validate()?;
    let event: Event = read_json(&paths.admission, "admission (run `sov admit NAME` first)")?;
    let admission = verify_admission(&event, &profile, &bytes)?;
    Ok((profile, bytes, admission, event))
}

/// Whether the process an activation recorded is still alive.
pub fn alive(pid: Option<u32>) -> bool {
    match pid {
        Some(pid) if pid > 0 => Path::new(&format!("/proc/{pid}")).exists(),
        _ => false,
    }
}

/// `sov admit NAME [--as AUTHORITY_PROFILE]`.
pub fn admit_command(output: &Output, home: &Path, name: &str, args: &Args) -> Result<u8, String> {
    let paths = Paths::for_name(home, name);
    let bytes = std::fs::read(&paths.profile).map_err(|e| {
        format!(
            "no profile draft `{name}` at {}: {e}",
            paths.profile.display()
        )
    })?;
    let profile: Profile =
        serde_json::from_slice(&bytes).map_err(|e| format!("profile does not parse: {e}"))?;
    let signer = crate::relay::signer_for(args.option("as"))?;
    let event = admit(&profile, &bytes, &signer, crate::relay::unix_now())?;
    write_json(&paths.admission, &event)?;
    let body: Admission = serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
    output.emit(
        &json!({
            "name": name,
            "agent": body.agent,
            "authority": body.authority,
            "revision": body.revision,
            "profile": body.profile,
            "custody_adapter": body.custody_adapter,
            "admission": event.id,
            "path": paths.admission,
        }),
        |value| {
            format!(
                "admitted {} revision {} by {}… (record {}…)",
                value["agent"].as_str().unwrap_or(""),
                value["revision"],
                &value["authority"].as_str().unwrap_or("")[..8],
                &value["admission"].as_str().unwrap_or("")[..12],
            )
        },
    );
    Ok(0)
}

/// The checks `spawn` runs, in order, and the activation it would bind.
fn plan(home: &Path, name: &str, args: &Args) -> Result<(Vec<Value>, Option<Activation>), String> {
    let paths = Paths::for_name(home, name);
    let mut checks = Vec::new();
    let refusal = |check: &str, why: String| json!({ "check": check, "ok": false, "why": why });
    let bytes = match std::fs::read(&paths.profile) {
        Ok(bytes) => bytes,
        Err(e) => {
            checks.push(refusal(
                "profile-contract",
                format!("no profile draft `{name}`: {e}"),
            ));
            return Ok((checks, None));
        }
    };
    let profile: Profile = match serde_json::from_slice(&bytes)
        .map_err(|e| e.to_string())
        .and_then(|p: Profile| p.validate().map(|()| p))
    {
        Ok(profile) => profile,
        Err(e) => {
            checks.push(refusal("profile-contract", e));
            return Ok((checks, None));
        }
    };
    checks.push(json!({ "check": "profile-contract", "ok": true }));
    let admission = match read_json::<Event>(&paths.admission, "admission")
        .and_then(|event| verify_admission(&event, &profile, &bytes).map(|a| (a, event.id)))
    {
        Ok(admission) => {
            checks
                .push(json!({ "check": "admitting-authority", "ok": true, "record": admission.1 }));
            admission
        }
        Err(e) => {
            checks.push(refusal(
                "admitting-authority",
                format!("{e}; run `openagents sov admit {name} --as AUTHORITY_PROFILE`"),
            ));
            return Ok((checks, None));
        }
    };
    let key_profile = args.option("key").unwrap_or(name);
    let identity = match verse::identity::load_or_create(&verse::identity::home(), key_profile) {
        Ok(identity) if identity.signer.pubkey() == profile.agent => identity,
        Ok(identity) => {
            checks.push(refusal(
                "custody-adapter",
                format!(
                    "key profile `{key_profile}` holds {}…, not the agent {}… (pass --key PROFILE)",
                    &identity.signer.pubkey()[..8],
                    &profile.agent[..8]
                ),
            ));
            return Ok((checks, None));
        }
        Err(e) => {
            checks.push(refusal("custody-adapter", e));
            return Ok((checks, None));
        }
    };
    checks.push(json!({ "check": "custody-adapter", "ok": true, "adapter": LOCAL_CUSTODY_ADAPTER, "key_profile": identity.profile }));
    checks.push(json!({ "check": "policy-store", "ok": true, "why": "no-spend, no guardian: the admitted profile's null treasury and guardian policy are the whole policy" }));
    let budget = match Budget::parse(args) {
        Ok(budget) => budget,
        Err(e) => {
            checks.push(refusal("plan-budget", e));
            return Ok((checks, None));
        }
    };
    checks.push(json!({ "check": "plan-budget", "ok": true, "plan": budget }));
    if let Ok(current) = read_json::<Activation>(&paths.activation, "activation")
        && current.ended.is_none()
        && alive(current.pid)
    {
        checks.push(refusal(
            "controller",
            format!(
                "activation {} is still running (pid {}); one lifecycle per agent, `sov stop {name}` first",
                current.activation,
                current.pid.unwrap_or(0)
            ),
        ));
        return Ok((checks, None));
    }
    checks.push(json!({ "check": "controller", "ok": true, "why": "this process is the controller at generation 1; the activation file is the exclusive claim" }));
    checks.push(json!({ "check": "environment", "ok": true, "why": "this machine" }));
    checks.push(json!({ "check": "checkpoints", "ok": true, "path": paths.checkpoints }));
    let now = crate::relay::unix_now();
    let display_name = match args.option("name") {
        Some(text) if text.is_empty() || text.len() > 64 => {
            return Err("--name is 1 to 64 bytes".into());
        }
        Some(text) => Some(text.to_owned()),
        None => None,
    };
    let activation = Activation {
        v: ACTIVATION_VERSION.to_owned(),
        requires: Vec::new(),
        activation: verse::identity::random_hex(16),
        agent: profile.agent.clone(),
        profile: admission.0.profile,
        admission: admission.1,
        plan: budget,
        controller: admission.0.authority,
        generation: 1,
        claim: format!("file:{}", paths.activation.display()),
        environment: json!({ "lease": "local", "materialization": "this machine" }),
        checkpoint: None,
        admitted_at: now,
        relay: crate::relay::relay_url(args.option("relay")),
        world: args
            .option("world")
            .map(str::to_owned)
            .unwrap_or_else(|| verse::session::WORLD.to_owned()),
        entity: "avatar".to_owned(),
        display_name,
        pid: None,
        ended: None,
    };
    Ok((checks, Some(activation)))
}

/// `sov spawn NAME --seconds N [--ticks N] [--tick S] [--key PROFILE] [--name TEXT] [--relay URL] [--foreground]`.
pub fn spawn_command(output: &Output, home: &Path, name: &str, args: &Args) -> Result<u8, String> {
    let paths = Paths::for_name(home, name);
    let (checks, activation) = plan(home, name, args)?;
    let Some(mut activation) = activation else {
        let refused = checks
            .iter()
            .find(|c| !c["ok"].as_bool().unwrap_or(false))
            .cloned();
        output.emit(
            &json!({ "name": name, "activated": false, "refused_by": refused.as_ref().map(|c| c["check"].clone()), "checks": checks }),
            |value| format!("{}\nspawn refused: nothing was started", render_checks(&value["checks"])),
        );
        return Ok(crate::EXIT_FAILURE);
    };
    std::fs::create_dir_all(&paths.checkpoints).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&paths.stop);
    if args.switch("foreground") {
        activation.pid = Some(std::process::id());
        write_json(&paths.activation, &activation)?;
        let ended = lifecycle(&paths, &activation, args.option("key").unwrap_or(name))?;
        output.emit(
            &json!({ "name": name, "activated": true, "activation": activation.activation, "ended": ended, "checks": checks }),
            |value| format!("{}\nended: {} after {} ticks", render_checks(&value["checks"]), value["ended"]["reason"].as_str().unwrap_or(""), value["ended"]["ticks"]),
        );
        return Ok(0);
    }
    write_json(&paths.activation, &activation)?;
    let exe = std::env::current_exe().map_err(|e| format!("cannot find this program: {e}"))?;
    let log = std::fs::File::create(&paths.log)
        .map_err(|e| format!("cannot open {}: {e}", paths.log.display()))?;
    let mut command = std::process::Command::new(exe);
    command
        .arg("--json")
        .args(["sov", "host", "run", name])
        .arg("--key")
        .arg(args.option("key").unwrap_or(name))
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone().map_err(|e| e.to_string())?)
        .stderr(log);
    detach(&mut command);
    let child = command
        .spawn()
        .map_err(|e| format!("cannot start the lifecycle: {e}"))?;
    activation.pid = Some(child.id());
    write_json(&paths.activation, &activation)?;
    output.emit(
        &json!({
            "name": name,
            "activated": true,
            "activation": activation.activation,
            "agent": activation.agent,
            "pid": activation.pid,
            "plan": activation.plan,
            "relay": activation.relay,
            "world": activation.world,
            "log": paths.log,
            "checks": checks,
        }),
        |value| {
            format!(
                "{}\nactivated {} as {}… (pid {}) for {} s; `openagents sov status {}` follows it",
                render_checks(&value["checks"]),
                value["activation"].as_str().unwrap_or(""),
                &value["agent"].as_str().unwrap_or("")[..8],
                value["pid"],
                value["plan"]["seconds"],
                value["name"].as_str().unwrap_or(""),
            )
        },
    );
    Ok(0)
}

#[cfg(unix)]
fn detach(command: &mut std::process::Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

#[cfg(not(unix))]
fn detach(_: &mut std::process::Command) {}

fn render_checks(checks: &Value) -> String {
    checks
        .as_array()
        .into_iter()
        .flatten()
        .map(|check| {
            format!(
                "{} {}{}",
                if check["ok"].as_bool().unwrap_or(false) {
                    "ok     "
                } else {
                    "refused"
                },
                check["check"].as_str().unwrap_or(""),
                check["why"]
                    .as_str()
                    .map(|w| format!(": {w}"))
                    .unwrap_or_default()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `sov host run NAME --key PROFILE`: the lifecycle process itself.
pub fn run_command(output: &Output, home: &Path, name: &str, args: &Args) -> Result<u8, String> {
    let paths = Paths::for_name(home, name);
    let activation: Activation = read_json(&paths.activation, "activation")?;
    if activation.ended.is_some() {
        return Err("this activation already ended".into());
    }
    let ended = lifecycle(&paths, &activation, args.option("key").unwrap_or(name))?;
    output.emit(
        &json!({ "name": name, "activation": activation.activation, "ended": ended }),
        |value| {
            format!(
                "ended: {} after {} ticks",
                value["ended"]["reason"].as_str().unwrap_or(""),
                value["ended"]["ticks"]
            )
        },
    );
    Ok(0)
}

/// Runs the plan to its end and records how it ended.
fn lifecycle(paths: &Paths, activation: &Activation, key_profile: &str) -> Result<Ended, String> {
    let (profile, _, admission, _) = admitted(paths)?;
    if admission.profile.digest != activation.profile.digest {
        return Err("the admitted profile is not the one this activation bound".into());
    }
    let identity = verse::identity::load_or_create(&verse::identity::home(), key_profile)?;
    if identity.signer.pubkey() != profile.agent {
        return Err("the key profile no longer holds the agent key".into());
    }
    let mut client = crate::relay::Client::connect(&activation.relay, identity.signer.clone());
    let deadline = std::time::Instant::now() + Duration::from_secs(activation.plan.seconds);
    let tick = Duration::from_secs(activation.plan.tick_seconds);
    let pos = Vec3::new(0.0, 0.0, 0.0);
    let rot = Quat::IDENTITY;
    let mut previous: Option<String> = None;
    let mut ticks = 0;
    let reason = loop {
        if paths.stop.exists() {
            break "stop";
        }
        if ticks >= activation.plan.ticks {
            break "budget";
        }
        if std::time::Instant::now() >= deadline {
            break "budget";
        }
        let now = crate::relay::unix_now();
        let state = verse::mv::State {
            v: 1,
            id: activation.entity.clone(),
            role: "agent".to_owned(),
            p: pos.to_array(),
            q: rot.to_array(),
            t: now * 1000,
            online: true,
            follows: None,
            name: activation.display_name.clone(),
        };
        let event = verse::mv::state_event(&identity.signer, &activation.world, &state, now);
        let published = client.publish(event, RELAY_WAIT)?;
        if !published.accepted {
            let _ = checkpoint(
                paths,
                activation,
                ticks,
                &previous,
                json!({ "refused": published.message }),
                &state,
            );
            break "relay-refused";
        }
        ticks += 1;
        previous = Some(checkpoint(
            paths,
            activation,
            ticks,
            &previous,
            json!({ "event": published.id, "accepted": true }),
            &state,
        )?);
        std::thread::sleep(tick);
    };
    let now = crate::relay::unix_now();
    let offline = verse::mv::State {
        v: 1,
        id: activation.entity.clone(),
        role: "agent".to_owned(),
        p: pos.to_array(),
        q: rot.to_array(),
        t: now * 1000,
        online: false,
        follows: None,
        name: activation.display_name.clone(),
    };
    let _ = client.publish(
        verse::mv::state_event(&identity.signer, &activation.world, &offline, now),
        RELAY_WAIT,
    );
    client.close();
    let ended = Ended {
        at: now,
        reason: reason.to_owned(),
        ticks,
    };
    let mut record: Activation = read_json(&paths.activation, "activation")?;
    record.ended = Some(ended.clone());
    record.checkpoint = previous;
    write_json(&paths.activation, &record)?;
    let _ = std::fs::remove_file(&paths.stop);
    Ok(ended)
}

/// Writes checkpoint `revision` and returns its digest.
fn checkpoint(
    paths: &Paths,
    activation: &Activation,
    revision: u64,
    previous: &Option<String>,
    record: Value,
    state: &verse::mv::State,
) -> Result<String, String> {
    let body = Checkpoint {
        v: CHECKPOINT_VERSION.to_owned(),
        requires: Vec::new(),
        activation: activation.activation.clone(),
        revision,
        previous: previous.clone(),
        controller: activation.controller.clone(),
        generation: activation.generation,
        record,
        state: json!({ "presence": state, "at": crate::relay::unix_now() }),
        obligations: Vec::new(),
    };
    let text = serde_json::to_string_pretty(&body).map_err(|e| e.to_string())?;
    let digest = format!("sha256:{}", hex(&Sha256::digest(text.as_bytes())));
    let path = paths.checkpoints.join(format!("{revision:06}.json"));
    std::fs::write(&path, text).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    Ok(digest)
}

/// `sov stop NAME`: asks the lifecycle to end at its next tick and waits
/// for the process to go.
pub fn stop_command(output: &Output, home: &Path, name: &str, args: &Args) -> Result<u8, String> {
    let paths = Paths::for_name(home, name);
    let mut activation: Activation = read_json(&paths.activation, "activation")?;
    if activation.ended.is_some() {
        return Err(format!(
            "activation {} already ended: {}",
            activation.activation,
            activation
                .ended
                .as_ref()
                .map(|e| e.reason.as_str())
                .unwrap_or("")
        ));
    }
    std::fs::write(&paths.stop, b"stop\n").map_err(|e| e.to_string())?;
    let wait: u64 = args.number(
        "timeout",
        activation.plan.tick_seconds + RELAY_WAIT.as_secs() + 2,
    )?;
    let deadline = std::time::Instant::now() + Duration::from_secs(wait);
    let stopped = loop {
        activation = read_json(&paths.activation, "activation")?;
        if activation.ended.is_some() {
            break true;
        }
        if !alive(activation.pid) {
            break false;
        }
        if std::time::Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(Duration::from_millis(250));
    };
    if !stopped && !alive(activation.pid) && activation.ended.is_none() {
        activation.ended = Some(Ended {
            at: crate::relay::unix_now(),
            reason: "process-gone".to_owned(),
            ticks: count_checkpoints(&paths.checkpoints),
        });
        write_json(&paths.activation, &activation)?;
    }
    output.emit(
        &json!({ "name": name, "activation": activation.activation, "stopped": activation.ended.is_some(), "ended": activation.ended, "pid": activation.pid, "alive": alive(activation.pid) }),
        |value| {
            if value["stopped"].as_bool().unwrap_or(false) {
                format!("stopped: {} after {} ticks", value["ended"]["reason"].as_str().unwrap_or(""), value["ended"]["ticks"])
            } else {
                format!("stop requested; the lifecycle (pid {}) has not acknowledged yet", value["pid"])
            }
        },
    );
    Ok(if activation.ended.is_some() {
        0
    } else {
        crate::EXIT_FAILURE
    })
}

fn count_checkpoints(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .map(|entries| entries.count() as u64)
        .unwrap_or(0)
}

/// The lifecycle records for NAME as one value.
pub fn status_value(home: &Path, name: &str) -> Value {
    let paths = Paths::for_name(home, name);
    let admitted = admitted(&paths);
    let activation = read_json::<Activation>(&paths.activation, "activation").ok();
    let running = activation
        .as_ref()
        .is_some_and(|a| a.ended.is_none() && alive(a.pid));
    json!({
        "name": name,
        "admitted": admitted.as_ref().ok().map(|(_, _, a, e)| json!({ "authority": a.authority, "revision": a.revision, "record": e.id, "profile": a.profile.digest })),
        "admission_error": admitted.as_ref().err(),
        "activation": activation,
        "running": running,
        "checkpoints": count_checkpoints(&paths.checkpoints),
    })
}

/// The names that have any lifecycle record.
pub fn names(home: &Path) -> Vec<String> {
    let mut names = std::collections::BTreeSet::new();
    if let Ok(entries) = std::fs::read_dir(home) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().into_owned();
            for suffix in [".profile.json", ".admission.json", ".activation.json"] {
                if let Some(name) = file.strip_suffix(suffix) {
                    names.insert(name.to_owned());
                }
            }
        }
    }
    names.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nostr::domain::RelaySigner;

    fn reference(media: &str) -> ArtifactRef {
        ArtifactRef {
            digest: format!("sha256:{}", "ab".repeat(32)),
            size: 1,
            media_type: media.to_owned(),
            schema: None,
            event: None,
            sources: None,
        }
    }

    fn keys() -> (RelaySigner, RelaySigner) {
        let a = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let b = RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
        (a, b)
    }

    fn profile(agent: &str, authority: &str, adapter: &str) -> Profile {
        Profile {
            v: PROFILE_VERSION.to_owned(),
            requires: Vec::new(),
            meta: None,
            agent: agent.to_owned(),
            authority: authority.to_owned(),
            revision: 0,
            previous: None,
            policy: reference("application/json"),
            custody: crate::sov::Custody {
                adapter: json!({ "id": adapter, "artifact": reference("application/json") }),
                policy: reference("application/json"),
            },
            guardian_policy: None,
            treasury: None,
            state_schema: reference("application/schema+json"),
            disclosure: reference("application/json"),
            evidence: Vec::new(),
        }
    }

    #[test]
    fn admission_needs_the_authority_key_and_the_local_adapter() {
        let (agent, authority) = keys();
        let p = profile(agent.pubkey(), authority.pubkey(), LOCAL_CUSTODY_ADAPTER);
        let bytes = serde_json::to_vec(&p).unwrap();
        let error = admit(&p, &bytes, &agent, 1).unwrap_err();
        assert!(error.contains("not the profile's authority"), "{error}");
        let other = profile(agent.pubkey(), authority.pubkey(), "nip46");
        let error = admit(&other, &serde_json::to_vec(&other).unwrap(), &authority, 1).unwrap_err();
        assert!(error.contains("not supported"), "{error}");
        let mut treasury = p.clone();
        treasury.treasury = Some(reference("application/json"));
        let error = admit(&treasury, &bytes, &authority, 1).unwrap_err();
        assert!(error.contains("no-spend"), "{error}");
        let event = admit(&p, &bytes, &authority, 1).unwrap();
        assert_eq!(event.kind, RECORD_KIND);
        let body = verify_admission(&event, &p, &bytes).unwrap();
        assert_eq!(body.agent, agent.pubkey());
        assert_eq!(body.profile, profile_ref(&bytes));
    }

    #[test]
    fn admission_binds_the_exact_bytes() {
        let (agent, authority) = keys();
        let p = profile(agent.pubkey(), authority.pubkey(), LOCAL_CUSTODY_ADAPTER);
        let bytes = serde_json::to_vec(&p).unwrap();
        let event = admit(&p, &bytes, &authority, 1).unwrap();
        let mut changed = bytes.clone();
        changed.push(b'\n');
        let error = verify_admission(&event, &p, &changed).unwrap_err();
        assert!(error.contains("changed after admission"), "{error}");
        let mut forged = event.clone();
        forged.content = forged.content.replace(agent.pubkey(), authority.pubkey());
        assert!(verify_admission(&forged, &p, &bytes).is_err());
    }

    #[test]
    fn budget_is_finite_and_explicit() {
        let parse = |words: &[&str]| {
            let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
            Budget::parse(&Args::parse(&words, &[]).unwrap())
        };
        assert!(parse(&[]).unwrap_err().contains("--seconds"));
        assert!(parse(&["--seconds", "0"]).is_err());
        assert!(parse(&["--seconds", "100000"]).is_err());
        let budget = parse(&["--seconds", "60"]).unwrap();
        assert_eq!(
            budget,
            Budget {
                seconds: 60,
                ticks: 12,
                tick_seconds: 5
            }
        );
        assert!(parse(&["--seconds", "60", "--tick", "61"]).is_err());
    }

    #[test]
    fn stop_and_checkpoints_use_the_name_paths() {
        let home =
            std::env::temp_dir().join(format!("sov-host-{}", verse::identity::random_hex(4)));
        std::fs::create_dir_all(&home).unwrap();
        let paths = Paths::for_name(&home, "luna");
        assert!(paths.stop.ends_with("luna.stop"));
        assert!(names(&home).is_empty());
        std::fs::write(&paths.profile, b"{}").unwrap();
        assert_eq!(names(&home), vec!["luna".to_owned()]);
        let status = status_value(&home, "luna");
        assert_eq!(status["running"], false);
        assert!(status["admission_error"].is_string());
        std::fs::remove_dir_all(&home).unwrap();
    }
}
