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
use crate::zones::everglade_pack::{Character, ZonePack};

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
pub(crate) struct Rig {
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

impl Rig {
    fn build(pack: &ZonePack, character: &Character) -> Result<Self, String> {
        let mut motions = [Loop {
            id: 0,
            duration: 1.0,
            distance: 0.0,
        }; Motion::ALL.len()];
        let mut clips = Vec::new();
        for motion in Motion::ALL {
            let clip = character
                .clip(motion.clip())
                .ok_or_else(|| format!("The Everglade player has no {} clip", motion.clip()))?;
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
pub(crate) struct Actor {
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
pub(crate) struct Cast {
    rig: Rig,
    player: Actor,
    /// Each seat's playback, by name, in the order last drawn.
    seats: Vec<(String, Actor)>,
    /// The scene for `copies` characters, rebuilt when the count changes.
    scene: Arc<TexturedScene>,
    copies: usize,
    vertices: Arc<Vec<TexturedVertex>>,
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
        let mut cast = Self {
            scene: Arc::new(rig.scene(1)),
            vertices: Arc::new(rig.template.clone()),
            rig,
            player: Actor::new(),
            seats: Vec::new(),
            copies: 1,
        };
        cast.advance(at, &[], 0.0);
        Ok(Some(cast))
    }

    /// What the player's character is doing: the motion whose clip plays.
    #[cfg(test)]
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
        let joints = self
            .player
            .advance(rig, Play::Motion(motion), at.speed, dt)
            .unwrap_or_default();
        let root = Mat4::from_rotation_translation(Quat::from_rotation_y(at.yaw), at.pos);
        rig.skin(&joints, root, None, &mut vertices);
        let mut actors = std::mem::take(&mut self.seats);
        for seat in seats {
            let mut actor = actors
                .iter()
                .position(|(name, _)| *name == seat.name)
                .map_or_else(Actor::new, |i| actors.swap_remove(i).1);
            let mut joints = actor
                .advance(rig, Play::of(seat), seat.speed, dt)
                .unwrap_or_default();
            let root = Mat4::from_rotation_translation(Quat::from_rotation_y(seat.yaw), seat.pos);
            let look = actor.aim(rig, &joints, root, seat.yaw, seat.look, dt);
            rig.turn_head(&mut joints, look);
            rig.skin(&joints, root, Some(tint(seat.tint)), &mut vertices);
            self.seats.push((seat.name.clone(), actor));
        }
        let copies = 1 + seats.len();
        if copies != self.copies {
            self.scene = Arc::new(rig.scene(copies));
            self.copies = copies;
        }
        self.vertices = Arc::new(vertices);
    }

    /// Everyone posed for this frame's dynamic mesh.
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
        // The typing seat sits: its highest point is lower than the
        // waiting seat's, who stands.
        let top = |i: usize| {
            all.vertices[(i + 1) * each..(i + 2) * each]
                .iter()
                .map(|v| v.pos[1])
                .fold(f32::MIN, f32::max)
        };
        assert!(top(0) < top(1) - 0.2, "{} {}", top(0), top(1));
        // A seat that leaves is no longer drawn.
        cast.advance(&at, &seats[1..], 0.05);
        assert_eq!(cast.figure().vertices.len(), 2 * each);
        cast.figure().validate().unwrap();
    }
}
