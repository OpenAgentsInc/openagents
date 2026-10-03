//! Deterministic cinematic cues, actor reactions, and directed bow projectiles.
//!
//! The director has no keyboard or chat-input path. It emits typed scene events
//! and presentation state from simulation time. Directed impacts do not claim
//! multiplayer authority or implement the complete WoW combat rules.
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Actor {
    pub id: u64,
    pub name: String,
    pub model: String,
    pub position: Vec3,
    pub yaw: f32,
    pub scale: f32,
    pub health: u32,
    pub nameplate: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Action {
    Yell { text: String, animation: u16 },
    CameraCut,
    Bow { target: u64, damage: u32 },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Cue {
    pub at: f32,
    pub actor: u64,
    #[serde(flatten)]
    pub action: Action,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scene {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub collision_profile: Option<String>,
    pub version: u32,
    pub duration: f32,
    pub origin_wow: [f32; 3],
    pub cut_at: f32,
    pub actors: Vec<Actor>,
    pub cues: Vec<Cue>,
}
#[derive(Clone, Debug)]
pub struct ActorFrame {
    pub actor: Actor,
    pub animation: u16,
    pub animation_time: f32,
    pub visible: bool,
    pub health: u32,
}
#[derive(Clone, Debug)]
pub struct Projectile {
    pub position: Vec3,
    pub direction: Vec3,
}
#[derive(Clone, Debug)]
pub struct Shot {
    pub fired: f32,
    pub impact: f32,
    pub start: Vec3,
    pub end: Vec3,
    pub target: u64,
    pub damage: u32,
}
#[derive(Clone, Debug)]
pub struct Frame {
    pub time: f32,
    pub actors: Vec<ActorFrame>,
    pub eye: Vec3,
    pub target: Vec3,
    pub fov: f32,
    pub yell: Option<Cue>,
    pub projectiles: Vec<Projectile>,
    pub shots: Vec<Shot>,
}
impl Scene {
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > 1024 * 1024 {
            return Err("Cinematic scene exceeds 1 MiB".into());
        }
        let s: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        s.validate()?;
        Ok(s)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self
            .collision_profile
            .as_ref()
            .is_some_and(|p| p.len() > 64)
            || self.version != 1
            || !self.duration.is_finite()
            || self.duration <= 0.0
            || self.duration > 600.0
            || !self.cut_at.is_finite()
            || self.cut_at < 0.0
            || self.cut_at > self.duration
            || self.actors.is_empty()
            || self.origin_wow.iter().any(|v| !v.is_finite())
            || self.actors.len() > 256
            || self.cues.len() > 1024
        {
            return Err("Invalid cinematic scene".into());
        }
        let mut ids = std::collections::BTreeSet::new();
        for a in &self.actors {
            if a.id == 0
                || !ids.insert(a.id)
                || !a.position.is_finite()
                || !a.yaw.is_finite()
                || !a.scale.is_finite()
                || a.scale <= 0.0
                || a.scale > 20.0
                || a.name.len() > 128
                || a.model.len() > 128
                || a.health == 0
            {
                return Err("Invalid cinematic actor".into());
            }
        }
        for c in &self.cues {
            if !c.at.is_finite() || c.at < 0.0 || c.at > self.duration || !ids.contains(&c.actor) {
                return Err("Invalid cinematic cue".into());
            }
            match &c.action {
                Action::Yell { text, .. } if text.is_empty() || text.len() > 512 => {
                    return Err("Invalid cinematic yell".into());
                }
                Action::Bow { target, damage }
                    if !ids.contains(target) || *target == c.actor || *damage == 0 =>
                {
                    return Err("Invalid directed shot".into());
                }
                _ => {}
            }
        }
        if self.cues.windows(2).any(|w| w[0].at > w[1].at) {
            return Err("Cinematic cues must be ordered".into());
        }
        Ok(())
    }
    fn actor_at(&self, a: &Actor, time: f32) -> Actor {
        let mut a = a.clone();
        if a.model == "adventurer" {
            a.position.z += ((time - self.cut_at) / 4.0).clamp(0.0, 1.0) * 5.0;
        } else if a.model == "cultist" && time > self.cut_at + 2.0 {
            let panic = ((time - self.cut_at - 2.0) / 5.0).clamp(0.0, 1.0);
            a.yaw = 0.0;
            a.position.x += a.position.x.signum() * panic * 1.2;
            a.position.z += panic * 1.0;
        }
        a
    }
    pub fn frame(&self, time: f32) -> Frame {
        let time = if time.is_finite() {
            time.clamp(0.0, self.duration)
        } else {
            0.0
        };
        let shots: Vec<_> = self
            .cues
            .iter()
            .filter_map(|c| {
                if let Action::Bow { target, damage } = c.action {
                    if c.at > time {
                        return None;
                    }
                    let actor = self.actors.iter().find(|a| a.id == c.actor)?;
                    let victim = self.actors.iter().find(|a| a.id == target)?;
                    let start = self.actor_at(actor, c.at).position + Vec3::new(0.0, 1.4, 0.0);
                    let end = self.actor_at(victim, c.at).position + Vec3::new(0.0, 1.1, 0.0);
                    Some(Shot {
                        fired: c.at,
                        impact: c.at + (end - start).length() / 24.0,
                        start,
                        end,
                        target,
                        damage,
                    })
                } else {
                    None
                }
            })
            .collect();
        let mut actors = Vec::new();
        for a in &self.actors {
            let actor = self.actor_at(a, time);
            let mut animation = 0;
            let mut animation_time = time + a.id as f32 * 0.19;
            if actor.model == "cultist" && time > self.cut_at + 2.0 && time < self.cut_at + 7.0 {
                animation = 5;
            }
            if actor.model == "adventurer" {
                animation = if time < self.cut_at + 4.0 { 5 } else { 109 };
            }
            if let Some(c) = self
                .cues
                .iter()
                .rev()
                .find(|c| c.actor == a.id && c.at <= time && time - c.at < 2.0)
            {
                match c.action {
                    Action::Yell {
                        animation: emote, ..
                    } => {
                        animation = emote;
                        animation_time = time - c.at;
                    }
                    Action::Bow { .. } => {
                        animation = 46;
                        animation_time = (time - c.at).min(0.65);
                    }
                    _ => {}
                }
            }
            let damage: u32 = shots
                .iter()
                .filter(|s| s.target == a.id && s.impact <= time)
                .map(|s| s.damage)
                .sum();
            actors.push(ActorFrame {
                visible: a.model != "adventurer" || time >= self.cut_at,
                actor,
                animation,
                animation_time,
                health: a.health.saturating_sub(damage),
            });
        }
        let player = actors
            .iter()
            .find(|a| a.actor.model == "adventurer")
            .map_or(Vec3::new(0.0, 0.0, -17.0), |a| a.actor.position);
        let (eye, target, fov) = if time < self.cut_at {
            (
                Vec3::new((time * 0.045).sin() * 0.7, 1.9, -27.4 + time * 0.07),
                Vec3::new(0.0, 3.0, 0.0),
                1.0,
            )
        } else {
            (
                player + Vec3::new(0.9, 2.5, -5.5),
                Vec3::new(0.0, 2.8, -1.0),
                1.0,
            )
        };
        let yell = self
            .cues
            .iter()
            .rev()
            .find(|c| matches!(c.action, Action::Yell { .. }) && c.at <= time && time - c.at < 5.0)
            .cloned();
        let projectiles = shots
            .iter()
            .filter(|s| time < s.impact)
            .map(|s| Projectile {
                position: s.start.lerp(
                    s.end,
                    ((time - s.fired) / (s.impact - s.fired)).clamp(0.0, 1.0),
                ),
                direction: (s.end - s.start).normalize(),
            })
            .collect();
        Frame {
            time,
            actors,
            eye,
            target,
            fov,
            yell,
            projectiles,
            shots,
        }
    }
}
impl Frame {
    pub fn view_projection(&self, aspect: f32) -> Mat4 {
        Mat4::perspective_rh(self.fov, aspect, 0.1, 500.0)
            * Mat4::look_at_rh(self.eye, self.target, Vec3::Y)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> Scene {
        Scene::from_json(include_bytes!("../../../assets/verse/wow/anthropic.json")).unwrap()
    }
    #[test]
    fn scene_cues_drive_dialogue_camera_projectiles_and_health() {
        let s = scene();
        let opening = s.frame(3.0);
        assert!(opening.yell.is_some());
        assert!(
            !opening
                .actors
                .iter()
                .find(|a| a.actor.id == 14)
                .unwrap()
                .visible
        );
        let shot = s.frame(46.1);
        assert_eq!(shot.projectiles.len(), 1);
        assert!(
            shot.actors
                .iter()
                .find(|a| a.actor.id == 14)
                .unwrap()
                .visible
        );
        let end = s.frame(71.0);
        assert_eq!(end.shots.len(), 5);
        assert_eq!(
            end.actors.iter().find(|a| a.actor.id == 2).unwrap().health,
            70
        );
        assert_eq!(
            end.actors
                .iter()
                .filter(|a| a.actor.nameplate && a.visible)
                .count(),
            13
        );
    }
    #[test]
    fn cues_cannot_target_missing_actors() {
        let mut s = scene();
        s.cues.push(Cue {
            at: 72.0,
            actor: 999,
            action: Action::CameraCut,
        });
        assert!(s.validate().is_err());
    }
}
