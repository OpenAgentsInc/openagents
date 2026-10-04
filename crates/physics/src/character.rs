//! Grounded upright capsule movement over instance-scoped triangle queries.
use crate::queries::{Capsule, ColliderKey, Filter, Hit, Pose, Scene};
use glam::DVec3;
use serde::{Deserialize, Serialize};

const SKIN: f64 = 1e-5;
const RECOVERY_CORRECTIONS: usize = 64;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Settings {
    pub radius: f64,
    pub height: f64,
    pub step_height: f64,
    pub ground_snap: f64,
    pub slope_cos: f64,
    pub gravity: f64,
    pub jump_speed: f64,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            radius: 0.35,
            height: 1.8,
            step_height: 0.4,
            ground_snap: 0.15,
            slope_cos: std::f64::consts::FRAC_1_SQRT_2,
            gravity: 19.62,
            jump_speed: 7.,
        }
    }
}
impl Settings {
    pub fn validate(self) -> Result<(), String> {
        if ![
            self.radius,
            self.height,
            self.step_height,
            self.ground_snap,
            self.slope_cos,
            self.gravity,
            self.jump_speed,
        ]
        .iter()
        .all(|v| v.is_finite())
            || self.radius <= 0.
            || self.height < self.radius * 2.
            || self.height > 100.
            || !(0. ..=self.height).contains(&self.step_height)
            || !(0. ..=self.height).contains(&self.ground_snap)
            || !(0.01..=1.).contains(&self.slope_cos)
            || !(0. ..=1000.).contains(&self.gravity)
            || !(0. ..=100.).contains(&self.jump_speed)
        {
            return Err("Invalid character movement settings".into());
        }
        Ok(())
    }
    pub fn capsule(self, feet: DVec3) -> Capsule {
        Capsule {
            a: feet + DVec3::Y * self.radius,
            b: feet + DVec3::Y * (self.height - self.radius),
            radius: self.radius,
        }
    }
}

/// Deceleration of external motion while a character stands on the ground,
/// m/s². A shove of speed `v` slides `v² / (2 a)` on flat ground.
pub const FRICTION_DECELERATION: f64 = 16.0;
/// Default terminal descent speed, m/s.
pub const TERMINAL_SPEED: f64 = 55.;

/// A per-character change to gravity: `scale` multiplies the movement
/// setting's gravity (a negative scale pulls upward) and `terminal` bounds the
/// speed it can reach.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct GravityOverride {
    pub scale: f64,
    pub terminal: f64,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Character {
    pub feet: DVec3,
    pub vertical_speed: f64,
    pub support: Option<ColliderKey>,
    support_pose: Option<Pose>,
    /// Horizontal velocity from outside the character's own walking, m/s:
    /// a shove, a gust, or a collision. It decays on the ground through
    /// [`FRICTION_DECELERATION`], carries on unchanged while airborne, and
    /// is removed against whatever stops the sweep.
    #[serde(default)]
    pub external: DVec3,
    #[serde(default)]
    pub gravity: Option<GravityOverride>,
    /// Highest feet height of the current airborne arc, m.
    #[serde(default)]
    pub peak: Option<f64>,
    /// Height fallen, from the arc's peak, when the last step landed, m.
    #[serde(default)]
    pub landed: Option<f64>,
}
impl Character {
    pub fn new(feet: DVec3) -> Self {
        Self {
            feet,
            vertical_speed: 0.,
            support: None,
            support_pose: None,
            external: DVec3::ZERO,
            gravity: None,
            peak: None,
            landed: None,
        }
    }
    /// Initial speed that slides exactly `distance` on flat ground.
    pub fn push_speed(distance: f64) -> f64 {
        (2. * FRICTION_DECELERATION * distance.max(0.)).sqrt()
    }
    /// Adds a velocity change: horizontal to the external motion, vertical to
    /// the vertical speed. An upward change lifts the character off its support.
    pub fn add_velocity(&mut self, change: DVec3) {
        self.external += DVec3::new(change.x, 0., change.z);
        self.external = self.external.clamp_length_max(100.);
        if change.y != 0. {
            self.vertical_speed = (self.vertical_speed + change.y).clamp(-55., 100.);
            if self.vertical_speed > 0. {
                self.support = None;
                self.support_pose = None;
            }
        }
    }
    /// Off the ground: jumping, falling, or knocked into the air.
    pub fn airborne(&self) -> bool {
        self.support.is_none()
    }
    /// Moving under external motion, on the ground or in the air.
    pub fn knocked(&self) -> bool {
        self.external.length_squared() > 1e-6
    }
    fn gravity_terms(&self, settings: Settings) -> (f64, f64) {
        match self.gravity {
            Some(g) => (settings.gravity * g.scale, g.terminal),
            None => (settings.gravity, TERMINAL_SPEED),
        }
    }
    pub fn validate(self) -> Result<(), String> {
        if !self.external.is_finite()
            || self.external.y != 0.
            || self.external.length() > 100.
            || self.gravity.is_some_and(|g| {
                !g.scale.is_finite()
                    || !g.terminal.is_finite()
                    || g.scale.abs() > 10.
                    || !(0. ..=55.).contains(&g.terminal)
            })
            || self.peak.is_some_and(|p| !p.is_finite())
            || self.landed.is_some_and(|p| !p.is_finite() || p < 0.)
        {
            return Err("Invalid character external motion".into());
        }
        if !self.feet.is_finite()
            || self.feet.abs().max_element() > 1_000_000.
            || !self.vertical_speed.is_finite()
            || !(-55. ..=100.).contains(&self.vertical_speed)
            || self.support.is_some() != self.support_pose.is_some()
            || self.support_pose.is_some_and(|pose| {
                !pose.position.is_finite()
                    || !pose.rotation.is_finite()
                    || (pose.rotation.length_squared() - 1.).abs() > 1e-8
            })
        {
            return Err("Invalid character movement state".into());
        }
        Ok(())
    }
    /// Advances one bounded physics step. The caller owns the fixed-step clock.
    pub fn step(
        &mut self,
        scene: &Scene,
        filter: Filter,
        settings: Settings,
        horizontal_velocity: DVec3,
        jump: bool,
        dt: f64,
    ) -> Result<(), String> {
        settings.validate()?;
        self.validate()?;
        if self
            .support
            .is_some_and(|key| key.life.instance != filter.instance)
        {
            return Err("Character support belongs to another instance".into());
        }
        if !dt.is_finite()
            || !(0. ..=1. / 30.).contains(&dt)
            || !self.feet.is_finite()
            || !self.vertical_speed.is_finite()
            || !horizontal_velocity.is_finite()
            || horizontal_velocity.y.abs() > 1e-8
            || horizontal_velocity.length() > 100.
        {
            return Err("Invalid character movement step".into());
        }
        if dt == 0. {
            return Ok(());
        }
        // Commit only after all queries succeed.
        let mut next = *self;
        let start_height = next.feet.y;
        let (gravity, terminal) = next.gravity_terms(settings);
        if let (Some(key), Some(old)) = (next.support, next.support_pose) {
            if let Some(pose) = scene.pose(key) {
                let carried = pose.point(old.inverse_point(next.feet)) - next.feet;
                let mut carry_filter = filter;
                carry_filter.exclude = Some(key);
                next.feet = slide(scene, carry_filter, settings, next.feet, carried, false)?;
            }
        }
        next.feet = recover(scene, filter, settings, next.feet)?;
        let initial_ground = ground(scene, filter, settings, next.feet, settings.ground_snap)?;
        next.support = if next.vertical_speed <= 0. && gravity > 0. {
            initial_ground.map(|h| h.collider)
        } else {
            None
        };
        if jump && next.support.is_some() {
            next.vertical_speed = settings.jump_speed;
            next.support = None;
        } else if next.support.is_some() {
            next.vertical_speed = 0.;
        }
        let start = next.feet;
        // External motion decays at the friction deceleration on the ground
        // (integrated exactly, so a shove of `push_speed(d)` slides `d`) and
        // carries unchanged in the air.
        let speed = next.external.length();
        let (shove, external) = if speed == 0. {
            (DVec3::ZERO, DVec3::ZERO)
        } else if next.support.is_some() {
            let loss = FRICTION_DECELERATION * dt;
            let direction = next.external / speed;
            if speed > loss {
                (
                    direction * (speed - loss * 0.5) * dt,
                    direction * (speed - loss),
                )
            } else {
                (
                    direction * speed * speed / (2. * FRICTION_DECELERATION),
                    DVec3::ZERO,
                )
            }
        } else {
            (next.external * dt, next.external)
        };
        next.external = external;
        let horizontal = horizontal_velocity * dt + shove;
        let walked = slide(scene, filter, settings, start, horizontal, true)?;
        let requested = horizontal.length_squared();
        let progress = (walked - start).dot(horizontal);
        next.feet = walked;
        if shove != DVec3::ZERO && progress < requested * 0.99 {
            // Whatever stopped the sweep takes the external motion along the
            // blocked direction; the rest slides along the obstacle.
            let moved = walked - start;
            let moved = DVec3::new(moved.x, 0., moved.z);
            next.external = if moved.length_squared() < 1e-12 {
                DVec3::ZERO
            } else {
                let along = moved.normalize();
                along * next.external.dot(along).max(0.)
            };
        }
        if next.support.is_some()
            && requested > 1e-12
            && progress < requested * 0.99
            && settings.step_height > 0.
        {
            // A step is admitted only if the complete capsule clears the rise,
            // travels farther forward, and lands on a walkable surface.
            let rise = DVec3::Y * (settings.step_height + SKIN);
            if first(scene, filter, settings.capsule(start), rise)?.is_none() {
                let raised = start + rise;
                let across = slide(scene, filter, settings, raised, horizontal, true)?;
                if let Some(hit) = ground(
                    scene,
                    filter,
                    settings,
                    across,
                    settings.step_height + settings.ground_snap + SKIN,
                )? {
                    let landed = across - DVec3::Y * (hit.distance - SKIN).max(0.);
                    if (landed - start).dot(horizontal) > progress + SKIN
                        && landed.y - start.y <= settings.step_height + SKIN * 2.
                    {
                        next.feet = landed;
                        next.support = Some(hit.collider);
                    }
                }
            }
        }
        if next.support.is_none() {
            next.vertical_speed = if gravity >= 0. {
                (next.vertical_speed - gravity * dt).max(-terminal)
            } else {
                (next.vertical_speed - gravity * dt).min(terminal)
            };
        }
        let vertical = DVec3::Y * next.vertical_speed * dt;
        if let Some(hit) = first(scene, filter, settings.capsule(next.feet), vertical)? {
            if next.vertical_speed < 0. && hit.surface_normal.y < settings.slope_cos {
                next.feet = slide(scene, filter, settings, next.feet, vertical, false)?;
            } else {
                next.feet += vertical * (hit.fraction - SKIN / vertical.length().max(SKIN)).max(0.);
                if next.vertical_speed < 0. {
                    next.support = Some(hit.collider);
                }
                next.vertical_speed = 0.;
            }
        } else {
            next.feet += vertical;
        }
        if next.vertical_speed <= 0. && gravity > 0. {
            if let Some(hit) = ground(scene, filter, settings, next.feet, settings.ground_snap)? {
                next.feet -= DVec3::Y * (hit.distance - SKIN).max(0.);
                next.support = Some(hit.collider);
                next.vertical_speed = 0.;
            } else {
                next.support = None;
            }
        }
        next.support_pose = next.support.and_then(|key| scene.pose(key));
        if next.support.is_none() {
            next.peak = Some(next.peak.unwrap_or(start_height).max(next.feet.y));
            next.landed = None;
        } else {
            next.landed = next.peak.take().map(|peak| (peak - next.feet.y).max(0.));
        }
        *self = next;
        Ok(())
    }
    /// Checks an endpoint without sweeping through intervening geometry.
    pub fn teleport(
        &mut self,
        scene: &Scene,
        filter: Filter,
        settings: Settings,
        feet: DVec3,
    ) -> Result<(), String> {
        settings.validate()?;
        if !feet.is_finite() {
            return Err("Invalid character teleport".into());
        }
        let overlaps = scene.overlap(settings.capsule(feet), filter)?;
        if overlaps.truncated || overlaps.hits.iter().any(|h| h.penetration > SKIN) {
            return Err("Character teleport endpoint is obstructed".into());
        }
        let gravity = self.gravity;
        *self = Self::new(feet);
        self.gravity = gravity;
        Ok(())
    }
}

fn first(
    scene: &Scene,
    mut filter: Filter,
    capsule: Capsule,
    delta: DVec3,
) -> Result<Option<Hit>, String> {
    if delta.length_squared() < 1e-20 {
        return Ok(None);
    }
    filter.limit = 1;
    Ok(scene.sweep(capsule, delta, filter)?.hits.first().copied())
}
fn ground(
    scene: &Scene,
    filter: Filter,
    settings: Settings,
    feet: DVec3,
    distance: f64,
) -> Result<Option<Hit>, String> {
    let hits = scene.sweep(settings.capsule(feet), -DVec3::Y * distance, filter)?;
    let nearest = hits
        .hits
        .first()
        .map(|h| h.distance)
        .unwrap_or(f64::INFINITY);
    Ok(hits
        .hits
        .into_iter()
        .find(|h| h.distance <= nearest + SKIN && h.surface_normal.y >= settings.slope_cos))
}
fn recover(
    scene: &Scene,
    filter: Filter,
    settings: Settings,
    mut feet: DVec3,
) -> Result<DVec3, String> {
    let start = feet;
    let mut last = None;
    let mut previous: Option<(Hit, DVec3)> = None;
    let mut contacts = std::collections::BTreeSet::new();
    for correction in 0..=RECOVERY_CORRECTIONS {
        let result = scene.overlap(settings.capsule(feet), filter)?;
        if result.truncated {
            return Err("Character recovery query budget exceeded".into());
        }
        let Some(hit) = result
            .hits
            .into_iter()
            .filter(|h| h.penetration > SKIN)
            .max_by(|a, b| a.penetration.total_cmp(&b.penetration))
        else {
            return Ok(feet);
        };
        contacts.insert(hit.collider);
        last = Some((hit.collider, hit.penetration, hit.normal));
        if correction == RECOVERY_CORRECTIONS {
            break;
        }
        let depth = hit.penetration + SKIN;
        let mut displacement = hit.normal * depth;
        if let Some((old, origin)) = previous {
            let dot = hit.normal.dot(old.normal);
            let determinant = 1. - dot * dot;
            let remaining = old.penetration + SKIN - old.normal.dot(feet - origin);
            if dot < -0.25 && determinant > 1e-8 && old.normal.dot(displacement) < remaining {
                // Preserve the preceding contact plane while resolving its opposing contact.
                let a = (depth - dot * remaining) / determinant;
                let b = (remaining - dot * depth) / determinant;
                if a >= 0. && b >= 0. {
                    let joint = hit.normal * a + old.normal * b;
                    if joint.is_finite() {
                        displacement = joint.clamp_length_max(depth.max(settings.radius));
                    }
                }
            }
        }
        previous = Some((hit, feet));
        feet += displacement;
    }
    let capsules = scene.snapshot(filter.instance).ok().map(|snapshot| {
        snapshot
            .colliders
            .into_iter()
            .filter(|shape| {
                contacts.contains(&shape.key)
                    && matches!(
                        shape.geometry,
                        crate::queries::GeometrySnapshot::Capsule { .. }
                    )
            })
            .collect::<Vec<_>>()
    });
    if let Some(shapes) = &capsules {
        let initial = scene.overlap(settings.capsule(start), filter)?;
        let embedded: std::collections::BTreeSet<_> = initial
            .hits
            .iter()
            .filter(|hit| {
                hit.penetration > SKIN && shapes.iter().any(|shape| shape.key == hit.collider)
            })
            .map(|hit| hit.collider)
            .collect();
        if !initial.truncated && shapes.len() >= 3 && !embedded.is_empty() {
            // Enclosing capsule crowds can have no feasible local contact plane.
            // Search a bounded horizontal exit, crossing only the capsules that
            // already embed this character. Walls and new obstacles still block it.
            for ring in 1..=8 {
                let distance = settings.radius * ring as f64 * 0.5;
                for direction in 0..32 {
                    let angle = std::f64::consts::TAU * direction as f64 / 32.;
                    let delta = DVec3::new(angle.cos(), 0., angle.sin()) * distance;
                    let candidate = start + delta;
                    let endpoint = scene.overlap(settings.capsule(candidate), filter)?;
                    if endpoint.truncated || endpoint.hits.iter().any(|hit| hit.penetration > SKIN)
                    {
                        continue;
                    }
                    let path = scene.sweep(settings.capsule(start), delta, filter)?;
                    if !path.truncated
                        && path.hits.iter().all(|hit| {
                            embedded.contains(&hit.collider)
                                || (hit.penetration <= SKIN && delta.dot(hit.normal) >= -SKIN)
                        })
                    {
                        return Ok(candidate);
                    }
                }
            }
        }
    }
    Err(format!(
        "Character spawn recovery did not converge: actor {:?}, start {start:?}, end {feet:?}, last contact {last:?}, contacted capsules {capsules:?}",
        filter.ignore
    ))
}
pub fn slide(
    scene: &Scene,
    filter: Filter,
    settings: Settings,
    mut feet: DVec3,
    mut delta: DVec3,
    horizontal: bool,
) -> Result<DVec3, String> {
    settings.validate()?;
    if !feet.is_finite() || !delta.is_finite() || delta.length() > 1_000_000. {
        return Err("Invalid character displacement".into());
    }
    for _ in 0..8 {
        let Some(hit) = first(scene, filter, settings.capsule(feet), delta)? else {
            return Ok(feet + delta);
        };
        let fraction = (hit.fraction - SKIN / delta.length().max(SKIN)).max(0.);
        feet += delta * fraction;
        delta *= 1. - fraction;
        let mut normal = hit.normal;
        if horizontal && hit.surface_normal.y < settings.slope_cos {
            normal.y = 0.;
            normal = normal.normalize_or_zero();
        }
        delta -= normal * delta.dot(normal).min(0.);
        if delta.length_squared() < 1e-16 {
            return Ok(feet);
        }
    }
    // A crowded corner stops movement instead of consuming unbounded work.
    Ok(feet)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queries::{Life, Mesh, MeshCollider, Usage};
    fn box_in(scene: &mut Scene, id: u32, min: DVec3, max: DVec3) {
        scene
            .insert(MeshCollider {
                key: ColliderKey {
                    life: Life {
                        instance: 1,
                        entity: 0,
                        generation: 0,
                    },
                    shape: id,
                },
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::from_box(min, max).unwrap(),
            })
            .unwrap();
    }
    fn floor() -> Scene {
        let mut s = Scene::default();
        box_in(
            &mut s,
            0,
            DVec3::new(-20., -1., -20.),
            DVec3::new(20., 0., 20.),
        );
        s
    }
    fn advance(c: &mut Character, s: &Scene, velocity: DVec3, jump: bool, steps: usize) {
        for i in 0..steps {
            c.step(
                s,
                Filter::blocking(1),
                Settings::default(),
                velocity,
                jump && i == 0,
                1. / 120.,
            )
            .unwrap();
        }
    }
    #[test]
    fn ground_jump_land_and_ceiling() {
        let mut s = floor();
        let mut c = Character::new(DVec3::ZERO);
        advance(&mut c, &s, DVec3::ZERO, false, 1);
        assert!(c.support.is_some());
        advance(&mut c, &s, DVec3::ZERO, true, 20);
        assert!(c.feet.y > 0.7 && c.support.is_none());
        advance(&mut c, &s, DVec3::ZERO, false, 150);
        assert!(c.feet.y < 0.001 && c.support.is_some());
        box_in(&mut s, 1, DVec3::new(-2., 2., -2.), DVec3::new(2., 2.1, 2.));
        advance(&mut c, &s, DVec3::ZERO, true, 20);
        assert!(c.feet.y < 0.201);
        advance(&mut c, &s, DVec3::ZERO, false, 100);
        assert!(c.support.is_some());
    }
    #[test]
    fn jump_is_blocked_without_headroom() {
        let mut s = floor();
        box_in(&mut s, 1, DVec3::new(-2., 1.8, -2.), DVec3::new(2., 2., 2.));
        let mut c = Character::new(DVec3::ZERO);
        advance(&mut c, &s, DVec3::ZERO, false, 1);
        advance(&mut c, &s, DVec3::ZERO, true, 1);
        assert!(c.feet.y.abs() < 0.001 && c.vertical_speed == 0. && c.support.is_some());
    }
    #[test]
    fn thin_wall_and_slide() {
        let mut s = floor();
        box_in(
            &mut s,
            1,
            DVec3::new(2., 0., -10.),
            DVec3::new(2.001, 4., 10.),
        );
        let mut c = Character::new(DVec3::ZERO);
        advance(&mut c, &s, DVec3::new(6., 0., 3.), false, 120);
        assert!(c.feet.x < 1.651 && c.feet.x > 1.64);
        assert!(c.feet.z > 2.99);
    }
    #[test]
    fn admitted_stair_and_tall_step() {
        for (height, pass) in [(0.25, true), (0.8, false)] {
            let mut s = floor();
            box_in(
                &mut s,
                1,
                DVec3::new(1., 0., -2.),
                DVec3::new(4., height, 2.),
            );
            let mut c = Character::new(DVec3::ZERO);
            advance(&mut c, &s, DVec3::X * 2., false, 120);
            assert_eq!(c.feet.x > 1.5, pass, "{c:?}");
            if pass {
                assert!((c.feet.y - height).abs() < 0.001);
            }
        }
    }
    #[test]
    fn walkable_slope_climbs_and_steep_slope_blocks() {
        use crate::queries::Triangle;
        for (angle, climbs) in [(20_f64.to_radians(), true), (70_f64.to_radians(), false)] {
            let mut s = floor();
            let h = 10. * angle.tan();
            let a = DVec3::new(0., 0., -5.);
            let b = DVec3::new(10., h, -5.);
            let c = DVec3::new(10., h, 5.);
            let d = DVec3::new(0., 0., 5.);
            s.insert(MeshCollider {
                key: ColliderKey {
                    life: Life {
                        instance: 1,
                        entity: 1,
                        generation: 0,
                    },
                    shape: 0,
                },
                layers: 1,
                usage: Usage::Blocking,
                mesh: Mesh::compile(vec![Triangle([a, b, c]), Triangle([a, c, d])]).unwrap(),
            })
            .unwrap();
            let mut character = Character::new(DVec3::new(-1., 0., 0.));
            advance(&mut character, &s, DVec3::X * 2., false, 240);
            if climbs {
                assert!(
                    character.feet.x > 2. && character.feet.y > 0.7,
                    "{character:?}"
                );
            } else {
                assert!(character.feet.x < 0.1, "{character:?}");
            }
        }
    }
    #[test]
    fn moving_platform_carries_and_removed_support_releases() {
        let mut s = Scene::default();
        box_in(
            &mut s,
            0,
            DVec3::new(-3., -0.5, -3.),
            DVec3::new(3., 0., 3.),
        );
        let mut c = Character::new(DVec3::X);
        advance(&mut c, &s, DVec3::ZERO, false, 1);
        let key = c.support.unwrap();
        s.set_pose(
            key,
            Pose {
                position: DVec3::new(0.5, 0.1, 0.),
                rotation: glam::DQuat::from_rotation_y(0.1),
            },
        )
        .unwrap();
        advance(&mut c, &s, DVec3::ZERO, false, 1);
        assert!((c.feet.x - (0.5 + 0.1_f64.cos())).abs() < 0.001);
        assert!((c.feet.z + 0.1_f64.sin()).abs() < 0.001);
        assert!((c.feet.y - 0.1).abs() < 0.001);
        s.remove(key);
        advance(&mut c, &s, DVec3::ZERO, false, 1);
        assert!(c.support.is_none() && c.vertical_speed < 0.);
    }
    #[test]
    fn a_calibrated_shove_slides_its_distance_and_a_wall_stops_it() {
        let s = floor();
        let mut c = Character::new(DVec3::ZERO);
        advance(&mut c, &s, DVec3::ZERO, false, 1);
        c.add_velocity(DVec3::X * Character::push_speed(3.048));
        assert!(c.knocked() && !c.airborne());
        advance(&mut c, &s, DVec3::ZERO, false, 240);
        assert!((c.feet.x - 3.048).abs() < 1e-6, "{c:?}");
        assert!(!c.knocked());
        let mut s = floor();
        box_in(&mut s, 1, DVec3::new(2., 0., -5.), DVec3::new(2.5, 4., 5.));
        let mut c = Character::new(DVec3::ZERO);
        advance(&mut c, &s, DVec3::ZERO, false, 1);
        c.add_velocity(DVec3::X * Character::push_speed(3.048));
        advance(&mut c, &s, DVec3::ZERO, false, 240);
        assert!((c.feet.x - 1.65).abs() < 0.01, "{c:?}");
        assert!(!c.knocked());
    }
    #[test]
    fn airborne_motion_is_ballistic_and_landing_reports_the_fall() {
        let mut s = floor();
        box_in(&mut s, 1, DVec3::new(-3., 0., -3.), DVec3::new(1., 6., 3.));
        let mut c = Character::new(DVec3::new(0., 6., 0.));
        advance(&mut c, &s, DVec3::ZERO, false, 1);
        assert!(c.support.is_some());
        c.add_velocity(DVec3::new(6., 3., 0.));
        advance(&mut c, &s, DVec3::ZERO, false, 10);
        assert!(c.airborne() && (c.external.x - 6.).abs() < 1e-12);
        let mut landed = None;
        for _ in 0..240 {
            advance(&mut c, &s, DVec3::ZERO, false, 1);
            landed = landed.or(c.landed);
        }
        let rise = 3. * 3. / (2. * Settings::default().gravity);
        assert!((landed.unwrap() - (6. + rise)).abs() < 0.05, "{landed:?}");
        assert!(c.feet.y.abs() < 1e-3 && !c.knocked());
    }
    #[test]
    fn gravity_override_scales_descent_and_bounds_its_speed() {
        let s = floor();
        let mut c = Character::new(DVec3::Y * 20.);
        c.gravity = Some(GravityOverride {
            scale: 0.5,
            terminal: 2.,
        });
        advance(&mut c, &s, DVec3::ZERO, false, 120);
        assert!((c.vertical_speed + 2.).abs() < 1e-12);
        c.gravity = Some(GravityOverride {
            scale: 0.,
            terminal: 2.,
        });
        c.vertical_speed = 0.;
        let height = c.feet.y;
        advance(&mut c, &s, DVec3::ZERO, false, 120);
        assert_eq!(c.feet.y, height);
    }
    #[test]
    fn filled_box_spawn_and_teleport() {
        let mut s = floor();
        box_in(&mut s, 1, DVec3::new(-3., 0., -3.), DVec3::new(3., 4., 3.));
        let mut c = Character::new(DVec3::Y);
        assert!(
            c.teleport(&s, Filter::blocking(1), Settings::default(), DVec3::Y)
                .is_err()
        );
        advance(&mut c, &s, DVec3::ZERO, false, 1);
        assert!(
            c.feet.y >= 3.99 || c.feet.x.abs() >= 3.34 || c.feet.z.abs() >= 3.34,
            "{c:?}"
        );
    }
    #[test]
    fn unresolvable_recovery_preserves_character_state() {
        let mut scene = floor();
        box_in(
            &mut scene,
            1,
            DVec3::new(-2., 0., -10.),
            DVec3::new(-0.2, 4., 10.),
        );
        box_in(
            &mut scene,
            2,
            DVec3::new(0.2, 0., -10.),
            DVec3::new(2., 4., 10.),
        );
        let mut character = Character::new(DVec3::ZERO);
        let before = serde_json::to_value(character).unwrap();
        let error = character
            .step(
                &scene,
                Filter::blocking(1),
                Settings::default(),
                DVec3::ZERO,
                false,
                1. / 120.,
            )
            .unwrap_err();
        assert!(error.contains("Character spawn recovery did not converge"));
        assert_eq!(serde_json::to_value(character).unwrap(), before);
    }
    #[test]
    fn crowded_capsules_finish_recovery_before_movement() {
        use crate::queries::{CapsuleCollider, Pose};
        for (start, positions) in [
            (
                DVec3::new(1.6808813704470373, 0., 9.030238365165646),
                [
                    DVec3::new(1.2038122415542603, 1.1400007247924804, 8.91481876373291),
                    DVec3::new(2.1531760692596436, 0.9000099999997474, 9.679643630981445),
                ],
            ),
            (
                DVec3::new(3.6894473888929173, 0., -10.496250386948253),
                [
                    DVec3::new(2.9981443881988525, 1.0394337862730025, -10.172544479370117),
                    DVec3::new(4.293120384216309, 0.9, -10.599199295043945),
                ],
            ),
        ] {
            let mut scene = floor();
            for (index, position) in positions.into_iter().enumerate() {
                let key = ColliderKey {
                    life: Life {
                        instance: 1,
                        entity: 100 + index as u64,
                        generation: 0,
                    },
                    shape: 0,
                };
                scene
                    .insert_capsule(CapsuleCollider {
                        key,
                        layers: 2,
                        usage: Usage::Blocking,
                        capsule: Capsule {
                            a: -DVec3::Y * 0.55,
                            b: DVec3::Y * 0.55,
                            radius: 0.35,
                        },
                    })
                    .unwrap();
                scene
                    .set_pose(
                        key,
                        Pose {
                            position,
                            rotation: glam::DQuat::IDENTITY,
                        },
                    )
                    .unwrap();
            }
            let settings = Settings::default();
            let filter = Filter::blocking(1);
            let feet = recover(&scene, filter, settings, start).unwrap();
            let contacts = scene.overlap(settings.capsule(feet), filter).unwrap();
            assert!(!contacts.truncated);
            assert!(contacts.hits.iter().all(|hit| hit.penetration <= SKIN));
            assert!(feet.distance(start) < 0.3);
        }
    }
    #[test]
    fn enclosed_four_capsules_recover_without_crossing_walls() {
        use crate::queries::{CapsuleCollider, Pose};
        let start = DVec3::new(2.8624002933502197, 0., -8.559691429138184);
        let mut scene = floor();
        for (index, position) in [
            [2.2770602703094482, 0.9, -8.075368881225586],
            [2.9786927700042725, 0.9, -7.869404315948486],
            [2.171818733215332, 0.9, -8.767414093017578],
            [3.3549814224243164, 0.9, -8.545398712158203],
        ]
        .into_iter()
        .enumerate()
        {
            let key = ColliderKey {
                life: Life {
                    instance: 1,
                    entity: 100 + index as u64,
                    generation: 0,
                },
                shape: 0,
            };
            scene
                .insert_capsule(CapsuleCollider {
                    key,
                    layers: 2,
                    usage: Usage::Blocking,
                    capsule: Capsule {
                        a: -DVec3::Y * 0.55,
                        b: DVec3::Y * 0.55,
                        radius: 0.35,
                    },
                })
                .unwrap();
            scene
                .set_pose(
                    key,
                    Pose {
                        position: DVec3::from_array(position),
                        rotation: glam::DQuat::IDENTITY,
                    },
                )
                .unwrap();
        }
        let settings = Settings::default();
        let filter = Filter::blocking(1);
        let feet = recover(&scene, filter, settings, start).unwrap();
        assert!(feet.distance(start) <= settings.radius * 4.);
        assert_eq!(feet.y, start.y);
        assert!(
            scene
                .overlap(settings.capsule(feet), filter)
                .unwrap()
                .hits
                .iter()
                .all(|hit| hit.penetration <= SKIN)
        );
        // A surrounding wall enclosure must not turn crowd recovery into a teleport.
        for (id, min, max) in [
            (10, [2.3, 0., -9.1], [2.4, 4., -8.0]),
            (11, [3.3, 0., -9.1], [3.4, 4., -8.0]),
            (12, [2.3, 0., -9.1], [3.4, 4., -9.0]),
            (13, [2.3, 0., -8.1], [3.4, 4., -8.0]),
        ] {
            box_in(
                &mut scene,
                id,
                DVec3::from_array(min),
                DVec3::from_array(max),
            );
        }
        assert!(recover(&scene, filter, settings, start).is_err());
        let mut character = Character::new(start);
        let before = serde_json::to_value(character).unwrap();
        assert!(
            character
                .step(&scene, filter, settings, DVec3::ZERO, false, 1. / 120.)
                .is_err()
        );
        assert_eq!(serde_json::to_value(character).unwrap(), before);
    }
    #[test]
    fn endpoint_and_surface_recovery() {
        let mut s = floor();
        box_in(&mut s, 1, DVec3::new(1., 0., -2.), DVec3::new(1.1, 4., 2.));
        let mut c = Character::new(DVec3::new(0.8, 0., 0.));
        advance(&mut c, &s, DVec3::ZERO, false, 1);
        assert!(c.feet.x < 0.651);
        let old = c.feet;
        assert!(
            c.teleport(
                &s,
                Filter::blocking(1),
                Settings::default(),
                DVec3::new(1., 0., 0.)
            )
            .is_err()
        );
        assert_eq!(c.feet, old);
        c.teleport(
            &s,
            Filter::blocking(1),
            Settings::default(),
            DVec3::new(3., 0., 0.),
        )
        .unwrap();
        assert_eq!(c.feet.x, 3.);
    }
}
