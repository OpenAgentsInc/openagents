use crate::{Class, DT, V3, finite_position};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Controls {
    #[serde(default)]
    pub forward: f32,
    #[serde(default)]
    pub turn: f32,
    #[serde(default)]
    pub sprint: bool,
    #[serde(default)]
    pub attack: bool,
    #[serde(default)]
    pub dance: bool,
}
impl Controls {
    pub fn valid(self) -> bool {
        self.forward.is_finite()
            && self.turn.is_finite()
            && self.forward.abs() <= 1.0
            && self.turn.abs() <= 1.0
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Motion {
    #[default]
    Idle,
    Walk,
    Run,
    Attack,
    Dance,
    Death,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Prediction {
    pub position: V3,
    pub yaw: f32,
    pub vertical_speed: f32,
    pub grounded: bool,
    pub velocity: f32,
    pub state: Motion,
    pub state_ticks: u32,
    pub cooldown_ticks: u16,
}
impl Prediction {
    pub fn new(position: V3, yaw: f32) -> Self {
        Self {
            position,
            yaw,
            vertical_speed: 0.,
            grounded: false,
            velocity: 0.,
            state: Motion::Idle,
            state_ticks: 0,
            cooldown_ticks: 0,
        }
    }
    pub fn valid(&self) -> bool {
        finite_position(self.position)
            && self.yaw.is_finite()
            && self.yaw.abs() <= 100_000.
            && self.vertical_speed.is_finite()
            && self.vertical_speed.abs() <= 100.
            && self.velocity.is_finite()
            && self.velocity.abs() <= 100.
            && self.state_ticks <= 216_000
            && self.cooldown_ticks <= 600
    }
    pub fn set_state(&mut self, motion: Motion) {
        if self.state != motion {
            self.state = motion;
            self.state_ticks = 0;
        }
    }
}

/// Shared surface queries. Browser and server bind the same released collision.
pub trait Surface {
    fn height(&self, x: f32, z: f32) -> f32;
    fn support(&self, position: V3, _max_y: f32) -> f32 {
        self.height(position[0], position[2])
    }
    fn step_support(&self, position: V3, max_y: f32) -> f32 {
        self.support(position, max_y)
    }
    fn can_occupy(&self, position: V3, _height: f32) -> bool {
        finite_position(position) && position[1] >= self.height(position[0], position[2]) - 0.001
    }
    fn slide_down(&self, _position: V3, _height: f32) -> Option<V3> {
        None
    }
    fn clear(&self, _from: V3, _to: V3) -> bool {
        true
    }
}
pub fn height_line_clear(from: V3, to: V3, height: impl Fn(f32, f32) -> f32) -> bool {
    let steps = (crate::distance(from, to) / 0.25).ceil().clamp(1., 128.) as usize;
    (1..steps).all(|i| {
        let t = i as f32 / steps as f32;
        let p: V3 = std::array::from_fn(|axis| from[axis] + (to[axis] - from[axis]) * t);
        p[1] >= height(p[0], p[2]) - 0.001
    })
}
#[derive(Clone, Copy, Default)]
pub struct FlatSurface(pub f32);
impl Surface for FlatSurface {
    fn height(&self, _: f32, _: f32) -> f32 {
        self.0
    }
}

/// Source longitudinal acceleration, drag, and 90-degree/second turning.
pub fn longitudinal(
    velocity: &mut f32,
    controls: Controls,
    scale: f32,
    locked: bool,
    dt: f32,
) -> (f32, f32) {
    if locked {
        return (0., 0.);
    }
    *velocity += (-5.0 * *velocity * dt).clamp(-velocity.abs(), velocity.abs());
    *velocity += controls.forward * 100.0 * scale * if controls.sprint { 2. } else { 1. } * dt;
    (
        *velocity * dt,
        controls.turn * std::f32::consts::FRAC_PI_2 * dt,
    )
}

/// Advances source actions and movement once, returning an attack-entry edge.
pub fn predict(
    state: &mut Prediction,
    input: Controls,
    class: Class,
    scale: f32,
    surface: &impl Surface,
) -> bool {
    let mut attack_started = false;
    state.state_ticks = state.state_ticks.saturating_add(1).min(216_000);
    state.cooldown_ticks = state.cooldown_ticks.saturating_sub(1);
    match state.state {
        Motion::Death => return false,
        Motion::Attack if state.state_ticks >= class.attack_duration() as u32 => {
            state.set_state(Motion::Idle)
        }
        Motion::Dance if state.state_ticks >= class.dance_duration() as u32 => {
            state.set_state(Motion::Idle)
        }
        Motion::Idle => {
            if input.forward != 0. {
                state.set_state(Motion::Walk);
            } else if input.attack && state.cooldown_ticks == 0 {
                state.set_state(Motion::Attack);
                state.cooldown_ticks = class.attack_cooldown();
                attack_started = true;
            } else if input.dance {
                state.set_state(Motion::Dance);
            }
        }
        Motion::Walk | Motion::Run => {
            state.set_state(if input.forward == 0. {
                Motion::Idle
            } else if input.sprint {
                Motion::Run
            } else {
                Motion::Walk
            });
        }
        _ => {}
    }
    let locked = matches!(state.state, Motion::Attack | Motion::Dance);
    // The source animation mixer advances a newly entered action in this frame.
    if state.state_ticks == 0 {
        state.state_ticks = 1;
    }
    let (distance, turn) = longitudinal(&mut state.velocity, input, scale, locked, DT);
    state.yaw = (state.yaw + turn + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    move_body(
        state,
        [state.yaw.sin() * distance, 0., -state.yaw.cos() * distance],
        class.height(),
        surface,
    );
    attack_started
}

/// Bounded capsule substeps, stair following, grounding, and slope sliding.
pub fn move_body(body: &mut Prediction, mut delta: V3, height: f32, surface: &impl Surface) {
    const EPS: f32 = 0.001;
    let was_grounded = body.grounded;
    body.grounded = false;
    body.vertical_speed = (body.vertical_speed - 9.81 * 1.6 * DT).max(-50.);
    delta[1] = body.vertical_speed * DT;
    let steps = ((delta.iter().map(|v| v.abs()).fold(0.0f32, f32::max) / 0.08).ceil() as usize)
        .clamp(1, 32);
    let horizontal = delta[0] != 0. || delta[2] != 0.;
    for _ in 0..steps {
        let mut together = body.position;
        together[0] += delta[0] / steps as f32;
        together[2] += delta[2] / steps as f32;
        let support = if horizontal {
            surface.step_support(together, body.position[1] + 0.25)
        } else {
            surface.support(together, body.position[1] + 0.25)
        };
        let follows = (was_grounded || body.grounded)
            && body.vertical_speed <= 0.
            && (support - body.position[1]).abs() <= 0.3;
        if follows {
            together[1] = support;
        }
        if surface.can_occupy(together, height) {
            body.position = together;
            if follows {
                body.grounded = true;
                body.vertical_speed = 0.;
            }
        } else {
            for axis in [0, 2] {
                let mut candidate = body.position;
                candidate[axis] += delta[axis] / steps as f32;
                let support = surface.step_support(candidate, body.position[1] + 0.25);
                let follows = (was_grounded || body.grounded)
                    && body.vertical_speed <= 0.
                    && (support - body.position[1]).abs() <= 0.3;
                if follows {
                    candidate[1] = support;
                }
                if surface.can_occupy(candidate, height) {
                    body.position = candidate;
                    if follows {
                        body.grounded = true;
                        body.vertical_speed = 0.;
                    }
                }
            }
        }
        let mut candidate = body.position;
        if !body.grounded || body.vertical_speed != 0. {
            candidate[1] += delta[1] / steps as f32;
        }
        let support = surface.support(candidate, body.position[1] + EPS);
        if delta[1] <= 0. && candidate[1] <= support + EPS && body.position[1] >= support - EPS {
            candidate[1] = support;
            body.vertical_speed = 0.;
            body.grounded = true;
        }
        if surface.can_occupy(candidate, height) {
            body.position = candidate;
        } else if delta[1] < 0. && !body.grounded {
            if let Some(slid) = surface
                .slide_down(candidate, height)
                .filter(|p| surface.can_occupy(*p, height))
            {
                body.position = slid;
            } else {
                body.vertical_speed = 0.;
            }
        } else {
            body.vertical_speed = 0.;
        }
    }
}
