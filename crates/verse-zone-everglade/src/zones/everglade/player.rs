//! The characters in Everglade: the player, as the ritual chamber's
//! outfitted character from the pinned pack's skinned character, instead of
//! the Grid's boxy avatar, and the Agent Studio's seats as the same
//! character in their own outfit colors.
//!
//! The character's skeleton and clips play through Verse Engine's
//! local-space blending (`verse_engine::animation::Playback`). Each tick
//! picks a clip for every character: the player's from its movement state
//! ([`Motion::of`]), and a seat's from its pace or, standing still, its
//! [`Posture`], whose clips `pose::authored` makes from the pack's idle
//! clip when the zone loads. A gait advances by the distance travelled, so
//! the feet keep pace with the ground. A seat's head then turns toward
//! what it looks at, and every vertex is skinned on the CPU into world
//! space. The renderer draws all of them as one textured figure
//! ([`crate::pbr::textured::Figure`]), a copy of the character per
//! person: no storage buffers or GPU skinning, so OpenGL ES 3.0 and WebGL2
//! draw it as Metal, Vulkan, and WebGPU do.

use std::ops::Range;
use std::sync::Arc;

use glam::{Mat4, Quat, Vec3};
use verse_engine::animation::Playback;
use verse_engine::assets::{Bone, BoneKeys, Clip, Model, RestPose, Skin};

use super::pose::{self, Skeleton};
use super::scene::{Copied, copy_material};
use super::studio::{Posture, SeatFigure};
use crate::controller::PlayerController;
use crate::pbr::textured::{
    Figure, Primitive, TexturedMesh, TexturedScene, TexturedVertex, UNBAKED,
};
use crate::zones::everglade_pack::{Character, Clip as PackClip, ZonePack};

/// What the player is doing, and so which clip plays.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Motion {
    Idle,
    Walk,
    Run,
    Jump,
    Backpedal,
    StrafeLeft,
    StrafeRight,
}

impl Motion {
    /// Every motion, in clip order.
    pub const ALL: [Self; 7] = [
        Self::Idle,
        Self::Walk,
        Self::Run,
        Self::Jump,
        Self::Backpedal,
        Self::StrafeLeft,
        Self::StrafeRight,
    ];
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

    /// The motion for the player as `at` moves: a sideways step strafes and
    /// a backward one backpedals; otherwise as [`Self::of`].
    #[must_use]
    pub fn of_player(at: &PlayerController) -> Self {
        match Self::of(at.speed, at.airborne()) {
            Self::Walk | Self::Run if at.ahead < 0.0 => Self::Backpedal,
            Self::Walk | Self::Run if at.ahead == 0.0 && at.side < 0.0 => Self::StrafeLeft,
            Self::Walk | Self::Run if at.ahead == 0.0 && at.side > 0.0 => Self::StrafeRight,
            motion => motion,
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
            Self::Backpedal => "backpedal",
            Self::StrafeLeft => "strafe_left",
            Self::StrafeRight => "strafe_right",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// What a character plays: a motion, or a seat's posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Play {
    Motion(Motion),
    Posture(Posture),
}

impl Play {
    /// What a seat drawn as `seat` plays: a gait while it moves, else its
    /// posture.
    #[must_use]
    pub fn of(seat: &SeatFigure) -> Self {
        match Motion::of(seat.speed, false) {
            Motion::Idle => Self::Posture(seat.posture),
            motion => Self::Motion(motion),
        }
    }
}

/// The farthest a head turns from the body's heading, radians.
const LOOK_YAW: f32 = 1.1;
/// The farthest a head tips up or down, radians.
const LOOK_PITCH: f32 = 0.5;
/// A target farther behind than this, radians, is not looked at.
const LOOK_BEHIND: f32 = 2.2;
/// How fast a head turns to a new target: the share of the way it covers
/// per second, as an exponential rate.
const LOOK_RATE: f32 = 8.0;

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

/// The head a seat turns: its joint, its bind-pose position, and every
/// joint it carries, itself included.
struct Head {
    joint: usize,
    bind: Vec3,
    carried: Vec<usize>,
}

/// The pack's character, ready to pose any number of times.
pub struct Rig {
    model: Model,
    motions: [Loop; Motion::ALL.len()],
    /// Each posture's clip, in [`Posture::ALL`] order; `None` plays idle.
    postures: [Option<Loop>; Posture::ALL.len()],
    /// The images and materials, without a mesh.
    base: TexturedScene,
    /// The character's primitives in the bind pose.
    primitives: Vec<Primitive>,
    template: Vec<TexturedVertex>,
    bound: Vec<Bound>,
    /// The vertices of the outfit, the primitive with the most triangles,
    /// which a seat's tint colors.
    outfit: Range<usize>,
    head: Option<Head>,
    /// The demolition yard's swing, when the pack has it.
    swing: Option<Loop>,
    hands: Option<Hands>,
    /// A form's every clip by its pack name, such as a dragon's `fly` or
    /// `breath`, for [`Beast::pose_clip`]; empty for the player's
    /// character.
    named: Vec<(String, Loop)>,
}

/// The pack's name for the demolition yard's two-handed swing, and the
/// engine ID it plays under.
const SWING: &str = "swing";
const SWING_ID: u16 = 60;
/// A form's attack clip, which plays where the player's swing would.
const ATTACK: &str = "attack";
/// The engine ID of a form's first clip by name; the rest follow, clear
/// of the motion, posture, and swing IDs (a pack's character has at most
/// eight clips).
const NAMED_ID: u16 = 70;
/// The sledgehammer: how far the handle runs past the lower hand to its
/// butt, and from that hand to the head's center, m.
pub const BUTT: f32 = 0.16;
pub const HEAD_AT: f32 = 0.86;
/// Samples taken of the swing when the zone loads.
const SWING_SAMPLES: usize = 64;
/// How long the hold takes to move between the carry and both hands on
/// the handle at a swing's start and end, s.
const TAKE_UP: f32 = 0.18;

/// How a character holds the sledgehammer: the lower hand's grip, the
/// handle's direction from it toward the head, and the head's striking
/// direction, at right angles to the handle.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hold {
    pub grip: Vec3,
    pub axis: Vec3,
    pub face: Vec3,
}

impl Hold {
    /// The head's center.
    #[must_use]
    pub fn head(&self) -> Vec3 {
        self.grip + self.axis * HEAD_AT
    }

    /// This hold placed by `m`.
    #[must_use]
    pub fn moved(&self, m: Mat4) -> Self {
        Self {
            grip: m.transform_point3(self.grip),
            axis: m.transform_vector3(self.axis).normalize_or(self.axis),
            face: m.transform_vector3(self.face).normalize_or(self.face),
        }
    }

    /// The hold `k` of the way from this one to `other`.
    fn toward(&self, other: &Self, k: f32) -> Self {
        let axis = self.axis.lerp(other.axis, k).normalize_or(other.axis);
        let face = self.face.lerp(other.face, k);
        Self {
            grip: self.grip.lerp(other.grip, k),
            axis,
            face: (face - axis * face.dot(axis)).normalize_or(other.face),
        }
    }
}

/// The swing as the character plays it, sampled when the zone loads, in
/// the character's model space: when the head lands, and the hold at each
/// sample.
#[derive(Clone, Debug, PartialEq)]
pub struct SwingTrack {
    pub duration: f32,
    /// When the head lands, s into the clip: where it reaches farthest
    /// forward.
    pub impact: f32,
    pub holds: Vec<Hold>,
}

impl SwingTrack {
    /// The sampled hold nearest `t` seconds into the swing.
    #[must_use]
    pub fn hold(&self, t: f32) -> Hold {
        let last = self.holds.len().saturating_sub(1);
        let i = ((t / self.duration).clamp(0.0, 1.0) * last as f32).round() as usize;
        self.holds[i.min(last)]
    }

    /// How fully both hands hold the handle `t` seconds into the swing:
    /// rising from the carry at the start and falling back at the end.
    #[must_use]
    pub fn grasp(&self, t: f32) -> f32 {
        let ease = |x: f32| {
            let x = x.clamp(0.0, 1.0);
            x * x * (3.0 - 2.0 * x)
        };
        ease(t / TAKE_UP).min(ease((self.duration - t) / TAKE_UP))
    }
}

/// The hands that hold the sledgehammer.
struct Hands {
    /// The right hand's joint and bind transform, then the left's.
    joints: [(usize, Mat4); 2],
    /// The handle's and the face's directions in the right hand's frame
    /// as it carries the hammer.
    carry: (Vec3, Vec3),
    /// Whether the right hand is the lower one on the handle in the swing.
    right_lower: bool,
    track: Option<SwingTrack>,
}

impl Hands {
    /// The hands of `skeleton`, the carry fitted to `model`'s `idle` pose,
    /// and the `swing` sampled when the pack has it.
    fn find(
        model: &Model,
        joints: &[crate::zones::everglade_pack::Joint],
        skeleton: &Skeleton,
        idle: Loop,
        swing: Option<Loop>,
    ) -> Option<Self> {
        let bind = |j: usize| {
            joints
                .get(j)
                .map(|joint| Mat4::from_cols_array(&joint.inverse_bind).inverse())
        };
        // The model faces +Z, so the character's right is -X: the second
        // arm.
        let [left, right] = skeleton.arms.map(|arm| arm.end);
        let hands = [(right, bind(right)?), (left, bind(left)?)];
        let pose = |id: u16, t: f32| {
            Playback::default()
                .update_selected(model, id.into(), t, 0.0)
                .ok()
        };
        let world = |skin: &[Mat4], k: usize| skin.get(hands[k].0).map(|m| *m * hands[k].1);
        let rest = world(&pose(idle.id, 0.0)?, 0)?;
        // Carried in the right hand, the handle points ahead and down and the
        // head's face looks ahead and up.
        let axis = Vec3::new(0.0, -0.55, 0.85).normalize();
        let face = axis.cross(Vec3::X).normalize();
        let inverse = rest.inverse();
        let carry = (
            inverse.transform_vector3(axis),
            inverse.transform_vector3(face),
        );
        let mut right_lower = true;
        let track = swing.and_then(|swing| {
            let samples: Vec<[Vec3; 2]> = (0..SWING_SAMPLES)
                .map(|i| {
                    let t = swing.duration * i as f32 / (SWING_SAMPLES - 1) as f32;
                    let skin = pose(swing.id, t)?;
                    Some(
                        [0, 1].map(|k| world(&skin, k).map_or(Vec3::ZERO, |m| m.w_axis.truncate())),
                    )
                })
                .collect::<Option<_>>()?;
            // The lower hand is the one that puts the head farthest ahead.
            let reach = |lower: usize| {
                samples
                    .iter()
                    .map(|p| {
                        let axis = (p[1 - lower] - p[lower]).normalize_or_zero();
                        (p[lower] + axis * HEAD_AT).z
                    })
                    .fold(f32::NEG_INFINITY, f32::max)
            };
            right_lower = reach(0) >= reach(1);
            let lower = usize::from(!right_lower);
            let mut holds: Vec<Hold> = samples
                .iter()
                .map(|p| {
                    let axis = (p[1 - lower] - p[lower]).normalize_or(Vec3::Y);
                    Hold {
                        grip: p[lower],
                        axis,
                        face: Vec3::ZERO,
                    }
                })
                .collect();
            // The face looks the way the head moves.
            let heads: Vec<Vec3> = holds.iter().map(Hold::head).collect();
            let mut previous = Vec3::Z;
            for (i, hold) in holds.iter_mut().enumerate() {
                let a = heads[i.saturating_sub(1)];
                let b = heads[(i + 1).min(heads.len() - 1)];
                let moving = b - a;
                let face = moving - hold.axis * moving.dot(hold.axis);
                hold.face = if face.length() > 1e-3 {
                    face.normalize()
                } else {
                    previous
                };
                previous = hold.face;
            }
            let impact = heads
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.z.total_cmp(&b.1.z))
                .map_or(0.0, |(i, _)| {
                    swing.duration * i as f32 / (SWING_SAMPLES - 1) as f32
                });
            Some(SwingTrack {
                duration: swing.duration,
                impact,
                holds,
            })
        });
        Some(Self {
            joints: hands,
            carry,
            right_lower,
            track,
        })
    }

    /// The hold under skin matrices `skin`, `swing` seconds into a swing
    /// or carried, in model space.
    fn hold(&self, skin: &[Mat4], swing: Option<f32>) -> Option<Hold> {
        let world = |k: usize| skin.get(self.joints[k].0).map(|m| *m * self.joints[k].1);
        let right = world(0)?;
        let carry = Hold {
            grip: right.w_axis.truncate(),
            axis: right.transform_vector3(self.carry.0).normalize_or(Vec3::Z),
            face: right.transform_vector3(self.carry.1).normalize_or(Vec3::Y),
        };
        let (Some(t), Some(track)) = (swing, &self.track) else {
            return Some(carry);
        };
        let left = world(1)?.w_axis.truncate();
        let [lower, upper] = if self.right_lower {
            [carry.grip, left]
        } else {
            [left, carry.grip]
        };
        let axis = (upper - lower).normalize_or(carry.axis);
        let sampled = track.hold(t);
        let both = Hold {
            grip: lower,
            axis,
            face: (sampled.face - axis * sampled.face.dot(axis)).normalize_or(carry.face),
        };
        Some(carry.toward(&both, track.grasp(t)))
    }
}

/// The engine ID of the motion clip at `index`, clear of the IDs the
/// engine treats specially (0 is the fallback and 1 holds its last frame).
fn clip_id(index: usize) -> u16 {
    10 + index as u16
}

/// The engine ID of the posture clip at `index` in [`Posture::ALL`].
fn posture_id(index: usize) -> u16 {
    30 + index as u16
}

/// The clip a form without `motion`'s own clip plays instead: a gait
/// walks, or flaps for a bird, and the rest stand idle.
fn form_clip(character: &Character, motion: Motion) -> Option<&PackClip> {
    character.clip(motion.clip()).or_else(|| match motion {
        Motion::Idle => character.clips.first(),
        // A bird beats its wings in the air; anything else holds its idle.
        Motion::Jump => character
            .clip("flap")
            .or_else(|| form_clip(character, Motion::Idle)),
        Motion::Walk => character
            .clip("flap")
            .or_else(|| form_clip(character, Motion::Idle)),
        _ => form_clip(character, Motion::Walk),
    })
}

impl Rig {
    /// The rig for the player's character, which needs a clip for every
    /// motion. A form ([`Beast`]) plays a stand-in for a missing one.
    fn build(pack: &ZonePack, character: &Character) -> Result<Self, String> {
        Self::build_with(pack, character, false)
    }

    fn build_with(pack: &ZonePack, character: &Character, form: bool) -> Result<Self, String> {
        let mut motions = [Loop {
            id: 0,
            duration: 1.0,
            distance: 0.0,
        }; Motion::ALL.len()];
        let mut clips = Vec::new();
        for motion in Motion::ALL {
            let clip = if form {
                form_clip(character, motion)
            } else {
                character.clip(motion.clip())
            }
            .ok_or_else(|| format!("{} has no {} clip", character.name, motion.clip()))?;
            let id = clip_id(motion.index());
            motions[motion.index()] = Loop {
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
        // The demolition yard's swing, which a pack before it lacks, or a
        // form's attack.
        let attack = character.clip(ATTACK).filter(|_| form);
        let swing = character.clip(SWING).or(attack).map(|clip| {
            clips.push(Clip {
                id: SWING_ID,
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
            Loop {
                id: SWING_ID,
                duration: clip.duration,
                distance: 0.0,
            }
        });
        // A form's every clip by name, which [`Beast::pose_clip`] plays.
        let mut named = Vec::new();
        if form {
            for (index, clip) in character.clips.iter().enumerate() {
                let id = NAMED_ID + index as u16;
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
                named.push((
                    clip.name.clone(),
                    Loop {
                        id,
                        duration: clip.duration,
                        distance: clip.distance,
                    },
                ));
            }
        }
        // The seats' postures, authored from idle on the skeleton's shape.
        // A skeleton that is not a humanoid's plays idle for each.
        let skeleton = Skeleton::find(&character.joints);
        let mut postures = [None; Posture::ALL.len()];
        if let (Some(skeleton), Some(idle)) = (&skeleton, character.clip("idle")) {
            for (index, posture) in Posture::ALL.into_iter().enumerate() {
                let id = posture_id(index);
                if let Some(clip) = pose::authored(skeleton, &character.joints, idle, posture, id) {
                    postures[index] = Some(Loop {
                        id,
                        duration: clip.duration,
                        distance: 0.0,
                    });
                    clips.push(clip);
                }
            }
        }
        let head = skeleton.map(|skeleton| {
            let joint = skeleton.head;
            let bind = Mat4::from_cols_array(&character.joints[joint].inverse_bind)
                .inverse()
                .w_axis
                .truncate();
            let mut carried = vec![joint];
            for (i, j) in character.joints.iter().enumerate().skip(joint + 1) {
                if usize::try_from(j.parent).is_ok_and(|p| carried.contains(&p)) {
                    carried.push(i);
                }
            }
            Head {
                joint,
                bind,
                carried,
            }
        });
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
        let hands = skeleton.as_ref().and_then(|skeleton| {
            Hands::find(
                &model,
                &character.joints,
                skeleton,
                motions[Motion::Idle.index()],
                swing,
            )
        });
        let mut base = TexturedScene::default();
        let mut copied = Copied::default();
        let mut primitives: Vec<Primitive> = Vec::new();
        let mut template: Vec<TexturedVertex> = Vec::new();
        let mut bound: Vec<Bound> = Vec::new();
        let mut outfit = 0..0;
        for primitive in &character.primitives {
            let material = copy_material(pack, primitive.material, &mut base, &mut copied)?;
            let vertices: Vec<TexturedVertex> = primitive
                .vertices
                .iter()
                .map(|v| TexturedVertex {
                    pos: v.vertex.position,
                    normal: v.vertex.normal,
                    uv: v.vertex.uv,
                    color: v.vertex.color,
                    light: UNBAKED,
                })
                .collect();
            let start = template.len();
            template.extend_from_slice(&vertices);
            if primitive.indices.len()
                > primitives
                    .iter()
                    .map(|p| p.indices.len())
                    .max()
                    .unwrap_or(0)
            {
                outfit = start..template.len();
            }
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
            primitives.push(Primitive {
                vertices,
                indices: primitive.indices.clone(),
                material,
            });
        }
        let rig = Self {
            model,
            motions,
            postures,
            base,
            primitives,
            template,
            bound,
            outfit,
            head,
            swing,
            hands,
            named,
        };
        rig.scene(1).validate()?;
        Ok(rig)
    }

    /// The loop `play` plays.
    fn clip(&self, play: Play) -> Loop {
        match play {
            Play::Motion(motion) => self.motions[motion.index()],
            Play::Posture(posture) => Posture::ALL
                .iter()
                .position(|p| *p == posture)
                .and_then(|i| self.postures[i])
                .unwrap_or(self.motions[Motion::Idle.index()]),
        }
    }

    /// Whether `posture` has an authored clip rather than playing idle.
    #[cfg(test)]
    fn authored(&self, posture: Posture) -> bool {
        Posture::ALL
            .iter()
            .position(|p| *p == posture)
            .is_some_and(|i| self.postures[i].is_some())
    }

    /// A scene of `copies` copies of the character, which a figure of that
    /// many characters draws.
    fn scene(&self, copies: usize) -> TexturedScene {
        let mut scene = self.base.clone();
        let mut mesh = TexturedMesh::default();
        for _ in 0..copies {
            mesh.primitives.extend(self.primitives.iter().cloned());
        }
        scene.add_mesh(mesh);
        scene
    }

    /// The posed head's position in model space under skin matrices
    /// `joints`.
    fn head_at(&self, joints: &[Mat4]) -> Option<Vec3> {
        let head = self.head.as_ref()?;
        joints
            .get(head.joint)
            .map(|m| m.transform_point3(head.bind))
    }

    /// Turns the head under `joints` by `yaw` about the model's up axis and
    /// `pitch` down, about the posed head.
    fn turn_head(&self, joints: &mut [Mat4], [yaw, pitch]: [f32; 2]) {
        let Some(head) = &self.head else {
            return;
        };
        if yaw.abs() < 1e-4 && pitch.abs() < 1e-4 {
            return;
        }
        let Some(at) = self.head_at(joints) else {
            return;
        };
        let turn = Mat4::from_translation(at)
            * Mat4::from_quat(Quat::from_rotation_y(yaw) * Quat::from_rotation_x(pitch))
            * Mat4::from_translation(-at);
        for &joint in &head.carried {
            if let Some(m) = joints.get_mut(joint) {
                *m = turn * *m;
            }
        }
    }

    /// Skins one copy of the character under `joints` placed by `root`,
    /// its outfit in `tint` when given, onto `out`. Without joints, or with
    /// a nonfinite root, it stands in its bind pose.
    fn skin(
        &self,
        joints: &[Mat4],
        root: Mat4,
        tint: Option<[u8; 4]>,
        out: &mut Vec<TexturedVertex>,
    ) {
        let root = if root.is_finite() {
            root
        } else {
            Mat4::IDENTITY
        };
        let joints: Vec<Mat4> = if joints.is_empty() {
            vec![root; self.model.bones.len().max(1)]
        } else {
            joints.iter().map(|m| root * *m).collect()
        };
        let start = out.len();
        for (template, bound) in self.template.iter().zip(&self.bound) {
            let mut position = Vec3::ZERO;
            let mut normal = Vec3::ZERO;
            for k in 0..4 {
                let weight = bound.weights[k];
                if weight > 0.0
                    && let Some(joint) = joints.get(usize::from(bound.joints[k]))
                {
                    position += joint.transform_point3(bound.position) * weight;
                    normal += joint.transform_vector3(bound.normal) * weight;
                }
            }
            out.push(TexturedVertex {
                pos: position.to_array(),
                normal: normal.normalize_or(Vec3::Y).to_array(),
                ..*template
            });
        }
        if let Some(tint) = tint {
            let outfit = start + self.outfit.start..start + self.outfit.end;
            for vertex in &mut out[outfit] {
                vertex.color = tint;
            }
        }
    }
}

/// One character's playback state.
pub struct Actor {
    playback: Playback,
    clip: u16,
    time: f32,
    clock: f32,
    /// The head's turn now: yaw and pitch, radians.
    look: [f32; 2],
}

impl Actor {
    fn new() -> Self {
        Self {
            playback: Playback::default(),
            clip: u16::MAX,
            time: 0.0,
            clock: 0.0,
            look: [0.0; 2],
        }
    }

    /// Advances `play`'s clip for a character moving at `speed` over `dt`
    /// seconds, and returns the pose's skin matrices in model space, or
    /// `None` when the clip cannot play.
    fn advance(&mut self, rig: &Rig, play: Play, speed: f32, dt: f32) -> Option<Vec<Mat4>> {
        let dt = if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        let clip = rig.clip(play);
        if clip.id != self.clip {
            self.clip = clip.id;
            self.time = 0.0;
        }
        let step = if clip.distance > 0.0 {
            speed.max(0.0) * dt / clip.distance * clip.duration
        } else {
            dt
        };
        self.time = (self.time + step) % clip.duration;
        if !self.time.is_finite() {
            self.time = 0.0;
        }
        self.clock += dt;
        self.playback
            .update_selected(&rig.model, clip.id.into(), self.time, self.clock)
            .ok()
    }

    /// Plays `clip` at `time` seconds in, after `dt` more seconds of the
    /// clock, so a change of clip still blends; returns the skin matrices.
    fn hold_at(&mut self, rig: &Rig, clip: Loop, time: f32, dt: f32) -> Option<Vec<Mat4>> {
        self.clip = clip.id;
        self.time = time.clamp(0.0, clip.duration);
        self.clock += if dt.is_finite() { dt.max(0.0) } else { 0.0 };
        self.playback
            .update_selected(&rig.model, clip.id.into(), self.time, self.clock)
            .ok()
    }

    /// Eases the head toward `target`, seen from the posed head of a
    /// character under `joints` placed by `root` facing `yaw`, and
    /// returns the turn.
    fn aim(
        &mut self,
        rig: &Rig,
        joints: &[Mat4],
        root: Mat4,
        yaw: f32,
        target: Option<Vec3>,
        dt: f32,
    ) -> [f32; 2] {
        let want = match (target, rig.head_at(joints)) {
            (Some(target), Some(head)) if target.is_finite() && root.is_finite() => {
                aim(root.transform_point3(head), yaw, target)
            }
            _ => [0.0; 2],
        };
        let k = 1.0 - (-LOOK_RATE * dt.max(0.0)).exp();
        for (now, want) in self.look.iter_mut().zip(want) {
            *now += (want - *now) * k;
            if !now.is_finite() {
                *now = 0.0;
            }
        }
        self.look
    }
}

/// The head turn, yaw and pitch, radians, that faces a head at `head` on a
/// body facing `yaw` toward `target`: none for a target straight above,
/// below, or behind, and at most [`LOOK_YAW`] and [`LOOK_PITCH`].
#[must_use]
pub fn aim(head: Vec3, yaw: f32, target: Vec3) -> [f32; 2] {
    let d = Quat::from_rotation_y(-yaw) * (target - head);
    let level = d.x.hypot(d.z);
    if !level.is_finite() || level < 0.05 {
        return [0.0; 2];
    }
    let turn = d.x.atan2(d.z);
    if turn.abs() > LOOK_BEHIND {
        return [0.0; 2];
    }
    [
        turn.clamp(-LOOK_YAW, LOOK_YAW),
        (-d.y).atan2(level).clamp(-LOOK_PITCH, LOOK_PITCH),
    ]
}

/// A seat's outfit color as a vertex color.
fn tint(color: [f32; 3]) -> [u8; 4] {
    let [r, g, b] = color.map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u8);
    [r, g, b, 255]
}

/// Everyone Everglade draws as the pack's character: the player first,
/// then each seat.
pub struct Cast {
    rig: Rig,
    /// The pack's placed characters' rigs, by form, such as Alice's: a
    /// seat whose figure names one draws as it ([`SeatFigure::form`]).
    forms: Vec<(String, Rig)>,
    player: Actor,
    /// Each seat's playback, by name, in the order last drawn.
    seats: Vec<(String, Actor)>,
    /// The scene for `copies` characters, rebuilt when the count changes.
    scene: Arc<TexturedScene>,
    copies: usize,
    /// The forms the scene draws after the copies, in order.
    drawn_forms: Vec<String>,
    vertices: Arc<Vec<TexturedVertex>>,
    /// Seconds into the player's sledgehammer swing, while one plays.
    swing: Option<f32>,
    /// How the player holds the sledgehammer this frame, in the world.
    hold: Option<Hold>,
}

impl Cast {
    /// The pack's characters, the player posed idle at `at`, or `None` when
    /// the pack carries no character.
    ///
    /// # Errors
    ///
    /// Returns a message when the character lacks a clip a motion plays.
    pub fn new(pack: &ZonePack, at: &PlayerController) -> Result<Option<Self>, String> {
        let Some(character) = &pack.character else {
            return Ok(None);
        };
        let rig = Rig::build(pack, character)?;
        // A placed character that cannot play is left out; its seat draws
        // as the player's character.
        let forms = pack
            .forms
            .iter()
            .filter(|form| form.name.starts_with("npc/"))
            .filter_map(|form| {
                Rig::build_with(pack, form, true)
                    .ok()
                    .map(|rig| (form.name.clone(), rig))
            })
            .collect();
        let mut cast = Self {
            scene: Arc::new(rig.scene(1)),
            vertices: Arc::new(rig.template.clone()),
            rig,
            forms,
            player: Actor::new(),
            seats: Vec::new(),
            copies: 1,
            drawn_forms: Vec::new(),
            swing: None,
            hold: None,
        };
        cast.advance(at, &[], 0.0);
        Ok(Some(cast))
    }

    /// What the player's character is doing: the motion whose clip plays.
    #[must_use]
    pub fn motion(&self) -> Motion {
        Motion::ALL
            .into_iter()
            .find(|m| clip_id(m.index()) == self.player.clip)
            .unwrap_or(Motion::Idle)
    }

    /// Advances every clip over `dt` seconds and poses the player where
    /// `at` stands, facing its yaw, and each of `seats` as it says.
    pub fn advance(&mut self, at: &PlayerController, seats: &[SeatFigure], dt: f32) {
        let rig = &self.rig;
        let mut vertices = Vec::with_capacity(rig.template.len() * (1 + seats.len()));
        let motion = Motion::of_player(at);
        let swing = self.swing.zip(rig.swing);
        let joints = match swing {
            Some((t, clip)) => self.player.hold_at(rig, clip, t, dt),
            None => self.player.advance(rig, Play::Motion(motion), at.speed, dt),
        }
        .unwrap_or_default();
        let root = Mat4::from_rotation_translation(Quat::from_rotation_y(at.yaw), at.pos);
        self.hold = rig
            .hands
            .as_ref()
            .and_then(|hands| hands.hold(&joints, swing.map(|s| s.0)))
            .map(|hold| hold.moved(root));
        rig.skin(&joints, root, None, &mut vertices);
        let mut actors = std::mem::take(&mut self.seats);
        // Seats drawn as the player's character first, then each seat drawn
        // as a placed character's form, in the scene's order.
        let form_of = |seat: &SeatFigure| {
            seat.form
                .and_then(|form| self.forms.iter().position(|(name, _)| name == form))
        };
        let ordered: Vec<(&SeatFigure, Option<usize>)> = seats
            .iter()
            .map(|seat| (seat, form_of(seat)))
            .filter(|(_, form)| form.is_none())
            .chain(
                seats
                    .iter()
                    .map(|seat| (seat, form_of(seat)))
                    .filter(|(_, form)| form.is_some()),
            )
            .collect();
        let mut forms_drawn = Vec::new();
        for (seat, form) in &ordered {
            let mut actor = actors
                .iter()
                .position(|(name, _)| *name == seat.name)
                .map_or_else(Actor::new, |i| actors.swap_remove(i).1);
            let (seat_rig, tinted) = match form {
                Some(index) => {
                    forms_drawn.push(self.forms[*index].0.clone());
                    (&self.forms[*index].1, None)
                }
                None => (rig, Some(tint(seat.tint))),
            };
            let mut joints = actor
                .advance(seat_rig, Play::of(seat), seat.speed, dt)
                .unwrap_or_default();
            let root = Mat4::from_rotation_translation(Quat::from_rotation_y(seat.yaw), seat.pos);
            let look = actor.aim(seat_rig, &joints, root, seat.yaw, seat.look, dt);
            seat_rig.turn_head(&mut joints, look);
            seat_rig.skin(&joints, root, tinted, &mut vertices);
            self.seats.push((seat.name.clone(), actor));
        }
        let copies = 1 + seats.len() - forms_drawn.len();
        if copies != self.copies || forms_drawn != self.drawn_forms {
            let mut scene = rig.scene(copies);
            for name in &forms_drawn {
                if let Some((_, form)) = self.forms.iter().find(|(n, _)| n == name) {
                    scene = super::demolition::join(&scene, &form.scene(1));
                }
            }
            self.scene = Arc::new(scene);
            self.copies = copies;
            self.drawn_forms = forms_drawn;
        }
        self.vertices = Arc::new(vertices);
    }

    /// Plays the sledgehammer swing `t` seconds in from the next advance,
    /// or, with `None`, the player's movement again.
    pub fn set_swing(&mut self, t: Option<f32>) {
        self.swing = t;
    }

    /// How the player holds the sledgehammer, in the world, as last posed:
    /// carried in the right hand, or in both through a swing.
    #[must_use]
    pub fn hold(&self) -> Option<Hold> {
        self.hold
    }

    /// The swing as the character plays it, when the pack has the clip.
    #[must_use]
    pub fn swing_track(&self) -> Option<&SwingTrack> {
        self.rig.hands.as_ref()?.track.as_ref()
    }

    /// Everyone posed for this frame's dynamic mesh.
    #[must_use]
    pub fn figure(&self) -> Figure {
        Figure {
            scene: self.scene.clone(),
            vertices: self.vertices.clone(),
        }
    }

    /// As [`Self::figure`], with the player's own character collapsed to a
    /// point, so it neither draws nor casts a shadow. The seats still draw.
    #[must_use]
    pub fn figure_without_player(&self) -> Figure {
        let mut vertices = self.vertices.as_ref().clone();
        let player = self.rig.template.len().min(vertices.len());
        if let Some(point) = vertices.first().map(|v| v.pos) {
            for vertex in &mut vertices[..player] {
                vertex.pos = point;
            }
        }
        Figure {
            scene: self.scene.clone(),
            vertices: Arc::new(vertices),
        }
    }
}

/// A form the player takes, such as a Wild Shape beast: one of the pack's
/// forms posed where the player stands, playing its gait by the player's
/// speed and its attack clip when struck from [`Self::advance`].
pub struct Beast {
    rig: Rig,
    actor: Actor,
    vertices: Vec<TexturedVertex>,
}

impl Beast {
    /// The form `character` from `pack`, standing in its bind pose.
    ///
    /// # Errors
    ///
    /// Returns a message when the form has no clip to stand in.
    pub fn new(pack: &ZonePack, character: &Character) -> Result<Self, String> {
        let rig = Rig::build_with(pack, character, true)?;
        Ok(Self {
            vertices: rig.template.clone(),
            rig,
            actor: Actor::new(),
        })
    }

    /// The form in its bind pose, for a figure that draws it beside others:
    /// its images, materials, and one mesh.
    #[must_use]
    pub fn figure(&self) -> Figure {
        Figure {
            scene: Arc::new(self.rig.scene(1)),
            vertices: Arc::new(self.rig.template.clone()),
        }
    }

    /// How long the form's attack clip plays, s, when it has one.
    #[must_use]
    pub fn attack_length(&self) -> Option<f32> {
        self.rig.swing.map(|clip| clip.duration)
    }

    /// Poses the form for `at` after `dt` seconds: at its place and facing,
    /// `scale` times its modeled size, playing its attack `attack` seconds
    /// in, or else its flight when `aloft`, or else the gait for its speed.
    pub fn advance(
        &mut self,
        at: &PlayerController,
        scale: f32,
        attack: Option<f32>,
        aloft: bool,
        dt: f32,
    ) {
        let rig = &self.rig;
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let joints = match attack.zip(rig.swing) {
            Some((t, clip)) => self.actor.hold_at(rig, clip, t, dt),
            // A larger body covers more ground per stride.
            None => self.actor.advance(
                rig,
                // In the air a bird's Jump motion is its wingbeat.
                Play::Motion(if aloft {
                    Motion::Jump
                } else {
                    Motion::of_player(at)
                }),
                at.speed / scale,
                dt,
            ),
        }
        .unwrap_or_default();
        let root = Mat4::from_scale_rotation_translation(
            Vec3::splat(scale),
            Quat::from_rotation_y(at.yaw),
            at.pos,
        );
        self.vertices.clear();
        rig.skin(&joints, root, None, &mut self.vertices);
    }

    /// How far `motion`'s clip carries the form in one loop at its modeled
    /// size, m, and how long the loop lasts, s.
    #[must_use]
    pub fn stride(&self, motion: Motion) -> (f32, f32) {
        let clip = self.rig.clip(Play::Motion(motion));
        (clip.distance, clip.duration)
    }

    /// Poses the form at `at` facing `yaw`, `scale` times its modeled size,
    /// `time` seconds into `motion`'s clip (or its stand-in), looping, after
    /// `dt` seconds of the clock, so a change of clip blends.
    pub fn pose(&mut self, at: Vec3, yaw: f32, scale: f32, motion: Motion, time: f32, dt: f32) {
        let clip = self.rig.clip(Play::Motion(motion));
        let time = if time.is_finite() {
            time.rem_euclid(clip.duration.max(1e-3))
        } else {
            0.0
        };
        let joints = self
            .actor
            .hold_at(&self.rig, clip, time, dt)
            .unwrap_or_default();
        let root = Mat4::from_scale_rotation_translation(
            Vec3::splat(scale),
            Quat::from_rotation_y(yaw),
            at,
        );
        self.vertices.clear();
        self.rig.skin(&joints, root, None, &mut self.vertices);
    }

    /// Poses the form where `at` stands but facing `yaw`, `scale` times its
    /// modeled size, playing its gait for `at`'s speed after `dt` seconds;
    /// a jump stands idle.
    pub fn pose_walking(&mut self, at: &PlayerController, yaw: f32, scale: f32, dt: f32) {
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let motion = match Motion::of_player(at) {
            Motion::Jump => Motion::Idle,
            motion => motion,
        };
        let joints = self
            .actor
            .advance(&self.rig, Play::Motion(motion), at.speed / scale, dt)
            .unwrap_or_default();
        let root = Mat4::from_scale_rotation_translation(
            Vec3::splat(scale),
            Quat::from_rotation_y(yaw),
            at.pos,
        );
        self.vertices.clear();
        self.rig.skin(&joints, root, None, &mut self.vertices);
    }

    /// How long the form's clip `name` lasts, s, when it has one.
    #[must_use]
    pub fn clip_length(&self, name: &str) -> Option<f32> {
        self.rig
            .named
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, clip)| clip.duration)
    }

    /// Poses the form at `at` facing `yaw`, `scale` times its modeled
    /// size, `time` seconds into its clip `name`, after `dt` seconds of the
    /// clock, so a change of clip blends. A looping clip wraps; one that
    /// doesn't holds its last frame. Returns false, posing nothing, when
    /// the form has no such clip.
    #[allow(clippy::too_many_arguments)]
    pub fn pose_clip(
        &mut self,
        at: Vec3,
        yaw: f32,
        scale: f32,
        name: &str,
        time: f32,
        looping: bool,
        dt: f32,
    ) -> bool {
        let Some(clip) = self
            .rig
            .named
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, clip)| *clip)
        else {
            return false;
        };
        let length = clip.duration.max(1e-3);
        let time = match (time.is_finite(), looping) {
            (false, _) => 0.0,
            (true, true) => time.rem_euclid(length),
            (true, false) => time.clamp(0.0, length - 1e-3),
        };
        let joints = self
            .actor
            .hold_at(&self.rig, clip, time, dt)
            .unwrap_or_default();
        let scale = if scale.is_finite() && scale > 0.0 {
            scale
        } else {
            1.0
        };
        let root = Mat4::from_scale_rotation_translation(
            Vec3::splat(scale),
            Quat::from_rotation_y(yaw),
            at,
        );
        self.vertices.clear();
        self.rig.skin(&joints, root, None, &mut self.vertices);
        true
    }

    /// The posed vertices, in [`Self::figure`]'s order.
    #[must_use]
    pub fn vertices(&self) -> &[TexturedVertex] {
        &self.vertices
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
        assert_eq!(
            clips,
            [
                "idle",
                "walk",
                "run",
                "jump",
                "backpedal",
                "strafe_left",
                "strafe_right"
            ]
        );
        assert!(Motion::ALL.iter().enumerate().all(|(i, m)| m.index() == i));
    }

    #[test]
    fn engine_clip_ids_avoid_the_fallback_and_held_clips() {
        assert!((0..Motion::ALL.len()).all(|i| clip_id(i) > 1));
        // Posture clips never share an ID with a motion's.
        for i in 0..Posture::ALL.len() {
            assert!(posture_id(i) > clip_id(Motion::ALL.len() - 1));
        }
    }

    fn figure(name: &str, speed: f32, posture: Posture) -> SeatFigure {
        SeatFigure {
            name: name.into(),
            pos: Vec3::new(1.0, 0.0, 2.0),
            yaw: 0.0,
            speed,
            posture,
            look: None,
            tint: [0.45, 0.75, 1.0],
            form: None,
        }
    }

    #[test]
    fn a_moving_seat_walks_or_runs_and_a_still_one_holds_its_posture() {
        let typing = figure("ada", 0.0, Posture::Type);
        assert_eq!(Play::of(&typing), Play::Posture(Posture::Type));
        let walking = figure("ada", super::super::studio::WALK_SPEED, Posture::Type);
        assert_eq!(Play::of(&walking), Play::Motion(Motion::Walk));
        let running = figure("ada", super::super::studio::RUN_SPEED, Posture::Stand);
        assert_eq!(Play::of(&running), Play::Motion(Motion::Run));
    }

    #[test]
    fn a_head_turns_toward_what_it_looks_at_within_its_reach() {
        let head = Vec3::new(0.0, 1.6, 0.0);
        // Ahead and level: no turn.
        let [yaw, pitch] = aim(head, 0.0, Vec3::new(0.0, 1.6, 3.0));
        assert!(yaw.abs() < 1e-5 && pitch.abs() < 1e-5);
        // To the side: a turn toward it, bounded.
        let [yaw, _] = aim(head, 0.0, Vec3::new(3.0, 1.6, 3.0));
        assert!((yaw - std::f32::consts::FRAC_PI_4).abs() < 1e-4, "{yaw}");
        let [yaw, _] = aim(head, 0.0, Vec3::new(3.0, 1.6, 0.2));
        assert_eq!(yaw, LOOK_YAW);
        // Below: the head tips down.
        let [_, pitch] = aim(head, 0.0, Vec3::new(0.0, 1.0, 1.0));
        assert!(pitch > 0.0);
        // Behind, or straight above: no turn.
        assert_eq!(aim(head, 0.0, Vec3::new(0.0, 1.6, -3.0)), [0.0; 2]);
        assert_eq!(aim(head, 0.0, Vec3::new(0.0, 4.0, 0.0)), [0.0; 2]);
        // The body's heading counts: facing +x, a target on +x is ahead.
        let [yaw, _] = aim(head, std::f32::consts::FRAC_PI_2, Vec3::new(3.0, 1.6, 0.0));
        assert!(yaw.abs() < 1e-4, "{yaw}");
    }

    #[test]
    fn the_player_carries_the_sledgehammer_and_chops_with_both_hands() {
        let pack = super::super::tests::pack();
        let at = PlayerController::new(Vec3::new(3.0, 0.0, -2.0), 0.7);
        let mut cast = Cast::new(pack, &at).unwrap().expect("the pack's character");
        let track = cast.swing_track().expect("the pack has the swing").clone();
        assert!(track.impact > 0.1 && track.impact < track.duration);
        // The head lands ahead of the character, about chest to knee high.
        let head = track.hold(track.impact).head();
        assert!(head.z > 0.5 && (0.3..1.8).contains(&head.y), "{head}");
        // Carried, the hammer hangs from the right hand, ahead of it.
        cast.advance(&at, &[], 0.0);
        let carry = cast.hold().expect("the character holds the hammer");
        let right = at.forward().cross(Vec3::Y);
        assert!((carry.grip - at.pos).dot(right) > 0.05, "{carry:?}");
        assert!(carry.axis.dot(at.forward()) > 0.3, "{carry:?}");
        // Through the chop both hands hold the handle, the head out past
        // them.
        cast.set_swing(Some(track.impact));
        for _ in 0..30 {
            cast.advance(&at, &[], 1.0 / 30.0);
        }
        let chop = cast.hold().unwrap();
        assert!((chop.axis.length() - 1.0).abs() < 1e-3);
        assert!(chop.face.dot(chop.axis).abs() < 1e-3);
        assert!((chop.head() - at.pos).dot(at.forward()) > 0.5, "{chop:?}");
    }

    #[test]
    fn the_workshop_agent_draws_as_alice_and_stands_at_her_desk() {
        let pack = super::super::tests::pack();
        let at = PlayerController::new(Vec3::ZERO, 0.0);
        let mut cast = Cast::new(pack, &at).unwrap().expect("the pack's character");
        let alice = pack
            .form(crate::zones::everglade_pack::compile::ALICE_FORM)
            .expect("the committed pack carries Alice");
        let player = cast.figure().vertices.len();
        let rig = &cast.forms.iter().find(|(n, _)| n == &alice.name).unwrap().1;
        let hers = rig.template.len();
        // She stands typing at her standing desk: her rig authors the
        // typing posture from her idle, as it does for the player's
        // character, and idle at the desk is her idle itself.
        assert!(rig.authored(Posture::Type) && !rig.authored(Posture::Stand));
        let seats = [
            SeatFigure {
                form: super::super::npcs::form_of("alice"),
                ..figure("alice", 0.0, Posture::Type)
            },
            figure("grace", 0.0, Posture::Wait),
        ];
        for _ in 0..5 {
            cast.advance(&at, &seats, 0.05);
        }
        let all = cast.figure();
        all.validate().unwrap();
        assert_eq!(all.vertices.len(), 2 * player + hers);
        // Walking to the Workbench she plays her walk.
        let walking = [SeatFigure {
            form: super::super::npcs::form_of("alice"),
            ..figure("alice", 1.4, Posture::Stand)
        }];
        cast.advance(&at, &walking, 0.05);
        let moved = cast.figure();
        moved.validate().unwrap();
        assert_eq!(moved.vertices.len(), player + hers);
    }

    #[test]
    fn the_cast_draws_the_player_and_every_seat_as_one_figure() {
        let pack = super::super::tests::pack();
        let at = PlayerController::new(Vec3::ZERO, 0.0);
        let mut cast = Cast::new(pack, &at).unwrap().expect("the pack's character");
        let one = cast.figure();
        one.validate().unwrap();
        // Every posture has a clip of its own but standing, which is idle.
        for posture in Posture::ALL {
            assert_eq!(
                cast.rig.authored(posture),
                posture != Posture::Stand,
                "{posture:?}"
            );
        }
        let seats = [
            figure("ada", 0.0, Posture::Type),
            SeatFigure {
                pos: Vec3::new(-2.0, 0.0, 4.0),
                look: Some(Vec3::new(0.0, 1.6, 0.0)),
                tint: [1.0, 0.82, 0.35],
                form: None,
                ..figure("grace", 0.0, Posture::Wait)
            },
        ];
        for _ in 0..10 {
            cast.advance(&at, &seats, 0.05);
        }
        let all = cast.figure();
        all.validate().unwrap();
        let each = one.vertices.len();
        assert_eq!(all.vertices.len(), 3 * each);
        // The player's copy is untinted; each seat's outfit wears its tint.
        let outfit = cast.rig.outfit.clone();
        assert!(!outfit.is_empty());
        assert_eq!(
            all.vertices[outfit.start].color,
            cast.rig.template[outfit.start].color
        );
        assert_eq!(all.vertices[each + outfit.start].color, tint(seats[0].tint));
        assert_eq!(
            all.vertices[2 * each + outfit.start].color,
            tint(seats[1].tint)
        );
        // Each seat stands where it says.
        for (i, seat) in seats.iter().enumerate() {
            let copy = &all.vertices[(i + 1) * each..(i + 2) * each];
            let (sum, n) = copy.iter().fold((Vec3::ZERO, 0.0), |(s, n), v| {
                (s + Vec3::from(v.pos), n + 1.0)
            });
            let center = sum / n;
            assert!(
                (center.x - seat.pos.x).hypot(center.z - seat.pos.z) < 0.6,
                "{center}"
            );
        }
        // The typing seat stands at its desk: its highest point is as
        // high as the waiting seat's, who stands too.
        let top = |i: usize| {
            all.vertices[(i + 1) * each..(i + 2) * each]
                .iter()
                .map(|v| v.pos[1])
                .fold(f32::MIN, f32::max)
        };
        assert!((top(0) - top(1)).abs() < 0.1, "{} {}", top(0), top(1));
        // A seat that leaves is no longer drawn.
        cast.advance(&at, &seats[1..], 0.05);
        assert_eq!(cast.figure().vertices.len(), 2 * each);
        cast.figure().validate().unwrap();
    }
}
