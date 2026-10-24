//! Owned patrols and capsule combat, informed by public OpenAgents Verse wildlife
//! routes and Grove dummy damage/reset at 83fe8b7f6b3cd437ae9b7af213cf9f04875dd793.
//! Movement, targeting, cover checks, and preparation binding are implemented here.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

type V3 = [f32; 3];
pub const RADIUS: f32 = 0.3;
pub const HEIGHT: f32 = 1.8;
const DT: f32 = 1. / 60.;
fn sub(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] - b[i])
}
fn add(a: V3, b: V3) -> V3 {
    std::array::from_fn(|i| a[i] + b[i])
}
fn mul(a: V3, n: f32) -> V3 {
    a.map(|v| v * n)
}
fn dot(a: V3, b: V3) -> f32 {
    a.into_iter().zip(b).map(|(a, b)| a * b).sum()
}
fn length(v: V3) -> f32 {
    dot(v, v).sqrt()
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Spec {
    pub id: u32,
    pub route: Vec<V3>,
    pub speed: f32,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Actor {
    pub id: u32,
    pub position: V3,
    pub yaw: f32,
    pub alive: bool,
    pub death_seconds: f32,
    pub animation_time: f32,
    #[serde(skip)]
    spec: Spec,
    #[serde(skip)]
    waypoint: usize,
    #[serde(skip)]
    velocity_y: f32,
    #[serde(skip)]
    grounded: bool,
}
impl Actor {
    fn new(spec: Spec) -> Self {
        let delta = sub(spec.route[1], spec.route[0]);
        Self {
            id: spec.id,
            position: spec.route[0],
            yaw: delta[0].atan2(-delta[2]),
            alive: true,
            death_seconds: 0.,
            animation_time: 0.,
            waypoint: 1,
            velocity_y: 0.,
            grounded: false,
            spec,
        }
    }
    fn closest(&self, point: V3) -> V3 {
        let axis = [
            self.position[0],
            point[1].clamp(
                self.position[1] + RADIUS,
                self.position[1] + HEIGHT - RADIUS,
            ),
            self.position[2],
        ];
        let delta = sub(point, axis);
        let distance = length(delta);
        if distance <= RADIUS {
            point
        } else {
            add(axis, mul(delta, RADIUS / distance))
        }
    }
}
#[derive(Clone, Default)]
pub struct Enemies {
    pub actors: Vec<Actor>,
}
#[derive(Clone)]
pub struct Hit {
    pub id: u32,
    pub point: V3,
    pub normal: V3,
    pub distance: f32,
}
#[derive(Clone)]
pub struct Impact {
    pub point: V3,
    pub normal: V3,
    pub radius: f32,
    pub damage: u32,
}
impl Enemies {
    pub fn new(specs: Vec<Spec>) -> Result<Self, &'static str> {
        if specs.len() > 16 {
            return Err("This world supports up to 16 enemies.");
        }
        let mut ids = HashSet::new();
        for spec in &specs {
            if spec.id == u32::MAX
                || !ids.insert(spec.id)
                || !(2..=32).contains(&spec.route.len())
                || !spec.speed.is_finite()
                || !(0.1..=6.4).contains(&spec.speed)
                || spec
                    .route
                    .iter()
                    .flatten()
                    .any(|n| !n.is_finite() || n.abs() > 20_000.)
            {
                return Err(
                    "An enemy needs a unique identifier, a bounded route, and a valid speed.",
                );
            }
            for i in 0..spec.route.len() {
                let delta = sub(spec.route[(i + 1) % spec.route.len()], spec.route[i]);
                if delta[0].hypot(delta[2]) < 0.05 {
                    return Err("Each patrol waypoint must lead to a different place.");
                }
            }
        }
        Ok(Self {
            actors: specs.into_iter().map(Actor::new).collect(),
        })
    }
    pub fn advance(&mut self, mut motion: impl FnMut(V3, V3, f32, bool) -> (V3, f32, bool)) {
        for actor in &mut self.actors {
            if !actor.alive {
                actor.death_seconds = (actor.death_seconds + DT).min(3600.);
                continue;
            }
            let mut target = actor.spec.route[actor.waypoint];
            let delta = sub(target, actor.position);
            if delta[0].hypot(delta[2]) < 0.015 && delta[1].abs() < 0.75 {
                actor.waypoint = (actor.waypoint + 1) % actor.spec.route.len();
                target = actor.spec.route[actor.waypoint];
            }
            let delta = sub(target, actor.position);
            let distance = delta[0].hypot(delta[2]);
            let step = (actor.spec.speed * DT).min(distance);
            let desired = if distance > 0.001 {
                actor.yaw = delta[0].atan2(-delta[2]);
                [delta[0] / distance * step, 0., delta[2] / distance * step]
            } else {
                [0.; 3]
            };
            let previous = actor.position;
            (actor.position, actor.velocity_y, actor.grounded) =
                motion(actor.position, desired, actor.velocity_y, actor.grounded);
            let moved = sub(actor.position, previous);
            let accepted_distance = moved[0].hypot(moved[2]);
            if accepted_distance > 0.00001 {
                actor.animation_time =
                    (actor.animation_time + accepted_distance / 3.4).rem_euclid(3600.);
            }
        }
    }
    pub fn aim(&self, origin: V3, ray: V3, range: f32) -> Option<Hit> {
        self.actors
            .iter()
            .filter(|actor| actor.alive)
            .filter_map(|actor| {
                let low = actor.position[1] + RADIUS;
                let high = actor.position[1] + HEIGHT - RADIUS;
                let mut best = f32::INFINITY;
                let offset = sub(origin, actor.position);
                let a = ray[0] * ray[0] + ray[2] * ray[2];
                let b = offset[0] * ray[0] + offset[2] * ray[2];
                let c = offset[0] * offset[0] + offset[2] * offset[2] - RADIUS * RADIUS;
                if a > 1e-8 && b * b - a * c >= 0. {
                    let t = (-b - (b * b - a * c).sqrt()) / a;
                    let y = origin[1] + ray[1] * t;
                    if t > 0.01 && y >= low && y <= high {
                        best = t;
                    }
                }
                for y in [low, high] {
                    let offset = sub(origin, [actor.position[0], y, actor.position[2]]);
                    let b = dot(offset, ray);
                    let c = dot(offset, offset) - RADIUS * RADIUS;
                    let d = b * b - c;
                    if d >= 0. {
                        let t = -b - d.sqrt();
                        if t > 0.01 {
                            best = best.min(t);
                        }
                    }
                }
                if best > range {
                    return None;
                }
                let point = add(origin, mul(ray, best));
                let axis = [
                    actor.position[0],
                    point[1].clamp(low, high),
                    actor.position[2],
                ];
                let normal = sub(point, axis);
                Some(Hit {
                    id: actor.id,
                    point,
                    normal: mul(normal, 1. / length(normal).max(1e-6)),
                    distance: best,
                })
            })
            .min_by(|a, b| a.distance.total_cmp(&b.distance).then(a.id.cmp(&b.id)))
    }
    pub fn damage(&mut self, impact: &Impact, clear: impl Fn(V3, V3) -> bool) {
        let from = add(impact.point, mul(impact.normal, 0.035));
        for actor in &mut self.actors {
            if !actor.alive {
                continue;
            }
            let closest = actor.closest(impact.point);
            let distance = length(sub(closest, impact.point));
            if distance >= impact.radius
                || (impact.damage as f32 * (1. - distance / impact.radius)).round() < 1.
                || !clear(from, closest)
            {
                continue;
            }
            actor.alive = false;
            actor.death_seconds = 0.;
            actor.velocity_y = 0.;
        }
    }
    pub fn reset(&mut self) {
        for actor in &mut self.actors {
            *actor = Actor::new(actor.spec.clone());
        }
    }
    pub fn snapshot(&self) -> serde_json::Value {
        let alive = self.actors.iter().filter(|actor| actor.alive).count();
        serde_json::json!({"total":self.actors.len(),"alive":alive,
            "killed":self.actors.len()-alive,"actors":self.actors})
    }
}
