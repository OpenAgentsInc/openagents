//! Local battle movement uses the shared MMORPG motor, separate from the shared-world player.
use af_mmorpg_core::{Class, Controls, DT, Motion, Prediction, Surface, V3};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

const PLAYER_RADIUS: f32 = 0.4;
const CONTACT_MARGIN: f32 = 0.12;
const MAX_CONTACT_AGE: f32 = 0.12;
const MAX_CROWD_SPEED: f32 = 4.;
const MAX_BODY_RADIUS: f32 = 1.33;

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Contact {
    id: u32,
    position: V3,
    radius: f32,
    velocity: [f32; 2],
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContactSample {
    epoch: u32,
    tick: u32,
    position: V3,
    contacts: Vec<Contact>,
    overflow: u32,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CrowdContext {
    expected_epoch: u32,
    current_tick: u32,
    age_seconds: f32,
    sample: Option<ContactSample>,
}
impl CrowdContext {
    fn valid(&self) -> bool {
        if !self.age_seconds.is_finite() || !(0.0..=10.).contains(&self.age_seconds) {
            return false;
        }
        let Some(sample) = &self.sample else {
            return true;
        };
        let mut ids = std::collections::HashSet::new();
        af_mmorpg_core::finite_position(sample.position)
            && sample.contacts.len() <= 64
            && sample.contacts.iter().all(|c| {
                ids.insert(c.id)
                    && c.id < 1_000_000
                    && af_mmorpg_core::finite_position(c.position)
                    && c.radius.is_finite()
                    && (0.1..=MAX_BODY_RADIUS + 0.0001).contains(&c.radius)
                    && c.velocity.iter().all(|v| v.is_finite() && v.abs() <= 20.)
            })
    }
    fn fresh(&self, position: V3, minimum_epoch: Option<u32>) -> bool {
        let Some(sample) = &self.sample else {
            return false;
        };
        sample.epoch == self.expected_epoch
            && minimum_epoch.is_none_or(|epoch| sample.epoch >= epoch)
            && sample.tick > 0
            && self.current_tick >= sample.tick
            && self.current_tick - sample.tick <= 8
            && self.age() <= MAX_CONTACT_AGE
            && sample.overflow == 0
            && (position[0] - sample.position[0]).hypot(position[2] - sample.position[2]) <= 0.75
    }
    fn age(&self) -> f32 {
        self.age_seconds
            .max(self.sample.as_ref().map_or(0., |sample| {
                self.current_tick.saturating_sub(sample.tick) as f32 * DT
            }))
    }
    fn recovery_radius(&self) -> f32 {
        (4. - MAX_BODY_RADIUS - PLAYER_RADIUS - CONTACT_MARGIN - MAX_CROWD_SPEED * self.age())
            .min(2.)
            .max(0.)
    }
    fn circles(&self) -> Vec<Circle> {
        let Some(sample) = &self.sample else {
            return vec![];
        };
        let uncertainty = MAX_CROWD_SPEED * self.age();
        let mut contacts = sample.contacts.iter().collect::<Vec<_>>();
        contacts.sort_by_key(|c| c.id);
        contacts
            .into_iter()
            .map(|c| Circle {
                // An actor may stop or reverse after the sample; cover every reachable center.
                center: [c.position[0], c.position[2]],
                radius: c.radius + PLAYER_RADIUS + CONTACT_MARGIN + uncertainty,
            })
            .collect()
    }
}
#[derive(Clone, Copy, Debug)]
struct Circle {
    center: [f32; 2],
    radius: f32,
}
fn distance(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
fn clear_position(position: [f32; 2], circles: &[Circle]) -> bool {
    circles
        .iter()
        .all(|c| distance(position, c.center) >= c.radius - 0.0001)
}
fn safe_position(
    start: [f32; 2],
    origin: [f32; 2],
    circles: &[Circle],
    max_distance: f32,
) -> Option<[f32; 2]> {
    if clear_position(start, circles) {
        return Some(start);
    }
    let mut point = start;
    for _ in 0..8 {
        for c in circles {
            let d = distance(point, c.center);
            if d < c.radius {
                let normal = if d > 1e-5 {
                    [(point[0] - c.center[0]) / d, (point[1] - c.center[1]) / d]
                } else {
                    [1., 0.]
                };
                point = [
                    c.center[0] + normal[0] * (c.radius + 0.001),
                    c.center[1] + normal[1] * (c.radius + 0.001),
                ];
            }
        }
        if distance(point, origin) > max_distance {
            break;
        }
        if clear_position(point, circles) {
            return Some(point);
        }
    }
    // The four-metre GPU neighborhood covers every candidate plus the largest body.
    for ring in 1..=8 {
        for direction in 0..32 {
            let angle = direction as f32 * std::f32::consts::TAU / 32.;
            let candidate = [
                origin[0] + angle.cos() * ring as f32 * max_distance / 8.,
                origin[1] + angle.sin() * ring as f32 * max_distance / 8.,
            ];
            if clear_position(candidate, circles) {
                return Some(candidate);
            }
        }
    }
    None
}
fn sweep_slide(start: [f32; 2], desired: [f32; 2], circles: &[Circle]) -> [f32; 2] {
    let mut point = start;
    let mut remaining = [desired[0] - start[0], desired[1] - start[1]];
    for _ in 0..4 {
        let a = (remaining[0] as f64).powi(2) + (remaining[1] as f64).powi(2);
        if a < 1e-14 {
            break;
        }
        let mut first = 1f64;
        let mut hit = None;
        for c in circles {
            let offset = [
                (point[0] - c.center[0]) as f64,
                (point[1] - c.center[1]) as f64,
            ];
            let b = offset[0] * remaining[0] as f64 + offset[1] * remaining[1] as f64;
            let cc = offset[0] * offset[0] + offset[1] * offset[1] - (c.radius as f64).powi(2);
            let discriminant = b * b - a * cc;
            if b >= 0. || discriminant < 0. {
                continue;
            }
            let time = ((-b - discriminant.sqrt()) / a).max(0.);
            if time < first {
                first = time;
                hit = Some(c);
            }
        }
        let Some(c) = hit else {
            point[0] += remaining[0];
            point[1] += remaining[1];
            break;
        };
        let advance = (first as f32 - 0.0001 / (a.sqrt() as f32)).max(0.);
        point[0] += remaining[0] * advance;
        point[1] += remaining[1] * advance;
        remaining[0] *= 1. - first as f32;
        remaining[1] *= 1. - first as f32;
        let d = distance(point, c.center).max(1e-8);
        let normal = [(point[0] - c.center[0]) / d, (point[1] - c.center[1]) / d];
        let into = remaining[0] * normal[0] + remaining[1] * normal[1];
        if into < 0. {
            remaining[0] -= normal[0] * into;
            remaining[1] -= normal[1] * into;
        }
    }
    if clear_position(point, circles) {
        point
    } else {
        start
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Terrain {
    layout: String,
    resolution: usize,
    size: f32,
    center: [f32; 2],
    heights: Vec<f32>,
}
impl Terrain {
    fn valid(&self) -> bool {
        self.layout == "mmorpg-patch"
            && self.resolution == 257
            && self.size == 3328.
            && self.heights.len() == 257 * 257
            && self
                .center
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 20_000. && v % 16. == 0.)
            && self
                .heights
                .iter()
                .all(|v| v.is_finite() && v.abs() <= 20_000.)
    }
    fn contains(&self, p: V3) -> bool {
        af_mmorpg_core::finite_position(p)
            && (p[0] - self.center[0]).abs() <= 1663.5
            && (p[2] - self.center[1]).abs() <= 1663.5
    }
    fn coordinate(local: f32) -> f32 {
        let distance = local.abs();
        (128.
            + local.signum()
                * if distance <= 128. {
                    distance / 2.
                } else {
                    64. + (distance - 128.) / 24.
                })
        .clamp(0., 256.)
    }
}
impl Surface for Terrain {
    fn height(&self, x: f32, z: f32) -> f32 {
        let u = Self::coordinate(x - self.center[0]);
        let v = Self::coordinate(z - self.center[1]);
        let a = u.floor() as usize;
        let b = v.floor() as usize;
        let c = (a + 1).min(256);
        let d = (b + 1).min(256);
        let f = u - a as f32;
        let g = v - b as f32;
        let tl = self.heights[b * 257 + a];
        let tr = self.heights[b * 257 + c];
        let bl = self.heights[d * 257 + a];
        let br = self.heights[d * 257 + c];
        if f + g <= 1. {
            tl + (tr - tl) * f + (bl - tl) * g
        } else {
            br + (bl - br) * (1. - f) + (tr - br) * (1. - g)
        }
    }
    fn can_occupy(&self, p: V3, _: f32) -> bool {
        self.contains(p) && p[1] >= self.height(p[0], p[2]) - 0.001
    }
}

#[derive(Clone)]
pub struct Participant {
    terrain: Arc<Terrain>,
    class: Class,
    state: Prediction,
    accumulator: f32,
    dropped: f32,
    tick: u64,
    attack_sequence: u32,
    velocity: V3,
    pitch: f32,
    minimum_contact_epoch: Option<u32>,
    contact_epoch: Option<u32>,
    crowd_ready: bool,
    crowd_blocked: bool,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    class: Class,
    position: V3,
    eye: V3,
    yaw: f32,
    pitch: f32,
    height: f32,
    grounded: bool,
    velocity: V3,
    speed: f32,
    state: Motion,
    action_time: f32,
    animation_time: f32,
    alive: bool,
    attack_sequence: u32,
    tick: u64,
    dropped_seconds: f32,
    crowd_ready: bool,
    crowd_blocked: bool,
}
impl Participant {
    pub fn new(
        terrain: Terrain,
        class: Class,
        position: V3,
        yaw: f32,
    ) -> Result<Self, &'static str> {
        if !terrain.valid() || !class.is_player() {
            return Err("Choose a supported battlefield and player character.");
        }
        let mut value = Self {
            terrain: Arc::new(terrain),
            class,
            state: Prediction::new([0.; 3], 0.),
            accumulator: 0.,
            dropped: 0.,
            tick: 0,
            attack_sequence: 0,
            velocity: [0.; 3],
            pitch: 0.,
            minimum_contact_epoch: None,
            contact_epoch: None,
            crowd_ready: false,
            crowd_blocked: false,
        };
        value.respawn(position, yaw)?;
        Ok(value)
    }
    pub fn respawn(&mut self, position: V3, yaw: f32) -> Result<(), &'static str> {
        if !self.terrain.contains(position) || !yaw.is_finite() || yaw.abs() > 100_000. {
            return Err("Choose a position inside the battlefield.");
        }
        let feet = [
            position[0],
            self.terrain.height(position[0], position[2]),
            position[2],
        ];
        self.state = Prediction::new(
            feet,
            (yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI,
        );
        self.state.grounded = true;
        self.accumulator = 0.;
        self.velocity = [0.; 3];
        self.pitch = 0.;
        self.minimum_contact_epoch = self.contact_epoch.map(|epoch| epoch.saturating_add(1));
        self.crowd_ready = false;
        self.crowd_blocked = false;
        Ok(())
    }
    pub fn step(
        &mut self,
        elapsed: f32,
        input: Controls,
        alive: bool,
        yaw_delta: f32,
        pitch_delta: f32,
        crowd: Option<CrowdContext>,
    ) -> Result<(), &'static str> {
        if !elapsed.is_finite()
            || !(0.0..=10.).contains(&elapsed)
            || !input.valid()
            || !yaw_delta.is_finite()
            || yaw_delta.abs() > std::f32::consts::PI
            || !pitch_delta.is_finite()
            || pitch_delta.abs() > std::f32::consts::PI
            || crowd.as_ref().is_some_and(|context| !context.valid())
        {
            return Err("Use supported battle movement buttons and frame duration.");
        }
        if !alive {
            self.state.set_state(Motion::Death);
            self.state.velocity = 0.;
            self.state.vertical_speed = 0.;
        }
        if !matches!(
            self.state.state,
            Motion::Attack | Motion::Dance | Motion::Death
        ) {
            self.state.yaw = (self.state.yaw + yaw_delta + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
        }
        if self.state.state != Motion::Death {
            self.pitch = (self.pitch + pitch_delta).clamp(-1.45, 1.45);
        }
        self.accumulator += elapsed.min(0.25);
        self.dropped += (elapsed - 0.25).max(0.);
        let mut ticks = 0;
        self.crowd_ready = crowd
            .as_ref()
            .is_some_and(|context| context.fresh(self.state.position, self.minimum_contact_epoch));
        self.crowd_blocked = !self.crowd_ready;
        let circles = if self.crowd_ready {
            crowd.as_ref().unwrap().circles()
        } else {
            vec![]
        };
        if self.crowd_ready && self.state.state != Motion::Death {
            self.contact_epoch = Some(crowd.as_ref().unwrap().expected_epoch);
            let sample = crowd.as_ref().unwrap().sample.as_ref().unwrap();
            if let Some(safe) = safe_position(
                [self.state.position[0], self.state.position[2]],
                [sample.position[0], sample.position[2]],
                &circles,
                crowd.as_ref().unwrap().recovery_radius(),
            )
            .filter(|p| {
                self.terrain
                    .contains([p[0], self.terrain.height(p[0], p[1]), p[1]])
            }) {
                self.state.position = [safe[0], self.terrain.height(safe[0], safe[1]), safe[1]];
            } else {
                self.crowd_ready = false;
                self.crowd_blocked = true;
            }
        }
        let previous = self.state.position;
        while self.accumulator + 1e-7 >= DT && ticks < 4 {
            let before = self.state.position;
            if !self.crowd_ready {
                self.state.velocity = 0.;
            }
            if af_mmorpg_core::predict(
                &mut self.state,
                input,
                self.class,
                1. / 6.,
                self.terrain.as_ref(),
            ) {
                self.attack_sequence = self.attack_sequence.wrapping_add(1);
            }
            let desired = self.state.position;
            let mut resolved = if self.crowd_ready {
                sweep_slide([before[0], before[2]], [desired[0], desired[2]], &circles)
            } else {
                [before[0], before[2]]
            };
            if !self.terrain.contains([
                resolved[0],
                self.terrain.height(resolved[0], resolved[1]),
                resolved[1],
            ]) {
                resolved = [before[0], before[2]];
            }
            if distance(resolved, [desired[0], desired[2]]) > 0.0001 {
                self.crowd_blocked = true;
            }
            self.state.position = [
                resolved[0],
                self.terrain.height(resolved[0], resolved[1]),
                resolved[1],
            ];
            self.state.grounded = true;
            self.state.vertical_speed = 0.;
            if distance(resolved, [before[0], before[2]]) < 0.00001 {
                self.state.velocity = 0.;
            }
            self.accumulator = (self.accumulator - DT).max(0.);
            self.tick = self.tick.saturating_add(1);
            ticks += 1;
        }
        if self.accumulator >= DT {
            let keep = self.accumulator % DT;
            self.dropped += self.accumulator - keep;
            self.accumulator = keep;
        }
        self.velocity = if ticks > 0 {
            std::array::from_fn(|i| (self.state.position[i] - previous[i]) / (DT * ticks as f32))
        } else {
            self.velocity
        };
        if self.state.state == Motion::Death {
            self.velocity = [0.; 3];
        }
        Ok(())
    }
    pub fn snapshot(&self) -> Snapshot {
        let position = self.state.position;
        let time = self.state.state_ticks as f32 * DT;
        Snapshot {
            class: self.class,
            position,
            eye: [position[0], position[1] + 1.62, position[2]],
            yaw: self.state.yaw,
            pitch: self.pitch,
            height: self.class.height(),
            grounded: self.state.grounded,
            velocity: self.velocity,
            speed: self.velocity[0].hypot(self.velocity[2]),
            state: self.state.state,
            action_time: time,
            animation_time: time,
            alive: self.state.state != Motion::Death,
            attack_sequence: self.attack_sequence,
            tick: self.tick,
            dropped_seconds: self.dropped,
            crowd_ready: self.crowd_ready,
            crowd_blocked: self.crowd_blocked,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Engine, Request};
    use serde_json::json;

    fn terrain() -> Terrain {
        Terrain {
            layout: "mmorpg-patch".into(),
            resolution: 257,
            size: 3328.,
            center: [-16., 32.],
            heights: vec![0.; 257 * 257],
        }
    }
    fn participant() -> Participant {
        Participant::new(terrain(), Class::Paladin, [-16., 999., 32.], 0.).unwrap()
    }
    fn clear_contact(player: &Participant) -> CrowdContext {
        let tick = (player.tick as u32).saturating_add(1);
        let epoch = player.minimum_contact_epoch.unwrap_or(1);
        CrowdContext {
            expected_epoch: epoch,
            current_tick: tick,
            age_seconds: 0.,
            sample: Some(ContactSample {
                epoch,
                tick,
                position: player.state.position,
                contacts: vec![],
                overflow: 0,
            }),
        }
    }
    fn contact_frame(player: &Participant, contacts: Vec<Contact>) -> CrowdContext {
        let mut frame = clear_contact(player);
        frame.sample.as_mut().unwrap().contacts = contacts;
        frame
    }
    fn body(id: u32, position: V3, radius: f32) -> Contact {
        Contact {
            id,
            position,
            radius,
            velocity: [0.; 2],
        }
    }
    #[test]
    fn swept_contacts_stop_tunneling_and_preserve_tangent_motion() {
        let circles = [Circle {
            center: [0., 0.],
            radius: 1.85,
        }];
        let stopped = sweep_slide([-5., 0.], [5., 0.], &circles);
        assert!(stopped[0] < -1.8499 && stopped[0] > -1.851);
        assert!(clear_position(stopped, &circles));
        let slid = sweep_slide([-3., -2.], [3., 0.], &circles);
        assert!(clear_position(slid, &circles));
        assert!(slid[0] > -1.5 && slid[1] < -1., "{slid:?}");
        let dense = [
            Circle {
                center: [0., -0.7],
                radius: 1.15,
            },
            Circle {
                center: [0., 0.7],
                radius: 1.15,
            },
        ];
        let blocked = sweep_slide([-3., 0.], [3., 0.], &dense);
        assert!(blocked[0] < 0. && clear_position(blocked, &dense));
    }
    #[test]
    fn live_warrok_clearance_blocks_run_and_keeps_exact_terrain_support() {
        let mut player = participant();
        let center = [-16., 0., 28.5];
        for _ in 0..240 {
            let frame = contact_frame(&player, vec![body(7, center, 1.33)]);
            player
                .step(
                    DT,
                    Controls {
                        forward: 1.,
                        sprint: true,
                        ..Default::default()
                    },
                    true,
                    0.,
                    0.,
                    Some(frame),
                )
                .unwrap();
            let p = player.state.position;
            assert!(
                (p[0] - center[0]).hypot(p[2] - center[2]) >= 1.8499,
                "{p:?}"
            );
            assert_eq!(p[1], player.terrain.height(p[0], p[2]));
        }
        assert!(player.state.position[2] <= 30.36 && player.state.position[2] >= 30.349);
        assert!(player.crowd_ready && player.crowd_blocked);
    }
    #[test]
    fn spawn_overlap_chooses_free_position_and_moving_contacts_predict_forward() {
        let mut player = participant();
        let frame = contact_frame(&player, vec![body(4, player.state.position, 1.33)]);
        player
            .step(0., Controls::default(), true, 0., 0., Some(frame))
            .unwrap();
        assert!(
            distance(
                [player.state.position[0], player.state.position[2]],
                [-16., 32.]
            ) >= 1.85
        );
        assert!(player.crowd_ready && player.snapshot().speed == 0.);
        let mut frame = clear_contact(&player);
        frame.age_seconds = 0.05;
        let mut moving = body(
            2,
            [player.state.position[0] + 2., 0., player.state.position[2]],
            0.63,
        );
        moving.velocity = [-2., 0.];
        frame.sample.as_mut().unwrap().contacts.push(moving);
        let circle = frame.circles()[0];
        assert!((circle.center[0] - (player.state.position[0] + 2.)).abs() < 0.0001);
        assert!((circle.radius - 1.35).abs() < 0.0001);
    }
    #[test]
    fn stale_velocity_cannot_open_a_gap_when_an_actor_stops_or_reverses() {
        let player = participant();
        let mut frame = contact_frame(&player, vec![body(7, [-13.9, 0., 32.], 1.33)]);
        frame.age_seconds = 0.12;
        frame.sample.as_mut().unwrap().contacts[0].velocity = [2., 0.];
        let circles = frame.circles();
        assert!((circles[0].radius - 2.33).abs() < 0.0001);
        assert!((frame.recovery_radius() - 1.67).abs() < 0.0001);
        let safe =
            safe_position([-16., 32.], [-16., 32.], &circles, frame.recovery_radius()).unwrap();
        let moved = sweep_slide(safe, [-10., 32.], &circles);
        for actual_x in [-13.9, -13.9 - 4. * 0.12] {
            assert!(
                distance(moved, [actual_x, 32.]) >= 1.73,
                "{moved:?} vs {actual_x}"
            );
        }
        // Query age and GPU tick age overlap; they must not be added together.
        frame.age_seconds = 0.05;
        frame.current_tick = frame.sample.as_ref().unwrap().tick + 3;
        assert!((frame.age() - 0.05).abs() < 0.0001);
    }
    #[test]
    fn stale_or_overflow_contacts_halt_translation_without_stopping_actions() {
        for reason in 0..5 {
            let mut player = participant();
            let before = player.state.position;
            let mut frame = clear_contact(&player);
            match reason {
                0 => frame.age_seconds = 0.2,
                1 => frame.sample.as_mut().unwrap().overflow = 1,
                2 => frame.sample.as_mut().unwrap().epoch += 1,
                3 => frame.current_tick += 10,
                _ => frame.sample.as_mut().unwrap().position[0] += 1.,
            }
            for _ in 0..20 {
                player
                    .step(
                        DT,
                        Controls {
                            forward: 1.,
                            ..Default::default()
                        },
                        true,
                        0.01,
                        0.01,
                        Some(frame.clone()),
                    )
                    .unwrap();
            }
            assert_eq!(player.state.position, before);
            assert!(!player.crowd_ready && player.state.yaw.abs() > 0.1 && player.pitch > 0.1);
        }
        let mut player = participant();
        player
            .step(
                DT,
                Controls {
                    attack: true,
                    ..Default::default()
                },
                true,
                0.,
                0.,
                None,
            )
            .unwrap();
        assert_eq!(player.attack_sequence, 1);
        for _ in 0..30 {
            player
                .step(DT, Controls::default(), true, 0., 0., None)
                .unwrap();
        }
        assert!(player.state.state_ticks > 30);
    }
    #[test]
    fn respawn_rejects_previous_life_samples_and_invalid_contacts_are_atomic() {
        let mut player = participant();
        let frame = clear_contact(&player);
        player
            .step(DT, Controls::default(), true, 0., 0., Some(frame.clone()))
            .unwrap();
        player.respawn([-16., 0., 32.], 0.).unwrap();
        player
            .step(
                DT,
                Controls {
                    forward: 1.,
                    ..Default::default()
                },
                true,
                0.,
                0.,
                Some(frame),
            )
            .unwrap();
        assert!(!player.crowd_ready);
        assert_eq!(player.state.position, [-16., 0., 32.]);
        let mut fresh = clear_contact(&player);
        assert_eq!(fresh.expected_epoch, 2);
        fresh
            .sample
            .as_mut()
            .unwrap()
            .contacts
            .push(body(1, [-16., 0., 27.], f32::NAN));
        let before = serde_json::to_value(player.snapshot()).unwrap();
        assert!(
            player
                .step(DT, Controls::default(), false, 0., 0., Some(fresh))
                .is_err()
        );
        assert_eq!(serde_json::to_value(player.snapshot()).unwrap(), before);
        let fresh = clear_contact(&player);
        player
            .step(
                DT,
                Controls {
                    forward: 1.,
                    ..Default::default()
                },
                true,
                0.,
                0.,
                Some(fresh),
            )
            .unwrap();
        assert!(player.crowd_ready && player.state.position[2] < 32.);
    }
    #[test]
    fn a_dead_participant_does_not_move_when_live_bodies_cross_the_corpse() {
        let mut player = participant();
        let position = player.state.position;
        let frame = contact_frame(&player, vec![body(4, position, 1.33)]);
        player
            .step(DT, Controls::default(), false, 0., 0., Some(frame))
            .unwrap();
        assert_eq!(player.state.position, position);
        assert_eq!(player.state.state, Motion::Death);
        assert_eq!(player.velocity, [0.; 3]);
    }
    #[test]
    fn ground_samples_exact_fine_coarse_and_rectangular_triangles() {
        let mut ground = terrain();
        let (x, z) = (192usize, 193usize);
        for (i, j, height) in [
            (x, z, 2.),
            (x + 1, z, 10.),
            (x, z + 1, 6.),
            (x + 1, z + 1, 30.),
        ] {
            ground.heights[j * 257 + i] = height;
        }
        // This is the 24×24 m strip beyond the 2 m central grid.
        assert!((ground.height(-16. + 128. + 6., 32. + 152. + 6.) - 5.).abs() < 1e-5);
        assert!((ground.height(-16. + 128. + 18., 32. + 152. + 18.) - 19.).abs() < 1e-5);
        // A mixed 2×24 m strip uses the same a-b-c / d-c-b diagonal.
        ground.heights[193 * 257 + 191] = 4.;
        ground.heights[194 * 257 + 191] = 12.;
        assert!((ground.height(-16. + 127., 32. + 152. + 6.) - 5.).abs() < 1e-5);
        assert_eq!(ground.height(-16., 32.), 0.);
    }
    #[test]
    fn source_walk_run_turn_and_terrain_grounding_use_the_existing_motor() {
        let mut player = participant();
        let mut expected = player.state.clone();
        for tick in 0..180 {
            let input = Controls {
                forward: 1.,
                turn: if tick < 40 { 0.4 } else { 0. },
                sprint: tick >= 60,
                ..Default::default()
            };
            af_mmorpg_core::predict(
                &mut expected,
                input,
                Class::Paladin,
                1. / 6.,
                player.terrain.as_ref(),
            );
            player
                .step(DT, input, true, 0., 0., Some(clear_contact(&player)))
                .unwrap();
            assert_eq!(player.state, expected);
            assert_eq!(player.state.position[1], 0.);
            assert!(player.state.grounded);
        }
        assert!(player.state.velocity > 6.6);
        assert!(player.snapshot().speed > 6.6);
    }
    #[test]
    fn source_heightfield_spawn_and_routes_stay_on_fine_coarse_and_seam_surfaces() {
        let mut ground = terrain();
        let displacement = |i: usize| {
            let offset = i as f64 - 128.;
            let a = offset.abs();
            offset.signum()
                * if a <= 64. {
                    a * 2.
                } else {
                    128. + (a - 64.) * 24.
                }
        };
        for z in 0..257 {
            for x in 0..257 {
                ground.heights[z * 257 + x] = af_mmorpg_core::terrain::height(
                    ground.center[0] as f64 + displacement(x),
                    ground.center[1] as f64 + displacement(z),
                ) as f32;
            }
        }
        for offset in [-1500., -129., -128., -127., 0., 127., 128., 129., 1500.] {
            let at = [
                ground.center[0] + offset,
                800.,
                ground.center[1] + offset / 3.,
            ];
            let mut player = Participant::new(ground.clone(), Class::Paladin, at, 1.).unwrap();
            assert_eq!(player.state.position[1], ground.height(at[0], at[2]));
            for _ in 0..120 {
                player
                    .step(
                        DT,
                        Controls {
                            forward: 1.,
                            ..Default::default()
                        },
                        true,
                        0.,
                        0.,
                        Some(clear_contact(&player)),
                    )
                    .unwrap();
                let p = player.state.position;
                assert!(player.state.grounded, "offset {offset}: {p:?}");
                assert!((p[1] - ground.height(p[0], p[2])).abs() < 0.0001);
            }
        }
    }
    #[test]
    fn mouse_pitch_is_bounded_and_heading_turn_is_independent_of_the_native_camera() {
        let mut player = participant();
        player
            .step(
                0.,
                Controls::default(),
                true,
                0.5,
                3.,
                Some(clear_contact(&player)),
            )
            .unwrap();
        assert!((player.state.yaw - 0.5).abs() < 0.00001);
        assert_eq!(player.pitch, 1.45);
        player
            .step(
                0.,
                Controls::default(),
                true,
                0.,
                -3.,
                Some(clear_contact(&player)),
            )
            .unwrap();
        assert_eq!(player.pitch, -1.45);
        let before = player.pitch;
        assert!(
            player
                .step(
                    0.,
                    Controls::default(),
                    false,
                    0.,
                    f32::NAN,
                    Some(clear_contact(&player))
                )
                .is_err()
        );
        assert_eq!(player.pitch, before);
        assert_ne!(player.state.state, Motion::Death);
    }
    #[test]
    fn attack_edges_locks_death_and_explicit_respawn_are_local_and_deterministic() {
        let mut player = participant();
        let attack = Controls {
            attack: true,
            ..Default::default()
        };
        player
            .step(DT, attack, true, 0., 0., Some(clear_contact(&player)))
            .unwrap();
        assert_eq!(player.attack_sequence, 1);
        assert_eq!(player.state.state, Motion::Attack);
        let position = player.state.position;
        let yaw = player.state.yaw;
        for _ in 0..30 {
            player
                .step(
                    DT,
                    Controls {
                        forward: 1.,
                        turn: 1.,
                        ..attack
                    },
                    true,
                    0.1,
                    0.,
                    Some(clear_contact(&player)),
                )
                .unwrap();
        }
        assert_eq!(player.state.position, position);
        assert_eq!(player.state.yaw, yaw);
        assert_eq!(player.attack_sequence, 1);
        player
            .step(
                DT,
                Controls::default(),
                false,
                0.,
                0.,
                Some(clear_contact(&player)),
            )
            .unwrap();
        for _ in 0..60 {
            player
                .step(
                    DT,
                    Controls {
                        forward: 1.,
                        ..attack
                    },
                    true,
                    0.1,
                    0.,
                    Some(clear_contact(&player)),
                )
                .unwrap();
        }
        assert_eq!(player.state.position, position);
        assert_eq!(player.state.state, Motion::Death);
        assert_eq!(player.velocity, [0.; 3]);
        player.respawn([128., 500., 128.], 1.).unwrap();
        assert_eq!(player.state.position, [128., 0., 128.]);
        assert_eq!(player.state.state, Motion::Idle);
        player
            .step(DT, attack, true, 0., 0., Some(clear_contact(&player)))
            .unwrap();
        assert_eq!(player.attack_sequence, 2);
    }
    #[test]
    fn invalid_inputs_spawn_and_terrain_are_atomic_and_work_is_bounded() {
        let mut player = participant();
        let before = serde_json::to_value(player.snapshot()).unwrap();
        assert!(
            player
                .step(
                    f32::NAN,
                    Controls::default(),
                    false,
                    0.,
                    0.,
                    Some(clear_contact(&player))
                )
                .is_err()
        );
        assert!(
            player
                .step(
                    DT,
                    Controls {
                        turn: 2.,
                        ..Default::default()
                    },
                    false,
                    0.,
                    0.,
                    Some(clear_contact(&player))
                )
                .is_err()
        );
        assert!(player.respawn([20_001., 0., 0.], 0.).is_err());
        assert_eq!(serde_json::to_value(player.snapshot()).unwrap(), before);
        let mut invalid = terrain();
        invalid.heights[0] = f32::INFINITY;
        assert!(Participant::new(invalid, Class::Paladin, [0.; 3], 0.).is_err());
        assert!(Participant::new(terrain(), Class::Zombie, [0.; 3], 0.).is_err());
        player
            .step(
                1.,
                Controls {
                    forward: 1.,
                    ..Default::default()
                },
                true,
                0.,
                0.,
                Some(clear_contact(&player)),
            )
            .unwrap();
        assert_eq!(player.tick, 4);
        assert!(player.dropped > 0.9);
    }
    #[test]
    fn requests_never_change_the_shared_player_world_health_or_tick() {
        let mut engine = Engine::default();
        let native = engine.dispatch(Request::Snapshot);
        let mut ground = terrain();
        let request = json!({"type":"battle_participant_init","terrain":{
            "layout":ground.layout,"resolution":ground.resolution,"size":ground.size,"center":ground.center,"heights":ground.heights
        },"class":"paladin","position":[-16,12,32],"yaw":0});
        let initial = engine.handle_json(&serde_json::to_vec(&request).unwrap());
        assert_eq!(initial["ok"], true);
        assert!(initial.get("player").is_none());
        for _ in 0..120 {
            assert_eq!(
                engine.dispatch(Request::BattleParticipantStep {
                    elapsed: DT,
                    input: Controls {
                        forward: 1.,
                        ..Default::default()
                    },
                    alive: true,
                    yaw_delta: 0.,
                    pitch_delta: 0.,
                    crowd: None
                })["ok"],
                true
            );
        }
        assert_eq!(engine.dispatch(Request::Snapshot), native);
        ground.heights = vec![0.; 2];
        assert_eq!(
            engine.dispatch(Request::BattleParticipantInit {
                terrain: ground,
                class: Class::Paladin,
                position: [0.; 3],
                yaw: 0.
            })["ok"],
            false
        );
        assert!(engine.battle_participant.is_some());
        engine.dispatch(Request::BattleParticipantClear);
        assert!(engine.battle_participant.is_none());
        assert_eq!(engine.dispatch(Request::Snapshot), native);
    }
}
