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
pub const WORLD_KIND: u16 = 33_300;
/// Entity state, addressable.
pub const STATE_KIND: u16 = 33_301;
/// Pose frame, ephemeral.
pub const FRAME_KIND: u16 = 23_300;
/// Gesture, ephemeral.
pub const GESTURE_KIND: u16 = 23_301;
/// Default cell size in meters.
pub const CELL: f32 = 64.0;
/// Most entities one frame may carry.
pub const MAX_ENTITIES: usize = 16;
/// Largest coordinate magnitude a receiver accepts, in meters.
const MAX_COORD: f32 = 1.0e6;

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
        if let Some(v) = self.v
            && !v.iter().all(|c| c.is_finite() && c.abs() < MAX_COORD)
        {
            return Err("velocity is not finite".into());
        }
        Ok(())
    }
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
}

impl State {
    /// The pose this state records.
    #[must_use]
    pub fn pose(&self) -> EntityPose {
        EntityPose {
            id: self.id.clone(),
            role: self.role.clone(),
            p: self.p,
            q: self.q,
            v: None,
            follows: self.follows.clone(),
            a: None,
        }
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

/// A received NIP-MV event, validated.
#[derive(Clone, Debug, PartialEq)]
pub enum Received {
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
            state.pose().check()?;
            Ok(Received::State { pubkey, state })
        }
        GESTURE_KIND => {
            let gesture: Gesture =
                serde_json::from_str(&event.content).map_err(|e| e.to_string())?;
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
        other => Err(format!("kind {other} is not a NIP-MV event")),
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
    fn cells_follow_the_floor() {
        assert_eq!(cell(Vec3::new(-0.5, 9.0, 64.0)), "-1,1");
        assert_eq!(cells_around(Vec3::ZERO, 1).len(), 9);
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
