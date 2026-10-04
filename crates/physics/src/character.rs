//! Grounded upright capsule movement over instance-scoped triangle queries.
use crate::queries::{Capsule, ColliderKey, Filter, Hit, Pose, Scene};
use glam::DVec3;
use serde::{Deserialize, Serialize};

const SKIN: f64 = 1e-5;

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

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Character {
    pub feet: DVec3,
    pub vertical_speed: f64,
    pub support: Option<ColliderKey>,
    support_pose: Option<Pose>,
}
impl Character {
    pub fn new(feet: DVec3) -> Self {
        Self {
            feet,
            vertical_speed: 0.,
            support: None,
            support_pose: None,
        }
    }
    pub fn validate(self) -> Result<(), String> {
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
        next.support = if next.vertical_speed <= 0. {
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
        let horizontal = horizontal_velocity * dt;
        let walked = slide(scene, filter, settings, start, horizontal, true)?;
        let requested = horizontal.length_squared();
        let progress = (walked - start).dot(horizontal);
        next.feet = walked;
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
            next.vertical_speed = (next.vertical_speed - settings.gravity * dt).max(-55.);
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
        if next.vertical_speed <= 0. {
            if let Some(hit) = ground(scene, filter, settings, next.feet, settings.ground_snap)? {
                next.feet -= DVec3::Y * (hit.distance - SKIN).max(0.);
                next.support = Some(hit.collider);
                next.vertical_speed = 0.;
            } else {
                next.support = None;
            }
        }
        next.support_pose = next.support.and_then(|key| scene.pose(key));
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
        self.feet = feet;
        self.vertical_speed = 0.;
        self.support = None;
        self.support_pose = None;
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
    for _ in 0..12 {
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
        feet += hit.normal * (hit.penetration + SKIN);
    }
    Err("Character spawn recovery did not converge".into())
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
