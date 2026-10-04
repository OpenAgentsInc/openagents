//! The player in Everglade: the ritual chamber's outfitted character, from
//! the pinned pack's skinned character, instead of the Grid's boxy avatar.
//!
//! The character's skeleton and clips play through Verse Engine's
//! local-space blending (`verse_engine::animation::Playback`). Each tick
//! picks a clip from the movement state ([`Motion::of`]), advances it (a
//! gait by the distance travelled, so the feet keep pace with the ground),
//! and skins every vertex on the CPU into world space. The renderer draws
//! the result as a textured figure ([`crate::pbr::textured::Figure`]): no
//! storage buffers or GPU skinning, so OpenGL ES 3.0 and WebGL2 draw it as
//! Metal, Vulkan, and WebGPU do.

use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use verse_engine::animation::Playback;
use verse_engine::assets::{Bone, BoneKeys, Clip, Model, RestPose, Skin};

use super::scene::{Copied, copy_material};
use crate::controller::PlayerController;
use crate::pbr::textured::{Figure, Primitive, TexturedMesh, TexturedScene, TexturedVertex};
use crate::zones::everglade_pack::{Character, ZonePack};

/// What the player is doing, and so which clip plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Idle,
    Walk,
    Run,
    Jump,
}

impl Motion {
    /// Every motion, in clip order.
    pub const ALL: [Self; 4] = [Self::Idle, Self::Walk, Self::Run, Self::Jump];
    /// Below this horizontal speed the character stands, m/s.
    pub const MOVING: f32 = 0.1;
    /// From this horizontal speed the character runs rather than walks, m/s:
    /// between backpedaling (4.1 m/s) and running (6.4 m/s).
    pub const RUN_FROM: f32 = 5.0;

    /// The motion for a character moving at `speed` m/s, off the ground or
    /// not.
    #[must_use]
    pub fn of(speed: f32, airborne: bool) -> Self {
        if airborne {
            Self::Jump
        } else if !speed.is_finite() || speed < Self::MOVING {
            Self::Idle
        } else if speed < Self::RUN_FROM {
            Self::Walk
        } else {
            Self::Run
        }
    }

    /// The pack clip it plays.
    #[must_use]
    pub fn clip(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Walk => "walk",
            Self::Run => "run",
            Self::Jump => "jump",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// One vertex in the bind pose, ready to skin.
#[derive(Clone, Copy)]
struct Bound {
    position: Vec3,
    normal: Vec3,
    joints: [u8; 4],
    /// Normalized to sum to one.
    weights: [f32; 4],
}

/// One clip's loop: its engine ID, length, and distance per loop.
#[derive(Clone, Copy)]
struct Loop {
    id: u16,
    duration: f32,
    distance: f32,
}

/// The posed character and its playback state.
pub(crate) struct Player {
    model: Model,
    loops: [Loop; 4],
    scene: Arc<TexturedScene>,
    template: Vec<TexturedVertex>,
    bound: Vec<Bound>,
    playback: Playback,
    motion: Motion,
    time: f32,
    clock: f32,
    vertices: Arc<Vec<TexturedVertex>>,
}

/// The engine ID of the clip at `index`, clear of the IDs the engine
/// treats specially (0 is the fallback and 1 holds its last frame).
fn clip_id(index: usize) -> u16 {
    10 + index as u16
}

impl Player {
    /// The pack's player, posed idle at `at`, or `None` when the pack
    /// carries no character.
    ///
    /// # Errors
    ///
    /// Returns a message when the character lacks a clip a motion plays.
    pub fn new(pack: &ZonePack, at: &PlayerController) -> Result<Option<Self>, String> {
        let Some(character) = &pack.character else {
            return Ok(None);
        };
        let mut player = Self::build(pack, character)?;
        player.advance(at, 0.0);
        Ok(Some(player))
    }

    fn build(pack: &ZonePack, character: &Character) -> Result<Self, String> {
        let mut loops = [Loop {
            id: 0,
            duration: 1.0,
            distance: 0.0,
        }; 4];
        let mut clips = Vec::new();
        for motion in Motion::ALL {
            let clip = character
                .clip(motion.clip())
                .ok_or_else(|| format!("The Everglade player has no {} clip", motion.clip()))?;
            let id = clip_id(motion.index());
            loops[motion.index()] = Loop {
                id,
                duration: clip.duration,
                distance: clip.distance,
            };
            clips.push(Clip {
                id,
                duration: clip.duration,
                bones: clip
                    .tracks
                    .iter()
                    .map(|t| BoneKeys {
                        bone: usize::from(t.joint),
                        translation: t.translation.clone(),
                        rotation: t.rotation.clone(),
                        scale: t.scale.clone(),
                    })
                    .collect(),
            });
        }
        let model = Model {
            graph: None,
            markers: Vec::new(),
            states: Default::default(),
            skin: Some(Skin {
                names: (0..character.joints.len()).map(|i| i.to_string()).collect(),
                rest: character
                    .joints
                    .iter()
                    .map(|j| RestPose {
                        translation: j.translation,
                        rotation: j.rotation,
                        scale: j.scale,
                    })
                    .collect(),
                inverse_bind: character.joints.iter().map(|j| j.inverse_bind).collect(),
                basis: Mat4::IDENTITY.to_cols_array(),
            }),
            source: character.name.clone(),
            source_sha256: String::new(),
            surfaces: Vec::new(),
            bones: character
                .joints
                .iter()
                .map(|j| Bone {
                    parent: j.parent,
                    pivot: [0.0; 3],
                })
                .collect(),
            clips,
            height: 0.0,
            attachments: Vec::new(),
        };
        let mut scene = TexturedScene::default();
        let mut copied = Copied::default();
        let mut mesh = TexturedMesh::default();
        let mut template = Vec::new();
        let mut bound = Vec::new();
        for primitive in &character.primitives {
            let material = copy_material(pack, primitive.material, &mut scene, &mut copied)?;
            let vertices: Vec<TexturedVertex> = primitive
                .vertices
                .iter()
                .map(|v| TexturedVertex {
                    pos: v.vertex.position,
                    normal: v.vertex.normal,
                    uv: v.vertex.uv,
                    color: v.vertex.color,
                })
                .collect();
            template.extend_from_slice(&vertices);
            bound.extend(primitive.vertices.iter().map(|v| {
                let total = v
                    .weights
                    .iter()
                    .map(|&w| f32::from(w))
                    .sum::<f32>()
                    .max(1.0);
                Bound {
                    position: v.vertex.position.into(),
                    normal: v.vertex.normal.into(),
                    joints: v.joints,
                    weights: v.weights.map(|w| f32::from(w) / total),
                }
            }));
            mesh.primitives.push(Primitive {
                vertices,
                indices: primitive.indices.clone(),
                material,
            });
        }
        scene.add_mesh(mesh);
        scene.validate()?;
        Ok(Self {
            model,
            loops,
            scene: Arc::new(scene),
            vertices: Arc::new(template.clone()),
            template,
            bound,
            playback: Playback::default(),
            motion: Motion::Idle,
            time: 0.0,
            clock: 0.0,
        })
    }

    /// What the character is doing.
    #[cfg(test)]
    #[must_use]
    pub fn motion(&self) -> Motion {
        self.motion
    }

    /// Advances the clip for `at`'s movement over `dt` seconds and poses the
    /// character where `at` stands, facing its yaw.
    pub fn advance(&mut self, at: &PlayerController, dt: f32) {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let motion = Motion::of(at.speed, at.airborne());
        if motion != self.motion {
            self.motion = motion;
            self.time = 0.0;
        }
        let clip = self.loops[motion.index()];
        let step = if clip.distance > 0.0 {
            at.speed.max(0.0) * dt / clip.distance * clip.duration
        } else {
            dt
        };
        self.time = (self.time + step) % clip.duration;
        if !self.time.is_finite() {
            self.time = 0.0;
        }
        self.clock += dt;
        let Ok(pose) =
            self.playback
                .update_selected(&self.model, clip.id.into(), self.time, self.clock)
        else {
            return;
        };
        let root = Mat4::from_rotation_translation(Quat::from_rotation_y(at.yaw), at.pos);
        if !root.is_finite() {
            return;
        }
        let joints: Vec<Mat4> = pose.iter().map(|m| root * *m).collect();
        let mut vertices = Vec::with_capacity(self.template.len());
        for (template, bound) in self.template.iter().zip(&self.bound) {
            let mut position = Vec3::ZERO;
            let mut normal = Vec3::ZERO;
            for k in 0..4 {
                let weight = bound.weights[k];
                if weight > 0.0 {
                    let joint = &joints[usize::from(bound.joints[k])];
                    position += joint.transform_point3(bound.position) * weight;
                    normal += joint.transform_vector3(bound.normal) * weight;
                }
            }
            vertices.push(TexturedVertex {
                pos: position.to_array(),
                normal: normal.normalize_or(Vec3::Y).to_array(),
                ..*template
            });
        }
        self.vertices = Arc::new(vertices);
    }

    /// The posed character for this frame's dynamic mesh.
    #[must_use]
    pub fn figure(&self) -> Figure {
        Figure {
            scene: self.scene.clone(),
            vertices: self.vertices.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controller::{BACKPEDAL_SPEED, RUN_SPEED, SPRINT_MULT};

    #[test]
    fn movement_state_picks_the_clip() {
        assert_eq!(Motion::of(0.0, false), Motion::Idle);
        assert_eq!(Motion::of(0.05, false), Motion::Idle);
        assert_eq!(Motion::of(f32::NAN, false), Motion::Idle);
        assert_eq!(Motion::of(1.5, false), Motion::Walk);
        assert_eq!(Motion::of(BACKPEDAL_SPEED, false), Motion::Walk);
        assert_eq!(Motion::of(RUN_SPEED, false), Motion::Run);
        assert_eq!(Motion::of(RUN_SPEED * SPRINT_MULT, false), Motion::Run);
        for speed in [0.0, 2.0, RUN_SPEED] {
            assert_eq!(Motion::of(speed, true), Motion::Jump);
        }
        let clips: Vec<_> = Motion::ALL.iter().map(|m| m.clip()).collect();
        assert_eq!(clips, ["idle", "walk", "run", "jump"]);
        assert!(Motion::ALL.iter().enumerate().all(|(i, m)| m.index() == i));
    }

    #[test]
    fn engine_clip_ids_avoid_the_fallback_and_held_clips() {
        assert!((0..Motion::ALL.len()).all(|i| clip_id(i) > 1));
    }
}
