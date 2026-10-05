//! The authoritative chamber: the `verse-world` service that runs combat,
//! movement, items, and quests for a scene at 30 Hz. `host` runs one from
//! its configuration on this machine, behind TLS and enrolled keys, or over
//! a NIP-REACH channel admitted by the Coder host's NIP-HOST `world` grants.
//! The other commands connect to one as a player or spectator and read or
//! drive it over its wire. Over TLS the key is an `openagents key` profile
//! the host enrolls; over REACH it is this device's key from the computers
//! store, with the grant the Coder host signed for it.

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use serde::Deserialize;
use serde_json::{Value, json};
use verse_world::service::client::Client;
use verse_world::service::wire::{Life, Reply, Response, State};
use verse_world::service::{host, net, persistence::Store, reach};
use verse_world::{Intent, play::Ability};

use crate::{Args, Output, argv::parse_command};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents chamber COMMAND [OPTIONS]
  host CONFIG.json [--state DIR] [--root DIR] [--keys DIR | --keychain]
       [--label TEXT] [--loopback-test]
                            Run a chamber host on this machine until stopped:
                            the configuration names the scene, pack, and
                            transport. Over TLS (the default) it also names
                            the certificate and key (DER) and enrolled keys.
                            Over REACH (transport type `reach`) the
                            Coder host's key and access store admit devices
                            with the `world` right; the access options are
                            the ones `openagents host serve` takes, and
                            --label names the instance in the owner
                            directory when the keys include the owner key.
  service install CONFIG.json [--binary PATH] [--state DIR] [--root DIR]
                            [--keys DIR | --keychain] [--label TEXT]
                            Run `chamber host CONFIG.json` from login on as
                            a launchd agent (macOS) or systemd user unit
                            (Linux); the access options pass through.
  service uninstall | status
                            Remove the unit, or report whether it runs.
  tls DIR [--name NAME]     Write a self-signed TLS certificate and key for a
                            host under DIR (cert.der, key.der; NAME defaults
                            to localhost).
  pack DIR                  Compile the original ritual asset pack under DIR
                            (runtime-pack.json) for a host and its clients.
  status                    This identity's admission, the host tick, the
                            living actors, and the population (players
                            present, guest count and cap when public).
  snapshot                  The full authoritative state the host serves.
  events [--after SERIAL] [--limit N]
                            Committed authority events: dialogue, damage,
                            deaths, respawns.
  watch [--wait SECONDS] [--hz N]
                            Follow snapshots and events as NDJSON until the
                            wait ends.
  move X,Z [--yaw DEGREES] [--ticks N]
                            Walk the owned adventurer along the axes (-1..1,
                            forward is Z) for N ticks (default 30).
  jump                      Jump once.
  cast ABILITY [--target ACTOR] [--aim X,Z]
                            Cast bow, fire-bolt, magic-missile, fireball,
                            misty-step, thunderwave, web, grease, light, or
                            shield at a hostile actor or along a horizontal
                            direction (default: forward, -Z). The host refuses
                            row-two catalog spells from remote players.
  respawn                   Return a dead adventurer to its spawn.
  inventory                 Items, quests, level, outfit, and equipment.
  use ITEM                  Use an inventory item by its ID.
  equip SLOT ITEM           Equip ITEM (head or main-hand), or an outfit with
                            `equip outfit ID`.
  quest accept ID --giver ACTOR
                            Accept a quest from a scene actor.
  quest claim ID            Claim a completed quest's reward.
Connection: --to HOST:PORT --instance N --trust CERT.der [--server-name NAME]
  [--content HEX] [--as PROFILE], or --chamber FILE with those fields as JSON
  (address, instance, trust_der, server_name, content or scene+pack+dir).
  The profile's key must be enrolled by the host as primary, player, or
  spectator; spectators read only.
  Over REACH: --to HOST:PORT --instance N --reach HOST [--websocket]
  [--store DIR] [--content HEX] (or reach, store, and websocket in the
  file). HOST is the Coder host's key, alias, or label; the device key and
  grant come from the computers store (default ~/.openagents/coder-computers),
  and the grant must hold the `world` right.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("host", Effect::LongRunning),
    Declared::computer("service", Effect::LocalWrite),
    Declared::computer("tls", Effect::LocalWrite),
    Declared::computer("pack", Effect::LocalWrite),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("snapshot", Effect::ReadOnly),
    Declared::computer("events", Effect::ReadOnly),
    Declared::computer("watch", Effect::LongRunning),
    Declared::computer("move", Effect::Publishes),
    Declared::computer("jump", Effect::Publishes),
    Declared::computer("cast", Effect::Publishes),
    Declared::computer("respawn", Effect::Publishes),
    Declared::computer("inventory", Effect::ReadOnly),
    Declared::computer("use", Effect::Publishes),
    Declared::computer("equip", Effect::Publishes),
    Declared::computer("quest accept", Effect::Publishes),
    Declared::computer("quest claim", Effect::Publishes),
];

const CONNECTION_OPTIONS: &[&str] = &[
    "to",
    "instance",
    "trust",
    "server-name",
    "content",
    "as",
    "chamber",
    "reach",
    "store",
];
const CONNECTION_SWITCHES: &[&str] = &["websocket"];
/// How long a REACH chamber's channel handshake may take.
const HANDSHAKE: Duration = Duration::from_secs(10);
const TICK: Duration = Duration::from_millis(33);

/// Where a chamber is and how to trust it, from `--chamber FILE` or flags.
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Connection {
    address: Option<SocketAddr>,
    instance: Option<u64>,
    trust_der: Option<PathBuf>,
    server_name: Option<String>,
    /// The content identity the host was configured with, as hex.
    content: Option<String>,
    scene: Option<PathBuf>,
    pack: Option<PathBuf>,
    dir: Option<PathBuf>,
    #[serde(rename = "as")]
    profile: Option<String>,
    /// The Coder host serving the chamber over a REACH channel: a key,
    /// alias, or label from the computers store.
    reach: Option<String>,
    /// The computers store holding this device's key and grants.
    store: Option<PathBuf>,
    /// Open the REACH channel over a WebSocket upgrade.
    #[serde(default)]
    websocket: bool,
}

impl Connection {
    fn from_args(args: &Args) -> Result<Self, String> {
        let mut connection = match args.option("chamber") {
            Some(path) => {
                let bytes = bounded(Path::new(path), 64 * 1024)?;
                serde_json::from_slice::<Self>(&bytes)
                    .map_err(|e| format!("{path}: invalid chamber connection file: {e}"))?
            }
            None => Self::default(),
        };
        if let Some(to) = args.option("to") {
            connection.address = Some(
                to.parse()
                    .map_err(|_| format!("--to {to}: not HOST:PORT"))?,
            );
        }
        if let Some(instance) = args.option("instance") {
            connection.instance = Some(
                instance
                    .parse()
                    .map_err(|_| format!("--instance {instance}: not a number"))?,
            );
        }
        if let Some(trust) = args.option("trust") {
            connection.trust_der = Some(PathBuf::from(trust));
        }
        if let Some(name) = args.option("server-name") {
            connection.server_name = Some(name.to_owned());
        }
        if let Some(content) = args.option("content") {
            connection.content = Some(content.to_owned());
        }
        if let Some(profile) = args.option("as") {
            connection.profile = Some(profile.to_owned());
        }
        if let Some(host) = args.option("reach") {
            connection.reach = Some(host.to_owned());
        }
        if let Some(store) = args.option("store") {
            connection.store = Some(PathBuf::from(store));
        }
        if args.switch("websocket") {
            connection.websocket = true;
        }
        Ok(connection)
    }

    fn content_identity(&self) -> Result<Option<[u8; 32]>, String> {
        if let Some(hex) = &self.content {
            let bytes = hex_bytes(hex)?;
            return <[u8; 32]>::try_from(bytes)
                .map(Some)
                .map_err(|_| "--content needs 64 hex digits".to_owned());
        }
        match (&self.scene, &self.pack) {
            (Some(scene), Some(pack)) => {
                let dir = self
                    .dir
                    .clone()
                    .or_else(|| pack.parent().map(Path::to_path_buf))
                    .unwrap_or_else(|| PathBuf::from("."));
                let pack = verse_engine::assets::Pack::read(pack)?;
                let scene =
                    verse_engine::director::Scene::from_json(&bounded(scene, 1024 * 1024)?)?;
                verse::imported::remote_content::identity(&pack, &scene, &dir).map(Some)
            }
            (None, None) => Ok(None),
            _ => Err("scene and pack go together".into()),
        }
    }

    async fn connect(&self) -> Result<Client, String> {
        let address = self
            .address
            .ok_or("--to HOST:PORT (or address in --chamber FILE) is required")?;
        let instance = self
            .instance
            .ok_or("--instance N (or instance in --chamber FILE) is required")?;
        if let Some(host) = &self.reach {
            return self.connect_reach(address, instance, host).await;
        }
        if self.store.is_some() || self.websocket {
            return Err("--store and --websocket go with --reach HOST".into());
        }
        let trust = self
            .trust_der
            .as_ref()
            .ok_or("--trust CERT.der (or trust_der in --chamber FILE) is required")?;
        let mut roots = rustls::RootCertStore::empty();
        roots
            .add(rustls::pki_types::CertificateDer::from(bounded(
                trust,
                1024 * 1024,
            )?))
            .map_err(|_| format!("{}: not a DER certificate", trust.display()))?;
        let tls = Arc::new(
            rustls::ClientConfig::builder_with_provider(Arc::new(
                rustls::crypto::ring::default_provider(),
            ))
            .with_safe_default_protocol_versions()
            .map_err(|_| "Cannot configure TLS protocol versions")?
            .with_root_certificates(roots)
            .with_no_client_auth(),
        );
        let name = self.server_name.clone().unwrap_or_else(|| {
            if address.ip().is_loopback() {
                "localhost".to_owned()
            } else {
                address.ip().to_string()
            }
        });
        let server_name = rustls::pki_types::ServerName::try_from(name.clone())
            .map_err(|_| format!("--server-name {name}: not a TLS server name"))?;
        let identity = crate::relay::identity_for(self.profile.as_deref())?;
        let key =
            secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &identity.secret);
        let content = self.content_identity()?;
        Client::connect_with_content(address, server_name, tls, instance, content, &key)
            .await
            .map_err(|e| format!("{e} ({address}, instance {instance})"))
    }
}

impl Connection {
    /// Join over a REACH channel as this device, with the grant `host`
    /// signed for it. The channel names the instance as its generation.
    async fn connect_reach(
        &self,
        address: SocketAddr,
        instance: u64,
        host: &str,
    ) -> Result<Client, String> {
        use coder_computers::live::{FileStore, Store as _};
        if self.trust_der.is_some() || self.server_name.is_some() || self.profile.is_some() {
            return Err(
                "--trust, --server-name, and --as go with TLS; a REACH chamber proves the host key and admits this device's grant"
                    .into(),
            );
        }
        let store = crate::computer::store_dir(self.store.as_deref().and_then(Path::to_str));
        let saved = FileStore::open(&store)?.load()?.unwrap_or_default();
        let known: Vec<crate::hosts::Known> = saved
            .hosts
            .iter()
            .map(|saved| crate::hosts::Known {
                key: saved.access.grant.host.clone(),
                label: saved.label.clone(),
            })
            .collect();
        let host = crate::hosts::resolve(&store, &known, host)?;
        let grant = &saved
            .hosts
            .iter()
            .find(|saved| saved.access.grant.host == host)
            .ok_or_else(|| {
                format!(
                    "this device holds no grant from {host}; link it with `openagents computer link`"
                )
            })?
            .access
            .grant;
        if !store.join("device.key").is_file() {
            return Err(format!("{}: no device key", store.display()));
        }
        let device = coder_computers::live::load_or_create_key(&store)?;
        if coder_reach::pubkey(&device) != grant.device {
            return Err("the saved grant names another device key".into());
        }
        let config = reach::ClientConfig {
            device,
            host: host.clone(),
            grant: grant.grant.clone(),
            epoch: grant.epoch,
            generation: instance,
            timeout: HANDSHAKE,
        };
        let content = self.content_identity()?;
        let socket = tokio::net::TcpStream::connect(address)
            .await
            .map_err(|e| format!("cannot reach {address}: {e}"))?;
        let _ = socket.set_nodelay(true);
        let joined = if self.websocket {
            let url = format!("ws://{address}/");
            let socket = tokio::time::timeout(HANDSHAKE, reach::websocket::client(&url, socket))
                .await
                .map_err(|_| "the chamber WebSocket upgrade timed out".to_owned())?
                .map_err(|e| format!("Chamber channel refused: {e}"))?;
            reach::join(socket, &config, instance, content).await
        } else {
            reach::join(socket, &config, instance, content).await
        };
        joined.map_err(|e| format!("{e} ({address}, instance {instance}, host {host})"))
    }
}

fn hex_bytes(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) || !text.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(format!("{text}: not hex"));
    }
    Ok((0..text.len())
        .step_by(2)
        .filter_map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect())
}

fn bounded(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    use std::io::Read;
    let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let metadata = file
        .metadata()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("{}: not a regular file", path.display()));
    }
    let mut bytes = vec![];
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if bytes.len() > limit {
        return Err(format!("{}: larger than {limit} bytes", path.display()));
    }
    Ok(bytes)
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("chamber", "a command is required", USAGE);
    };
    let result = match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            Ok(0)
        }
        "host" => parse_command(
            rest,
            "host",
            &["state", "root", "keys", "label"],
            &["keychain", "loopback-test"],
            1,
            1,
        )
        .map_err(Fail::Usage)
        .and_then(|args| host_command(output, &args)),
        "service" => Ok(service::run(output, rest)),
        "tls" => parse_command(rest, "tls", &["name"], &[], 1, 1)
            .map_err(Fail::Usage)
            .and_then(|args| tls_command(output, &args)),
        "pack" => parse_command(rest, "pack", &[], &[], 1, 1)
            .map_err(Fail::Usage)
            .and_then(|args| pack_command(output, Path::new(&args.positional()[0]))),
        "status" => connected(output, rest, "status", &[], 0, 0, |o, c, _| {
            Box::pin(status(o, c))
        }),
        "snapshot" => connected(output, rest, "snapshot", &[], 0, 0, |o, c, _| {
            Box::pin(snapshot(o, c))
        }),
        "events" => connected(
            output,
            rest,
            "events",
            &["after", "limit"],
            0,
            0,
            |o, c, a| Box::pin(events(o, c, a)),
        ),
        "watch" => connected(output, rest, "watch", &["wait", "hz"], 0, 0, |o, c, a| {
            Box::pin(watch(o, c, a))
        }),
        "move" => connected(output, rest, "move", &["yaw", "ticks"], 1, 1, |o, c, a| {
            Box::pin(move_command(o, c, a))
        }),
        "jump" => connected(output, rest, "jump", &[], 0, 0, |o, c, _| {
            Box::pin(act(o, c, "jump", Intent::Jump, "Jumped."))
        }),
        "cast" => connected(output, rest, "cast", &["target", "aim"], 1, 1, |o, c, a| {
            Box::pin(cast(o, c, a))
        }),
        "respawn" => connected(output, rest, "respawn", &[], 0, 0, |o, c, _| {
            Box::pin(async move {
                let response = c.respawn().await.map_err(Fail::Run)?;
                outcome(o, c, "respawn", &response, "Respawned.")
            })
        }),
        "inventory" => connected(output, rest, "inventory", &[], 0, 0, |o, c, _| {
            Box::pin(inventory(o, c))
        }),
        "use" => connected(output, rest, "use", &[], 1, 1, |o, c, a| {
            Box::pin(async move {
                let item = number(&a.positional()[0], "ITEM")?;
                let response = c.use_item(item, operation()).await.map_err(Fail::Run)?;
                outcome(o, c, "use", &response, &format!("Used item {item}."))
            })
        }),
        "equip" => connected(output, rest, "equip", &[], 2, 2, |o, c, a| {
            Box::pin(equip(o, c, a))
        }),
        "quest" => match rest.split_first() {
            Some((verb, rest)) if verb == "accept" => {
                connected(output, rest, "quest accept", &["giver"], 1, 1, |o, c, a| {
                    Box::pin(quest_accept(o, c, a))
                })
            }
            Some((verb, rest)) if verb == "claim" => {
                connected(output, rest, "quest claim", &[], 1, 1, |o, c, a| {
                    Box::pin(async move {
                        let quest = number(&a.positional()[0], "ID")?;
                        let response = c.claim_quest(quest).await.map_err(Fail::Run)?;
                        outcome(
                            o,
                            c,
                            "quest claim",
                            &response,
                            &format!("Quest {quest} claimed."),
                        )
                    })
                })
            }
            _ => Err(Fail::Usage(
                "quest takes accept ID --giver ACTOR or claim ID".into(),
            )),
        },
        other => Err(Fail::Usage(format!("unknown chamber command `{other}`"))),
    };
    match result {
        Ok(code) => code,
        Err(Fail::Usage(message)) => output.usage("chamber", &message, USAGE),
        Err(Fail::Run(message)) => output.fail(&format!("chamber {command}"), &message),
    }
}

enum Fail {
    Usage(String),
    Run(String),
}

type Step<'a> = std::pin::Pin<Box<dyn std::future::Future<Output = Result<u8, Fail>> + 'a>>;

/// Parse the command's own options plus the connection options, connect
/// as the profile, run `step`, and close the transport.
fn connected(
    output: &Output,
    words: &[String],
    command: &str,
    options: &[&str],
    min: usize,
    max: usize,
    step: for<'a> fn(&'a Output, &'a mut Client, &'a Args) -> Step<'a>,
) -> Result<u8, Fail> {
    let all: Vec<&str> = options.iter().chain(CONNECTION_OPTIONS).copied().collect();
    let args =
        parse_command(words, command, &all, CONNECTION_SWITCHES, min, max).map_err(Fail::Usage)?;
    let connection = Connection::from_args(&args).map_err(Fail::Usage)?;
    crate::runtime().block_on(async {
        let mut client = connection.connect().await.map_err(Fail::Run)?;
        let result = step(output, &mut client, &args).await;
        if client.connected() {
            let _ = client.close().await;
        }
        result
    })
}

fn number(text: &str, what: &str) -> Result<u64, Fail> {
    text.parse()
        .map_err(|_| Fail::Usage(format!("{what} must be a number, not `{text}`")))
}

fn operation() -> [u8; 16] {
    verse::identity::random_bytes()
}

fn life_json(life: &Life) -> Value {
    json!({ "instance": life.instance, "actor": life.actor, "generation": life.generation })
}

fn control_json(client: &Client) -> Value {
    match client.control() {
        Some(control) => json!({
            "role": "player",
            "life": life_json(&control.life),
            "epoch": control.epoch,
            "accepted_sequence": control.accepted_sequence,
        }),
        None => json!({ "role": "spectator" }),
    }
}

fn reply_json(reply: &Reply) -> Value {
    serde_json::to_value(reply).unwrap_or(Value::Null)
}

/// Report a command's acknowledgment: exit 1 on a gameplay refusal.
fn outcome(
    output: &Output,
    client: &Client,
    command: &str,
    response: &Response,
    done: &str,
) -> Result<u8, Fail> {
    let value = json!({
        "command": command,
        "tick": response.tick,
        "control": control_json(client),
        "reply": reply_json(&response.body),
    });
    if let Reply::Refused { code, message } = &response.body {
        return Err(Fail::Run(format!("{message} ({code})")));
    }
    output.emit(&value, |v| format!("{done} Tick {}.", v["tick"]));
    Ok(0)
}

fn actors_json(state: &State) -> Vec<Value> {
    state
        .snapshot
        .actors
        .iter()
        .map(|actor| {
            let life = state
                .actors
                .iter()
                .find(|binding| binding.source == actor.id)
                .map(|binding| life_json(&binding.life));
            json!({
                "id": actor.id,
                "kind": actor.kind,
                "faction": actor.faction,
                "pos": actor.pos,
                "yaw": actor.yaw,
                "hp": actor.hp,
                "max_hp": actor.max_hp,
                "alive": actor.alive,
                "life": life,
            })
        })
        .collect()
}

fn status_json(client: &Client, state: &State) -> Value {
    let own = state.hud.as_ref().map(|hud| {
        json!({
            "hp": hud.resources.hp,
            "max_hp": hud.resources.max_hp,
            "mana": hud.resources.mana,
            "max_mana": hud.resources.max_mana,
            "casting": hud.casting.is_some(),
        })
    });
    json!({
        "instance": client.instance(),
        "tick": client.tick(),
        "elapsed": state.snapshot.elapsed,
        "control": control_json(client),
        "own": own,
        "actors": actors_json(state),
        "projectiles": state.snapshot.projectiles.len(),
        "effects": state.snapshot.effects.len(),
        "population": population_json(state),
    })
}

/// Players present in the chamber: actors another identity controls.
/// Admitted player seats other than the chamber's own first seat, which
/// exists whether or not a primary key holds it.
fn population_json(state: &State) -> Value {
    let players = state
        .snapshot
        .actors
        .iter()
        .filter(|actor| actor.faction == "player" && actor.id != 0)
        .collect::<Vec<_>>();
    json!({
        "players": players.len(),
        "alive": players.iter().filter(|actor| actor.alive).count(),
    })
}

fn render_status(value: &Value) -> String {
    let mut lines = vec![format!(
        "chamber {} tick {} ({:.1}s) as {}",
        value["instance"],
        value["tick"],
        value["elapsed"].as_f64().unwrap_or(0.),
        value["control"]["role"].as_str().unwrap_or("?")
    )];
    if let Some(own) = value["own"].as_object() {
        lines.push(format!(
            "you: hp {}/{} mana {}/{} life {}#{} epoch {}",
            own["hp"],
            own["max_hp"],
            own["mana"],
            own["max_mana"],
            value["control"]["life"]["actor"],
            value["control"]["life"]["generation"],
            value["control"]["epoch"]
        ));
    }
    let mut rows = vec![vec![
        "actor".into(),
        "kind".into(),
        "faction".into(),
        "hp".into(),
        "pos".into(),
    ]];
    for actor in value["actors"].as_array().into_iter().flatten() {
        let pos = actor["pos"].as_array().map_or(String::new(), |p| {
            p.iter()
                .map(|n| format!("{:.1}", n.as_f64().unwrap_or(0.)))
                .collect::<Vec<_>>()
                .join(",")
        });
        rows.push(vec![
            actor["id"].to_string(),
            actor["kind"].as_str().unwrap_or("").to_owned(),
            actor["faction"].as_str().unwrap_or("").to_owned(),
            if actor["alive"].as_bool().unwrap_or(false) {
                format!("{}/{}", actor["hp"], actor["max_hp"])
            } else {
                "dead".into()
            },
            pos,
        ]);
    }
    lines.push(crate::out::table(&rows));
    lines.join("\n")
}

async fn status(output: &Output, client: &mut Client) -> Result<u8, Fail> {
    let state = client.snapshot().await.map_err(Fail::Run)?;
    output.emit(&status_json(client, &state), render_status);
    Ok(0)
}

async fn snapshot(output: &Output, client: &mut Client) -> Result<u8, Fail> {
    let state = client.snapshot().await.map_err(Fail::Run)?;
    let value = json!({
        "instance": client.instance(),
        "tick": client.tick(),
        "control": control_json(client),
        "state": serde_json::to_value(&state).map_err(|e| Fail::Run(e.to_string()))?,
    });
    output.emit(&value, |v| {
        serde_json::to_string_pretty(&v["state"]).unwrap_or_default()
    });
    Ok(0)
}

fn event_json(event: &verse_world::events::Event) -> Value {
    json!({
        "serial": event.serial,
        "tick": event.tick,
        "time": event.time,
        "actor": event.actor.map(|life| json!({ "actor": life.actor, "generation": life.generation })),
        "kind": serde_json::to_value(&event.kind).unwrap_or(Value::Null),
    })
}

fn render_event(value: &Value) -> String {
    let actor = value["actor"]["actor"]
        .as_u64()
        .map_or(String::new(), |a| format!(" actor {a}"));
    format!(
        "#{} t{} {:.2}s{actor} {}",
        value["serial"],
        value["tick"],
        value["time"].as_f64().unwrap_or(0.),
        value["kind"]
    )
}

async fn events(output: &Output, client: &mut Client, args: &Args) -> Result<u8, Fail> {
    let after = args
        .option("after")
        .map_or(Ok(0), |a| number(a, "--after"))?;
    let limit = args
        .option("limit")
        .map_or(Ok(64u64), |l| number(l, "--limit"))?
        .clamp(1, u64::from(u16::MAX)) as u16;
    let page = client.events(after, limit).await.map_err(Fail::Run)?;
    let value = json!({
        "events": page.events.iter().map(event_json).collect::<Vec<_>>(),
        "next": page.next,
        "latest": page.latest,
        "oldest": page.oldest,
        "gap": page.gap,
    });
    output.emit(&value, |v| {
        let mut lines: Vec<String> = v["events"]
            .as_array()
            .into_iter()
            .flatten()
            .map(render_event)
            .collect();
        if lines.is_empty() {
            lines.push("no events".into());
        }
        if v["gap"].as_bool() == Some(true) {
            lines.push("(gap: earlier events were not retained)".into());
        }
        lines.join("\n")
    });
    Ok(0)
}

async fn watch(output: &Output, client: &mut Client, args: &Args) -> Result<u8, Fail> {
    let wait = args
        .option("wait")
        .map_or(Ok(30), |w| number(w, "--wait"))?;
    let hz = args
        .option("hz")
        .map_or(Ok(2), |h| number(h, "--hz"))?
        .clamp(1, 30);
    let period = Duration::from_millis(1000 / hz);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(wait);
    let mut after = client.events(0, 1).await.map_err(Fail::Run)?.latest;
    while tokio::time::Instant::now() < deadline {
        let state = client.snapshot().await.map_err(Fail::Run)?;
        let mut value = status_json(client, &state);
        value["event"] = Value::String("snapshot".into());
        output.line(&value, render_status);
        let page = client.events(after, 64).await.map_err(Fail::Run)?;
        for event in &page.events {
            let mut value = event_json(event);
            value["event"] = Value::String("authority".into());
            output.line(&value, render_event);
        }
        after = page.latest.max(after);
        tokio::time::sleep(period).await;
    }
    Ok(0)
}

fn axes(text: &str) -> Result<[f32; 2], Fail> {
    let parts: Vec<f32> = text
        .split(',')
        .map(|p| p.trim().parse::<f32>())
        .collect::<Result<_, _>>()
        .map_err(|_| Fail::Usage(format!("X,Z must be two numbers, not `{text}`")))?;
    match parts[..] {
        [x, z] if x.abs() <= 1. && z.abs() <= 1. => Ok([x, z]),
        _ => Err(Fail::Usage("X,Z are two axes between -1 and 1".into())),
    }
}

async fn move_command(output: &Output, client: &mut Client, args: &Args) -> Result<u8, Fail> {
    let axes = axes(&args.positional()[0])?;
    let yaw = args
        .option("yaw")
        .map_or(Ok(0.), |y| {
            y.parse::<f32>()
                .map_err(|_| Fail::Usage(format!("--yaw {y}: not degrees")))
        })?
        .to_radians();
    let ticks = args
        .option("ticks")
        .map_or(Ok(30), |t| number(t, "--ticks"))?
        .clamp(1, 3000);
    let mut last = None;
    for _ in 0..ticks {
        let response = client
            .command(Intent::Move { axes, yaw })
            .await
            .map_err(Fail::Run)?;
        if let Reply::Refused { code, message } = &response.body {
            return Err(Fail::Run(format!("{message} ({code})")));
        }
        last = Some(response);
        tokio::time::sleep(TICK).await;
    }
    let state = client.snapshot().await.map_err(Fail::Run)?;
    let own = client.control().and_then(|control| {
        state
            .actors
            .iter()
            .find(|binding| binding.life == control.life)
            .and_then(|binding| {
                state
                    .snapshot
                    .actors
                    .iter()
                    .find(|a| a.id == binding.source)
            })
    });
    let value = json!({
        "command": "move",
        "ticks": ticks,
        "tick": last.as_ref().map(|r| r.tick),
        "control": control_json(client),
        "pos": own.map(|a| a.pos),
        "yaw": own.map(|a| a.yaw),
    });
    output.emit(&value, |v| {
        format!(
            "Moved {ticks} ticks; now at {} yaw {:.2} (tick {}).",
            v["pos"],
            v["yaw"].as_f64().unwrap_or(0.),
            v["tick"]
        )
    });
    Ok(0)
}

async fn act(
    output: &Output,
    client: &mut Client,
    command: &str,
    intent: Intent<Ability>,
    done: &str,
) -> Result<u8, Fail> {
    let response = client.command(intent).await.map_err(Fail::Run)?;
    outcome(output, client, command, &response, done)
}

fn ability(name: &str) -> Result<Ability, Fail> {
    let key = name.to_ascii_lowercase().replace('_', "-");
    Ok(match key.as_str() {
        "bow" => Ability::Bow,
        "fire-bolt" | "firebolt" => Ability::FireBolt,
        "magic-missile" => Ability::MagicMissile,
        "fireball" => Ability::Fireball,
        "misty-step" => Ability::MistyStep,
        "thunderwave" => Ability::Thunderwave,
        "web" => Ability::Web,
        "grease" => Ability::Grease,
        "light" => Ability::Light,
        "shield" => Ability::Shield,
        _ => {
            if let Ok(slot) = key.parse::<u8>()
                && verse_world::spells::spell_in_slot(slot).is_some()
            {
                return Ok(Ability::Spell(slot));
            }
            let spell = verse_world::spells::CATALOG
                .iter()
                .find(|spell| spell.key == key)
                .ok_or_else(|| {
                    let names: Vec<&str> =
                        verse_world::spells::CATALOG.iter().map(|s| s.key).collect();
                    Fail::Usage(format!(
                        "unknown ability `{name}`; row two: {}",
                        names.join(", ")
                    ))
                })?;
            Ability::Spell(spell.slot)
        }
    })
}

fn horizontal_aim(text: &str) -> Result<[f32; 3], Fail> {
    let [x, z] = axes(text)?;
    let length = (x * x + z * z).sqrt();
    if length < 1e-6 {
        return Err(Fail::Usage("--aim needs a non-zero X,Z direction".into()));
    }
    Ok([x / length, 0., z / length])
}

async fn cast(output: &Output, client: &mut Client, args: &Args) -> Result<u8, Fail> {
    let name = &args.positional()[0];
    let ability = ability(name)?;
    let aim = args
        .option("aim")
        .map_or(Ok([0., 0., -1.]), horizontal_aim)?;
    let target = match args.option("target") {
        Some(actor) => {
            let actor = number(actor, "--target")?;
            let state = client.snapshot().await.map_err(Fail::Run)?;
            let binding = state
                .actors
                .iter()
                .find(|binding| u64::from(binding.source) == actor)
                .ok_or_else(|| Fail::Run(format!("actor {actor} is not in the chamber")))?;
            Some(verse_engine::core::LifeId {
                instance: binding.life.instance,
                actor: binding.life.actor,
                generation: binding.life.generation,
            })
        }
        None => None,
    };
    act(
        output,
        client,
        "cast",
        Intent::Cast {
            ability,
            target,
            aim,
        },
        &format!("Cast {name}."),
    )
    .await
}

async fn inventory(output: &Output, client: &mut Client) -> Result<u8, Fail> {
    let inventory = client.inventory().await.map_err(Fail::Run)?;
    let value = serde_json::to_value(&inventory).map_err(|e| Fail::Run(e.to_string()))?;
    output.emit(&value, |v| {
        let mut lines = vec![format!(
            "life {}#{} revision {} experience {} outfit {}",
            v["life"]["actor"],
            v["life"]["generation"],
            v["revision"],
            v["experience"],
            v["outfit"]
        )];
        lines.push(format!("level: {}", v["level"]));
        lines.push(format!("items: {}", v["items"]));
        lines.push(format!("quests: {}", v["quests"]));
        lines.push(format!("quest log: {}", v["quest_log"]));
        lines.push(format!("equipped: {}", v["equipped"]));
        lines.join("\n")
    });
    Ok(0)
}

async fn equip(output: &Output, client: &mut Client, args: &Args) -> Result<u8, Fail> {
    let slot = args.positional()[0].to_ascii_lowercase();
    let item = number(&args.positional()[1], "ITEM")?;
    let response = match slot.as_str() {
        "outfit" => client.equip_outfit(item, operation()).await,
        "head" => {
            client
                .equip_gear(
                    verse_world::service::equipment::Slot::Head,
                    item,
                    operation(),
                )
                .await
        }
        "main-hand" | "mainhand" | "main_hand" => {
            client
                .equip_gear(
                    verse_world::service::equipment::Slot::MainHand,
                    item,
                    operation(),
                )
                .await
        }
        other => {
            return Err(Fail::Usage(format!(
                "SLOT is head, main-hand, or outfit, not `{other}`"
            )));
        }
    }
    .map_err(Fail::Run)?;
    outcome(
        output,
        client,
        "equip",
        &response,
        &format!("Equipped {item} in {slot}."),
    )
}

async fn quest_accept(output: &Output, client: &mut Client, args: &Args) -> Result<u8, Fail> {
    let quest = number(&args.positional()[0], "ID")?;
    let giver = number(
        args.option("giver")
            .ok_or_else(|| Fail::Usage("--giver ACTOR is required".into()))?,
        "--giver",
    )?;
    let state = client.snapshot().await.map_err(Fail::Run)?;
    let binding = state
        .actors
        .iter()
        .find(|binding| u64::from(binding.source) == giver)
        .ok_or_else(|| Fail::Run(format!("actor {giver} is not in the chamber")))?;
    let giver = verse_engine::core::LifeId {
        instance: binding.life.instance,
        actor: binding.life.actor,
        generation: binding.life.generation,
    };
    let response = client.accept_quest(quest, giver).await.map_err(Fail::Run)?;
    outcome(
        output,
        client,
        "quest accept",
        &response,
        &format!("Quest {quest} accepted."),
    )
}

fn tls_command(output: &Output, args: &Args) -> Result<u8, Fail> {
    let dir = PathBuf::from(&args.positional()[0]);
    let name = args.option("name").unwrap_or("localhost").to_owned();
    std::fs::create_dir_all(&dir).map_err(|e| Fail::Run(format!("{}: {e}", dir.display())))?;
    let certified = rcgen::generate_simple_self_signed(vec![name.clone()])
        .map_err(|e| Fail::Run(format!("cannot generate a certificate: {e}")))?;
    let cert = dir.join("cert.der");
    let key = dir.join("key.der");
    std::fs::write(&cert, certified.cert.der())
        .map_err(|e| Fail::Run(format!("{}: {e}", cert.display())))?;
    std::fs::write(&key, certified.signing_key.serialize_der())
        .map_err(|e| Fail::Run(format!("{}: {e}", key.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| Fail::Run(format!("{}: {e}", key.display())))?;
    }
    output.emit(
        &json!({ "certificate_der": cert, "private_key_der": key, "server_name": name }),
        |v| {
            format!(
                "Wrote {} and {} for {}.",
                v["certificate_der"].as_str().unwrap_or(""),
                v["private_key_der"].as_str().unwrap_or(""),
                v["server_name"].as_str().unwrap_or("")
            )
        },
    );
    Ok(0)
}

fn pack_command(output: &Output, dir: &Path) -> Result<u8, Fail> {
    let mut pack = verse::imported::original::generate(dir).map_err(Fail::Run)?;
    if let Some(mut inventory) = pack.inventory.take() {
        verse::imported::inventory::refresh(&mut inventory, &pack, dir).map_err(Fail::Run)?;
        pack.inventory = Some(inventory);
    }
    pack.validate().map_err(Fail::Run)?;
    let path = dir.join("runtime-pack.json");
    let bytes = serde_json::to_vec(&pack).map_err(|e| Fail::Run(e.to_string()))?;
    std::fs::write(&path, bytes).map_err(|e| Fail::Run(format!("{}: {e}", path.display())))?;
    let scene = std::fs::canonicalize("assets/verse/original/ritual.json").ok();
    output.emit(
        &json!({
            "pack": path,
            "models": pack.models.len(),
            "textures": pack.textures.len(),
            "scene": scene,
        }),
        |v| {
            format!(
                "Wrote {} ({} models, {} textures).",
                v["pack"].as_str().unwrap_or(""),
                v["models"],
                v["textures"]
            )
        },
    );
    Ok(0)
}

fn host_command(output: &Output, args: &Args) -> Result<u8, Fail> {
    let path = Path::new(&args.positional()[0]);
    let config = host::Config::from_json(&bounded(path, 64 * 1024).map_err(Fail::Run)?)
        .map_err(|e| Fail::Run(format!("{}: {e}", path.display())))?;
    let access = WorldAccess::from_args(args).map_err(Fail::Usage)?;
    if config.reach().is_none() && access.given {
        return Err(Fail::Usage(
            "--state, --root, --keys, --keychain, --label, and --loopback-test go with a REACH chamber (\"transport\":{\"type\":\"reach\"})".into(),
        ));
    }
    crate::runtime()
        .block_on(serve(*output, config, access))
        .map_err(Fail::Run)?;
    Ok(0)
}

/// Where a REACH chamber finds the Coder host's key and grants: the same
/// access store, host root, and key source `openagents host serve` uses.
struct WorldAccess {
    state: PathBuf,
    root: PathBuf,
    keys: Option<String>,
    keychain: bool,
    label: Option<String>,
    policy: coder_access::RelayPolicy,
    /// Any access option was given.
    given: bool,
}

impl WorldAccess {
    fn from_args(args: &Args) -> Result<Self, String> {
        let home = || {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .ok_or_else(|| "HOME is not set; pass --state and --root".to_owned())
        };
        let state = match args.option("state") {
            Some(path) => PathBuf::from(path),
            None => home()?.join(".openagents/coder-access"),
        };
        let root = match args.option("root") {
            Some(path) => PathBuf::from(path),
            None => home()?.join(".openagents/host"),
        };
        let keychain = args.switch("keychain");
        let keys = args.option("keys").map(str::to_owned);
        if keychain && keys.is_some() {
            return Err("--keychain and --keys do not go together".into());
        }
        let loopback = args.switch("loopback-test");
        let given = ["state", "root", "keys", "label"]
            .iter()
            .any(|name| args.option(name).is_some())
            || keychain
            || loopback;
        Ok(Self {
            state,
            root,
            keys,
            keychain,
            label: args.option("label").map(str::to_owned),
            policy: if loopback {
                coder_access::RelayPolicy::LoopbackTest
            } else {
                coder_access::RelayPolicy::Production
            },
            given,
        })
    }

    fn key_source(&self) -> Result<Option<coder_host::serve::keys::Keys>, String> {
        use coder_host::serve::keys::{FileKeySource, Keys};
        if self.keychain {
            return coder_host::cli::keychain_keys()
                .map(Some)
                .map_err(|e| e.to_string());
        }
        Ok(self
            .keys
            .as_ref()
            .map(|dir| Keys(Arc::new(FileKeySource::new(dir)))))
    }
}

/// The Coder host a REACH chamber serves as: its signing key, its grants
/// with the `world` right, and the owner key when the key source holds it.
struct WorldHost {
    secret: secp256k1::SecretKey,
    grants: coder_host::authority::WorldGrants,
    owner: Option<secp256k1::SecretKey>,
}

impl WorldHost {
    /// Open the access store the Coder host serves from. A store that was
    /// never initialized refuses: a chamber never establishes an owner.
    fn open(access: &WorldAccess) -> Result<Self, String> {
        let keys = access.key_source()?;
        let store =
            coder_host::serve::keys::access_store(&access.state, access.policy, keys.as_ref());
        if !store.state_path().exists() {
            return Err(format!(
                "{}: the host access store is not initialized; start the Coder host first (`openagents host serve`)",
                access.state.display()
            ));
        }
        let authority = coder_host::authority::Authority::open(store)
            .map_err(|e| format!("{}: {e}", access.state.display()))?;
        let secret = authority
            .signing_key()
            .map_err(|e| format!("the host key cannot be read: {e}"))?;
        let owner = match &keys {
            Some(keys) => coder_host::serve::keys::held_owner(keys.0.as_ref())
                .map_err(|_| "the owner key cannot be read".to_owned())?,
            None => None,
        };
        Ok(Self {
            secret,
            grants: coder_host::authority::WorldGrants(Arc::new(authority)),
            owner,
        })
    }
}

async fn serve(output: Output, config: host::Config, access: WorldAccess) -> Result<(), String> {
    let scene = verse_engine::director::Scene::from_json(&bounded(&config.scene, 1024 * 1024)?)?;
    let pack = verse_engine::assets::Pack::read(&config.pack)?;
    verse::imported::remote_content::outfit_models(&pack, &config.outfits)?;
    verse::imported::remote_content::equipment_models(
        &pack,
        &scene,
        &config.outfits,
        &config.equipment,
    )?;
    let content = verse::imported::remote_content::identity(
        &pack,
        &scene,
        config.pack.parent().unwrap_or(Path::new(".")),
    )?;
    let content = config.bind_content(content)?;
    let mut game = config.prepare_game(scene)?;
    verse::imported::props::admit_collision(&pack, &mut game)?;
    let mut store = config
        .state_dir
        .as_ref()
        .map(|path| Store::open(path, content, config.instance))
        .transpose()?;
    let gateway = match store.as_mut().and_then(Store::recover) {
        Some(gateway) => {
            config.validate_recovered_scene(&gateway, &game)?;
            gateway
        }
        None => config.gateway(game)?.with_content(content)?,
    };
    // Admission comes from TLS and the enrollment list, or from the Coder
    // host's grants; either is ready before anything binds.
    let transport = match config.reach() {
        None => Transport::Tls(Arc::new(tls_server(&config)?)),
        Some(carrier) => {
            let world = WorldHost::open(&access)?;
            // The instance number is the channel's generation: a client
            // knows it from the directory without reading presence.
            let server = reach::Server::new(world.secret, config.instance, world.grants, carrier);
            Transport::Reach(Box::new(server), carrier, world.owner)
        }
    };
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(|e| format!("cannot bind {}: {e}", config.listen))?;
    let address = listener
        .local_addr()
        .map_err(|_| "Cannot inspect chamber listener")?;
    let content_hex: String = content.iter().map(|b| format!("{b:02x}")).collect();
    let (transport_name, host_key) = match &transport {
        Transport::Tls(_) => ("tls", None),
        Transport::Reach(server, reach::Carrier::Tcp, _) => {
            ("reach", Some(server.host_key().to_owned()))
        }
        Transport::Reach(server, reach::Carrier::WebSocket, _) => {
            ("reach-websocket", Some(server.host_key().to_owned()))
        }
    };
    output.line(
        &json!({
            "event": "listening",
            "instance": config.instance,
            "address": address,
            "content": content_hex,
            "transport": transport_name,
            "host": host_key,
            "enrollments": config.enrollments.len(),
            "durable": config.state_dir.is_some(),
        }),
        |v| match v["host"].as_str() {
            Some(host) => format!(
                "Chamber {} listening on {} over REACH as host {host} (content {}); devices with the `world` right join",
                v["instance"],
                v["address"],
                v["content"].as_str().unwrap_or(""),
            ),
            None => format!(
                "Chamber {} listening on {} (content {}, {} enrolled keys)",
                v["instance"],
                v["address"],
                v["content"].as_str().unwrap_or(""),
                v["enrollments"]
            ),
        },
    );
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    let exit = match transport {
        Transport::Tls(tls) => match store {
            Some(store) => net::serve_durable(listener, tls, gateway, store, shutdown).await,
            None => net::serve(listener, tls, gateway, shutdown).await,
        },
        Transport::Reach(server, _, owner) => {
            let host = server.host_key().to_owned();
            let serving = async {
                match store {
                    Some(store) => {
                        reach::serve_durable(listener, *server, gateway, store, shutdown).await
                    }
                    None => reach::serve(listener, *server, gateway, shutdown).await,
                }
            };
            let world = coder_reach::directory::WorldInstance {
                instance: config.instance,
                label: access
                    .label
                    .clone()
                    .unwrap_or_else(|| format!("Chamber {}", config.instance)),
                wire: verse_world::service::wire::VERSION,
                content: Some(content_hex.clone()),
            };
            let listing = list_in_directory(output, &access, owner, &host, world);
            tokio::join!(serving, listing).0
        }
    };
    output.line(
        &json!({
            "event": "stopped",
            "ticks": exit.stats.ticks,
            "requests": exit.stats.requests,
            "connections": exit.stats.completed_connections,
            "admission": exit.stats.admission,
            "dropped_seconds": exit.stats.dropped_seconds,
            "checkpoint_commits": exit.stats.checkpoint_commits,
            "failure": exit.failure,
        }),
        |v| {
            format!(
                "Chamber stopped: {} ticks, {} requests, {} connections, {} dropped seconds",
                v["ticks"], v["requests"], v["connections"], v["dropped_seconds"]
            )
        },
    );
    match exit.failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

/// How the chamber admits connections.
enum Transport {
    Tls(Arc<rustls::ServerConfig>),
    Reach(
        Box<reach::Server<coder_host::authority::WorldGrants>>,
        reach::Carrier,
        Option<secp256k1::SecretKey>,
    ),
}

/// How long the chamber waits for the owner directory before it gives up
/// listing its instance; the chamber serves either way.
const DIRECTORY_WAIT: Duration = Duration::from_secs(20);

/// Name the instance in the host's owner-directory entry when this host
/// holds the owner key and the directory already lists it. Only the owner
/// edits the directory, so a host without the owner key reports why and
/// serves unlisted.
async fn list_in_directory(
    output: Output,
    access: &WorldAccess,
    owner: Option<secp256k1::SecretKey>,
    host: &str,
    world: coder_reach::directory::WorldInstance,
) {
    use coder_host::client::WorldListing;
    let report = |listed: bool, revision: Option<u64>, reason: &str| {
        output.line(
            &json!({
                "event": "directory",
                "listed": listed,
                "revision": revision,
                "reason": reason,
            }),
            |v| {
                if listed {
                    format!(
                        "Listed in the owner directory (revision {}).",
                        v["revision"]
                    )
                } else {
                    format!("Not listed in the owner directory: {reason}.")
                }
            },
        );
    };
    let Some(owner) = owner else {
        report(false, None, "this host does not hold the owner key");
        return;
    };
    let relays = match coder_host::settings::ServeSettings::load(&access.root) {
        Ok(settings) => settings.relays,
        Err(error) => {
            report(false, None, &error.to_string());
            return;
        }
    };
    let Some(relay) = relays.first() else {
        report(false, None, "the host root records no relay");
        return;
    };
    let listing = tokio::time::timeout(
        DIRECTORY_WAIT,
        coder_host::client::list_world(relay, &owner, host, world, access.policy),
    )
    .await;
    match listing {
        Ok(Ok(WorldListing::Listed { revision, .. })) => report(true, Some(revision), "listed"),
        Ok(Ok(WorldListing::HostNotListed)) => {
            report(false, None, "the owner directory does not list this host");
        }
        Ok(Ok(WorldListing::NoDirectory)) => {
            report(false, None, "the owner published no directory");
        }
        Ok(Err(error)) => report(false, None, &error.to_string()),
        Err(_) => report(false, None, "the relay did not answer in time"),
    }
}

fn tls_server(config: &host::Config) -> Result<rustls::ServerConfig, String> {
    let certificate =
        rustls::pki_types::CertificateDer::from(bounded(&config.certificate_der, 1024 * 1024)?);
    let key_bytes = bounded(&config.private_key_der, 64 * 1024)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&config.private_key_der)
            .map_err(|e| format!("{}: {e}", config.private_key_der.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "{}: the TLS private key must be readable by its owner only",
                config.private_key_der.display()
            ));
        }
    }
    let key = rustls::pki_types::PrivateKeyDer::try_from(key_bytes)
        .map_err(|_| "Invalid configured DER private key")?;
    rustls::ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
        .with_safe_default_protocol_versions()
        .map_err(|_| "Cannot configure TLS protocol versions")?
        .with_no_client_auth()
        .with_single_cert(vec![certificate], key)
        .map_err(|_| "Configured TLS certificate or private key refused".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abilities_parse_by_name_and_row_two_slot() {
        assert_eq!(ability("fire-bolt").ok(), Some(Ability::FireBolt));
        assert_eq!(ability("Shield").ok(), Some(Ability::Shield));
        assert_eq!(ability("wall-of-stone").ok(), Some(Ability::Spell(1)));
        assert_eq!(ability("reverse-gravity").ok(), Some(Ability::Spell(8)));
        assert_eq!(ability("3").ok(), Some(Ability::Spell(3)));
        assert!(ability("polymorph").is_err());
    }

    #[test]
    fn axes_are_bounded() {
        assert_eq!(axes("0,1").ok(), Some([0., 1.]));
        assert!(axes("2,0").is_err());
        assert!(axes("1").is_err());
    }

    #[test]
    fn connection_flags_override_the_file() {
        let dir = std::env::temp_dir().join(format!("oa-chamber-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("c.json");
        std::fs::write(
            &file,
            r#"{"address":"127.0.0.1:1","instance":7,"trust_der":"t.der"}"#,
        )
        .unwrap();
        let words: Vec<String> = ["--chamber", file.to_str().unwrap(), "--instance", "9"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let args = Args::parse(&words, &[]).unwrap();
        let connection = Connection::from_args(&args).unwrap();
        assert_eq!(connection.instance, Some(9));
        assert_eq!(connection.address.unwrap().port(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}

/// `chamber service`: a launchd agent or systemd user unit that runs
/// `openagents chamber host CONFIG.json` from login on.
mod service {
    use std::path::{Path, PathBuf};
    use std::process::Command;

    use coder_service::service::Platform;
    use serde_json::{Value, json};

    use crate::{Args, Output, argv::parse_command};

    const LABEL: &str = "com.openagents.chamber";

    pub fn run(output: &Output, words: &[String]) -> u8 {
        let Some(platform) = Platform::current() else {
            return output.fail("chamber service", "chamber service needs macOS or Linux");
        };
        let home_dir = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."));
        let path = platform
            .default_registration_dir(&home_dir)
            .join(match platform {
                Platform::Macos => format!("{LABEL}.plist"),
                Platform::Linux => format!("{LABEL}.service"),
            });
        let result = match words
            .split_first()
            .map(|(verb, rest)| (verb.as_str(), rest))
        {
            Some(("install", rest)) => parse_command(
                rest,
                "service install",
                &["binary", "state", "root", "keys", "label"],
                &["keychain"],
                1,
                1,
            )
            .map_err(|e| (true, e))
            .and_then(|args| install(platform, &path, &args).map_err(|e| (false, e))),
            Some(("uninstall", _)) => uninstall(platform, &path).map_err(|e| (false, e)),
            Some(("status", _)) => status(platform, &path).map_err(|e| (false, e)),
            _ => Err((
                true,
                "service needs install CONFIG.json, uninstall, or status".into(),
            )),
        };
        match result {
            Ok(value) => {
                output.emit(&value, |v| {
                    format!(
                        "{} {} ({})",
                        v["service"].as_str().unwrap_or(""),
                        v["state"].as_str().unwrap_or(""),
                        v["path"].as_str().unwrap_or("")
                    )
                });
                0
            }
            Err((true, message)) => output.usage("chamber", &message, super::USAGE),
            Err((false, message)) => output.fail("chamber service", &message),
        }
    }

    fn absolute(path: &str) -> Result<PathBuf, String> {
        let path = Path::new(path);
        if path.is_absolute() {
            return Ok(path.to_owned());
        }
        std::env::current_dir()
            .map(|dir| dir.join(path))
            .map_err(|error| format!("current directory: {error}"))
    }

    /// The `chamber host` words the unit runs, each path made absolute.
    fn host_words(args: &Args) -> Result<Vec<String>, String> {
        let config = absolute(&args.positional()[0])?;
        if !config.is_file() {
            return Err(format!("{}: not a file", config.display()));
        }
        super::host::Config::from_json(&super::bounded(&config, 64 * 1024)?)
            .map_err(|e| format!("{}: {e}", config.display()))?;
        let mut words = vec!["--json".to_owned(), "chamber".into(), "host".into()];
        words.push(config.display().to_string());
        for name in ["state", "root", "keys"] {
            if let Some(value) = args.option(name) {
                words.push(format!("--{name}"));
                words.push(absolute(value)?.display().to_string());
            }
        }
        if let Some(label) = args.option("label") {
            words.push("--label".into());
            words.push(label.to_owned());
        }
        if args.switch("keychain") {
            words.push("--keychain".into());
        }
        Ok(words)
    }

    fn xml(text: &str) -> String {
        text.replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
    }

    fn render(platform: Platform, binary: &Path, words: &[String], log: &Path) -> String {
        let binary = binary.display();
        let log = log.display();
        match platform {
            Platform::Macos => {
                let arguments: String = words
                    .iter()
                    .map(|word| format!("<string>{}</string>", xml(word)))
                    .collect();
                format!(
                    r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key><string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array><string>{binary}</string>{arguments}</array>
  <key>RunAtLoad</key><true/>
  <key>KeepAlive</key><true/>
  <key>StandardOutPath</key><string>{log}</string>
  <key>StandardErrorPath</key><string>{log}</string>
</dict>
</plist>
"#
                )
            }
            Platform::Linux => {
                let arguments: Vec<String> = words
                    .iter()
                    .map(|word| {
                        if word.contains([' ', '"', '\\']) {
                            format!("\"{}\"", word.replace('\\', "\\\\").replace('"', "\\\""))
                        } else {
                            word.clone()
                        }
                    })
                    .collect();
                format!(
                    "[Unit]\nDescription=openagents chamber host\nAfter=network-online.target\n\n[Service]\nExecStart={binary} {}\nRestart=always\nRestartSec=5\n\n[Install]\nWantedBy=default.target\n",
                    arguments.join(" ")
                )
            }
        }
    }

    fn sh(program: &str, args: &[&str]) -> Result<String, String> {
        let done = Command::new(program)
            .args(args)
            .output()
            .map_err(|error| format!("{program}: {error}"))?;
        let text = String::from_utf8_lossy(&done.stdout).into_owned()
            + &String::from_utf8_lossy(&done.stderr);
        if done.status.success() {
            Ok(text)
        } else {
            Err(format!("{program} {}: {}", args.join(" "), text.trim()))
        }
    }

    fn uid() -> String {
        // SAFETY: getuid has no preconditions.
        unsafe { libc::getuid() }.to_string()
    }

    fn install(platform: Platform, path: &Path, args: &Args) -> Result<Value, String> {
        let binary = match args.option("binary") {
            Some(path) => absolute(path)?,
            None => std::env::current_exe().map_err(|error| format!("current binary: {error}"))?,
        };
        let words = host_words(args)?;
        let log = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".openagents/chamber-host.log");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        if let Some(parent) = log.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|error| format!("{}: {error}", parent.display()))?;
        }
        std::fs::write(path, render(platform, &binary, &words, &log))
            .map_err(|error| format!("{}: {error}", path.display()))?;
        match platform {
            Platform::Macos => {
                let domain = format!("gui/{}", uid());
                let _ = sh("launchctl", &["bootout", &format!("{domain}/{LABEL}")]);
                sh(
                    "launchctl",
                    &["bootstrap", &domain, &path.display().to_string()],
                )?;
            }
            Platform::Linux => {
                sh("systemctl", &["--user", "daemon-reload"])?;
                sh(
                    "systemctl",
                    &["--user", "enable", "--now", &format!("{LABEL}.service")],
                )?;
            }
        }
        Ok(json!({
            "service": LABEL,
            "state": "installed",
            "path": path.display().to_string(),
            "binary": binary.display().to_string(),
            "config": words[3],
            "log": log.display().to_string(),
        }))
    }

    fn uninstall(platform: Platform, path: &Path) -> Result<Value, String> {
        match platform {
            Platform::Macos => {
                let _ = sh("launchctl", &["bootout", &format!("gui/{}/{LABEL}", uid())]);
            }
            Platform::Linux => {
                let _ = sh(
                    "systemctl",
                    &["--user", "disable", "--now", &format!("{LABEL}.service")],
                );
            }
        }
        let existed = path.exists();
        if existed {
            std::fs::remove_file(path).map_err(|error| format!("{}: {error}", path.display()))?;
        }
        if platform == Platform::Linux {
            let _ = sh("systemctl", &["--user", "daemon-reload"]);
        }
        Ok(json!({
            "service": LABEL,
            "state": if existed { "removed" } else { "absent" },
            "path": path.display().to_string(),
        }))
    }

    fn status(platform: Platform, path: &Path) -> Result<Value, String> {
        let registered = path.exists();
        let (active, detail) = match platform {
            Platform::Macos => match sh("launchctl", &["print", &format!("gui/{}/{LABEL}", uid())])
            {
                Ok(text) => (text.contains("state = running"), text),
                Err(text) => (false, text),
            },
            Platform::Linux => match sh(
                "systemctl",
                &["--user", "is-active", &format!("{LABEL}.service")],
            ) {
                Ok(text) => (text.trim() == "active", text),
                Err(text) => (false, text),
            },
        };
        Ok(json!({
            "service": LABEL,
            "state": if !registered { "absent" } else if active { "running" } else { "stopped" },
            "path": path.display().to_string(),
            "detail": detail.trim(),
        }))
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn units_quote_the_host_words() {
            let words = vec![
                "--json".to_owned(),
                "chamber".into(),
                "host".into(),
                "/tmp/a b.json".into(),
            ];
            let linux = render(
                Platform::Linux,
                Path::new("/bin/oa"),
                &words,
                Path::new("/l"),
            );
            assert!(
                linux.contains("ExecStart=/bin/oa --json chamber host \"/tmp/a b.json\""),
                "{linux}"
            );
            let mac = render(
                Platform::Macos,
                Path::new("/bin/oa"),
                &words,
                Path::new("/l"),
            );
            assert!(mac.contains("<string>/tmp/a b.json</string>"));
        }
    }
}
