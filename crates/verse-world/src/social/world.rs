//! The social rules profile: a world authority that hosts a zone such as
//! Everglade without the chamber's combat. A session joins as an avatar
//! with no adventurer or hostile actor required, walks the zone's
//! heightfield under the shared controller, and is stopped by the same
//! placement blockers a local player is. Commands pass the same admission
//! fences as the chamber's ([`crate::Admission`]); a cast is refused,
//! because the profile has no combat.

use super::controller::{AVATAR_HEIGHT, InputState, PlayerController};
use super::solids::Solids;
use crate::{Admission, Command, Controller, Intent, Refusal};
use glam::Vec3;
use std::collections::BTreeMap;
use verse_engine::core::LifeId;

/// Rules revision of the social profile.
pub const RULES_REVISION: &str = "verse-social-v1";
/// The most avatars one social instance hosts.
pub const CAPACITY: usize = 128;
/// The fixed step the authority walks avatars at, s.
pub const STEP: f32 = 1.0 / 60.0;

/// A zone hosted under the social rules: its pinned content, its ground and
/// blockers, its walkable square, and where avatars arrive.
#[derive(Clone, Debug)]
pub struct Profile {
    /// The zone's name, for logs and listings.
    pub zone: &'static str,
    /// The pinned content digest: the zone pack's SHA-256. The chamber's
    /// login challenge carries it, so a client and the host agree on the
    /// content before a session starts.
    pub content: [u8; 32],
    /// The zone's ground and blockers.
    pub solids: Solids,
    /// Half the walkable square, m.
    pub bound: f32,
    /// Where an avatar arrives, and its heading as the controller's yaw.
    pub spawn: Vec3,
    pub spawn_yaw: f32,
}

impl Profile {
    /// Everglade under the social rules: its heightfield and `solids`,
    /// built from the pinned pack whose digest is `content`.
    #[must_use]
    pub fn everglade(content: [u8; 32], solids: Solids) -> Self {
        use super::everglade::{HALF_EXTENT, SPAWN, SPAWN_YAW, height};
        Self {
            zone: "everglade",
            content,
            solids,
            bound: HALF_EXTENT,
            spawn: Vec3::new(SPAWN.x, height(SPAWN.x, SPAWN.z), SPAWN.z),
            spawn_yaw: SPAWN_YAW,
        }
    }
}

/// Decodes a pinned pack's hexadecimal SHA-256 into the 32-byte content
/// digest the chamber's login challenge carries.
///
/// # Errors
/// Returns a message when `hex` is not 64 hexadecimal digits.
pub fn content_digest(hex: &str) -> Result<[u8; 32], String> {
    let bytes = hex.as_bytes();
    if bytes.len() != 64 {
        return Err("A content digest is 64 hexadecimal digits".into());
    }
    let digit = |b: u8| match b {
        b'0'..=b'9' => Ok(b - b'0'),
        b'a'..=b'f' => Ok(b - b'a' + 10),
        b'A'..=b'F' => Ok(b - b'A' + 10),
        _ => Err("A content digest is 64 hexadecimal digits".to_string()),
    };
    let mut digest = [0; 32];
    for (i, pair) in bytes.chunks_exact(2).enumerate() {
        digest[i] = digit(pair[0])? << 4 | digit(pair[1])?;
    }
    Ok(digest)
}

struct Avatar {
    admission: Admission,
    body: PlayerController,
    /// The held movement: the last admitted move until another replaces it.
    held: InputState,
    /// A jump admitted since the last step.
    jump: bool,
}

/// A social instance's authority: avatars, their admission fences, and the
/// clock that walks them.
pub struct World {
    profile: Profile,
    instance: u64,
    tick: u64,
    next_actor: u64,
    avatars: BTreeMap<u64, Avatar>,
}

impl World {
    /// Hosts `profile` as instance `instance`. Unlike the chamber, nothing
    /// in the zone has to be an adventurer or a hostile.
    #[must_use]
    pub fn new(profile: Profile, instance: u64) -> Self {
        Self {
            profile,
            instance,
            tick: 0,
            next_actor: 1,
            avatars: BTreeMap::new(),
        }
    }

    /// The hosted profile.
    #[must_use]
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// The pinned content digest a login challenge carries.
    #[must_use]
    pub fn content(&self) -> [u8; 32] {
        self.profile.content
    }

    /// The authority's tick.
    #[must_use]
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Admits `controller`'s session: a new avatar at the profile's spawn.
    ///
    /// # Errors
    /// Returns a message when the controller already has an avatar or the
    /// instance is full.
    pub fn join(&mut self, controller: Controller) -> Result<LifeId, String> {
        if self
            .avatars
            .values()
            .any(|a| a.admission.controller() == controller)
        {
            return Err("This session already has an avatar".into());
        }
        if self.avatars.len() >= CAPACITY {
            return Err("This instance is full".into());
        }
        let actor = self.next_actor;
        self.next_actor = actor.checked_add(1).ok_or("Avatar IDs exhausted")?;
        let life = LifeId {
            instance: self.instance,
            actor,
            generation: 0,
        };
        self.avatars.insert(
            actor,
            Avatar {
                admission: Admission::new(life, controller),
                body: PlayerController::new(self.profile.spawn, self.profile.spawn_yaw),
                held: InputState::default(),
                jump: false,
            },
        );
        Ok(life)
    }

    /// Ends `controller`'s session and removes its avatar.
    pub fn leave(&mut self, controller: Controller) {
        self.avatars
            .retain(|_, a| a.admission.controller() != controller);
    }

    /// The admission fences of the avatar `life` names, for building the
    /// next command.
    #[must_use]
    pub fn admission(&self, life: LifeId) -> Option<&Admission> {
        self.avatars
            .get(&life.actor)
            .map(|a| &a.admission)
            .filter(|a| a.actor() == life)
    }

    /// Admits `command` from `sender`. A move is held until the next one: its
    /// `axes` are strafe right and forward, and its `yaw` the controller's.
    ///
    /// # Errors
    /// The admission fence's refusal, and `InvalidIntent` for a cast,
    /// because the social profile has no combat.
    pub fn command(&mut self, sender: Controller, command: &Command<()>) -> Result<(), Refusal> {
        let tick = self.tick;
        let avatar = self
            .avatars
            .get_mut(&command.actor.actor)
            .ok_or(Refusal::StaleLife)?;
        if sender != avatar.admission.controller() {
            return Err(Refusal::NotController);
        }
        if matches!(command.intent, Intent::Cast { .. }) {
            return Err(Refusal::InvalidIntent);
        }
        avatar.admission.admit(sender, command, tick)?;
        match command.intent {
            Intent::Jump => avatar.jump = true,
            Intent::Move { axes, yaw } => {
                avatar.body.yaw = super::controller::wrap(yaw);
                avatar.held = InputState {
                    forward: axes[1] > 0.0,
                    backward: axes[1] < 0.0,
                    strafe_right: axes[0] > 0.0,
                    strafe_left: axes[0] < 0.0,
                    mouse_look: true,
                    ..InputState::default()
                };
            }
            Intent::Cast { .. } => unreachable!("refused above"),
        }
        Ok(())
    }

    /// Advances the authority one [`STEP`]: every avatar walks its held
    /// movement over the ground and blockers, then steps out of the others.
    pub fn step(&mut self) {
        self.tick += 1;
        let solids = &self.profile.solids;
        for avatar in self.avatars.values_mut() {
            let mut input = avatar.held;
            input.jump = std::mem::take(&mut avatar.jump);
            solids.step(&mut avatar.body, &input, STEP, self.profile.bound, true);
        }
        let feet: Vec<(u64, Vec3)> = self
            .avatars
            .iter()
            .map(|(id, a)| (*id, a.body.pos))
            .collect();
        for (id, avatar) in &mut self.avatars {
            let others: Vec<Vec3> = feet
                .iter()
                .filter(|(other, p)| other != id && (p.y - avatar.body.pos.y).abs() < AVATAR_HEIGHT)
                .map(|(_, p)| *p)
                .collect();
            let blockers = solids.blocking(avatar.body.pos.y);
            avatar.body.separate(&others, &blockers, self.profile.bound);
        }
    }

    /// The avatar `life` names, while it is current.
    #[must_use]
    pub fn avatar(&self, life: LifeId) -> Option<&PlayerController> {
        self.avatars
            .get(&life.actor)
            .filter(|a| a.admission.actor() == life)
            .map(|a| &a.body)
    }

    /// Every avatar's feet, in actor order: what waiting seats meet.
    #[must_use]
    pub fn feet(&self) -> Vec<Vec3> {
        self.avatars.values().map(|a| a.body.pos).collect()
    }
}
