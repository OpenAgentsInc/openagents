//! Fixed-step Quick_3D_MMORPG combat. See third_party/quick-mmorpg/NOTICE.
//! Source stats, ranges, facing cone, and damage timing come from shared/defs.mjs
//! and server/src/world-{entity,client}.mjs at 547884332ca650abe96264f7230702d36481b9bc.

use serde::{Deserialize, Serialize};
use std::collections::{HashSet, VecDeque};

type V3 = [f32; 3];
const DT: f32 = 1.0 / 60.0;
pub const MAX_ZOMBIES: usize = 16;
pub const ZOMBIE_HEIGHT: f32 = 1.2;
const PLAYER_DAMAGE: u32 = 300;
const ZOMBIE_DAMAGE: u32 = 5;
const PLAYER_HIT: u16 = 21;
const PLAYER_COOLDOWN: u16 = 60;
const ZOMBIE_HIT: u16 = 60;
// The source server waits 3 seconds. Keep the full authored 3.866667-second
// zombie attack visible while that cooldown elapses, rather than cutting it off.
const ZOMBIE_ATTACK: u16 = 232;
const SCAN_TICKS: u16 = 300;
const MIN_SPACING: f32 = 0.601;

fn distance(a: V3, b: V3) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}
fn horizontal(a: V3, b: V3) -> f32 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}
fn overlaps(a: V3, b: V3) -> bool {
    (a[1] - b[1]).abs() < ZOMBIE_HEIGHT && horizontal(a, b) < MIN_SPACING
}
fn overlaps_player(zombie: V3, player: V3) -> bool {
    zombie[1] < player[1] + crate::HEIGHT
        && zombie[1] + ZOMBIE_HEIGHT > player[1]
        && horizontal(zombie, player) < MIN_SPACING
}
fn facing(from: V3, yaw: f32, to: V3, range: f32) -> bool {
    let distance = distance(from, to);
    distance <= range
        && distance > 0.00001
        && ((to[0] - from[0]) * yaw.sin() - (to[2] - from[2]) * yaw.cos()) / distance >= 0.9
}
fn chest(mut position: V3, height: f32) -> V3 {
    position[1] += height * 0.5;
    position
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub id: u32,
    pub position: V3,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Idle,
    Walk,
    Attack,
    Death,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Player {
    pub health: u32,
    pub max_health: u32,
    pub alive: bool,
    pub state: State,
    pub action_time: f32,
    pub cooldown: f32,
    pub attacks: u32,
    pub hits: u32,
    #[serde(skip)]
    attack_tick: Option<u16>,
}
impl Default for Player {
    fn default() -> Self {
        Self {
            health: 200,
            max_health: 200,
            alive: true,
            state: State::Idle,
            action_time: 0.0,
            cooldown: 0.0,
            attacks: 0,
            hits: 0,
            attack_tick: None,
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub id: u32,
    pub position: V3,
    pub yaw: f32,
    pub health: u32,
    pub max_health: u32,
    pub alive: bool,
    pub state: State,
    pub action_time: f32,
    pub animation_time: f32,
    #[serde(skip)]
    state_tick: u16,
    #[serde(skip)]
    velocity_y: f32,
    #[serde(skip)]
    grounded: bool,
}
impl Actor {
    fn new(spec: &Spec, player: V3, scale: f32) -> Self {
        let acquired = distance(spec.position, player) <= 50.0 * scale;
        Self {
            id: spec.id,
            position: spec.position,
            yaw: (player[0] - spec.position[0]).atan2(-(player[2] - spec.position[2])),
            health: 20,
            max_health: 50,
            alive: true,
            // Start the small encounter immediately; subsequent scans use 5 seconds.
            state: if acquired { State::Walk } else { State::Idle },
            action_time: 0.0,
            animation_time: 0.0,
            state_tick: 0,
            velocity_y: 0.0,
            grounded: false,
        }
    }
    fn set_state(&mut self, state: State) {
        if self.state != state {
            self.state = state;
            self.state_tick = 0;
            self.action_time = 0.0;
            self.animation_time = 0.0;
        }
    }
    fn advance_time(&mut self) {
        self.state_tick = self.state_tick.saturating_add(1);
        self.action_time = (self.action_time + DT).min(3600.0);
        self.animation_time = self.action_time;
    }
}

#[derive(Clone)]
pub struct Combat {
    pub player: Player,
    pub actors: Vec<Actor>,
    pub spawn: crate::Camera,
    pub specs: Vec<Spec>,
    world_scale: f32,
    attacks: VecDeque<(String, bool)>,
}
impl Combat {
    pub fn new(
        world_scale: f32,
        specs: Vec<Spec>,
        spawn: crate::Camera,
    ) -> Result<Self, &'static str> {
        if !world_scale.is_finite()
            || !(0.01..=1.0).contains(&world_scale)
            || specs.len() > MAX_ZOMBIES
        {
            return Err("Choose a valid scale and no more than 16 zombies.");
        }
        let mut ids = HashSet::new();
        for (index, spec) in specs.iter().enumerate() {
            if spec.id == u32::MAX
                || !ids.insert(spec.id)
                || spec
                    .position
                    .iter()
                    .any(|n| !n.is_finite() || n.abs() > 20_000.0)
                || overlaps_player(spec.position, spawn.position)
                || specs[..index]
                    .iter()
                    .any(|other| overlaps(spec.position, other.position))
            {
                return Err("Give each zombie a unique number and an open starting position.");
            }
        }
        Ok(Self {
            player: Player::default(),
            actors: specs
                .iter()
                .map(|spec| Actor::new(spec, spawn.position, world_scale))
                .collect(),
            spawn,
            specs,
            world_scale,
            attacks: VecDeque::new(),
        })
    }
    pub fn blocks_player(&self, position: V3) -> bool {
        self.actors
            .iter()
            .any(|actor| actor.alive && overlaps_player(actor.position, position))
    }
    pub fn attack(&mut self, id: &str) -> Result<(bool, bool), &'static str> {
        if !crate::valid_id(id) {
            return Err("Give this attack a valid name.");
        }
        if let Some((_, accepted)) = self.attacks.iter().find(|(old, _)| old == id) {
            return Ok((*accepted, true));
        }
        let accepted = self.player.alive && self.player.attack_tick.is_none();
        if accepted {
            self.player.state = State::Attack;
            self.player.action_time = 0.0;
            self.player.cooldown = PLAYER_COOLDOWN as f32 * DT;
            self.player.attack_tick = Some(0);
            self.player.attacks = self.player.attacks.saturating_add(1);
        }
        self.attacks.push_back((id.to_owned(), accepted));
        if self.attacks.len() > 64 {
            self.attacks.pop_front();
        }
        Ok((accepted, false))
    }
    pub fn advance(
        &mut self,
        player_position: V3,
        player_yaw: f32,
        mut motion: impl FnMut(V3, V3, f32, bool) -> (V3, f32, bool),
        clear: impl Fn(V3, V3) -> bool,
    ) {
        self.player.action_time = (self.player.action_time + DT).min(3600.0);
        if self.player.alive {
            if let Some(tick) = self.player.attack_tick.as_mut() {
                *tick += 1;
                if *tick == PLAYER_HIT {
                    for actor in &mut self.actors {
                        if actor.alive
                            && facing(
                                player_position,
                                player_yaw,
                                actor.position,
                                10.0 * self.world_scale,
                            )
                            && clear(
                                chest(player_position, crate::HEIGHT),
                                chest(actor.position, ZOMBIE_HEIGHT),
                            )
                        {
                            actor.health = actor.health.saturating_sub(PLAYER_DAMAGE);
                            self.player.hits = self.player.hits.saturating_add(1);
                            if actor.health == 0 {
                                actor.alive = false;
                                actor.set_state(State::Death);
                                actor.velocity_y = 0.0;
                            }
                        }
                    }
                }
                self.player.cooldown = (PLAYER_COOLDOWN - *tick) as f32 * DT;
                if *tick >= PLAYER_COOLDOWN {
                    self.player.attack_tick = None;
                    self.player.state = State::Idle;
                    self.player.action_time = 0.0;
                }
            }
        }

        let mut positions = [[0.0; 3]; MAX_ZOMBIES];
        let mut living = [false; MAX_ZOMBIES];
        for (index, actor) in self.actors.iter().enumerate() {
            positions[index] = actor.position;
            living[index] = actor.alive;
        }
        let count = self.actors.len();
        for index in 0..count {
            let actor = &mut self.actors[index];
            actor.advance_time();
            if !actor.alive {
                continue;
            }
            let distance_to_player = distance(actor.position, player_position);
            if !self.player.alive {
                actor.set_state(State::Idle);
            } else {
                match actor.state {
                    State::Idle if actor.state_tick >= SCAN_TICKS => {
                        actor.state_tick = 0;
                        if distance_to_player <= 50.0 * self.world_scale {
                            actor.set_state(State::Walk);
                        }
                    }
                    State::Walk => {
                        let delta = [
                            player_position[0] - actor.position[0],
                            player_position[2] - actor.position[2],
                        ];
                        actor.yaw = delta[0].atan2(-delta[1]);
                        if distance_to_player > 100.0 * self.world_scale {
                            actor.set_state(State::Idle);
                        } else if distance_to_player < 10.0 * self.world_scale
                            && clear(
                                chest(actor.position, ZOMBIE_HEIGHT),
                                chest(player_position, crate::HEIGHT),
                            )
                        {
                            actor.set_state(State::Attack);
                        }
                    }
                    State::Attack => {
                        if actor.state_tick == ZOMBIE_HIT
                            && facing(
                                actor.position,
                                actor.yaw,
                                player_position,
                                10.0 * self.world_scale,
                            )
                            && clear(
                                chest(actor.position, ZOMBIE_HEIGHT),
                                chest(player_position, crate::HEIGHT),
                            )
                        {
                            self.player.health = self.player.health.saturating_sub(ZOMBIE_DAMAGE);
                            if self.player.health == 0 {
                                self.player.alive = false;
                                self.player.state = State::Death;
                                self.player.action_time = 0.0;
                                self.player.cooldown = 0.0;
                                self.player.attack_tick = None;
                            }
                        }
                        if actor.state_tick >= ZOMBIE_ATTACK {
                            actor.set_state(State::Walk);
                        }
                    }
                    _ => {}
                }
            }
            let mut desired = [0.0; 3];
            if actor.state == State::Walk && self.player.alive {
                let dx = player_position[0] - actor.position[0];
                let dz = player_position[2] - actor.position[2];
                let length = dx.hypot(dz).max(0.00001);
                let mut direction = [dx / length, dz / length];
                for other in 0..count {
                    if other == index
                        || !living[other]
                        || (positions[other][1] - actor.position[1]).abs() >= ZOMBIE_HEIGHT
                    {
                        continue;
                    }
                    let dx = actor.position[0] - positions[other][0];
                    let dz = actor.position[2] - positions[other][2];
                    let distance = dx.hypot(dz);
                    if distance > 0.00001 && distance < 1.0 {
                        let strength = (1.0 - distance) * 2.0;
                        direction[0] += dx / distance * strength;
                        direction[1] += dz / distance * strength;
                    }
                }
                let length = direction[0].hypot(direction[1]).max(0.00001);
                let step = 10.0 * self.world_scale * DT;
                desired = [
                    direction[0] / length * step,
                    0.0,
                    direction[1] / length * step,
                ];
            }
            let (candidate, velocity_y, grounded) =
                motion(actor.position, desired, actor.velocity_y, actor.grounded);
            let open = !overlaps_player(candidate, player_position)
                && (0..count).all(|other| {
                    other == index || !living[other] || !overlaps(candidate, positions[other])
                });
            if open {
                actor.position = candidate;
                actor.velocity_y = velocity_y;
                actor.grounded = grounded;
                positions[index] = candidate;
            }
        }
    }
    pub fn snapshot(&self) -> serde_json::Value {
        let alive = self.actors.iter().filter(|actor| actor.alive).count();
        serde_json::json!({"player":self.player,"actors":self.actors,"total":self.actors.len(),
            "alive":alive,"killed":self.actors.len()-alive})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CameraMode, Engine, Input, Request};

    fn spawn() -> crate::Camera {
        crate::Camera {
            position: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            mode: CameraMode::Walk,
        }
    }
    fn specs(positions: &[V3]) -> Vec<Spec> {
        positions
            .iter()
            .enumerate()
            .map(|(id, position)| Spec {
                id: id as u32,
                position: *position,
            })
            .collect()
    }
    fn combat(positions: &[V3]) -> Combat {
        Combat::new(1.0 / 6.0, specs(positions), spawn()).unwrap()
    }
    fn advance(combat: &mut Combat, position: V3, yaw: f32, clear: bool) {
        combat.advance(
            position,
            yaw,
            |p, d, _, _| ([p[0] + d[0], 0.0, p[2] + d[2]], 0.0, true),
            |_, _| clear,
        );
    }
    fn engine(positions: &[V3]) -> Engine {
        let mut engine = Engine::default();
        engine.dispatch(Request::SetCamera {
            position: [0.0; 3],
            yaw: 0.0,
            pitch: 0.0,
            mode: CameraMode::Walk,
        });
        engine.dispatch(Request::SetCharacterController {
            controller: Some(crate::quick_mmorpg::Configuration {
                profile: "quick-mmorpg-v1".into(),
                world_scale: 1.0 / 6.0,
            }),
        });
        assert_eq!(
            engine.dispatch(Request::CombatInit {
                world_scale: 1.0 / 6.0,
                zombies: specs(positions)
            })["ok"],
            true
        );
        engine
    }

    #[test]
    fn source_sword_hits_once_at_windup_and_requires_range_facing_and_clear_cover() {
        for (position, clear, hit) in [
            ([0.0, 0.0, -1.2], true, true),
            ([0.0, 0.0, 1.2], true, false),
            ([1.2, 0.0, 0.0], true, false),
            ([0.0, 0.0, -3.0], true, false),
            ([0.0, 0.0, -1.2], false, false),
        ] {
            let mut value = combat(&[position]);
            assert_eq!(value.player.health, 200);
            assert_eq!(
                (value.actors[0].health, value.actors[0].max_health),
                (20, 50)
            );
            assert_eq!(value.attack("sword-1").unwrap(), (true, false));
            for _ in 0..20 {
                advance(&mut value, [0.0; 3], 0.0, clear);
            }
            assert!(value.actors[0].alive);
            advance(&mut value, [0.0; 3], 0.0, clear);
            assert_eq!(value.actors[0].alive, !hit);
            assert_eq!(value.player.hits, u32::from(hit));
            assert_eq!(value.attack("sword-1").unwrap(), (true, true));
            assert_eq!(value.attack("too-soon").unwrap(), (false, false));
            for _ in 0..90 {
                advance(&mut value, [0.0; 3], 0.0, clear);
            }
            assert_eq!(value.player.hits, u32::from(hit));
            assert_eq!(value.player.attacks, 1);
            assert_eq!(value.attack("too-soon").unwrap(), (false, true));
            assert_eq!(value.attack("sword-2").unwrap(), (true, false));
        }
    }

    #[test]
    fn zombies_wind_up_once_and_finish_the_authored_clip_before_attacking_again() {
        let mut value = combat(&[[0.0, 0.0, -1.2]]);
        advance(&mut value, [0.0; 3], 0.0, true);
        assert_eq!(value.actors[0].state, State::Attack);
        for _ in 0..59 {
            advance(&mut value, [0.0; 3], 0.0, true);
        }
        assert_eq!(value.player.health, 200);
        advance(&mut value, [0.0; 3], 0.0, true);
        assert_eq!(value.player.health, 195);
        for _ in 60..232 {
            advance(&mut value, [0.0; 3], 0.0, true);
        }
        assert_eq!(value.player.health, 195);
        assert_eq!(value.actors[0].state, State::Walk);
        advance(&mut value, [0.0; 3], 0.0, true);
        assert_eq!(value.actors[0].state, State::Attack);
        for _ in 0..60 {
            advance(&mut value, [0.0; 3], 0.0, true);
        }
        assert_eq!(value.player.health, 190);

        let mut escaped = combat(&[[0.0, 0.0, -1.2]]);
        advance(&mut escaped, [0.0; 3], 0.0, true);
        for _ in 0..60 {
            advance(&mut escaped, [0.0, 0.0, 5.0], 0.0, true);
        }
        assert_eq!(escaped.player.health, 200);
        let mut covered = combat(&[[0.0, 0.0, -1.2]]);
        advance(&mut covered, [0.0; 3], 0.0, true);
        for _ in 0..60 {
            advance(&mut covered, [0.0; 3], 0.0, false);
        }
        assert_eq!(covered.player.health, 200);
    }

    #[test]
    fn initial_chase_is_immediate_but_lost_targets_use_the_source_scan_interval() {
        let mut value = combat(&[[0.0, 0.0, -6.0]]);
        advance(&mut value, [0.0; 3], 0.0, true);
        assert!((value.actors[0].position[2] + 6.0 - 10.0 / 6.0 * DT).abs() < 0.000001);
        advance(&mut value, [0.0, 0.0, 30.0], 0.0, true);
        assert_eq!(value.actors[0].state, State::Idle);
        for _ in 0..299 {
            advance(&mut value, [0.0; 3], 0.0, true);
        }
        assert_eq!(value.actors[0].state, State::Idle);
        advance(&mut value, [0.0; 3], 0.0, true);
        assert_eq!(value.actors[0].state, State::Walk);
    }

    #[test]
    fn chasing_uses_real_triangle_walls_and_hits_do_not_pass_through_them() {
        let mut value = engine(&[[0.0, 0.0, -4.0]]);
        value.dispatch(Request::SetStaticCollision {
            triangles: vec![
                [[-4.0, 0.0, -2.0], [4.0, 0.0, -2.0], [4.0, 4.0, -2.0]],
                [[-4.0, 0.0, -2.0], [4.0, 4.0, -2.0], [-4.0, 4.0, -2.0]],
            ],
            owners: None,
            append: false,
        });
        for _ in 0..600 {
            value.step(DT, Input::default());
        }
        let combat = value.quick_combat.as_ref().unwrap();
        assert!(combat.actors[0].position[2] <= -2.299);
        assert!(combat.actors[0].position[2] > -4.0);
        assert_eq!(combat.player.health, 200);

        let mut covered = engine(&[[0.0, 0.0, -1.2]]);
        covered.dispatch(Request::SetStaticCollision {
            triangles: vec![
                [[-4.0, 0.0, -0.6], [4.0, 0.0, -0.6], [4.0, 4.0, -0.6]],
                [[-4.0, 0.0, -0.6], [4.0, 4.0, -0.6], [-4.0, 4.0, -0.6]],
            ],
            owners: None,
            append: false,
        });
        covered.dispatch(Request::CombatAttack {
            attack_id: "wall-covered".into(),
        });
        for _ in 0..300 {
            covered.step(DT, Input::default());
        }
        let combat = covered.quick_combat.as_ref().unwrap();
        assert_eq!(combat.actors[0].health, 20);
        assert_eq!(combat.player.health, 200);
        assert_eq!(combat.player.hits, 0);
    }

    #[test]
    fn eight_chasers_keep_separate_capsules_and_reproduce_the_same_result() {
        let positions: Vec<_> = (0..8)
            .map(|i| {
                let angle = i as f32 * std::f32::consts::TAU / 8.0;
                [6.0 * angle.sin(), 0.0, 6.0 * angle.cos()]
            })
            .collect();
        let mut a = engine(&positions);
        let mut b = a.clone();
        for _ in 0..600 {
            a.step(DT, Input::default());
            b.step(DT, Input::default());
            let combat = a.quick_combat.as_ref().unwrap();
            for (index, actor) in combat.actors.iter().enumerate() {
                assert!(!overlaps(actor.position, a.camera.position));
                for other in &combat.actors[..index] {
                    assert!(!overlaps(actor.position, other.position));
                }
            }
        }
        assert_eq!(
            a.quick_combat.as_ref().unwrap().snapshot(),
            b.quick_combat.as_ref().unwrap().snapshot()
        );
        assert!(
            a.quick_combat
                .as_ref()
                .unwrap()
                .actors
                .iter()
                .all(|actor| horizontal(actor.position, [0.0; 3]) < 1.7)
        );
    }

    #[test]
    fn death_stops_player_movement_and_restart_restores_the_complete_encounter() {
        let positions: Vec<_> = (0..8)
            .map(|i| {
                let angle = i as f32 * std::f32::consts::TAU / 8.0;
                [1.2 * angle.sin(), 0.0, 1.2 * angle.cos()]
            })
            .collect();
        let mut value = engine(&positions);
        for _ in 0..1200 {
            value.step(DT, Input::default());
        }
        let combat = value.quick_combat.as_ref().unwrap();
        assert_eq!(combat.player.health, 0);
        assert_eq!(combat.player.state, State::Death);
        assert!(!combat.player.alive);
        let position = value.camera.position;
        let yaw = value.camera.yaw;
        for _ in 0..60 {
            value.step(
                DT,
                Input {
                    forward: 1.0,
                    turn: 1.0,
                    sprint: true,
                    ..Input::default()
                },
            );
        }
        assert_eq!(value.camera.position, position);
        assert_eq!(value.camera.yaw, yaw);
        assert_eq!(
            value.dispatch(Request::CombatAttack {
                attack_id: "dead-strike".into()
            })["accepted"],
            false
        );
        assert_eq!(value.dispatch(Request::CombatReset)["ok"], true);
        let combat = value.quick_combat.as_ref().unwrap();
        assert_eq!(combat.player.health, 200);
        assert!(combat.player.alive);
        assert!(
            combat
                .actors
                .iter()
                .all(|actor| actor.alive && actor.health == 20)
        );
        assert_eq!(value.camera.position, [0.0; 3]);
        assert_eq!(value.revision, 0);
        assert_eq!(value.quick_controller.as_ref().unwrap().velocity, 0.0);
    }

    #[test]
    fn invalid_or_occupied_initialization_is_atomic_and_legacy_has_no_combat() {
        let mut value = engine(&[[0.0, 0.0, -4.0]]);
        let before = value.quick_combat.as_ref().unwrap().snapshot();
        for (scale, zombies) in [
            (1.0 / 6.0, specs(&[[0.0; 3]])),
            (0.5, specs(&[[0.0, 0.0, -4.0]])),
            (
                1.0 / 6.0,
                vec![Spec {
                    id: 0,
                    position: [f32::NAN, 0.0, 0.0],
                }],
            ),
            (1.0 / 6.0, specs(&[[0.0, 0.0, -4.0]; 17])),
        ] {
            assert_eq!(
                value.dispatch(Request::CombatInit {
                    world_scale: scale,
                    zombies
                })["ok"],
                false
            );
            assert_eq!(value.quick_combat.as_ref().unwrap().snapshot(), before);
        }
        value.dispatch(Request::SetCharacterController { controller: None });
        assert!(value.quick_combat.is_none());
        let mut legacy = Engine::default();
        assert_eq!(
            legacy.dispatch(Request::CombatInit {
                world_scale: 1.0 / 6.0,
                zombies: vec![]
            })["ok"],
            false
        );
        assert!(legacy.reply("state", false)["combat"].is_null());
    }

    #[test]
    fn occupied_restart_keeps_health_position_and_every_actor_unchanged() {
        let mut value = engine(&[[0.0, 0.0, -1.2]]);
        for _ in 0..62 {
            value.step(DT, Input::default());
        }
        assert_eq!(value.quick_combat.as_ref().unwrap().player.health, 195);
        let before = value.quick_combat.as_ref().unwrap().snapshot();
        let position = value.camera.position;
        value.dispatch(Request::SetStaticCollision {
            triangles: vec![
                [[-1.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 3.0, 0.0]],
                [[-1.0, 0.0, 0.0], [1.0, 3.0, 0.0], [-1.0, 3.0, 0.0]],
            ],
            owners: None,
            append: false,
        });
        assert_eq!(value.dispatch(Request::CombatReset)["ok"], false);
        assert_eq!(value.quick_combat.as_ref().unwrap().snapshot(), before);
        assert_eq!(value.camera.position, position);
    }

    #[test]
    fn short_zombie_capsule_fits_below_a_ceiling_that_blocks_the_player() {
        let mut value = engine(&[[0.0, 0.0, -4.0]]);
        value.dispatch(Request::SetStaticCollision {
            triangles: vec![
                [[-2.0, 1.4, -5.0], [2.0, 1.4, -5.0], [2.0, 1.4, -2.0]],
                [[-2.0, 1.4, -5.0], [2.0, 1.4, -2.0], [-2.0, 1.4, -2.0]],
            ],
            owners: None,
            append: false,
        });
        assert!(value.can_stand_height([0.0, 0.0, -4.0], ZOMBIE_HEIGHT));
        assert!(!value.can_stand_world([0.0, 0.0, -4.0]));
        for _ in 0..180 {
            value.step(DT, Input::default());
        }
        let actor = &value.quick_combat.as_ref().unwrap().actors[0];
        assert!(actor.position[2] > -1.7, "{:?}", actor.position);
        assert_eq!(actor.position[1], 0.0);
    }
}
