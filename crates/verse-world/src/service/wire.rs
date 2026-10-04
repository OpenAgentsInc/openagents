//! Bounded JSON messages dispatched through a transport-retained connection.
use serde::{Deserialize, Serialize};
use verse_engine::core::LifeId;

use super::auth::{Challenge, ConnectionId, Gateway};
use crate::{Command, Intent, events::Event, play::Ability, rules::Snapshot};

pub const VERSION: u16 = 8;
pub const MAX_REQUEST_BYTES: usize = 16 * 1024;
pub const MAX_RESPONSE_BYTES: usize = 2 * 1024 * 1024;

/// Unsolicited opening message sent on the newly assigned transport connection.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub version: u16,
    pub challenge: Challenge,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Life {
    pub instance: u64,
    pub actor: u64,
    pub generation: u64,
}
impl From<LifeId> for Life {
    fn from(life: LifeId) -> Self {
        Self {
            instance: life.instance,
            actor: life.actor,
            generation: life.generation,
        }
    }
}
impl From<Life> for LifeId {
    fn from(life: Life) -> Self {
        Self {
            instance: life.instance,
            actor: life.actor,
            generation: life.generation,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Jump {},
    Move {
        axes: [f32; 2],
        yaw: f32,
    },
    Cast {
        ability: Ability,
        target: Option<Life>,
        aim: [f32; 3],
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub actor: Life,
    pub epoch: u64,
    pub sequence: u64,
    pub tick: u64,
    pub intent: Action,
}
impl From<Command<Ability>> for Input {
    fn from(c: Command<Ability>) -> Self {
        let intent = match c.intent {
            Intent::Jump => Action::Jump {},
            Intent::Move { axes, yaw } => Action::Move { axes, yaw },
            Intent::Cast {
                ability,
                target,
                aim,
            } => Action::Cast {
                ability,
                target: target.map(Into::into),
                aim,
            },
        };
        Self {
            actor: c.actor.into(),
            epoch: c.epoch,
            sequence: c.sequence,
            tick: c.tick,
            intent,
        }
    }
}
impl From<Input> for Command<Ability> {
    fn from(c: Input) -> Self {
        let intent = match c.intent {
            Action::Jump {} => Intent::Jump,
            Action::Move { axes, yaw } => Intent::Move { axes, yaw },
            Action::Cast {
                ability,
                target,
                aim,
            } => Intent::Cast {
                ability,
                target: target.map(Into::into),
                aim,
            },
        };
        Self {
            actor: c.actor.into(),
            epoch: c.epoch,
            sequence: c.sequence,
            tick: c.tick,
            intent,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Body {
    Authenticate {
        public_key: [u8; 32],
        signature: Vec<u8>,
    },
    Command {
        command: Input,
    },
    Snapshot {},
    Events {
        after: u64,
        limit: u16,
    },
    Respawn {
        life: Life,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub version: u16,
    pub request_id: u64,
    pub body: Body,
}
impl State {
    pub fn validate_control(&self, instance: u64, control: &Option<Control>) -> Result<(), String> {
        self.validate(instance)?;
        if self.hud.as_ref().map(|h| h.life)
            != control
                .as_ref()
                .map(|c| verse_engine::core::LifeId::from(c.life))
        {
            return Err("Owned HUD does not match admitted control".into());
        }
        Ok(())
    }
    /// Produces renderer values only after complete remote state admission.
    pub fn combat_visuals(&self, instance: u64) -> Result<crate::visuals::Combat, String> {
        self.validate(instance)?;
        Ok(crate::visuals::Combat {
            time: self.presentation.time,
            projectiles: self.snapshot.projectiles.clone(),
            players: self
                .presentation
                .effects
                .iter()
                .map(|e| crate::visuals::Player {
                    position: e.position.into(),
                    shield: e.shield,
                    shield_until: e.shield_until,
                    light: e.light.map(Into::into),
                    areas: e.areas.clone(),
                })
                .collect(),
            hostile: self
                .presentation
                .hostile_casts
                .iter()
                .map(|c| crate::visuals::Hostile {
                    origin: c.origin.into(),
                    target: c.target.into(),
                    position: c.position.map(Into::into),
                    started: c.started,
                    release: c.release,
                    radius: c.radius,
                    boss: c.boss,
                })
                .collect(),
            impacts: self
                .presentation
                .impacts
                .iter()
                .map(|i| (i.position.into(), i.at, i.kind))
                .collect(),
        })
    }
    /// Admits the shared snapshot and its complete presentation life bindings.
    pub fn validate(&self, instance: u64) -> Result<(), String> {
        let mut sources = std::collections::BTreeSet::new();
        let mut lives = std::collections::BTreeSet::new();
        let snapshot_sources: std::collections::BTreeSet<_> =
            self.snapshot.actors.iter().map(|a| a.id).collect();
        if self.actors.len() != self.snapshot.actors.len()
            || self.actors.len() > 256
            || !self.snapshot.elapsed.is_finite()
            || self.snapshot.elapsed < 0.
            || self.actors.iter().any(|a| {
                a.life.instance != instance
                    || !sources.insert(a.source)
                    || !lives.insert(verse_engine::core::LifeId::from(a.life))
            })
            || self.snapshot.actors.iter().any(|a| {
                !sources.contains(&a.id)
                    || !a.pos.iter().all(|v| v.is_finite())
                    || !a.yaw.is_finite()
            })
            || sources != snapshot_sources
            || snapshot_sources.len() != self.snapshot.actors.len()
        {
            return Err("Invalid chamber snapshot life bindings".into());
        }
        let mut projectiles = std::collections::BTreeSet::new();
        if self.snapshot.projectiles.len() > 128
            || self.snapshot.projectiles.iter().any(|p| {
                !projectiles.insert(p.id)
                    || !sources.contains(&p.caster)
                    || !p
                        .pos
                        .iter()
                        .chain(&p.vel)
                        .all(|v| v.is_finite() && v.abs() <= 1_000_000.)
            })
        {
            return Err("Invalid chamber projectile presentation".into());
        }
        if let Some(hud) = &self.hud {
            hud.validate(instance)?;
            let p = &self.snapshot.player;
            if (
                hud.resources.hp,
                hud.resources.max_hp,
                hud.resources.mana,
                hud.resources.max_mana,
            ) != (p.hp, p.max_hp, p.mana, p.max_mana)
                || hud
                    .casting
                    .as_ref()
                    .is_some_and(|c| !lives.contains(&c.target_life))
            {
                return Err("Owned HUD resources or cast target mismatch".into());
            }
            if hud.time != self.presentation.time
                || !self.presentation.actors.iter().any(|p| {
                    verse_engine::core::LifeId::from(p.life) == hud.life
                        && p.actor.model == "adventurer"
                })
            {
                return Err("Owned HUD life or clock mismatch".into());
            }
        }
        self.presentation.validate(instance, &self.actors)
    }
}

impl Request {
    pub fn decode(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err("Chamber request exceeds byte budget".into());
        }
        let r: Self = serde_json::from_slice(bytes).map_err(|_| "Malformed chamber request")?;
        if r.version != VERSION || r.request_id == 0 {
            return Err("Unsupported chamber version or request identity".into());
        }
        Ok(r)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub life: Life,
    pub epoch: u64,
    /// Highest admitted envelope, including subsequent gameplay refusals.
    pub accepted_sequence: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActorBinding {
    pub source: u32,
    pub life: Life,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub hud: Option<crate::hud::Own>,
    pub snapshot: Snapshot,
    pub presentation: super::presentation::Presentation,
    pub actors: Vec<ActorBinding>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EventPage {
    pub events: Vec<Event>,
    pub next: u64,
    pub latest: u64,
    /// First retained serial; `None` when no authority events exist.
    pub oldest: Option<u64>,
    pub gap: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Reply {
    Accepted,
    Snapshot { state: State },
    Events { page: EventPage },
    Refused { code: String, message: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Response {
    pub version: u16,
    pub request_id: u64,
    pub instance: u64,
    pub tick: u64,
    pub control: Option<Control>,
    pub body: Reply,
}
impl Response {
    pub fn encode(&self) -> Result<Vec<u8>, String> {
        let bytes = serde_json::to_vec(self).map_err(|_| "Cannot encode chamber response")?;
        if bytes.len() > MAX_RESPONSE_BYTES {
            return Err("Chamber response exceeds byte budget".into());
        }
        Ok(bytes)
    }
}
impl Gateway {
    /// Assigns the connection and encodes its public opening challenge.
    pub fn open_json(&mut self, now_ms: u64) -> Result<(ConnectionId, Vec<u8>), String> {
        let (id, challenge) = self.open(now_ms)?;
        match serde_json::to_vec(&Hello {
            version: VERSION,
            challenge,
        }) {
            Ok(bytes) => Ok((id, bytes)),
            Err(_) => {
                let _ = self.close(id);
                Err("Cannot encode chamber opening challenge".into())
            }
        }
    }
    /// Decodes one bounded message; identity comes only from the host connection.
    pub fn dispatch_json(
        &mut self,
        connection: ConnectionId,
        now_ms: u64,
        bytes: &[u8],
    ) -> Result<Vec<u8>, String> {
        let (request_id, body) = match Request::decode(bytes) {
            Ok(r) => (r.request_id, self.dispatch_body(connection, now_ms, r.body)),
            Err(message) => (0, Err(("protocol", message))),
        };
        let control = self.admission(connection).ok().map(|a| Control {
            life: a.actor().into(),
            epoch: a.epoch(),
            accepted_sequence: a.accepted_sequence(),
        });
        Response {
            version: VERSION,
            request_id,
            instance: self.game().player_life().instance,
            tick: self.game().authority_tick,
            control,
            body: body.unwrap_or_else(|(code, message)| Reply::Refused {
                code: code.into(),
                message,
            }),
        }
        .encode()
    }
    fn dispatch_body(
        &mut self,
        id: ConnectionId,
        now: u64,
        body: Body,
    ) -> Result<Reply, (&'static str, String)> {
        match body {
            Body::Authenticate {
                public_key,
                signature,
            } => {
                let Ok(signature) = <[u8; 64]>::try_from(signature) else {
                    // Consume a pending challenge even for a malformed proof length.
                    let _ = self.authenticate(id, now, public_key, [0; 64]);
                    return Err(("protocol", "Signature must contain exactly 64 bytes".into()));
                };
                self.authenticate(id, now, public_key, signature)
                    .map_err(|e| ("authentication", e))?;
                Ok(Reply::Accepted)
            }
            Body::Command { command } => {
                self.submit(id, command.into())
                    .map_err(|e| ("command", e))?;
                Ok(Reply::Accepted)
            }
            Body::Respawn { life } => {
                self.respawn(id, life.into()).map_err(|e| ("command", e))?;
                Ok(Reply::Accepted)
            }
            Body::Snapshot {} => {
                let snapshot = self.snapshot(id).map_err(|e| ("authentication", e))?;
                let actors: Vec<_> = snapshot
                    .actors
                    .iter()
                    .filter_map(|actor| {
                        let life = self.game().projectile_caster_life(actor.id).or_else(|| {
                            self.game()
                                .ids
                                .iter()
                                .find(|(_, source)| **source == actor.id)
                                .and_then(|(id, _)| self.game().actor_life(*id))
                        })?;
                        Some(ActorBinding {
                            source: actor.id,
                            life: life.into(),
                        })
                    })
                    .collect();
                Ok(Reply::Snapshot {
                    state: State {
                        presentation: super::presentation::Presentation::extract(
                            self.game(),
                            &actors,
                        ),
                        hud: self
                            .admission(id)
                            .ok()
                            .map(|a| self.game().player_hud(a.actor()))
                            .transpose()
                            .map_err(|e| ("presentation", e))?,
                        snapshot,
                        actors,
                    },
                })
            }
            Body::Events { after, limit } => {
                self.snapshot(id).map_err(|e| ("authentication", e))?;
                if !(1..=64).contains(&limit) {
                    return Err(("cursor", "Event page limit must be between 1 and 64".into()));
                }
                let events = &self.game().events;
                let latest = events.last().map_or(0, |e| e.serial);
                if after > latest {
                    return Err(("cursor", "Event cursor is ahead of the authority".into()));
                }
                let oldest = events.first().map(|e| e.serial);
                let gap = oldest.is_some_and(|first| after.saturating_add(1) < first);
                let page: Vec<_> = events
                    .iter()
                    .filter(|e| e.serial > after)
                    .take(usize::from(limit))
                    .cloned()
                    .collect();
                let next = page.last().map_or(after, |e| e.serial);
                Ok(Reply::Events {
                    page: EventPage {
                        events: page,
                        next,
                        latest,
                        oldest,
                        gap,
                    },
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::Chamber;
    use super::*;
    use crate::play::Game;
    use glam::Vec3;
    use secp256k1::{Keypair, Secp256k1, SecretKey};
    use verse_engine::director::Scene;
    fn key(n: u8) -> Keypair {
        Keypair::from_secret_key(
            &Secp256k1::new(),
            &SecretKey::from_byte_array([n; 32]).unwrap(),
        )
    }
    fn public(k: &Keypair) -> [u8; 32] {
        k.x_only_public_key().0.serialize()
    }
    fn gateway() -> Gateway {
        let scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        let mut g = Game::combat_in(scene, false, 110).unwrap();
        g.time = g.scene.cut_at;
        g.tick(0., [0.; 2]).unwrap();
        Gateway::new(Chamber::new(g).unwrap()).unwrap()
    }
    fn send(g: &mut Gateway, id: ConnectionId, request_id: u64, body: Body) -> Response {
        let bytes = serde_json::to_vec(&Request {
            version: VERSION,
            request_id,
            body,
        })
        .unwrap();
        serde_json::from_slice(&g.dispatch_json(id, 0, &bytes).unwrap()).unwrap()
    }
    fn join(g: &mut Gateway, k: &Keypair) -> ConnectionId {
        let (id, bytes) = g.open_json(0).unwrap();
        let hello: Hello = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(hello.version, VERSION);
        let c = hello.challenge;
        let signature = Secp256k1::new()
            .sign_schnorr_no_aux_rand(&c.signing_digest(public(k)), k)
            .to_byte_array()
            .to_vec();
        assert!(matches!(
            send(
                g,
                id,
                1,
                Body::Authenticate {
                    public_key: public(k),
                    signature
                }
            )
            .body,
            Reply::Accepted
        ));
        id
    }
    #[test]
    fn signed_json_players_and_spectator_share_state_and_control_acknowledgments() {
        let mut g = gateway();
        let a = key(11);
        let b = key(12);
        let s = key(13);
        g.enroll_primary(public(&a)).unwrap();
        g.enroll_player(public(&b), Vec3::new(3., 0., -22.))
            .unwrap();
        g.enroll_spectator(public(&s)).unwrap();
        let a = join(&mut g, &a);
        let b = join(&mut g, &b);
        let s = join(&mut g, &s);
        let command = g
            .admission(a)
            .unwrap()
            .command(
                g.game().authority_tick,
                Intent::Move {
                    axes: [1., 0.],
                    yaw: 0.,
                },
            )
            .unwrap();
        assert!(matches!(
            send(
                &mut g,
                b,
                2,
                Body::Command {
                    command: command.clone().into()
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        assert!(matches!(
            send(
                &mut g,
                s,
                2,
                Body::Command {
                    command: command.clone().into()
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        let r = send(
            &mut g,
            a,
            3,
            Body::Command {
                command: command.clone().into(),
            },
        );
        assert!(matches!(r.body, Reply::Accepted));
        assert_eq!(r.control.unwrap().accepted_sequence, command.sequence);
        let duplicate = send(
            &mut g,
            a,
            4,
            Body::Command {
                command: command.into(),
            },
        );
        assert!(matches!(duplicate.body, Reply::Refused { .. }));
        assert_eq!(duplicate.control.unwrap().accepted_sequence, 1);
        g.tick(1. / 30.).unwrap();
        let Reply::Snapshot { state: player } = send(&mut g, a, 5, Body::Snapshot {}).body else {
            panic!()
        };
        let observer = send(&mut g, s, 5, Body::Snapshot {});
        assert!(observer.control.is_none());
        let Reply::Snapshot { state: spectator } = observer.body else {
            panic!()
        };
        assert_eq!(
            serde_json::to_vec(&player.snapshot.actors).unwrap(),
            serde_json::to_vec(&spectator.snapshot.actors).unwrap()
        );
        assert_eq!(player.actors.len(), player.snapshot.actors.len());
        assert!(player.actors.iter().all(|a| a.life.instance == 110));
        assert_eq!(
            player
                .actors
                .iter()
                .find(|a| a.source == 0)
                .unwrap()
                .life
                .actor,
            g.game().player_life().actor
        );
    }
    #[test]
    fn presentation_keeps_independent_movement_and_spell_visuals_on_one_tick() {
        let mut g = gateway();
        let ka = key(18);
        let kb = key(19);
        g.enroll_primary(public(&ka)).unwrap();
        g.enroll_player(public(&kb), Vec3::new(3., 0., -22.))
            .unwrap();
        let a = join(&mut g, &ka);
        let b = join(&mut g, &kb);
        for (id, ability) in [
            (a, Ability::Shield),
            (b, Ability::Shield),
            (a, Ability::Light),
        ] {
            let command = g
                .admission(id)
                .unwrap()
                .command(
                    g.game().authority_tick,
                    Intent::Cast {
                        ability,
                        target: None,
                        aim: [0., 0., 1.],
                    },
                )
                .unwrap();
            assert!(matches!(
                send(
                    &mut g,
                    id,
                    2,
                    Body::Command {
                        command: command.into()
                    }
                )
                .body,
                Reply::Accepted
            ));
        }
        let own = g.admission(b).unwrap().actor();
        let position = g.game().actor_position(own.actor).unwrap();
        let target = g
            .game()
            .frame()
            .actors
            .into_iter()
            .filter(|p| {
                p.actor.nameplate
                    && g.game()
                        .attack_clear(position + Vec3::Y * 1.4, p.actor.position + Vec3::Y * 1.1)
            })
            .min_by(|x, y| {
                x.actor
                    .position
                    .distance_squared(position)
                    .total_cmp(&y.actor.position.distance_squared(position))
            })
            .unwrap();
        let direction = Vec3::new(
            target.actor.position.x - position.x,
            0.,
            target.actor.position.z - position.z,
        )
        .normalize();
        let command = g
            .admission(b)
            .unwrap()
            .command(
                g.game().authority_tick,
                Intent::Cast {
                    ability: Ability::Web,
                    target: target.life,
                    aim: direction.to_array(),
                },
            )
            .unwrap();
        assert!(matches!(
            send(
                &mut g,
                b,
                3,
                Body::Command {
                    command: command.into()
                }
            )
            .body,
            Reply::Accepted
        ));
        for _ in 0..36 {
            g.tick(1. / 30.).unwrap();
        }
        for (id, axes) in [(a, [0., 1.]), (b, [0., -1.])] {
            let command = g
                .admission(id)
                .unwrap()
                .command(g.game().authority_tick, Intent::Move { axes, yaw: 0. })
                .unwrap();
            assert!(matches!(
                send(
                    &mut g,
                    id,
                    4,
                    Body::Command {
                        command: command.into()
                    }
                )
                .body,
                Reply::Accepted
            ));
        }
        g.tick(1. / 30.).unwrap();
        let Reply::Snapshot { state } = send(&mut g, a, 5, Body::Snapshot {}).body else {
            panic!()
        };
        state.presentation.validate(110, &state.actors).unwrap();
        assert_eq!(state.presentation.time, g.game().time);
        assert_eq!(
            state
                .presentation
                .effects
                .iter()
                .filter(|e| e.shield > 0)
                .count(),
            2
        );
        assert_eq!(
            state
                .presentation
                .effects
                .iter()
                .filter(|e| e.light.is_some())
                .count(),
            1
        );
        assert!(
            state
                .presentation
                .effects
                .iter()
                .find(|e| e.life.actor == own.actor)
                .unwrap()
                .areas
                .iter()
                .any(|e| e.kind == crate::utilities::Utility::Web)
        );
        let primary = state
            .presentation
            .actors
            .iter()
            .find(|p| p.life.actor == g.game().player_life().actor)
            .unwrap();
        let extra = state
            .presentation
            .actors
            .iter()
            .find(|p| p.life.actor == own.actor)
            .unwrap();
        assert_eq!(primary.animation, verse_engine::motion::State::Run.into());
        assert_eq!(
            extra.animation,
            verse_engine::motion::State::Backpedal.into()
        );
        assert!(primary.animation_time > 0. && extra.animation_time > 0.);
        let mut invalid = state.presentation.clone();
        invalid.actors[0].life.generation += 1;
        assert!(invalid.validate(110, &state.actors).is_err());
        let mut invalid = state.presentation.clone();
        invalid.effects.push(invalid.effects[0].clone());
        assert!(invalid.validate(110, &state.actors).is_err());
        let mut invalid = state.presentation.clone();
        invalid.actors[0].actor.position.x = f32::NAN;
        assert!(invalid.validate(110, &state.actors).is_err());
        let mut invalid = state.presentation;
        invalid.effects[0].areas.resize(
            129,
            crate::utilities::Area {
                kind: crate::utilities::Utility::Web,
                position: Vec3::ZERO,
                until: 20.,
            },
        );
        assert!(invalid.validate(110, &state.actors).is_err());
        let primary = g.admission(a).unwrap().actor();
        g.chamber.game.hostile_hit_player(primary, 1000).unwrap();
        let Reply::Snapshot { state: dead } = send(&mut g, a, 6, Body::Snapshot {}).body else {
            panic!()
        };
        let corpse = dead
            .presentation
            .actors
            .iter()
            .find(|p| p.life.actor == primary.actor)
            .unwrap();
        assert_eq!(corpse.health, 0);
        assert_eq!(corpse.animation, verse_engine::motion::State::Death.into());
    }

    #[test]
    fn admitted_remote_combat_visuals_match_local_extraction() {
        let mut g = gateway();
        let k = key(24);
        g.enroll_primary(public(&k)).unwrap();
        let id = join(&mut g, &k);
        g.chamber.game.activate(Ability::Shield).unwrap();
        g.chamber.game.activate(Ability::Light).unwrap();
        let time = g.game().time;
        g.chamber.game.impacts.push((Vec3::Y, time, 1));
        let frame = g.game().frame();
        let caster = frame
            .actors
            .iter()
            .find(|p| p.actor.model != "adventurer")
            .unwrap();
        let target = frame
            .actors
            .iter()
            .find(|p| p.actor.model == "adventurer")
            .unwrap();
        g.chamber
            .game
            .encounter
            .as_mut()
            .unwrap()
            .casts
            .push(crate::combat::EnemyCast {
                actor: caster.actor.id,
                life: caster.life.unwrap(),
                target_life: target.life.unwrap(),
                position: Some(caster.actor.position),
                origin: caster.actor.position,
                target: target.actor.position,
                started: time,
                release: time + 1.,
                impact: time + 2.,
                damage: 8,
                radius: 1.6,
                boss: false,
            });
        let local = crate::visuals::Combat::extract(g.game());
        let r = send(&mut g, id, 2, Body::Snapshot {});
        let Reply::Snapshot { mut state } = r.body else {
            panic!("Expected snapshot")
        };
        let remote = state.combat_visuals(110).unwrap();
        assert_eq!(local.time, remote.time);
        assert_eq!(
            format!("{:?}", local.players),
            format!("{:?}", remote.players)
        );
        assert_eq!(
            format!("{:?}", local.hostile),
            format!("{:?}", remote.hostile)
        );
        assert_eq!(
            format!("{:?}", local.impacts),
            format!("{:?}", remote.impacts)
        );
        assert_eq!(
            format!("{:?}", local.projectiles),
            format!("{:?}", remote.projectiles)
        );
        let caster = state.actors[0].source;
        let projectile = crate::rules::Projectile {
            id: 1,
            caster,
            kind: crate::rules::ProjectileKind::Fireball,
            pos: [0.; 3],
            vel: [1., 0., 0.],
        };
        state.snapshot.projectiles.push(projectile.clone());
        assert!(state.combat_visuals(110).is_ok());
        for case in 0..4 {
            let mut bad = state.clone();
            match case {
                0 => bad.snapshot.projectiles[0].pos[0] = f32::NAN,
                1 => bad.snapshot.projectiles[0].caster = u32::MAX,
                2 => bad.snapshot.projectiles.push(projectile.clone()),
                _ => bad.snapshot.projectiles = vec![projectile.clone(); 129],
            }
            assert!(bad.combat_visuals(110).is_err());
        }
    }
    #[test]
    fn owned_hud_is_scoped_to_each_authenticated_life_and_spectators_have_none() {
        let mut g = gateway();
        let ka = key(25);
        let kb = key(26);
        let ks = key(27);
        g.enroll_primary(public(&ka)).unwrap();
        g.enroll_player(public(&kb), Vec3::new(3., 0., -22.))
            .unwrap();
        g.enroll_spectator(public(&ks)).unwrap();
        let a = join(&mut g, &ka);
        let b = join(&mut g, &kb);
        let spectator = join(&mut g, &ks);
        let command = g
            .admission(a)
            .unwrap()
            .command(
                g.game().authority_tick,
                Intent::Cast {
                    ability: Ability::Shield,
                    target: None,
                    aim: [0., 0., 1.],
                },
            )
            .unwrap();
        g.submit(a, command).unwrap();
        let ra = send(&mut g, a, 2, Body::Snapshot {});
        let rb = send(&mut g, b, 2, Body::Snapshot {});
        let rs = send(&mut g, spectator, 2, Body::Snapshot {});
        let Reply::Snapshot { state: sa } = &ra.body else {
            panic!("Expected snapshot")
        };
        let Reply::Snapshot { state: sb } = &rb.body else {
            panic!("Expected snapshot")
        };
        let Reply::Snapshot { state: ss } = &rs.body else {
            panic!("Expected snapshot")
        };
        sa.validate_control(110, &ra.control).unwrap();
        sb.validate_control(110, &rb.control).unwrap();
        ss.validate_control(110, &rs.control).unwrap();
        let ha = sa.hud.as_ref().unwrap();
        let hb = sb.hud.as_ref().unwrap();
        assert_ne!(ha.life, hb.life);
        assert!(ss.hud.is_none());
        assert_eq!(ha.resources.mana, 19);
        assert_eq!(hb.resources.mana, 20);
        assert!(
            !ha.slots
                .iter()
                .find(|s| s.ability == Ability::Shield)
                .unwrap()
                .ready
        );
        assert!(
            hb.slots
                .iter()
                .find(|s| s.ability == Ability::Shield)
                .unwrap()
                .ready
        );
        assert!(sa.validate_control(110, &rb.control).is_err());
        for case in 0..6 {
            let mut bad = sa.clone();
            let h = bad.hud.as_mut().unwrap();
            match case {
                0 => h.life.generation += 1,
                1 => h.resources.hp = h.resources.max_hp + 1,
                2 => h.slots[0].remaining = f32::NAN,
                3 => h.slots.swap(0, 1),
                4 => h.resources.mana = 18,
                _ => h.time += 1.,
            }
            assert!(bad.validate_control(110, &ra.control).is_err());
        }
        assert!(
            g.game()
                .player_hud(verse_engine::core::LifeId {
                    generation: ha.life.generation + 1,
                    ..ha.life
                })
                .is_err()
        );
        let cast = g
            .admission(b)
            .unwrap()
            .command(
                g.game().authority_tick,
                Intent::Cast {
                    ability: Ability::Fireball,
                    target: Some(g.game().actor_life(1).unwrap()),
                    aim: [0., 0., 1.],
                },
            )
            .unwrap();
        g.submit(b, cast).unwrap();
        let hud = g.game().player_hud(hb.life).unwrap();
        hud.validate(110).unwrap();
        assert!(hud.casting.is_some());
        assert!(hud.slots.iter().all(|s| !s.ready));
    }
    #[test]
    fn strict_request_budget_versions_and_nested_fields_are_enforced() {
        let mut g = gateway();
        let (id, _) = g.open(0).unwrap();
        let reply = send(&mut g, id, 1, Body::Snapshot {});
        assert!(matches!(reply.body, Reply::Refused { .. }));
        for bytes in [
            br#"{"version":9,"request_id":1,"body":{"type":"snapshot"}}"#.to_vec(),
            br#"{"version":8,"request_id":1,"controller":1,"body":{"type":"snapshot"}}"#.to_vec(),
            br#"{"version":8,"request_id":1,"body":{"type":"snapshot","principal":"fake"}}"#
                .to_vec(),
            vec![b' '; MAX_REQUEST_BYTES + 1],
        ] {
            assert!(Request::decode(&bytes).is_err());
        }
        let k = key(14);
        g.enroll_primary(public(&k)).unwrap();
        let id = join(&mut g, &k);
        let input: Input = g
            .admission(id)
            .unwrap()
            .command(g.game().authority_tick, Intent::Jump)
            .unwrap()
            .into();
        let mut value = serde_json::to_value(Request {
            version: VERSION,
            request_id: 3,
            body: Body::Command { command: input },
        })
        .unwrap();
        value["body"]["command"]["actor"]["forged"] = serde_json::json!(true);
        assert!(Request::decode(&serde_json::to_vec(&value).unwrap()).is_err());
        value["body"]["command"]["actor"]
            .as_object_mut()
            .unwrap()
            .remove("forged");
        value["body"]["command"]["intent"]["controller"] = serde_json::json!(1);
        assert!(Request::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn oversized_responses_are_refused_instead_of_sent_without_a_bound() {
        let response = Response {
            version: VERSION,
            request_id: 1,
            instance: 110,
            tick: 0,
            control: None,
            body: Reply::Refused {
                code: "fixture".into(),
                message: "x".repeat(MAX_RESPONSE_BYTES),
            },
        };
        assert!(response.encode().is_err());
    }

    #[test]
    fn event_pages_report_retention_gaps_and_never_replay_past_cursor() {
        let mut g = gateway();
        let k = key(15);
        g.enroll_spectator(public(&k)).unwrap();
        let id = join(&mut g, &k);
        // Model the authority's retained suffix after bounded event eviction.
        let mut chamber = g.chamber.game.checkpoint().unwrap();
        let mut saved: serde_json::Value = serde_json::from_slice(&chamber).unwrap();
        let world = &mut saved["world"];
        world["events"] = serde_json::json!([
            {"instance":110,"serial":7,"tick":0,"time":0.,"actor":null,"kind":"CameraHandoff"},
            {"instance":110,"serial":8,"tick":0,"time":0.,"actor":null,"kind":"CameraHandoff"}
        ]);
        world["event_serial"] = serde_json::json!(8);
        chamber = serde_json::to_vec(&saved).unwrap();
        g.chamber.game = Game::restore(&chamber).unwrap();
        let Reply::Events { page } = send(&mut g, id, 2, Body::Events { after: 0, limit: 1 }).body
        else {
            panic!()
        };
        assert!(page.gap);
        assert_eq!(page.oldest, Some(7));
        assert_eq!(page.next, 7);
        assert_eq!(page.latest, 8);
        let Reply::Events { page } = send(
            &mut g,
            id,
            3,
            Body::Events {
                after: page.next,
                limit: 64,
            },
        )
        .body
        else {
            panic!()
        };
        assert!(!page.gap);
        assert_eq!(page.events.len(), 1);
        assert_eq!(page.next, 8);
        assert!(matches!(
            send(
                &mut g,
                id,
                4,
                Body::Events {
                    after: 9,
                    limit: 64
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        assert!(matches!(
            send(&mut g, id, 5, Body::Events { after: 0, limit: 0 }).body,
            Reply::Refused { .. }
        ));
    }
    #[test]
    fn gameplay_refusal_reports_consumed_sequence_for_client_reconciliation() {
        let mut g = gateway();
        let k = key(17);
        g.enroll_primary(public(&k)).unwrap();
        let id = join(&mut g, &k);
        for expected in 1..=2 {
            let command = g
                .admission(id)
                .unwrap()
                .command(
                    g.game().authority_tick,
                    Intent::Cast {
                        ability: Ability::Shield,
                        target: None,
                        aim: [0., 0., 1.],
                    },
                )
                .unwrap();
            let response = send(
                &mut g,
                id,
                expected + 1,
                Body::Command {
                    command: command.into(),
                },
            );
            assert_eq!(response.control.unwrap().accepted_sequence, expected);
            if expected == 1 {
                assert!(matches!(response.body, Reply::Accepted));
            } else {
                assert!(matches!(response.body, Reply::Refused { .. }));
            }
        }
    }

    #[test]
    fn malformed_signature_consumes_challenge_and_respawn_is_owned() {
        let mut g = gateway();
        let k = key(16);
        g.enroll_primary(public(&k)).unwrap();
        let (id, c) = g.open(0).unwrap();
        assert!(matches!(
            send(
                &mut g,
                id,
                1,
                Body::Authenticate {
                    public_key: public(&k),
                    signature: vec![0; 63]
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        let signature = Secp256k1::new()
            .sign_schnorr_no_aux_rand(&c.signing_digest(public(&k)), &k)
            .to_byte_array()
            .to_vec();
        assert!(matches!(
            send(
                &mut g,
                id,
                2,
                Body::Authenticate {
                    public_key: public(&k),
                    signature
                }
            )
            .body,
            Reply::Refused { .. }
        ));
        let id = join(&mut g, &k);
        let life = g.admission(id).unwrap().actor();
        g.chamber.game.hostile_hit_player(life, 1000).unwrap();
        let response = send(&mut g, id, 3, Body::Respawn { life: life.into() });
        assert!(matches!(response.body, Reply::Accepted));
        assert_eq!(
            response.control.unwrap().life.generation,
            life.generation + 1
        );
        assert!(matches!(
            send(&mut g, id, 4, Body::Respawn { life: life.into() }).body,
            Reply::Refused { .. }
        ));
    }
}
