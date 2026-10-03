//! Verse from the command line: who is in the world, what is near a point,
//! what people are saying, and the events this identity publishes — its
//! position, its words, and its gestures. Every event is a NIP-MV event
//! built by [`verse::mv`], so the desktop client sees this identity exactly
//! as it sees another player.

use std::collections::BTreeMap;
use std::time::Duration;

use glam::{Quat, Vec3};
use nostr::domain::Event;
use serde_json::{Value, json};
use verse::mv::{self, EntityPose, Frame, Gesture, Received, State};

use crate::relay::{Client, DEFAULT_WAIT, identity_for, relay_url, unix_now};
use crate::{Args, Output, out};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents verse COMMAND [OPTIONS]
  who                       Every entity with a state in the world, nearest first.
  look [--at X,Y,Z] [--radius CELLS] [--wait SECONDS]
                            Listen for live poses around a point (default: where
                            this identity stands) and list what is there.
  chat [--limit N]          Recent world chat lines.
  tail [--wait SECONDS]     Follow poses, gestures, states, and chat as they arrive.
  me                        This identity's public key and last known state.
  move X,Y,Z [--yaw DEGREES] [--name NAME]
                            Stand at a point: publish the avatar state and a frame.
  say TEXT [--to all|ads|zone|near|here] [--zone NAME]
                            Speak in world chat from where this identity stands.
  gesture NAME [--to PUBKEY,ENTITY] [--at X,Y,Z] [--duration SECONDS]
                            Perform a gesture (for example greet or look-around).
  name NAME                 Publish a kind 0 profile with this display name.
  leave                     Mark this identity's avatar offline.
  control ENTITY move X,Y,Z [--yaw DEGREES] [--role ROLE] [--name NAME]
  control ENTITY gesture NAME [--to PUBKEY,ENTITY] [--at X,Y,Z]
  control ENTITY leave      Drive another entity this identity publishes (for
                            example an agent it spawned): the same events as
                            move, gesture, and leave, under that entity id.
  quests                    Every quest on the XP relay, trusted referees first.
  xp [--pubkey KEY]...      The XP ledger and level for this identity's keys,
                            or for the keys given.
  xp verify-card CARD       Re-derive a trainer card (a signed 30194 as a JSON
                            file, - for standard input, or an naddr) from the
                            relays under the card's own trust list; report
                            every difference and exit 1 when there is one.
                            Also openagents xp verify-card.
  board                     What the plaza's quest board shows: counts,
                            standings, this identity's level, and the quests.
Options for every command: --as PROFILE (key), --relay URL, --world ID
(default verse-plaza), --entity ID (default avatar). Quest commands also take
--xp-relay URL (default VERSE_XP_RELAY, then the world relay) and
--referee KEY to trust another referee for this reading.";

/// `openagents xp --help`: the xp rows of [`USAGE`] under xp's own usage
/// line, and the options they take.
pub(crate) fn xp_usage() -> String {
    let mut text = vec!["usage: openagents xp [--pubkey KEY]... | verify-card CARD".to_owned()];
    let mut in_xp = false;
    for line in USAGE.lines().skip(1) {
        if let Some(row) = line.strip_prefix("  ")
            && !row.starts_with(' ')
        {
            in_xp = row.starts_with("xp ");
        }
        if !line.starts_with(' ') {
            break;
        }
        if in_xp && !line.contains("Also openagents xp verify-card") {
            text.push(line.replacen("  xp ", "  ", 1));
        }
    }
    text.push(
        "Options: --as PROFILE (key), --xp-relay URL (default VERSE_XP_RELAY, then\n\
         the world relay), --referee KEY to trust another referee for this reading."
            .to_owned(),
    );
    text.join("\n")
}

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("who", Effect::ReadOnly),
    Declared::computer("look", Effect::ReadOnly),
    Declared::computer("chat", Effect::ReadOnly),
    Declared::computer("tail", Effect::LongRunning),
    Declared::computer("me", Effect::ReadOnly),
    Declared::computer("move", Effect::Publishes),
    Declared::computer("say", Effect::Publishes),
    Declared::computer("gesture", Effect::Publishes),
    Declared::computer("name", Effect::Publishes),
    Declared::computer("leave", Effect::Publishes),
    Declared::computer("control move", Effect::Publishes),
    Declared::computer("control gesture", Effect::Publishes),
    Declared::computer("control leave", Effect::Publishes),
    Declared::computer("quests", Effect::ReadOnly),
    Declared::computer("xp", Effect::ReadOnly),
    Declared::computer("xp verify-card", Effect::ReadOnly),
    Declared::computer("board", Effect::ReadOnly),
];

/// Every entity the world knows about, by publisher and entity id.
#[derive(Default)]
pub struct Scene {
    pub entities: BTreeMap<(String, String), Seen>,
    pub names: BTreeMap<String, String>,
}

/// The last thing seen of one entity.
#[derive(Clone, Debug)]
pub struct Seen {
    pub pose: EntityPose,
    pub online: bool,
    pub name: Option<String>,
    /// Publisher time in milliseconds.
    pub t: u64,
    /// True when a live frame carried it, not just a stored state.
    pub live: bool,
}

impl Scene {
    pub fn absorb(&mut self, event: &Event, world: &str) -> Option<Received> {
        let received = mv::decode(event, world).ok()?;
        match &received {
            Received::State { pubkey, state } => {
                if let Some(name) = &state.name {
                    self.names.insert(pubkey.clone(), name.clone());
                }
                let key = (pubkey.clone(), state.id.clone());
                let stale = self.entities.get(&key).is_some_and(|seen| seen.t > state.t);
                if !stale {
                    self.entities.insert(
                        key,
                        Seen {
                            pose: state.pose(),
                            online: state.online,
                            name: state.name.clone(),
                            t: state.t,
                            live: false,
                        },
                    );
                }
            }
            Received::Frame { pubkey, frame } => {
                for pose in &frame.e {
                    let key = (pubkey.clone(), pose.id.clone());
                    let name = self.names.get(pubkey).cloned();
                    self.entities.insert(
                        key,
                        Seen {
                            pose: pose.clone(),
                            online: true,
                            name,
                            t: frame.t,
                            live: true,
                        },
                    );
                }
            }
            Received::Gesture { .. } | Received::Command { .. } => {}
        }
        Some(received)
    }

    /// Entities as JSON rows, nearest to `origin` first.
    pub fn rows(&self, origin: Option<Vec3>, exclude: Option<&str>) -> Vec<Value> {
        let mut rows: Vec<(f32, Value)> = self
            .entities
            .iter()
            .filter(|((pubkey, _), _)| exclude != Some(pubkey.as_str()))
            .map(|((pubkey, id), seen)| {
                let pos = seen.pose.pos();
                let distance = origin.map_or(0.0, |origin| origin.distance(pos));
                (
                    distance,
                    json!({
                        "pubkey": pubkey,
                        "entity": id,
                        "role": seen.pose.role,
                        "name": seen.name.clone().or_else(|| self.names.get(pubkey).cloned()),
                        "pos": seen.pose.p,
                        "distance": origin.map(|_| distance),
                        "online": seen.online,
                        "live": seen.live,
                        "follows": seen.pose.follows,
                        "animation": seen.pose.a,
                        "t": seen.t,
                    }),
                )
            })
            .collect();
        rows.sort_by(|a, b| a.0.total_cmp(&b.0));
        rows.into_iter().map(|(_, row)| row).collect()
    }
}

/// What every command needs: the identity, the relay, and the world.
pub struct Context {
    pub client: Client,
    pub identity: verse::identity::Identity,
    pub world: String,
    pub entity: String,
    /// The role the entity's state and frames carry, `avatar` by default.
    pub role: String,
}

impl Context {
    pub fn open(args: &Args) -> Result<Self, String> {
        Self::open_as(args, false)
    }

    /// Opens the context; a `read_only` command signs with the existing key
    /// or a temporary one, never creating an identity on disk (#10320).
    pub fn open_as(args: &Args, read_only: bool) -> Result<Self, String> {
        let identity = if read_only {
            crate::relay::reader_identity_for(args.option("as"))?
        } else {
            identity_for(args.option("as"))?
        };
        let signer = identity.signer.clone();
        let world = args
            .option("world")
            .map(str::to_owned)
            .unwrap_or_else(|| verse::session::WORLD.to_owned());
        if world.is_empty() || world.len() > 128 {
            return Err("--world is 1 to 128 bytes".into());
        }
        let entity = args
            .option("entity")
            .map(str::to_owned)
            .unwrap_or_else(|| "avatar".to_owned());
        Ok(Self {
            client: Client::connect(&relay_url(args.option("relay")), signer),
            identity,
            world,
            entity,
            role: "avatar".to_owned(),
        })
    }

    pub fn pubkey(&self) -> &str {
        self.identity.signer.pubkey()
    }

    /// The states the relay holds for the world.
    pub fn states(&mut self, scene: &mut Scene, wait: Duration) -> Result<(), String> {
        let world = self.world.clone();
        self.client.subscribe(
            vec![json!({"kinds": [mv::STATE_KIND], "#w": [world], "limit": 500})],
            false,
            wait,
            |event| {
                scene.absorb(event, &world);
            },
        )
    }

    /// This identity's own last state, if the relay holds one.
    pub fn own_state(&mut self, wait: Duration) -> Result<Option<State>, String> {
        let world = self.world.clone();
        let address = mv::state_address(&world, &self.entity);
        let mut found: Option<State> = None;
        self.client.subscribe(
            vec![json!({
                "kinds": [mv::STATE_KIND],
                "authors": [self.pubkey()],
                "#d": [address],
                "limit": 1,
            })],
            false,
            wait,
            |event| {
                if let Ok(Received::State { state, .. }) = mv::decode(event, &world)
                    && found.as_ref().is_none_or(|old| old.t < state.t)
                {
                    found = Some(state);
                }
            },
        )?;
        Ok(found)
    }

    /// Where this identity stands: `--at`, else its stored state, else the origin.
    pub fn origin(&mut self, args: &Args) -> Result<(Vec3, Quat), String> {
        if let Some(at) = args.option("at") {
            return Ok((Vec3::from(Args::vec3(at)?), Quat::IDENTITY));
        }
        Ok(self
            .own_state(DEFAULT_WAIT)?
            .map(|state| (Vec3::from(state.p), Quat::from_array(state.q)))
            .unwrap_or((Vec3::ZERO, Quat::IDENTITY)))
    }

    /// Publish this identity's state and one frame at `pos`.
    pub fn stand(
        &mut self,
        pos: Vec3,
        rot: Quat,
        name: Option<String>,
        online: bool,
    ) -> Result<crate::relay::Published, String> {
        let now = unix_now();
        let millis = now * 1000;
        let state = State {
            v: 1,
            id: self.entity.clone(),
            role: self.role.clone(),
            p: pos.to_array(),
            q: rot.to_array(),
            t: millis,
            online,
            follows: None,
            name,
            set: None,
            b: None,
        };
        let state_event = mv::state_event(&self.identity.signer, &self.world, &state, now);
        if online {
            let frame = Frame {
                v: 1,
                s: verse::identity::random_hex(4),
                n: 1,
                t: millis,
                e: vec![EntityPose::new(&self.entity, &self.role, pos, rot)],
            };
            self.client.send(mv::frame_event(
                &self.identity.signer,
                &self.world,
                &frame,
                now,
            ));
        }
        self.client.publish(state_event, DEFAULT_WAIT)
    }
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("verse", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("verse", &message, USAGE),
    };
    let wait = match args.number::<u64>("wait", 0) {
        Ok(seconds) => seconds,
        Err(message) => return output.usage("verse", &message, USAGE),
    };
    let quest = match command.as_str() {
        "quests" => Some(crate::quest::quests(output, &args)),
        "xp" => Some(crate::quest::xp(output, &args)),
        "board" => Some(crate::quest::board(output, &args)),
        _ => None,
    };
    if let Some(result) = quest {
        return match result {
            Ok(code) => code,
            Err(message) => output.fail("verse", &message),
        };
    }
    let read_only = matches!(
        command.as_str(),
        "who" | "look" | "nearby" | "chat" | "tail"
    );
    let mut context = match Context::open_as(&args, read_only) {
        Ok(context) => context,
        Err(message) => return output.fail("verse", &message),
    };
    let result = match command.as_str() {
        "who" => who(output, &mut context, &args),
        "look" | "nearby" => look(output, &mut context, &args, wait),
        "chat" => chat(output, &mut context, &args),
        "tail" => tail(output, &mut context, wait),
        "me" => me(output, &mut context),
        "move" | "go" => move_to(output, &mut context, &args),
        "say" => say(output, &mut context, &args),
        "gesture" => gesture(output, &mut context, &args),
        "name" => name(output, &mut context, &args),
        "leave" => leave(output, &mut context),
        "control" => control(output, &mut context, &args),
        other => return output.usage("verse", &format!("unknown command `{other}`"), USAGE),
    };
    context.client.close();
    match result {
        Ok(code) => code,
        Err(message) => output.fail("verse", &message),
    }
}

/// Offline entities whose last state is older than this are left out of
/// the text listing (`--json` keeps every one).
const STALE_OFFLINE_MS: u64 = 24 * 60 * 60 * 1000;

fn render_rows(value: &Value) -> String {
    render_rows_at(value, unix_now().saturating_mul(1000))
}

/// The listing as text: one row per publisher (its entities, such as an
/// `agent` and its `avatar`, together), people who are here first, and
/// long-offline entities counted rather than listed.
fn render_rows_at(value: &Value, now_ms: u64) -> String {
    let Some(rows) = value["entities"].as_array() else {
        return String::new();
    };
    let state_rank = |row: &Value| match (
        row["live"].as_bool().unwrap_or(false),
        row["online"].as_bool().unwrap_or(false),
    ) {
        (true, _) => 0,
        (false, true) => 1,
        (false, false) => 2,
    };
    // Group by publisher, keeping the first (nearest) row's order.
    let mut groups: Vec<(String, Vec<&Value>)> = Vec::new();
    for row in rows {
        let pubkey = row["pubkey"].as_str().unwrap_or("").to_owned();
        match groups.iter_mut().find(|(key, _)| *key == pubkey) {
            Some((_, members)) => members.push(row),
            None => groups.push((pubkey, vec![row])),
        }
    }
    let mut hidden = 0usize;
    let mut kept: Vec<(u8, usize, Vec<&Value>)> = Vec::new();
    for (index, (_, members)) in groups.into_iter().enumerate() {
        let rank = members.iter().map(|row| state_rank(row)).min().unwrap_or(2);
        let newest = members
            .iter()
            .filter_map(|row| row["t"].as_u64())
            .max()
            .unwrap_or(0);
        if rank == 2 && now_ms.saturating_sub(newest) > STALE_OFFLINE_MS {
            hidden += 1;
            continue;
        }
        kept.push((rank, index, members));
    }
    kept.sort_by_key(|(rank, index, _)| (*rank, *index));
    if kept.is_empty() {
        return match hidden {
            0 => "Nobody here.".into(),
            n => format!("Nobody here now ({n} offline for over a day; --json lists them)."),
        };
    }
    let mut table = vec![vec![
        "distance".to_owned(),
        "name".to_owned(),
        "role".to_owned(),
        "pos".to_owned(),
        "state".to_owned(),
        "pubkey".to_owned(),
    ]];
    for (rank, _, members) in &kept {
        let row = members[0];
        let pos = row["pos"]
            .as_array()
            .map(|pos| {
                pos.iter()
                    .map(|n| format!("{:.1}", n.as_f64().unwrap_or(0.0)))
                    .collect::<Vec<_>>()
                    .join(",")
            })
            .unwrap_or_default();
        let name = members
            .iter()
            .find_map(|row| {
                row["name"]
                    .as_str()
                    .filter(|name| !name.is_empty() && *name != "-")
            })
            .unwrap_or("(unnamed)");
        let mut roles: Vec<&str> = members
            .iter()
            .filter_map(|row| row["role"].as_str())
            .filter(|role| !role.is_empty())
            .collect();
        roles.dedup();
        table.push(vec![
            row["distance"]
                .as_f64()
                .map(|d| format!("{d:.1}m"))
                .unwrap_or_default(),
            name.to_owned(),
            roles.join("+"),
            pos,
            match rank {
                0 => "live",
                1 => "online",
                _ => "offline",
            }
            .to_owned(),
            row["pubkey"].as_str().unwrap_or("").to_owned(),
        ]);
    }
    let mut text = out::table(&table);
    if hidden > 0 {
        text.push_str(&format!(
            "\n{hidden} more offline for over a day; --json lists them."
        ));
    }
    text
}

fn who(output: &Output, context: &mut Context, args: &Args) -> Result<u8, String> {
    let mut scene = Scene::default();
    context.states(&mut scene, DEFAULT_WAIT)?;
    let origin = context.origin(args)?.0;
    let me = context.pubkey().to_owned();
    let rows = scene.rows(Some(origin), None);
    output.emit(
        &json!({
            "world": context.world,
            "origin": origin.to_array(),
            "me": me,
            "entities": rows,
        }),
        render_rows,
    );
    Ok(0)
}

fn look(output: &Output, context: &mut Context, args: &Args, wait: u64) -> Result<u8, String> {
    let radius: i64 = args.number("radius", 1)?;
    let wait = Duration::from_secs(if wait == 0 { 3 } else { wait });
    let (origin, _) = context.origin(args)?;
    let mut scene = Scene::default();
    context.states(&mut scene, DEFAULT_WAIT)?;
    let cells = mv::cells_around(origin, radius.clamp(0, 8));
    let world = context.world.clone();
    context.client.subscribe(
        vec![json!({
            "kinds": [mv::FRAME_KIND, mv::GESTURE_KIND, mv::STATE_KIND],
            "#w": [world],
            "#c": cells,
        })],
        true,
        wait,
        |event| {
            scene.absorb(event, &world);
        },
    )?;
    let reach = (radius as f32 + 0.5) * mv::CELL;
    let me = context.pubkey().to_owned();
    let rows: Vec<Value> = scene
        .rows(Some(origin), Some(&me))
        .into_iter()
        .filter(|row| row["distance"].as_f64().unwrap_or(f64::MAX) <= f64::from(reach))
        .collect();
    output.emit(
        &json!({
            "world": context.world,
            "origin": origin.to_array(),
            "radius_m": reach,
            "entities": rows,
        }),
        render_rows,
    );
    Ok(0)
}

fn chat_value(line: &mv::ChatLine) -> Value {
    json!({
        "id": line.id,
        "created_at": line.created_at,
        "pubkey": line.pubkey,
        "channel": line.channel,
        "room": line.room,
        "zone": line.zone,
        "pos": line.pos.map(|pos| pos.to_array()),
        "text": line.text,
    })
}

fn render_chat(value: &Value) -> String {
    format!(
        "{} [{}] {}: {}",
        value["created_at"]
            .as_u64()
            .map_or_else(|| value["created_at"].to_string(), crate::relay::when),
        value["channel"].as_str().unwrap_or("room"),
        value["pubkey"]
            .as_str()
            .map(|key| &key[..key.len().min(8)])
            .unwrap_or(""),
        value["text"].as_str().unwrap_or("")
    )
}

fn chat(output: &Output, context: &mut Context, args: &Args) -> Result<u8, String> {
    let limit: u64 = args.number("limit", 50)?;
    let world = context.world.clone();
    let mut lines = Vec::new();
    context.client.subscribe(
        vec![json!({"kinds": [mv::CHAT_KIND], "#w": [world], "limit": limit.min(500)})],
        false,
        DEFAULT_WAIT,
        |event| {
            if let Ok(line) = mv::decode_chat(event, &world) {
                lines.push(line);
            }
        },
    )?;
    lines.sort_by_key(|line| line.created_at);
    let values: Vec<Value> = lines.iter().map(chat_value).collect();
    output.emit(
        &json!({ "world": context.world, "lines": values }),
        |value| {
            value["lines"]
                .as_array()
                .map(|lines| lines.iter().map(render_chat).collect::<Vec<_>>().join("\n"))
                .unwrap_or_default()
        },
    );
    Ok(0)
}

fn tail(output: &Output, context: &mut Context, wait: u64) -> Result<u8, String> {
    let wait = Duration::from_secs(if wait == 0 { 30 } else { wait });
    let world = context.world.clone();
    let mut scene = Scene::default();
    let mut shown = 0usize;
    context.client.subscribe(
        vec![json!({
            "kinds": [mv::FRAME_KIND, mv::GESTURE_KIND, mv::STATE_KIND, mv::CHAT_KIND],
            "#w": [world],
            "since": unix_now().saturating_sub(1),
        })],
        true,
        wait,
        |event| {
            if event.kind == mv::CHAT_KIND {
                if let Ok(line) = mv::decode_chat(event, &world) {
                    let mut value = chat_value(&line);
                    value["type"] = "chat".into();
                    shown += 1;
                    output.line(&value, render_chat);
                }
                return;
            }
            let Some(received) = scene.absorb(event, &world) else {
                return;
            };
            shown += 1;
            let value = match received {
                Received::Frame { pubkey, frame } => json!({
                    "type": "frame", "pubkey": pubkey, "t": frame.t,
                    "entities": frame.e,
                }),
                Received::State { pubkey, state } => json!({
                    "type": "state", "pubkey": pubkey, "state": state,
                }),
                Received::Gesture { pubkey, gesture } => json!({
                    "type": "gesture", "pubkey": pubkey, "gesture": gesture,
                }),
                Received::Command {
                    pubkey,
                    to,
                    command,
                } => json!({
                    "type": "command", "pubkey": pubkey, "to": to, "command": command,
                }),
            };
            output.line(&value, |value| {
                let who = value["pubkey"]
                    .as_str()
                    .map(|key| &key[..key.len().min(8)])
                    .unwrap_or("");
                match value["type"].as_str() {
                    Some("frame") => {
                        let poses = value["entities"]
                            .as_array()
                            .map(|poses| {
                                poses
                                    .iter()
                                    .map(|pose| {
                                        format!(
                                            "{}@{}",
                                            pose["id"].as_str().unwrap_or(""),
                                            pose["p"]
                                                .as_array()
                                                .map(|p| p
                                                    .iter()
                                                    .map(|n| format!(
                                                        "{:.1}",
                                                        n.as_f64().unwrap_or(0.0)
                                                    ))
                                                    .collect::<Vec<_>>()
                                                    .join(","))
                                                .unwrap_or_default()
                                        )
                                    })
                                    .collect::<Vec<_>>()
                                    .join(" ")
                            })
                            .unwrap_or_default();
                        format!("frame   {who} {poses}")
                    }
                    Some("state") => format!(
                        "state   {who} {} online={}",
                        value["state"]["id"].as_str().unwrap_or(""),
                        value["state"]["online"]
                    ),
                    _ => format!(
                        "gesture {who} {} to={}",
                        value["gesture"]["g"].as_str().unwrap_or(""),
                        value["gesture"]["to"]
                    ),
                }
            });
        },
    )?;
    if shown == 0 && !output.json() {
        println!("Nothing arrived in {} s.", wait.as_secs());
    }
    Ok(0)
}

fn me(output: &Output, context: &mut Context) -> Result<u8, String> {
    let state = context.own_state(DEFAULT_WAIT)?;
    output.emit(
        &json!({
            "profile": context.identity.profile,
            "pubkey": context.pubkey(),
            "world": context.world,
            "entity": context.entity,
            "state": state,
        }),
        |value| {
            let state = &value["state"];
            if state.is_null() {
                format!(
                    "{} hasn't entered {} yet. Enter it with: openagents verse move 0,0,0",
                    value["pubkey"].as_str().unwrap_or(""),
                    value["world"].as_str().unwrap_or("")
                )
            } else {
                format!(
                    "{} at {} online={} name={}",
                    value["pubkey"].as_str().unwrap_or(""),
                    state["p"],
                    state["online"],
                    state["name"].as_str().unwrap_or("-")
                )
            }
        },
    );
    Ok(0)
}

/// `control ENTITY ACTION ...`: the move, gesture, and leave commands under
/// another entity id this identity publishes. NIP-MV entities belong to
/// the key that signs them, so this drives only what `--as` already owns.
fn control(output: &Output, context: &mut Context, args: &Args) -> Result<u8, String> {
    let positional = args.positional();
    let (Some(entity), Some(action)) = (positional.first(), positional.get(1)) else {
        return Err("ENTITY and an action (move, gesture, or leave) are required".into());
    };
    if entity.is_empty()
        || entity.len() > 64
        || !entity
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err("ENTITY is 1 to 64 characters of a-z, A-Z, 0-9, -, or _".into());
    }
    if entity == "avatar" {
        return Err("ENTITY is another entity; use move, gesture, and leave for the avatar".into());
    }
    context.entity = entity.clone();
    if let Some(role) = args.option("role") {
        if role.is_empty() || role.len() > 32 {
            return Err("--role is 1 to 32 bytes".into());
        }
        context.role = role.to_owned();
    } else {
        context.role = "agent".to_owned();
    }
    let rest = Args::from_positional(&positional[2..], args);
    match action.as_str() {
        "move" | "go" => move_to(output, context, &rest),
        "gesture" => gesture(output, context, &rest),
        "leave" => leave(output, context),
        other => Err(format!("unknown control action `{other}`")),
    }
}

fn move_to(output: &Output, context: &mut Context, args: &Args) -> Result<u8, String> {
    let Some(target) = args.positional().first() else {
        return Err("a target X,Y,Z is required".into());
    };
    let pos = Vec3::from(Args::vec3(target)?);
    let yaw: f32 = args.number("yaw", 0.0)?;
    let rot = Quat::from_rotation_y(yaw.to_radians());
    let name = match args.option("name") {
        Some(name) => Some(name.to_owned()),
        None => context
            .own_state(DEFAULT_WAIT)?
            .and_then(|state| state.name),
    };
    let published = context.stand(pos, rot, name, true)?;
    output.emit(
        &json!({
            "pubkey": context.pubkey(),
            "entity": context.entity,
            "role": context.role,
            "pos": pos.to_array(),
            "yaw": yaw,
            "accepted": published.accepted,
            "message": published.message,
        }),
        |value| {
            format!(
                "{} standing at {}{}",
                if value["accepted"].as_bool().unwrap_or(false) {
                    "ok"
                } else {
                    "refused"
                },
                value["pos"],
                value["message"]
                    .as_str()
                    .filter(|m| !m.is_empty())
                    .map(|m| format!(": {m}"))
                    .unwrap_or_default()
            )
        },
    );
    Ok(if published.accepted {
        0
    } else {
        crate::EXIT_FAILURE
    })
}

fn say(output: &Output, context: &mut Context, args: &Args) -> Result<u8, String> {
    let text = args.positional().join(" ");
    if text.trim().is_empty() {
        return Err("TEXT is required".into());
    }
    if text.len() > 1024 {
        return Err("TEXT is at most 1024 bytes".into());
    }
    let channel = args.option("to").unwrap_or("near");
    if !["all", "ads", "zone", "near", "here"].contains(&channel) {
        return Err("--to is all, ads, zone, near, or here".into());
    }
    let zone = args.option("zone").unwrap_or("plaza");
    let (pos, _) = context.origin(args)?;
    let event = mv::world_chat_event(
        &context.identity.signer,
        &context.world,
        channel,
        zone,
        pos,
        &text,
        unix_now(),
    );
    let published = context.client.publish(event, DEFAULT_WAIT)?;
    output.emit(
        &json!({
            "id": published.id,
            "accepted": published.accepted,
            "message": published.message,
            "channel": channel,
            "pos": pos.to_array(),
        }),
        |value| {
            format!(
                "{} said on {}",
                if value["accepted"].as_bool().unwrap_or(false) {
                    "ok"
                } else {
                    "refused"
                },
                value["channel"].as_str().unwrap_or("")
            )
        },
    );
    Ok(if published.accepted {
        0
    } else {
        crate::EXIT_FAILURE
    })
}

fn gesture(output: &Output, context: &mut Context, args: &Args) -> Result<u8, String> {
    let Some(name) = args.positional().first() else {
        return Err("a gesture NAME is required".into());
    };
    if name.is_empty()
        || name.len() > 32
        || !name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("NAME is 1 to 32 characters of a-z, 0-9, or -".into());
    }
    let to = match args.option("to") {
        Some(to) => {
            let (pubkey, entity) = to
                .split_once(',')
                .map(|(pubkey, entity)| (pubkey.to_owned(), entity.to_owned()))
                .unwrap_or_else(|| (to.to_owned(), "avatar".to_owned()));
            if pubkey.len() != 64 || !pubkey.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("--to takes a 64-hex public key, optionally ,ENTITY".into());
            }
            Some([pubkey, entity])
        }
        None => None,
    };
    let at = match args.option("at") {
        Some(at) => vec![Args::vec3(at)?],
        None => Vec::new(),
    };
    let duration: Option<f32> = args
        .option("duration")
        .map(str::parse)
        .transpose()
        .ok()
        .flatten();
    let pos = context
        .own_state(DEFAULT_WAIT)?
        .map(|state| Vec3::from(state.p))
        .unwrap_or(Vec3::ZERO);
    let gesture = Gesture {
        v: 1,
        id: context.entity.clone(),
        g: name.clone(),
        t: unix_now() * 1000,
        d: duration,
        at,
        to,
    };
    let event = mv::gesture_event(
        &context.identity.signer,
        &context.world,
        &gesture,
        pos,
        unix_now(),
    );
    let published = context.client.publish(event, DEFAULT_WAIT)?;
    output.emit(
        &json!({
            "id": published.id,
            "accepted": published.accepted,
            "message": published.message,
            "gesture": gesture,
        }),
        |value| {
            format!(
                "{} {}",
                if value["accepted"].as_bool().unwrap_or(false) {
                    "ok"
                } else {
                    "refused"
                },
                value["gesture"]["g"].as_str().unwrap_or("")
            )
        },
    );
    Ok(if published.accepted {
        0
    } else {
        crate::EXIT_FAILURE
    })
}

fn name(output: &Output, context: &mut Context, args: &Args) -> Result<u8, String> {
    let display = args.positional().join(" ");
    if display.trim().is_empty() || display.len() > 64 {
        return Err("NAME is 1 to 64 bytes".into());
    }
    let event = mv::profile_event(&context.identity.signer, &display, unix_now());
    let published = context.client.publish(event, DEFAULT_WAIT)?;
    if let Some(state) = context.own_state(DEFAULT_WAIT)? {
        context.stand(
            Vec3::from(state.p),
            Quat::from_array(state.q),
            Some(display.clone()),
            state.online,
        )?;
    }
    output.emit(
        &json!({ "accepted": published.accepted, "name": display, "pubkey": context.pubkey() }),
        |value| {
            format!(
                "{} {} is now {}",
                if value["accepted"].as_bool().unwrap_or(false) {
                    "ok"
                } else {
                    "refused"
                },
                value["pubkey"].as_str().unwrap_or(""),
                value["name"].as_str().unwrap_or("")
            )
        },
    );
    Ok(if published.accepted {
        0
    } else {
        crate::EXIT_FAILURE
    })
}

fn leave(output: &Output, context: &mut Context) -> Result<u8, String> {
    let (pos, rot, name) = match context.own_state(DEFAULT_WAIT)? {
        Some(state) => (Vec3::from(state.p), Quat::from_array(state.q), state.name),
        None => (Vec3::ZERO, Quat::IDENTITY, None),
    };
    let published = context.stand(pos, rot, name, false)?;
    output.emit(&json!({ "accepted": published.accepted }), |value| {
        if value["accepted"].as_bool().unwrap_or(false) {
            "ok offline".to_owned()
        } else {
            "refused".to_owned()
        }
    });
    Ok(if published.accepted {
        0
    } else {
        crate::EXIT_FAILURE
    })
}

#[cfg(test)]
mod listing_tests {
    use super::*;

    #[test]
    fn a_publishers_entities_share_one_row_and_long_offline_ones_are_counted() {
        let now = 10 * STALE_OFFLINE_MS;
        let value = json!({ "entities": [
            {"pubkey": "aa", "entity": "agent", "role": "agent", "name": "Ada",
             "pos": [0.0, 0.0, 0.0], "distance": 1.0, "online": true, "live": false, "t": now},
            {"pubkey": "aa", "entity": "avatar", "role": "avatar", "name": null,
             "pos": [0.0, 0.0, 0.0], "distance": 1.0, "online": true, "live": false, "t": now},
            {"pubkey": "bb", "entity": "avatar", "role": "avatar", "name": "Old Test",
             "pos": [1.0, 0.0, 0.0], "distance": 0.5, "online": false, "live": false, "t": 1},
            {"pubkey": "cc", "entity": "avatar", "role": "avatar", "name": "-",
             "pos": [2.0, 0.0, 0.0], "distance": 2.0, "online": false, "live": false, "t": now},
        ]});
        let text = render_rows_at(&value, now);
        assert_eq!(text.matches("Ada").count(), 1, "{text}");
        assert!(text.contains("agent+avatar"), "{text}");
        assert!(!text.contains("Old Test"), "{text}");
        assert!(text.contains("(unnamed)"), "{text}");
        assert!(text.contains("1 more offline for over a day"), "{text}");
        assert!(
            text.find("Ada").unwrap() < text.find("(unnamed)").unwrap(),
            "{text}"
        );
    }

    #[test]
    fn an_empty_place_says_so() {
        assert_eq!(render_rows_at(&json!({"entities": []}), 0), "Nobody here.");
    }
}
