//! Read-only combat values shared by local and remote rendering adapters.
use crate::{play::Game, rules::Projectile, utilities::Area};
use glam::Vec3;

#[derive(Clone, Debug)]
pub struct Player {
    pub position: Vec3,
    pub shield: i32,
    pub shield_until: f32,
    pub light: Option<Vec3>,
    pub areas: Vec<Area>,
}
#[derive(Clone, Debug)]
pub struct Hostile {
    pub origin: Vec3,
    pub target: Vec3,
    pub position: Option<Vec3>,
    pub started: f32,
    pub release: f32,
    pub radius: f32,
    pub boss: bool,
}
/// Contains no command, damage execution, or authority handle.
#[derive(Clone, Debug)]
pub struct Combat {
    pub time: f32,
    pub projectiles: Vec<Projectile>,
    pub players: Vec<Player>,
    pub hostile: Vec<Hostile>,
    pub impacts: Vec<(Vec3, f32, u8)>,
}
impl Combat {
    pub fn extract(game: &Game) -> Self {
        Self {
            time: game.time,
            projectiles: game.snapshot().projectiles,
            players: game
                .controlled_effects()
                .map(|(_, position, c)| Player {
                    position,
                    shield: c.shield,
                    shield_until: c.shield_until,
                    light: c.light,
                    areas: c.areas.clone(),
                })
                .collect(),
            hostile: game
                .encounter
                .iter()
                .flat_map(|e| &e.casts)
                .map(|c| Hostile {
                    origin: c.origin,
                    target: c.target,
                    position: c.position,
                    started: c.started,
                    release: c.release,
                    radius: c.radius,
                    boss: c.boss,
                })
                .collect(),
            impacts: game.impacts.clone(),
        }
    }
}
