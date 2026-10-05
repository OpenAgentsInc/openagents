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
  who [--wait SECONDS]      Every entity with a state in the world, nearest first,
                            and the world's population: players online, players
                            whose frames arrive in a short listen (default 2 s),
                            and their frames a second.
  look [--at X,Y,Z] [--radius METERS] [--wait SECONDS]
                            Listen for live poses around a point (default: where
                            this identity stands) and list what is there.
  chat [--limit N]          Recent world chat lines.
  tail [--wait SECONDS]     Follow poses, gestures, states, and chat as they arrive,
                            then report the live population and frame cadence.
  me                        This identity's public key and last known state.
  load [--players N] [--wait SECONDS]
                            Listen to every pose frame in the world (default 30 s)
                            and report each publisher's rate, gaps, and frame age;
                            exit 1 when fewer than N publishers sent a frame.
  walkers N [--hz RATE] [--loopback] [--wait SECONDS]
                            Walk N simulated players with fresh keys in loops in
                            front of the spawn (default world verse-bare) at the
                            shared 5 Hz cadence, or RATE frames a second, until
                            stopped or for SECONDS; --loopback starts an in-process
                            relay and prints its address. NDJSON progress.
  move X,Y,Z [--yaw DEGREES] [--name NAME]
                            Stand at a point: publish the avatar state and a frame.
  say TEXT [--to all|ads|zone|near|here] [--zone NAME]
                            Speak in world chat from where this identity stands.
  gesture NAME [--to PUBKEY,ENTITY] [--at X,Y,Z] [--duration SECONDS]
                            Perform a gesture (for example greet or look-around).
  name NAME                 Publish a kind 0 profile with this display name.
  leave                     Mark this identity's avatar offline.
  block PLAYER              Hide a player (a public key, npub, or name) from every
                            Verse client on this computer; it stays hidden after
                            relaunch.
  unblock PLAYER            Show a blocked player again.
  mute PLAYER               Keep a player's avatar but hide their chat, private
                            messages, and gestures.
  unmute PLAYER             Hear a muted player again.
  blocked                  The players this computer blocked or muted.
  terminal COMMAND          Drive the terminal overlay of the Verse window on
                            this computer: status, open, hide, split, focus,
                            close, send, key, read, tab, zoom
                            (openagents verse terminal --help).
  control ENTITY move X,Y,Z [--yaw DEGREES] [--role ROLE] [--name NAME]
  control ENTITY gesture NAME [--to PUBKEY,ENTITY] [--at X,Y,Z]
  control ENTITY leave      Drive another entity this identity publishes (for
                            example an agent it spawned): the same events as
                            move, gesture, and leave, under that entity id.
  trust list                List trusted referees (OpenAgents is trusted by default).
  trust add KEY             Trust a referee locally.
  trust remove KEY          Remove a referee from local trust.
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
    Declared::computer("load", Effect::LongRunning),
    Declared::computer("walkers", Effect::Publishes),
    Declared::computer("terminal", Effect::LocalWrite),
    Declared::computer("move", Effect::Publishes),
    Declared::computer("say", Effect::Publishes),
    Declared::computer("gesture", Effect::Publishes),
    Declared::computer("name", Effect::Publishes),
    Declared::computer("leave", Effect::Publishes),
    Declared::computer("block", Effect::LocalWrite),
    Declared::computer("unblock", Effect::LocalWrite),
    Declared::computer("mute", Effect::LocalWrite),
    Declared::computer("unmute", Effect::LocalWrite),
    Declared::computer("blocked", Effect::ReadOnly),
    Declared::computer("control move", Effect::Publishes),
    Declared::computer("control gesture", Effect::Publishes),
    Declared::computer("control leave", Effect::Publishes),
    Declared::computer("trust list", Effect::ReadOnly),
    Declared::computer("trust add", Effect::LocalWrite),
    Declared::computer("trust remove", Effect::LocalWrite),
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
            .map(|name| {
                verse::zones::ZoneId::from_name(name)
                    .map_or_else(|| name.to_owned(), |zone| zone.world_id().to_owned())
            })
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
    run_group(output, words, "verse")
}

pub fn run_xp(output: &Output, words: &[String]) -> u8 {
    let words = std::iter::once("xp".to_owned())
        .chain(words.iter().cloned())
        .collect::<Vec<_>>();
    run_group(output, &words, "xp")
}

fn run_group(output: &Output, words: &[String], group: &str) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage(group, "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    if group == "verse" && command == "terminal" {
        return crate::verse_terminal::run(output, rest);
    }
    let canonical = match command.as_str() {
        "nearby" => "look",
        "go" => "move",
        other => other,
    };
    let (specific, min, max): (&[&str], usize, usize) = match canonical {
        "who" => (&["at", "wait"], 0, 0),
        "look" => (&["at", "radius", "wait"], 0, 0),
        "chat" => (&["limit"], 0, 0),
        "tail" => (&["wait"], 0, 0),
        "load" => (&["wait", "players"], 0, 0),
        "walkers" => (&["wait", "hz"], 1, 1),
        "me" | "leave" | "blocked" => (&[], 0, 0),
        "block" | "unblock" | "mute" | "unmute" => (&[], 1, 1),
        "move" => (&["yaw", "name"], 1, 1),
        "say" => (&["to", "zone", "at"], 1, usize::MAX),
        "gesture" => (&["to", "at", "duration"], 1, 1),
        "name" => (&[], 1, usize::MAX),
        "control" => (&["yaw", "role", "name", "to", "at", "duration"], 2, 3),
        "trust" => (&[], 1, 2),
        "quests" | "board" => (&["xp-relay", "referee", "xp-referee"], 0, 0),
        "xp" => (&["xp-relay", "referee", "xp-referee", "pubkey"], 0, 2),
        other => return output.usage(group, &format!("unknown command `{other}`"), USAGE),
    };
    let usage_command =
        if canonical == "xp" && rest.first().is_some_and(|word| word == "verify-card") {
            "xp verify-card"
        } else {
            canonical
        };
    let usage = if group == "xp" {
        if usage_command == "xp verify-card" {
            crate::argv::command_usage("verse", usage_command, USAGE)
                .unwrap()
                .replace("openagents verse xp", "openagents xp")
        } else {
            xp_usage()
        }
    } else {
        crate::argv::command_usage(group, usage_command, USAGE).unwrap_or_else(|| USAGE.to_owned())
    };
    let label = if group == "xp" {
        usage_command.to_owned()
    } else {
        format!("verse {usage_command}")
    };
    let mut options = vec!["as", "relay", "world", "entity"];
    options.extend_from_slice(specific);
    let switches: &[&str] = if canonical == "walkers" {
        &["loopback"]
    } else {
        &[]
    };
    let args = match crate::argv::parse_command(rest, &label, &options, switches, min, max) {
        Ok(args) => args,
        Err(message) => return output.usage(group, &message, &usage),
    };
    if canonical == "xp"
        && !args.positional().is_empty()
        && (args.positional().first().map(String::as_str) != Some("verify-card")
            || args.positional().len() != 2)
    {
        return output.usage(
            group,
            "expected verify-card CARD, or no positional arguments",
            &usage,
        );
    }
    if canonical == "control" {
        let action = args.positional()[1].as_str();
        let (allowed, count): (&[&str], usize) = match action {
            "move" | "go" => (&["yaw", "role", "name"], 3),
            "gesture" => (&["to", "at", "duration", "role"], 3),
            "leave" => (&["role"], 2),
            _ => return output.usage(group, "control takes move, gesture, or leave", &usage),
        };
        if args.positional().len() != count {
            return output.usage(group, "wrong number of control arguments", &usage);
        }
        for name in args.option_names() {
            if !["as", "relay", "world", "entity"].contains(&name) && !allowed.contains(&name) {
                return output.usage(
                    group,
                    &format!("--{name} isn't an option of verse control {action}"),
                    &usage,
                );
            }
        }
    }
    let wait = match args.number::<u64>("wait", 0) {
        Ok(seconds) => seconds,
        Err(message) => return output.usage(group, &message, USAGE),
    };
    let quest = match command.as_str() {
        "trust" => Some(crate::quest::trust(output, &args)),
        "quests" => Some(crate::quest::quests(output, &args)),
        "xp" => Some(crate::quest::xp(output, &args)),
        "board" => Some(crate::quest::board(output, &args)),
        _ => None,
    };
    if let Some(result) = quest {
        return match result {
            Ok(code) => code,
            Err(message) => output.fail(group, &message),
        };
    }
    if canonical == "walkers" {
        return match crate::walkers::walkers(output, &args, wait) {
            Ok(code) => code,
            Err(message) => output.fail(group, &message),
        };
    }
    if matches!(
        canonical,
        "block" | "unblock" | "mute" | "unmute" | "blocked"
    ) {
        return match people(output, &args, canonical) {
            Ok(code) => code,
            Err(message) => output.fail(group, &message),
        };
    }
    let read_only = matches!(
        command.as_str(),
        "who" | "look" | "nearby" | "chat" | "tail" | "load"
    );
    let mut context = match Context::open_as(&args, read_only) {
        Ok(context) => context,
        Err(message) => return output.fail(group, &message),
    };
    let result = match command.as_str() {
        "who" => who(output, &mut context, &args, wait),
        "look" | "nearby" => look(output, &mut context, &args, wait),
        "chat" => chat(output, &mut context, &args),
        "tail" => tail(output, &mut context, wait),
        "load" => crate::walkers::load(output, &mut context, &args, wait),
        "me" => me(output, &mut context),
        "move" | "go" => move_to(output, &mut context, &args),
        "say" => say(output, &mut context, &args),
        "gesture" => gesture(output, &mut context, &args),
        "name" => name(output, &mut context, &args),
        "leave" => leave(output, &mut context),
        "control" => control(output, &mut context, &args),
        other => return output.usage(group, &format!("unknown command `{other}`"), USAGE),
    };
    context.client.close();
    match result {
        Ok(code) => code,
        Err(message) => output.fail(group, &message),
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

fn who(output: &Output, context: &mut Context, args: &Args, wait: u64) -> Result<u8, String> {
    let mut scene = Scene::default();
    context.states(&mut scene, DEFAULT_WAIT)?;
    let origin = context.origin(args)?.0;
    let me = context.pubkey().to_owned();
    let listen = Duration::from_secs(if wait == 0 { 2 } else { wait });
    let world = context.world.clone();
    let mut cadence = Cadence::default();
    context.client.subscribe(
        vec![json!({
            "kinds": [mv::FRAME_KIND],
            "#w": [world],
            "since": unix_now().saturating_sub(1),
        })],
        true,
        listen,
        |event| {
            if let Some(Received::Frame { pubkey, .. }) = scene.absorb(event, &world) {
                cadence.frame(&pubkey);
            }
        },
    )?;
    let rows = scene.rows(Some(origin), None);
    let population = cadence.population(&scene, listen);
    output.emit(
        &json!({
            "world": context.world,
            "origin": origin.to_array(),
            "me": me,
            "population": population,
            "entities": rows,
        }),
        |value| {
            format!(
                "{}\n{}",
                render_population(&value["population"], value["world"].as_str().unwrap_or("")),
                render_rows(value)
            )
        },
    );
    Ok(0)
}

/// Pose frames counted by publisher during a listen.
#[derive(Default)]
struct Cadence {
    frames: BTreeMap<String, u64>,
}

impl Cadence {
    fn frame(&mut self, pubkey: &str) {
        *self.frames.entry(pubkey.to_owned()).or_default() += 1;
    }

    /// The world's population: players whose avatar state says online,
    /// players whose frames arrived during `listened`, and the median of
    /// their frames a second.
    fn population(&self, scene: &Scene, listened: Duration) -> Value {
        let online = scene
            .entities
            .iter()
            .filter(|((_, id), seen)| id == "avatar" && seen.online)
            .map(|((pubkey, _), _)| pubkey)
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let seconds = listened.as_secs_f64().max(f64::EPSILON);
        let mut rates: Vec<f64> = self
            .frames
            .values()
            .map(|&count| count as f64 / seconds)
            .collect();
        rates.sort_by(f64::total_cmp);
        let median = rates.get(rates.len() / 2).copied();
        json!({
            "online": online,
            "live": self.frames.len(),
            "frames_per_second": median.map(|rate| (rate * 10.0).round() / 10.0),
            "listened_s": listened.as_secs(),
        })
    }
}

fn render_population(value: &Value, world: &str) -> String {
    let rate = value["frames_per_second"]
        .as_f64()
        .map(|rate| format!(", {rate:.1} frames a second each"))
        .unwrap_or_default();
    format!(
        "{world}: {} live in {} s{rate}; {} online.",
        value["live"], value["listened_s"], value["online"]
    )
}

/// `block`, `unblock`, `mute`, `unmute`, and `blocked`: the lists every
/// Verse client on this computer reads from [`verse::identity::home`].
fn people(output: &Output, args: &Args, command: &str) -> Result<u8, String> {
    use verse::blocklist::Blocklist;
    let dir = verse::identity::home();
    let mut list = Blocklist::load(&dir)?;
    let mut changed = None;
    if command != "blocked" {
        let player = args.positional()[0].as_str();
        let listed = match command {
            "unblock" => Some(&list.blocked),
            "unmute" => Some(&list.muted),
            _ => None,
        };
        let pubkey = resolve_player(args, player, listed)?;
        let did = match command {
            "block" => list.block(&pubkey)?,
            "mute" => list.mute(&pubkey)?,
            "unblock" => list.unblock(&pubkey),
            _ => list.unmute(&pubkey),
        };
        if did {
            list.save(&dir)?;
        }
        changed = Some((pubkey, did));
    }
    output.emit(
        &json!({
            "file": Blocklist::path(&dir),
            "pubkey": changed.as_ref().map(|(pubkey, _)| pubkey),
            "changed": changed.as_ref().map(|(_, did)| did),
            "blocked": list.blocked,
            "muted": list.muted,
        }),
        |value| {
            let keys = |list: &str| {
                value[list]
                    .as_array()
                    .map(|keys| {
                        keys.iter()
                            .filter_map(Value::as_str)
                            .collect::<Vec<_>>()
                            .join("\n  ")
                    })
                    .filter(|text| !text.is_empty())
                    .unwrap_or_else(|| "(none)".to_owned())
            };
            format!(
                "blocked:\n  {}\nmuted:\n  {}",
                keys("blocked"),
                keys("muted")
            )
        },
    );
    Ok(0)
}

/// A player's hex public key from `player`: a hex key, an npub, a prefix of
/// a key already in `listed`, or the start of a name a player in the world
/// shows.
fn resolve_player(
    args: &Args,
    player: &str,
    listed: Option<&std::collections::BTreeSet<String>>,
) -> Result<String, String> {
    let lower = player.to_ascii_lowercase();
    if verse::blocklist::is_pubkey(&lower) {
        return Ok(lower);
    }
    if lower.starts_with("npub1") {
        return nostr::nip19::decode_npub(&lower)
            .map(|bytes| bytes.iter().map(|byte| format!("{byte:02x}")).collect())
            .map_err(|error| format!("{player} isn't a valid npub: {error}"));
    }
    if let Some(listed) = listed {
        let matches: Vec<&String> = listed
            .iter()
            .filter(|key| key.starts_with(&lower))
            .collect();
        if let [only] = matches.as_slice() {
            return Ok((*only).clone());
        }
    }
    let mut context = Context::open_as(args, true)?;
    let mut scene = Scene::default();
    let read = context.states(&mut scene, DEFAULT_WAIT);
    context.client.close();
    read?;
    let found: Vec<&String> = scene
        .names
        .iter()
        .filter(|(_, name)| name.to_lowercase().starts_with(&lower))
        .map(|(pubkey, _)| pubkey)
        .collect();
    match found.as_slice() {
        [only] => Ok((*only).clone()),
        [] => Err(format!(
            "no player named {player} in {}; give a public key or npub",
            context.world
        )),
        _ => Err(format!(
            "{} players' names start with {player}; give more of the name or the key",
            found.len()
        )),
    }
}

fn look(output: &Output, context: &mut Context, args: &Args, wait: u64) -> Result<u8, String> {
    let reach: f32 = args.number("radius", 1.5 * mv::CELL)?;
    if !reach.is_finite() || reach < 0.0 {
        return Err("--radius takes a finite, nonnegative distance in meters".into());
    }
    let radius = (reach / mv::CELL).ceil() as i64;
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
    let mut cadence = Cadence::default();
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
            if let Received::Frame { pubkey, .. } = &received {
                cadence.frame(pubkey);
            }
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
    let mut population = cadence.population(&scene, wait);
    population["type"] = "population".into();
    let world = context.world.clone();
    output.line(&population, |value| render_population(value, &world));
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
    let display = verse::session::display_name(&args.positional().join(" ")).ok_or_else(|| {
        format!(
            "NAME needs 1 to {} characters the name tag can draw",
            verse::session::MAX_DISPLAY_NAME
        )
    })?;
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
