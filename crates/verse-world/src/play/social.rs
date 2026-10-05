//! Closed social profiles use the chamber's identity, movement, and persistence boundary.
use super::*;
use physics::queries::{GeometrySnapshot, SceneSnapshot, Usage};
use serde::{Deserialize, Serialize};
use verse_engine::core::LifeId;

pub const PROFILE_REVISION: u16 = 1;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Zone {
    Plaza,
    Everglade,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub revision: u16,
    pub zone: Zone,
    /// Static collision geometry normalized to instance zero. Its digest binds presentation.
    pub geometry: SceneSnapshot,
    pub objects: Vec<Object>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Object {
    pub id: u64,
    pub feet: [f32; 3],
    pub yaw: f32,
    pub kind: Kind,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Seat,
    Switch,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Sit { object: u64 },
    Stand {},
    Toggle { object: u64 },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Input {
    pub life: LifeId,
    pub epoch: u64,
    pub sequence: u64,
    pub tick: u64,
    pub action: Action,
}
/// Public seat placement carries no task, command, source path, or Studio permission.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SeatActor {
    pub seat: u64,
    pub feet: [f32; 3],
    pub yaw: f32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Occupant {
    pub seat: u64,
    pub life: LifeId,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Toggle {
    pub object: u64,
    pub on: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct State {
    pub profile: Profile,
    pub revision: u64,
    pub occupants: Vec<Occupant>,
    pub switches: Vec<Toggle>,
    pub studio: Vec<SeatActor>,
}
impl Profile {
    pub fn validate(&self) -> Result<(), String> {
        if self.revision != PROFILE_REVISION
            || self.objects.len() > 64
            || self.geometry.colliders.is_empty()
            || self.geometry.colliders.len() > 64
        {
            return Err("Unsupported hosted social profile or budget".into());
        }
        self.geometry.validate(0)?;
        let mut triangles = 0;
        for shape in &self.geometry.colliders {
            if shape.key.life.entity != 0
                || shape.key.life.generation != 0
                || shape.layers != 1
                || shape.usage != Usage::Blocking
                || shape.pose != Default::default()
            {
                return Err("Social geometry must be immutable static blocking geometry".into());
            }
            match &shape.geometry {
                GeometrySnapshot::Box { min, max } => {
                    if !bounded(min.as_vec3()) || !bounded(max.as_vec3()) {
                        return Err("Social geometry exceeds coordinate bounds".into());
                    }
                }
                GeometrySnapshot::Triangles { triangles: source } => {
                    triangles += source.len();
                    if triangles > 512
                        || source
                            .iter()
                            .any(|t| t.0.iter().any(|p| !bounded(p.as_vec3())))
                    {
                        return Err("Social terrain exceeds triangle or coordinate bounds".into());
                    }
                }
                _ => return Err("Unsupported social geometry".into()),
            }
        }
        let mut ids = std::collections::BTreeSet::new();
        if self.objects.iter().any(|o| {
            o.id == 0
                || !ids.insert(o.id)
                || !bounded(Vec3::from_array(o.feet))
                || !o.yaw.is_finite()
        }) {
            return Err("Invalid social interaction object".into());
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<[u8; 32], String> {
        use sha2::{Digest, Sha256};
        self.validate()?;
        let bytes = serde_json::to_vec(self).map_err(|_| "Cannot encode social profile")?;
        let mut hash = Sha256::new();
        hash.update(b"verse.hosted.social.v1\0");
        hash.update(bytes);
        Ok(hash.finalize().into())
    }
    pub fn geometry_in(&self, instance: u64) -> Result<SceneSnapshot, String> {
        self.validate()?;
        let mut geometry = self.geometry.clone();
        geometry.instance = instance;
        for s in &mut geometry.colliders {
            s.key.life.instance = instance;
        }
        Ok(geometry)
    }
    pub(super) fn bounds(&self) -> Result<Vec<physics::kinematic::Aabb>, String> {
        self.validate()?;
        Ok(self
            .geometry
            .colliders
            .iter()
            .map(|s| match &s.geometry {
                GeometrySnapshot::Box { min, max } => physics::kinematic::Aabb {
                    min: *min,
                    max: *max,
                },
                GeometrySnapshot::Triangles { triangles } => {
                    let mut min = glam::DVec3::splat(f64::INFINITY);
                    let mut max = glam::DVec3::splat(f64::NEG_INFINITY);
                    for t in triangles {
                        for p in t.0 {
                            min = min.min(p);
                            max = max.max(p);
                        }
                    }
                    physics::kinematic::Aabb {
                        min: min - glam::DVec3::splat(0.01),
                        max: max + glam::DVec3::splat(0.01),
                    }
                }
                _ => unreachable!(),
            })
            .collect())
    }
}
fn bounded(p: Vec3) -> bool {
    p.is_finite() && p.x.abs() <= 512. && p.z.abs() <= 512. && (-20. ..=80.).contains(&p.y)
}
impl State {
    pub fn occupant(&self, seat: u64) -> Option<LifeId> {
        self.occupants
            .iter()
            .find(|a| a.seat == seat)
            .map(|a| a.life)
    }
    pub fn switch_on(&self, object: u64) -> bool {
        self.switches
            .iter()
            .find(|a| a.object == object)
            .is_some_and(|a| a.on)
    }
    pub fn validate(&self, instance: u64) -> Result<(), String> {
        self.profile.validate()?;
        if self.occupants.len() > 64 || self.switches.len() > 64 || self.studio.len() > 64 {
            return Err("Social state exceeds budget".into());
        }
        let kind = |id, kind| {
            self.profile
                .objects
                .iter()
                .any(|o| o.id == id && o.kind == kind)
        };
        let mut lives = std::collections::BTreeSet::new();
        let mut seats = std::collections::BTreeSet::new();
        let mut occupied = std::collections::BTreeSet::new();
        let mut switched = std::collections::BTreeSet::new();
        if self.occupants.iter().any(|a| {
            !kind(a.seat, Kind::Seat)
                || !occupied.insert(a.seat)
                || a.life.instance != instance
                || !lives.insert(a.life)
        }) || self
            .switches
            .iter()
            .any(|a| !kind(a.object, Kind::Switch) || !switched.insert(a.object))
            || self.studio.iter().any(|a| {
                !kind(a.seat, Kind::Seat)
                    || !seats.insert(a.seat)
                    || self.occupant(a.seat).is_some()
                    || !bounded(Vec3::from_array(a.feet))
                    || !a.yaw.is_finite()
            })
        {
            return Err("Invalid social occupancy or public seat pose".into());
        }
        Ok(())
    }
}
impl Game {
    /// Creates an explicit hosted social world without requiring hostile actors.
    pub fn social_in(scene: Scene, instance: u64, profile: Profile) -> Result<Self, String> {
        profile.validate()?;
        scene.validate()?;
        if scene.cut_at != 0.
            || !scene.cues.is_empty()
            || scene.collision_profile.is_some()
            || scene.actors.iter().any(|a| a.nameplate && !a.friendly)
        {
            return Err(
                "Social scenes require immediate control and no combat or cinematic cues".into(),
            );
        }
        let social = State {
            profile,
            revision: 0,
            occupants: Default::default(),
            switches: Default::default(),
            studio: vec![],
        };
        Self::new_owned(scene, instance, Some(social))
    }
    pub(super) fn validate_social(&self) -> Result<(), String> {
        if let Some(s) = &self.social {
            s.validate(self.player_life().instance)?;
            if self.encounter.is_some()
                || self.scene.cut_at != 0.
                || !self.scene.cues.is_empty()
                || self.scene.collision_profile.is_some()
                || self.scene.actors.iter().any(|a| a.nameplate && !a.friendly)
                || s.occupants.iter().any(|o| {
                    self.player_admission(o.life.actor)
                        .is_none_or(|a| a.actor() != o.life)
                })
            {
                return Err("Invalid social world checkpoint".into());
            }
        }
        Ok(())
    }
    pub(crate) fn restart_social(&mut self, agent: bool) -> Result<(), String> {
        let old = self.social.as_ref().ok_or("World is not social")?;
        let mut fresh = Self::social_in(
            self.scene.clone(),
            self.player_life().instance,
            old.profile.clone(),
        )?;
        fresh.adopt_restart_fences(self)?;
        fresh.rebuild_players_after_restart(self)?;
        fresh.social.as_mut().unwrap().revision = old
            .revision
            .checked_add(1)
            .ok_or("Social revision exhausted")?;
        fresh.agent_controlled = agent;
        *self = fresh;
        Ok(())
    }
    pub fn social_state(&self) -> Option<&State> {
        self.social.as_ref()
    }
    pub(super) fn static_bounds(&self) -> Result<Vec<physics::kinematic::Aabb>, String> {
        self.social.as_ref().map_or_else(
            || crate::room::profile_colliders(self.scene.collision_profile.as_deref()),
            |s| s.profile.bounds(),
        )
    }
    pub(super) fn static_queries(&self) -> Result<physics::queries::Scene, String> {
        let instance = self.player_life().instance;
        self.social.as_ref().map_or_else(
            || crate::room::profile_query_scene(self.scene.collision_profile.as_deref(), instance),
            |s| s.profile.geometry_in(instance)?.compile(instance),
        )
    }
    pub(super) fn clear_social_seat(&mut self, life: LifeId) {
        if let Some(s) = &mut self.social {
            let before = s.occupants.len();
            s.occupants.retain(|o| o.life != life);
            if before != s.occupants.len() {
                s.revision = s.revision.saturating_add(1);
            }
        }
    }
    /// Trusted host projection only; world clients cannot mutate Studio work or actor poses.
    pub fn set_social_studio(&mut self, actors: Vec<SeatActor>) -> Result<(), String> {
        let old = self
            .social
            .as_ref()
            .ok_or("World is not a social profile")?;
        let mut next = old.clone();
        next.studio = actors;
        next.validate(self.player_life().instance)?;
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or("Social revision exhausted")?;
        self.social = Some(next);
        Ok(())
    }
    pub fn submit_social(&mut self, sender: crate::Controller, input: Input) -> Result<(), String> {
        let actor = input.life.actor;
        let admission = self
            .player_admission(actor)
            .ok_or("Unknown social character")?;
        if admission.actor() != input.life
            || admission.controller() != sender
            || !self.unlocked()
            || self.player_snapshot(input.life)?.player.hp <= 0
        {
            return Err("Social control is stale or unavailable".into());
        }
        let state = self
            .social
            .as_ref()
            .ok_or("World is not a social profile")?;
        let mut next = state.clone();
        match input.action {
            Action::Stand {} => next.occupants.retain(|o| o.life != input.life),
            Action::Sit { object } | Action::Toggle { object } => {
                let expected = if matches!(input.action, Action::Sit { .. }) {
                    Kind::Seat
                } else {
                    Kind::Switch
                };
                let object = next
                    .profile
                    .objects
                    .iter()
                    .find(|o| o.id == object && o.kind == expected)
                    .ok_or("Unknown social interaction")?;
                let feet = if actor == self.player_actor() {
                    self.player
                } else {
                    self.additional_players[&actor].position
                };
                if feet.distance(Vec3::from_array(object.feet)) > 2.5 {
                    return Err("Social interaction is out of reach".into());
                }
                if expected == Kind::Seat {
                    if next.studio.iter().any(|a| a.seat == object.id)
                        || next.occupant(object.id).is_some_and(|l| l != input.life)
                    {
                        return Err("Social seat is occupied".into());
                    }
                    next.occupants.retain(|o| o.life != input.life);
                    next.occupants.push(Occupant {
                        seat: object.id,
                        life: input.life,
                    });
                    next.occupants.sort_by_key(|a| a.seat);
                } else {
                    if let Some(value) = next.switches.iter_mut().find(|a| a.object == object.id) {
                        value.on = !value.on;
                    } else {
                        next.switches.push(Toggle {
                            object: object.id,
                            on: true,
                        });
                        next.switches.sort_by_key(|a| a.object);
                    }
                }
            }
        }
        next.revision = next
            .revision
            .checked_add(1)
            .ok_or("Social revision exhausted")?;
        let command = crate::Command {
            actor: input.life,
            epoch: input.epoch,
            sequence: input.sequence,
            tick: input.tick,
            intent: crate::Intent::Cast {
                ability: input.action,
                target: None,
                aim: [0., 0., -1.],
            },
        };
        if actor == self.player_actor() {
            self.admission.admit(sender, &command, self.authority_tick)
        } else {
            self.additional_players
                .get_mut(&actor)
                .unwrap()
                .admission
                .admit(sender, &command, self.authority_tick)
        }
        .map_err(|e| format!("Social command refused: {e:?}"))?;
        self.handoff_player(input.life, sender)?;
        self.social = Some(next);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) fn profile(zone: Zone) -> Profile {
        use physics::queries::{ColliderKey, Life, Mesh, MeshCollider, Scene, Usage};
        let mut geometry = Scene::default();
        for (shape, (min, max)) in [
            (
                glam::DVec3::new(-12., -1., -12.),
                glam::DVec3::new(12., 0., 12.),
            ),
            (glam::DVec3::new(3., 0., -2.), glam::DVec3::new(4., 4., 2.)),
        ]
        .into_iter()
        .enumerate()
        {
            geometry
                .insert(MeshCollider {
                    key: ColliderKey {
                        life: Life {
                            instance: 0,
                            entity: 0,
                            generation: 0,
                        },
                        shape: shape as u32,
                    },
                    layers: 1,
                    usage: Usage::Blocking,
                    mesh: Mesh::from_box(min, max).unwrap(),
                })
                .unwrap();
        }
        Profile {
            revision: 1,
            zone,
            geometry: geometry.snapshot(0).unwrap(),
            objects: vec![
                Object {
                    id: 1,
                    feet: [0., 0., 0.],
                    yaw: 0.,
                    kind: Kind::Seat,
                },
                Object {
                    id: 2,
                    feet: [1., 0., 0.],
                    yaw: 0.,
                    kind: Kind::Switch,
                },
                Object {
                    id: 3,
                    feet: [-1., 0., 0.],
                    yaw: 1.,
                    kind: Kind::Seat,
                },
            ],
        }
    }
    pub(crate) fn game(instance: u64, zone: Zone) -> Game {
        let mut scene = Scene::from_json(include_bytes!(
            "../../../../assets/verse/original/ritual.json"
        ))
        .unwrap();
        scene.actors.retain(|a| a.model == "adventurer");
        scene.actors[0].position = Vec3::ZERO;
        scene.cut_at = 0.;
        scene.cues.clear();
        scene.collision_profile = None;
        Game::social_in(scene, instance, profile(zone)).unwrap()
    }
    fn input(g: &Game, life: LifeId, action: Action) -> Input {
        let admission = g.player_admission(life.actor).unwrap();
        Input {
            life,
            epoch: admission.epoch(),
            sequence: admission.accepted_sequence() + 1,
            tick: g.authority_tick,
            action,
        }
    }
    #[test]
    fn social_seats_share_control_fences_and_refusals_do_not_mutate() {
        let mut g = game(2001, Zone::Plaza);
        let a = g.player_life();
        let b = g
            .add_player(crate::Controller(3), Vec3::new(1., 0., 1.))
            .unwrap();
        let command = input(&g, a, Action::Sit { object: 1 });
        g.submit_social(crate::Controller(1), command.clone())
            .unwrap();
        assert_eq!(g.social_state().unwrap().occupant(1), Some(a));
        let before = g.checkpoint().unwrap();
        assert!(g.submit_social(crate::Controller(1), command).is_err());
        assert!(
            g.submit_social(
                crate::Controller(3),
                input(&g, b, Action::Sit { object: 1 })
            )
            .is_err()
        );
        assert!(
            g.submit_social(crate::Controller(3), input(&g, a, Action::Stand {}))
                .is_err()
        );
        assert_eq!(before, g.checkpoint().unwrap());
        let stale = g
            .player_admission(a.actor)
            .unwrap()
            .command(
                g.authority_tick,
                crate::Intent::<Ability>::Move {
                    axes: [1., 0.],
                    yaw: 0.,
                },
            )
            .unwrap();
        g.submit_social(
            crate::Controller(1),
            input(&g, a, Action::Toggle { object: 2 }),
        )
        .unwrap();
        assert!(g.submit(crate::Controller(1), stale).is_err());
        assert!(g.social_state().unwrap().switch_on(2));
        g.submit_social(crate::Controller(1), input(&g, a, Action::Stand {}))
            .unwrap();
        assert!(g.social_state().unwrap().occupants.is_empty());
        g.set_social_studio(vec![SeatActor {
            seat: 3,
            feet: [-1., 0., 0.],
            yaw: 1.,
        }])
        .unwrap();
        assert!(
            g.submit_social(
                crate::Controller(3),
                input(&g, b, Action::Sit { object: 3 })
            )
            .is_err()
        );
        println!(
            "social seats: exclusive occupancy, replay/foreign refusal, shared epoch fence, public Studio pose"
        );
    }
    #[test]
    fn social_movement_uses_authored_collision_and_disables_combat() {
        let mut g = game(2001, Zone::Everglade);
        g.tick(1. / 30., [0.; 2]).unwrap();
        let life = g.player_life();
        let command = g
            .player_admission(life.actor)
            .unwrap()
            .command(
                g.authority_tick,
                crate::Intent::Move {
                    axes: [1., 0.],
                    yaw: 0.,
                },
            )
            .unwrap();
        g.submit(crate::Controller(1), command).unwrap();
        for _ in 0..20 {
            g.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert!(g.player.x.abs() > 0.1);
        assert!(g.activate(Ability::Fireball).is_err());
        let cast = g
            .player_admission(life.actor)
            .unwrap()
            .command(
                g.authority_tick,
                crate::Intent::Cast {
                    ability: Ability::Shield,
                    target: None,
                    aim: [0., 0., -1.],
                },
            )
            .unwrap();
        assert!(g.submit(crate::Controller(1), cast).is_err());
        assert!(g.snapshot().actors.iter().all(|a| a.faction != "undead"));
        assert!(g.encounter.is_none());
        let restored = Game::restore(&g.checkpoint().unwrap()).unwrap();
        assert_eq!(g.social_state(), restored.social_state());
        assert_eq!(
            g.query_scene.snapshot(2001).unwrap(),
            restored.query_scene.snapshot(2001).unwrap()
        );
        println!(
            "social movement: owned motor, no required enemies, combat refused, exact collision/checkpoint recovery"
        );
    }
    #[test]
    fn social_terrain_uses_the_same_sloped_mesh_after_recovery() {
        use physics::queries::{GeometrySnapshot, Triangle};
        let base = game(2001, Zone::Everglade);
        let mut profile = profile(Zone::Everglade);
        let corners = [
            glam::DVec3::new(-12., -1.8, -12.),
            glam::DVec3::new(-12., -0.6, 12.),
            glam::DVec3::new(12., 1.8, 12.),
            glam::DVec3::new(12., 0.6, -12.),
        ];
        profile.geometry.colliders[0].geometry = GeometrySnapshot::Triangles {
            triangles: vec![
                Triangle([corners[0], corners[1], corners[2]]),
                Triangle([corners[0], corners[2], corners[3]]),
            ],
        };
        let mut g = Game::social_in(base.scene, 2001, profile).unwrap();
        let command = g
            .player_admission(g.player_actor())
            .unwrap()
            .command(
                g.authority_tick,
                crate::Intent::Move {
                    axes: [-1., 0.],
                    yaw: 0.,
                },
            )
            .unwrap();
        g.submit(crate::Controller(1), command).unwrap();
        for _ in 0..10 {
            g.tick(1. / 30., [0.; 2]).unwrap();
        }
        assert!((g.player.y - (g.player.x * 0.1 + g.player.z * 0.05)).abs() < 0.05);
        let restored = Game::restore(&g.checkpoint().unwrap()).unwrap();
        assert_eq!(
            g.query_scene.snapshot(2001).unwrap(),
            restored.query_scene.snapshot(2001).unwrap()
        );
    }
    #[test]
    fn social_profiles_and_checkpoint_rules_are_closed() {
        let mut g = game(2001, Zone::Plaza);
        let mut p = profile(Zone::Plaza);
        p.objects.push(p.objects[0].clone());
        assert!(p.validate().is_err());
        p = profile(Zone::Plaza);
        p.revision = 2;
        assert!(p.validate().is_err());
        let mut json = serde_json::to_value(profile(Zone::Plaza)).unwrap();
        json["zone"] = "ruins".into();
        assert!(serde_json::from_value::<Profile>(json).is_err());
        let mut saved: serde_json::Value =
            serde_json::from_slice(&g.checkpoint().unwrap()).unwrap();
        saved["rules_revision"] = "verse-chamber-owned-v20".into();
        assert!(Game::restore(&serde_json::to_vec(&saved).unwrap()).is_err());
        let old = g.player_life();
        g.restart_combat(false).unwrap();
        assert!(g.player_life().generation > old.generation);
        assert!(g.social_state().is_some());
        Game::restore(&g.checkpoint().unwrap()).unwrap();
    }
}
