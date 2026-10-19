//! The demolition yard's rules and rigid bodies (`cad_world::site`, where
//! they moved so the website can run the CAD physics without the renderer,
//! #12183), and what the sledgehammer and Meteor Swarm act on.

pub use cad_world::site::*;
use glam::Vec3;

/// What the sledgehammer and Meteor Swarm act on: the yard's site, or a
/// town that raises its buildings into a site when something reaches them.
pub trait Target {
    /// Whether `point` is within `reach` of something a meteor bursts on.
    fn touches(&self, point: Vec3, reach: f32) -> bool;
    /// An explosion at `center`, as [`Site::explode`].
    fn explode(&mut self, center: Vec3, radius: f32, damage: i32, speed: f32) -> Vec<Blow>;
    /// An explosion on a wall facing `face`, as [`Site::explode_facing`].
    fn explode_facing(
        &mut self,
        center: Vec3,
        radius: f32,
        damage: i32,
        speed: f32,
        face: Vec3,
    ) -> Vec<Blow> {
        let _ = face;
        self.explode(center, radius, damage, speed)
    }
    /// A hammer blow along `path`, as [`Site::strike`].
    fn strike(&mut self, path: &[Vec3], push: Vec3, reach: f32) -> Option<Blow>;
    /// A die roll from 1 to `sides`.
    fn roll(&mut self, sides: u32) -> i32;
    /// Where, from 0 to 1, the segment from `from` to `to` first meets
    /// something standing or the ground, or `None` when it stays clear:
    /// what a targeting ray sees and what a meteor's path must miss.
    fn ray(&self, from: Vec3, to: Vec3) -> Option<f32>;
}

impl Target for Site {
    fn touches(&self, point: Vec3, reach: f32) -> bool {
        Site::touches(self, point, reach)
    }

    fn explode(&mut self, center: Vec3, radius: f32, damage: i32, speed: f32) -> Vec<Blow> {
        Site::explode(self, center, radius, damage, speed)
    }

    fn explode_facing(
        &mut self,
        center: Vec3,
        radius: f32,
        damage: i32,
        speed: f32,
        face: Vec3,
    ) -> Vec<Blow> {
        Site::explode_facing(self, center, radius, damage, speed, face)
    }

    fn strike(&mut self, path: &[Vec3], push: Vec3, reach: f32) -> Option<Blow> {
        Site::strike(self, path, push, reach)
    }

    fn roll(&mut self, sides: u32) -> i32 {
        Site::roll(self, sides)
    }

    fn ray(&self, from: Vec3, to: Vec3) -> Option<f32> {
        let ground =
            verse_world::social::sight::ground_hit(from, to, 0.0, &crate::zones::everglade::height);
        match (Site::ray(self, from, to), ground) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }
}
