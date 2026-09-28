//! NIP-MV, shared 3D worlds: event kinds, content, and validation.
//!
//! Pure protocol, no I/O. Builds the tags and content of pose frames
//! (`23300`), gestures (`23301`), and entity states (`33301`), signs them
//! with a [`RelaySigner`], and parses received events back into typed
//! values, refusing anything malformed. See `nips/openagents/NIP-MV.md`.

use glam::{Quat, Vec3};
use nostr::domain::{Event, RelaySigner, Tag};
use serde::{Deserialize, Serialize};

/// World definition, addressable.
pub const WORLD_KIND: u16 = nostr::kinds::MV_WORLD;
/// Entity state, addressable.
pub const STATE_KIND: u16 = nostr::kinds::MV_STATE;
/// Pose frame, ephemeral.
pub const FRAME_KIND: u16 = nostr::kinds::MV_FRAME;
/// Gesture, ephemeral.
pub const GESTURE_KIND: u16 = nostr::kinds::MV_GESTURE;
/// Zone command, ephemeral.
pub const COMMAND_KIND: u16 = nostr::kinds::MV_COMMAND;
/// Most arguments one zone command carries.
pub const MAX_COMMAND_ARGS: usize = 16;
/// NIP-C7 chat message, used for world chat.
pub const CHAT_KIND: u16 = 9;
/// Default cell size in meters.
pub const CELL: f32 = 64.0;
/// Most entities one frame may carry.
pub const MAX_ENTITIES: usize = 16;
/// Largest coordinate magnitude a receiver accepts, in meters.
const MAX_COORD: f32 = 1.0e6;
/// Role of a shared body: an entity the world defines and any participant
/// may move, under the shared-body authority rules.
pub const BODY_ROLE: &str = "body";
/// Role and entity id of a shared-body snapshot: one participant's record of
/// every shared body's rest pose.
pub const BODIES_ROLE: &str = "bodies";
/// Most bodies one snapshot records.
pub const MAX_BODIES: usize = 64;

/// One entity's pose inside a frame or a state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EntityPose {
    /// Entity id, unique per publisher.
    pub id: String,
    /// `avatar`, `agent`, `object`, or an unknown role.
    pub role: String,
    /// Position in meters.
    pub p: [f32; 3],
    /// Orientation as a unit quaternion `[x, y, z, w]`.
    pub q: [f32; 4],
    /// Velocity in meters per second.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub v: Option<[f32; 3]>,
    /// Entity id this one accompanies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follows: Option<String>,
    /// Short animation state.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub a: Option<String>,
    /// A shared body's angular velocity, world frame, rad/s.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub w: Option<[f32; 3]>,
    /// A shared body's authority stamp: `[epoch, rev]`. The publisher is
    /// the owner.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub k: Option<[u64; 2]>,
    /// A shared body that came to rest at this pose.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub r: bool,
}

impl EntityPose {
    /// A pose for `id` from engine types.
    #[must_use]
    pub fn new(id: &str, role: &str, pos: Vec3, rot: Quat) -> Self {
        Self {
            id: id.to_owned(),
            role: role.to_owned(),
            p: pos.to_array(),
            q: rot.to_array(),
            v: None,
            follows: None,
            a: None,
            w: None,
            k: None,
            r: false,
        }
    }

    /// Position as a vector.
    #[must_use]
    pub fn pos(&self) -> Vec3 {
        Vec3::from(self.p)
    }

    /// Orientation, normalized.
    #[must_use]
    pub fn rot(&self) -> Quat {
        Quat::from_array(self.q).normalize()
    }

    fn check(&self) -> Result<(), String> {
        check_id(&self.id)?;
        if self.role.is_empty() || self.role.len() > 32 {
            return Err("role must be 1 to 32 bytes".into());
        }
        if !self.p.iter().all(|c| c.is_finite() && c.abs() < MAX_COORD) {
            return Err("position is not finite or is out of range".into());
        }
        if !self.q.iter().all(|c| c.is_finite()) || Quat::from_array(self.q).length() < 1e-3 {
            return Err("orientation is not a usable quaternion".into());
        }
        for v in [self.v, self.w].into_iter().flatten() {
            if !v.iter().all(|c| c.is_finite() && c.abs() < MAX_COORD) {
                return Err("velocity is not finite".into());
            }
        }
        if (self.role == BODY_ROLE) != self.k.is_some() {
            return Err("a shared body, and only a shared body, carries a stamp".into());
        }
        Ok(())
    }
}

/// One shared body's rest pose in a snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BodyRest {
    /// Body id, from the world's body set.
    pub id: String,
    /// Rest position.
    pub p: [f32; 3],
    /// Rest orientation.
    pub q: [f32; 4],
    /// Authority stamp, `[epoch, rev]`, of the motion that ended here.
    pub k: [u64; 2],
    /// The owner that stamped it, when not the snapshot's publisher.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub o: Option<String>,
}

/// Content of a pose frame.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    /// Content version.
    pub v: u32,
    /// Session id.
    pub s: String,
    /// Sequence number within the session.
    pub n: u64,
    /// Publisher time in milliseconds.
    pub t: u64,
    /// Entity poses.
    pub e: Vec<EntityPose>,
}

/// Content of an entity state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct State {
    /// Content version.
    pub v: u32,
    /// Entity id.
    pub id: String,
    /// Entity role.
    pub role: String,
    /// Last known position.
    pub p: [f32; 3],
    /// Last known orientation.
    pub q: [f32; 4],
    /// Publisher time in milliseconds.
    pub t: u64,
    /// Whether the publisher is streaming frames for this entity.
    pub online: bool,
    /// Entity id this one accompanies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub follows: Option<String>,
    /// Display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// A snapshot's body set: the world-defined catalog its ids name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set: Option<String>,
    /// A snapshot's rest poses, one per body.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub b: Option<Vec<BodyRest>>,
}

impl State {
    /// The pose this state records.
    #[must_use]
    pub fn pose(&self) -> EntityPose {
        EntityPose {
            follows: self.follows.clone(),
            ..EntityPose::new(
                &self.id,
                &self.role,
                Vec3::from(self.p),
                Quat::from_array(self.q),
            )
        }
    }

    /// Checks a shared-body snapshot's set and bodies.
    fn check_bodies(&self) -> Result<(), String> {
        let is_snapshot = self.role == BODIES_ROLE;
        if is_snapshot != (self.set.is_some() && self.b.is_some()) {
            return Err("a snapshot, and only a snapshot, names a set and bodies".into());
        }
        if self.role == BODY_ROLE {
            return Err("a shared body's state belongs in a snapshot".into());
        }
        let (Some(set), Some(bodies)) = (&self.set, &self.b) else {
            return Ok(());
        };
        if set.is_empty() || set.len() > 64 {
            return Err("a body set is 1 to 64 bytes".into());
        }
        if bodies.len() > MAX_BODIES {
            return Err("a snapshot records at most 64 bodies".into());
        }
        let mut ids = std::collections::HashSet::new();
        for body in bodies {
            let pose = EntityPose {
                k: Some(body.k),
                ..EntityPose::new(
                    &body.id,
                    BODY_ROLE,
                    Vec3::ZERO,
                    Quat::from_array(body.q).normalize(),
                )
            };
            EntityPose {
                p: body.p,
                q: body.q,
                ..pose
            }
            .check()?;
            if !ids.insert(body.id.as_str()) {
                return Err("a snapshot records each body once".into());
            }
            if body.o.as_deref().is_some_and(|o| !is_hex_key(o)) {
                return Err("a body's owner is a malformed pubkey".into());
            }
        }
        Ok(())
    }
}

/// Content of a gesture.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Gesture {
    /// Content version.
    pub v: u32,
    /// Entity performing it.
    pub id: String,
    /// Gesture name.
    pub g: String,
    /// Publisher time in milliseconds.
    pub t: u64,
    /// Duration in seconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub d: Option<f32>,
    /// Positions the gesture is directed at.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub at: Vec<[f32; 3]>,
    /// The entity the gesture is for: `[pubkey, entity id]`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<[String; 2]>,
}

/// One argument of a zone command.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Arg {
    /// A number.
    Number(f64),
    /// A short string.
    Text(String),
}

/// Content of a zone command: one verb for the client that simulates a
/// loaded zone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Command {
    /// Content version.
    pub v: u32,
    /// The loaded zone the command is for.
    pub zone: String,
    /// The verb.
    pub cmd: String,
    /// Arguments.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub args: Vec<Arg>,
    /// Publisher time in milliseconds.
    pub t: u64,
    /// A short opaque id the operator echoes in its report.
    pub id: String,
}

impl Command {
    /// The numeric arguments, when every argument is a number.
    #[must_use]
    pub fn numbers(&self) -> Option<Vec<f64>> {
        self.args
            .iter()
            .map(|arg| match arg {
                Arg::Number(n) => Some(*n),
                Arg::Text(_) => None,
            })
            .collect()
    }
}

/// A received NIP-MV event, validated.
#[derive(Clone, Debug, PartialEq)]
pub enum Received {
    /// A zone command from `pubkey`, addressed to `to`.
    Command {
        /// Sender.
        pubkey: String,
        /// The operator the command is for.
        to: String,
        /// Command content.
        command: Command,
    },
    /// A pose frame from `pubkey`.
    Frame {
        /// Publisher.
        pubkey: String,
        /// Frame content.
        frame: Frame,
    },
    /// An entity state from `pubkey`.
    State {
        /// Publisher.
        pubkey: String,
        /// State content.
        state: State,
    },
    /// A gesture from `pubkey`.
    Gesture {
        /// Publisher.
        pubkey: String,
        /// Gesture content.
        gesture: Gesture,
    },
}

/// The cell tag value for a position.
#[must_use]
pub fn cell(pos: Vec3) -> String {
    format!(
        "{},{}",
        (pos.x / CELL).floor() as i64,
        (pos.z / CELL).floor() as i64
    )
}

/// The cell tag values in a square of `radius` cells around `pos`.
#[must_use]
pub fn cells_around(pos: Vec3, radius: i64) -> Vec<String> {
    let cx = (pos.x / CELL).floor() as i64;
    let cz = (pos.z / CELL).floor() as i64;
    let mut out = Vec::new();
    for dx in -radius..=radius {
        for dz in -radius..=radius {
            out.push(format!("{},{}", cx + dx, cz + dz));
        }
    }
    out
}

/// The `d` tag of an entity state.
#[must_use]
pub fn state_address(world: &str, id: &str) -> String {
    format!("{world}/{id}")
}

fn tag(values: &[&str]) -> Tag {
    Tag::new(values.iter().map(|v| (*v).to_owned()).collect())
}

fn cell_tags(poses: impl Iterator<Item = Vec3>) -> Vec<Tag> {
    let mut cells: Vec<String> = poses.map(cell).collect();
    cells.sort();
    cells.dedup();
    cells.iter().map(|c| tag(&["c", c])).collect()
}

/// Signs a pose frame.
///
/// # Panics
///
/// Never in practice: serializing owned plain data cannot fail.
#[must_use]
pub fn frame_event(signer: &RelaySigner, world: &str, frame: &Frame, now: u64) -> Event {
    let mut tags = vec![tag(&["w", world])];
    tags.extend(cell_tags(frame.e.iter().map(EntityPose::pos)));
    let content = serde_json::to_string(frame).expect("a frame serializes");
    signer.sign(now, FRAME_KIND, tags, content)
}

/// Signs an entity state.
///
/// # Panics
///
/// Never in practice: serializing owned plain data cannot fail.
#[must_use]
pub fn state_event(signer: &RelaySigner, world: &str, state: &State, now: u64) -> Event {
    let mut tags = vec![
        tag(&["d", &state_address(world, &state.id)]),
        tag(&["w", world]),
        tag(&["role", &state.role]),
    ];
    tags.extend(cell_tags(std::iter::once(Vec3::from(state.p))));
    let content = serde_json::to_string(state).expect("a state serializes");
    signer.sign(now, STATE_KIND, tags, content)
}

/// Signs a gesture at `pos`.
///
/// # Panics
///
/// Never in practice: serializing owned plain data cannot fail.
#[must_use]
pub fn gesture_event(
    signer: &RelaySigner,
    world: &str,
    gesture: &Gesture,
    pos: Vec3,
    now: u64,
) -> Event {
    let mut tags = vec![tag(&["w", world]), tag(&["c", &cell(pos)])];
    if let Some([pubkey, _]) = &gesture.to {
        tags.push(tag(&["p", pubkey]));
    }
    let content = serde_json::to_string(gesture).expect("a gesture serializes");
    signer.sign(now, GESTURE_KIND, tags, content)
}

/// Signs a zone command for the operator `to`.
///
/// # Panics
///
/// Never in practice: serializing owned plain data cannot fail.
#[must_use]
pub fn command_event(
    signer: &RelaySigner,
    world: &str,
    to: &str,
    command: &Command,
    now: u64,
) -> Event {
    let tags = vec![
        tag(&["w", world]),
        tag(&["z", &command.zone]),
        tag(&["p", to]),
    ];
    let content = serde_json::to_string(command).expect("a command serializes");
    signer.sign(now, COMMAND_KIND, tags, content)
}

/// A world chat line: NIP-C7 kind `9` scoped by NIP-MV tags. `channel` is
/// the `t` value (`all`, `ads`, `zone`, `near`, `here`), `zone` the `z`
/// value, and `pos` the speaker's position, carried as the `c` cell and a
/// `pos` tag so listeners can apply distance scopes.
#[must_use]
pub fn world_chat_event(
    signer: &RelaySigner,
    world: &str,
    channel: &str,
    zone: &str,
    pos: Vec3,
    text: &str,
    now: u64,
) -> Event {
    let tags = vec![
        tag(&["w", world]),
        tag(&["t", channel]),
        tag(&["z", zone]),
        tag(&["c", &cell(pos)]),
        tag(&[
            "pos",
            &format!("{:.2}", pos.x),
            &format!("{:.2}", pos.y),
            &format!("{:.2}", pos.z),
        ]),
    ];
    signer.sign(now, CHAT_KIND, tags, text.to_owned())
}

/// A NIP-29 group chat line: kind `9` with the group's `h` tag.
#[must_use]
pub fn room_chat_event(signer: &RelaySigner, room: &str, text: &str, now: u64) -> Event {
    signer.sign(now, CHAT_KIND, vec![tag(&["h", room])], text.to_owned())
}

/// A decoded chat line.
#[derive(Clone, Debug, PartialEq)]
pub struct ChatLine {
    /// Speaker.
    pub pubkey: String,
    /// `t` channel for world chat, or `None` for a room line.
    pub channel: Option<String>,
    /// NIP-29 group for a room line.
    pub room: Option<String>,
    /// `z` zone.
    pub zone: Option<String>,
    /// Speaker position, when given.
    pub pos: Option<Vec3>,
    /// Text.
    pub text: String,
    /// Event time, seconds.
    pub created_at: u64,
    /// Event id.
    pub id: String,
}

/// Validates and decodes a kind `9` world or room chat line.
///
/// # Errors
///
/// Returns why the event is not a usable chat line.
pub fn decode_chat(event: &Event, world: &str) -> Result<ChatLine, String> {
    if event.kind != CHAT_KIND {
        return Err("not a chat line".into());
    }
    event.validate_id().map_err(|e| e.to_string())?;
    event.validate_crypto().map_err(|e| e.to_string())?;
    if event.content.chars().count() > 2_000 {
        return Err("chat line too long".into());
    }
    let one = |name: &str| event.tag_values(name).next().map(str::to_owned);
    let room = one("h");
    let channel = one("t");
    if room.is_none() {
        let worlds: Vec<&str> = event.tag_values("w").collect();
        if worlds != [world] || channel.is_none() {
            return Err("not a chat line for this world".into());
        }
    }
    let pos = event
        .tags
        .iter()
        .find(|t| t.name() == Some("pos"))
        .and_then(|t| {
            let v = t.as_slice();
            let n = |i: usize| {
                v.get(i)?
                    .parse::<f32>()
                    .ok()
                    .filter(|x| x.is_finite() && x.abs() < MAX_COORD)
            };
            Some(Vec3::new(n(1)?, n(2)?, n(3)?))
        });
    Ok(ChatLine {
        pubkey: event.pubkey.clone(),
        channel,
        room,
        zone: one("z"),
        pos,
        text: event.content.clone(),
        created_at: event.created_at,
        id: event.id.clone(),
    })
}

/// A NIP-01 profile (kind 0) naming this player.
///
/// # Panics
///
/// Never in practice: serializing a JSON object cannot fail.
#[must_use]
pub fn profile_event(signer: &RelaySigner, name: &str, now: u64) -> Event {
    let content = serde_json::json!({ "name": name }).to_string();
    signer.sign(now, 0, Vec::new(), content)
}

/// Validates and decodes a received event for `world`.
///
/// # Errors
///
/// Returns why the event is not a usable NIP-MV event for this world.
pub fn decode(event: &Event, world: &str) -> Result<Received, String> {
    event.validate_id().map_err(|e| e.to_string())?;
    event.validate_crypto().map_err(|e| e.to_string())?;
    let worlds: Vec<&str> = event.tag_values("w").collect();
    if worlds != [world] {
        return Err("the event names a different world, or more than one".into());
    }
    if event.content.len() > 16 * 1024 {
        return Err("content is too large".into());
    }
    let pubkey = event.pubkey.clone();
    match event.kind {
        FRAME_KIND => {
            let frame: Frame = serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
            if frame.v != 1 {
                return Err(format!("unsupported frame version {}", frame.v));
            }
            if frame.s.is_empty() || frame.s.len() > 16 {
                return Err("session id must be 1 to 16 bytes".into());
            }
            if frame.e.is_empty() || frame.e.len() > MAX_ENTITIES {
                return Err("a frame carries 1 to 16 entities".into());
            }
            for pose in &frame.e {
                pose.check()?;
            }
            Ok(Received::Frame { pubkey, frame })
        }
        STATE_KIND => {
            let state: State = serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
            if state.v != 1 {
                return Err(format!("unsupported state version {}", state.v));
            }
            let d: Vec<&str> = event.tag_values("d").collect();
            if d != [state_address(world, &state.id).as_str()] {
                return Err("the d tag does not match the entity".into());
            }
            if event.tag_values("role").collect::<Vec<_>>() != [state.role.as_str()] {
                return Err("the role tag does not match the entity".into());
            }
            state.pose().check()?;
            state.check_bodies()?;
            Ok(Received::State { pubkey, state })
        }
        GESTURE_KIND => {
            let gesture: Gesture =
                serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
            if gesture.v != 1
                || gesture
                    .d
                    .is_some_and(|d| !d.is_finite() || !(0.0..=60.0).contains(&d))
                || gesture
                    .at
                    .iter()
                    .flatten()
                    .any(|n| !n.is_finite() || n.abs() >= MAX_COORD)
            {
                return Err("gesture version, duration, or target is invalid".into());
            }
            check_id(&gesture.id)?;
            let name_ok = !gesture.g.is_empty()
                && gesture.g.len() <= 32
                && gesture
                    .g
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
            if !name_ok || gesture.at.len() > 8 {
                return Err("malformed gesture".into());
            }
            if let Some([pubkey, id]) = &gesture.to {
                let hex = pubkey.len() == 64
                    && pubkey
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
                if !hex {
                    return Err("a gesture's `to` names a malformed pubkey".into());
                }
                check_id(id)?;
            }
            Ok(Received::Gesture { pubkey, gesture })
        }
        COMMAND_KIND => {
            let command: Command =
                serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
            if command.v != 1 {
                return Err(format!("unsupported command version {}", command.v));
            }
            check_name(&command.zone)?;
            check_name(&command.cmd)?;
            if command.id.is_empty() || command.id.len() > 16 {
                return Err("command id must be 1 to 16 bytes".into());
            }
            if command.args.len() > MAX_COMMAND_ARGS {
                return Err("a command carries at most 16 arguments".into());
            }
            for arg in &command.args {
                match arg {
                    Arg::Number(n) if !n.is_finite() || n.abs() >= f64::from(MAX_COORD) => {
                        return Err("a command argument is not a finite number".into());
                    }
                    Arg::Text(text) if text.len() > 128 => {
                        return Err("a command argument is longer than 128 bytes".into());
                    }
                    _ => {}
                }
            }
            if event.tag_values("z").collect::<Vec<_>>() != [command.zone.as_str()] {
                return Err("the z tag does not match the zone".into());
            }
            let to: Vec<&str> = event.tag_values("p").collect();
            let [to] = to.as_slice() else {
                return Err("a command names exactly one operator".into());
            };
            if !is_hex_key(to) {
                return Err("a command's operator is a malformed pubkey".into());
            }
            Ok(Received::Command {
                pubkey,
                to: (*to).to_owned(),
                command,
            })
        }
        other => Err(format!("kind {other} is not a NIP-MV event")),
    }
}

fn is_hex_key(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn check_name(name: &str) -> Result<(), String> {
    let ok = !name.is_empty()
        && name.len() <= 32
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(format!("{name:?} is not 1 to 32 bytes of [a-z0-9-]"))
    }
}

fn check_id(id: &str) -> Result<(), String> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(format!(
            "entity id {id:?} is not 1 to 64 bytes of [a-z0-9_-]"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORLD: &str = "test-world";

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"11".repeat(32)).expect("a valid key")
    }

    fn frame() -> Frame {
        Frame {
            v: 1,
            s: "abcd".into(),
            n: 7,
            t: 1_790_000_000_000,
            e: vec![EntityPose::new(
                "avatar",
                "avatar",
                Vec3::new(3.0, 0.0, -70.0),
                Quat::from_rotation_y(0.5),
            )],
        }
    }

    #[test]
    fn a_frame_round_trips() {
        let event = frame_event(&signer(), WORLD, &frame(), 1_790_000_000);
        assert_eq!(event.kind, FRAME_KIND);
        assert!(event.tag_values("c").any(|c| c == "0,-2"));
        match decode(&event, WORLD) {
            Ok(Received::Frame { pubkey, frame: got }) => {
                assert_eq!(pubkey, signer().pubkey());
                assert_eq!(got, frame());
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_state_round_trips_with_its_address() {
        let state = State {
            v: 1,
            id: "agent".into(),
            role: "agent".into(),
            p: [1.0, 2.0, 3.0],
            q: [0.0, 0.0, 0.0, 1.0],
            t: 5,
            online: false,
            follows: Some("avatar".into()),
            name: None,
            set: None,
            b: None,
        };
        let event = state_event(&signer(), WORLD, &state, 1_790_000_000);
        assert_eq!(event.kind, STATE_KIND);
        assert_eq!(
            event.tag_values("d").collect::<Vec<_>>(),
            ["test-world/agent"]
        );
        assert!(matches!(
            decode(&event, WORLD),
            Ok(Received::State { state: got, .. }) if got == state
        ));
    }

    fn snapshot() -> State {
        State {
            v: 1,
            id: BODIES_ROLE.into(),
            role: BODIES_ROLE.into(),
            p: [0.0; 3],
            q: [0.0, 0.0, 0.0, 1.0],
            t: 9,
            online: true,
            follows: None,
            name: None,
            set: Some("verse-bare.bodies.v1".into()),
            b: Some(vec![
                BodyRest {
                    id: "ball".into(),
                    p: [0.0, 1.2, 12.0],
                    q: [0.0, 0.0, 0.0, 1.0],
                    k: [1, 4],
                    o: None,
                },
                BodyRest {
                    id: "cube-0".into(),
                    p: [-5.0, 0.4, 8.0],
                    q: [0.0, 0.38, 0.0, 0.92],
                    k: [1, 2],
                    o: Some("ab".repeat(32)),
                },
            ]),
        }
    }

    #[test]
    fn a_body_snapshot_round_trips_and_is_checked() {
        let event = state_event(&signer(), WORLD, &snapshot(), 1);
        assert_eq!(
            event.tag_values("d").collect::<Vec<_>>(),
            ["test-world/bodies"]
        );
        assert!(matches!(
            decode(&event, WORLD),
            Ok(Received::State { state, .. }) if state == snapshot()
        ));
        let mut twice = snapshot();
        let first = twice.b.as_ref().unwrap()[0].clone();
        twice.b.as_mut().unwrap().push(first);
        let mut owner = snapshot();
        owner.b.as_mut().unwrap()[1].o = Some("nobody".into());
        let mut far = snapshot();
        far.b.as_mut().unwrap()[0].p = [2.0e6, 0.0, 0.0];
        let mut unnamed = snapshot();
        unnamed.set = None;
        let mut crowded = snapshot();
        let one = crowded.b.as_ref().unwrap()[0].clone();
        crowded.b = Some(
            (0..=MAX_BODIES)
                .map(|n| BodyRest {
                    id: format!("b-{n}"),
                    ..one.clone()
                })
                .collect(),
        );
        let mut avatar = snapshot();
        avatar.id = "avatar".into();
        avatar.role = "avatar".into();
        for bad in [twice, owner, far, unnamed, crowded, avatar] {
            let event = state_event(&signer(), WORLD, &bad, 1);
            assert!(decode(&event, WORLD).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn only_a_shared_body_carries_a_stamp() {
        let mut body = EntityPose {
            k: Some([0, 1]),
            w: Some([0.0, 3.0, 0.0]),
            ..EntityPose::new("ball", BODY_ROLE, Vec3::new(0.0, 1.2, 4.0), Quat::IDENTITY)
        };
        let mut with_body = frame();
        with_body.e.push(body.clone());
        let event = frame_event(&signer(), WORLD, &with_body, 1);
        assert!(matches!(
            decode(&event, WORLD),
            Ok(Received::Frame { frame, .. }) if frame == with_body
        ));
        body.k = None;
        let mut unstamped = frame();
        unstamped.e.push(body);
        assert!(decode(&frame_event(&signer(), WORLD, &unstamped, 1), WORLD).is_err());
        let mut stamped = frame();
        stamped.e[0].k = Some([0, 1]);
        assert!(decode(&frame_event(&signer(), WORLD, &stamped, 1), WORLD).is_err());
    }

    #[test]
    fn another_world_is_refused() {
        let event = frame_event(&signer(), "elsewhere", &frame(), 1);
        assert!(decode(&event, WORLD).is_err());
    }

    #[test]
    fn a_tampered_frame_is_refused() {
        let mut event = frame_event(&signer(), WORLD, &frame(), 1);
        event.content = event.content.replace("\"n\":7", "\"n\":8");
        assert!(decode(&event, WORLD).is_err());
    }

    #[test]
    fn bad_numbers_are_refused() {
        let mut bad = frame();
        bad.e[0].q = [0.0, 0.0, 0.0, 0.0];
        assert!(decode(&frame_event(&signer(), WORLD, &bad, 1), WORLD).is_err());
        let mut far = frame();
        far.e[0].p = [1.0e9, 0.0, 0.0];
        assert!(decode(&frame_event(&signer(), WORLD, &far, 1), WORLD).is_err());
    }

    #[test]
    fn gesture_version_duration_and_coordinates_are_checked() {
        let valid = Gesture {
            v: 1,
            id: "agent".into(),
            g: "greet".into(),
            t: 1,
            d: Some(1.0),
            at: vec![[0.0; 3]],
            to: None,
        };
        for variant in 0..3 {
            let mut bad = valid.clone();
            match variant {
                0 => bad.v = 99,
                1 => bad.d = Some(-1.0),
                _ => bad.at[0][0] = 1.0e9,
            };
            let event = gesture_event(&signer(), WORLD, &bad, Vec3::ZERO, 1);
            assert!(decode(&event, WORLD).is_err());
        }
    }

    #[test]
    fn cells_follow_the_floor() {
        assert_eq!(cell(Vec3::new(-0.5, 9.0, 64.0)), "-1,1");
        assert_eq!(cells_around(Vec3::ZERO, 1).len(), 9);
    }

    #[test]
    fn world_chat_round_trips_with_its_scope() {
        let event = world_chat_event(
            &signer(),
            WORLD,
            "near",
            "plaza",
            Vec3::new(1.5, 0.0, -2.25),
            "anyone here?",
            1,
        );
        let line = decode_chat(&event, WORLD).expect("decodes");
        assert_eq!(line.channel.as_deref(), Some("near"));
        assert_eq!(line.zone.as_deref(), Some("plaza"));
        assert_eq!(line.pos, Some(Vec3::new(1.5, 0.0, -2.25)));
        assert!(decode_chat(&event, "elsewhere").is_err());
        let room = room_chat_event(&signer(), "lounge", "hi", 1);
        assert_eq!(
            decode_chat(&room, WORLD).expect("decodes").room.as_deref(),
            Some("lounge")
        );
    }

    #[test]
    fn a_gesture_round_trips() {
        let gesture = Gesture {
            v: 1,
            id: "agent".into(),
            g: "look-around".into(),
            t: 9,
            d: Some(2.4),
            at: vec![[1.0, 2.0, 3.0]],
            to: Some(["ab".repeat(32), "agent".into()]),
        };
        let event = gesture_event(&signer(), WORLD, &gesture, Vec3::ZERO, 1);
        assert!(event.tag_values("p").any(|p| p == "ab".repeat(32)));
        assert!(matches!(
            decode(&event, WORLD),
            Ok(Received::Gesture { gesture: got, .. }) if got == gesture
        ));
    }
}
