//! Admitted character rigs and presentation-only locomotion adjustments.
use crate::{assets::Model, core::LifeId, motion::State};
use glam::{Mat4, Quat, Vec3};
use serde::{Deserialize, Serialize};

/// Animation never supplies displacement to world authority or prediction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootMotion {
    InPlace,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Leg {
    pub hip: String,
    pub knee: String,
    pub foot: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Definition {
    pub root_motion: RootMotion,
    pub speed_parameter: usize,
    pub walk_speed: f32,
    pub run_speed: f32,
    pub walk_stride: f32,
    pub run_stride: f32,
    pub root: String,
    pub spine: String,
    pub legs: [Leg; 2],
    pub sole: f32,
    pub max_adjustment: f32,
    pub minimum_normal_y: f32,
}
impl Definition {
    pub fn universal(speed_parameter: usize) -> Self {
        Self {
            root_motion: RootMotion::InPlace,
            speed_parameter,
            walk_speed: 1.8,
            run_speed: 4.5,
            walk_stride: 1.3,
            run_stride: 5.0,
            root: "root".into(),
            spine: "spine_03".into(),
            legs: ["l", "r"].map(|side| Leg {
                hip: format!("thigh_{side}"),
                knee: format!("calf_{side}"),
                foot: format!("foot_{side}"),
            }),
            sole: 0.065,
            max_adjustment: 0.45,
            minimum_normal_y: 0.65,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if !self.walk_stride.is_finite()
            || !self.run_stride.is_finite()
            || !(0.1..=10.).contains(&self.walk_stride)
            || !(0.1..=10.).contains(&self.run_stride)
            || !self.walk_speed.is_finite()
            || !self.run_speed.is_finite()
            || !(0.1..=10.).contains(&self.walk_speed)
            || !(self.walk_speed + 0.1..=20.).contains(&self.run_speed)
            || !self.sole.is_finite()
            || !(0. ..=0.2).contains(&self.sole)
            || !self.max_adjustment.is_finite()
            || !(0.01..=0.75).contains(&self.max_adjustment)
            || !self.minimum_normal_y.is_finite()
            || !(0.5..=1.).contains(&self.minimum_normal_y)
        {
            return Err("Invalid locomotion speeds, sole, reach, or slope limit".into());
        }
        let names = std::iter::once(&self.root)
            .chain(std::iter::once(&self.spine))
            .chain(
                self.legs
                    .iter()
                    .flat_map(|leg| [&leg.hip, &leg.knee, &leg.foot]),
            );
        let mut unique = std::collections::BTreeSet::new();
        for name in names {
            if name.is_empty()
                || name.len() > 64
                || name.chars().any(char::is_control)
                || !unique.insert(name)
            {
                return Err("Locomotion joints must have unique bounded names".into());
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ground {
    pub position: Vec3,
    pub normal: Vec3,
}
impl Ground {
    pub fn validate(self) -> Result<Self, String> {
        if !self.position.is_finite()
            || self.position.abs().max_element() > 1_000_000.
            || !self.normal.is_finite()
            || (self.normal.length_squared() - 1.).abs() > 0.001
        {
            return Err("Invalid animation ground sample".into());
        }
        Ok(self)
    }
}
/// The application supplies read-only contacts from its admitted geometry.
pub trait Support {
    fn sample(&self, life: LifeId, position: Vec3, reach: f32) -> Result<Option<Ground>, String>;
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Aim {
    pub yaw: f32,
    pub pitch: f32,
}
impl Aim {
    pub fn validate(self) -> Result<(), String> {
        if !self.yaw.is_finite()
            || !self.pitch.is_finite()
            || self.yaw.abs() > 1.2
            || self.pitch.abs() > 1.2
        {
            return Err("Animation aim must stay within 1.2 radians".into());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub life: LifeId,
    pub aim: Aim,
}
/// Distance-based pose cadence changes presentation work only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Full,
    Half,
    Quarter,
}
impl Tier {
    pub fn interval(self) -> u64 {
        match self {
            Self::Full => 1,
            Self::Half => 2,
            Self::Quarter => 4,
        }
    }
    pub fn at_distance(distance: f32) -> Self {
        if distance < 20. {
            Self::Full
        } else if distance < 60. {
            Self::Half
        } else {
            Self::Quarter
        }
    }
    pub fn sample(self, life: LifeId, frame: u64) -> bool {
        (frame % self.interval() + life.actor % self.interval()) % self.interval() == 0
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct Inputs {
    pub speed: f32,
    pub turn_rate: f32,
    pub aim: Aim,
    pub reset: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Report {
    pub inputs: Inputs,
    pub probes: usize,
    #[serde(default)]
    pub contacts: [Option<Ground>; 2],
    pub planted: [bool; 2],
    pub foot_error: [f32; 2],
    pub reach_clamps: usize,
    pub root_horizontal_removed: f32,
    pub root_yaw_removed: f32,
}

/// Named admission supports reordered joints and different admitted proportions.
pub struct Rig {
    pub definition: Definition,
    root: usize,
    spine: usize,
    legs: [[usize; 3]; 2],
    parents: Vec<i16>,
    bind: Vec<Mat4>,
    basis: Mat4,
}
impl Rig {
    pub fn admit(model: &Model, definition: &Definition) -> Result<Self, String> {
        definition.validate()?;
        model.validate_animation()?;
        let skin = model
            .skin
            .as_ref()
            .ok_or("Locomotion requires a named bind skeleton")?;
        let mut names = std::collections::BTreeSet::new();
        if skin.names.iter().any(|name| !names.insert(name)) {
            return Err("Locomotion skeleton joint names are ambiguous".into());
        }
        let find = |name: &str| {
            skin.names
                .iter()
                .position(|n| n == name)
                .ok_or_else(|| format!("Missing locomotion joint {name}"))
        };
        let legs = definition
            .legs
            .iter()
            .map(|leg| Ok([find(&leg.hip)?, find(&leg.knee)?, find(&leg.foot)?]))
            .collect::<Result<Vec<_>, String>>()?;
        let bind = skin
            .inverse_bind
            .iter()
            .map(|m| Mat4::from_cols_array(m).inverse())
            .collect::<Vec<_>>();
        if bind
            .iter()
            .any(|m| !m.is_finite() || m.determinant().abs() < 1e-8)
        {
            return Err("Locomotion bind matrices must be invertible".into());
        }
        let rig = Self {
            definition: definition.clone(),
            root: find(&definition.root)?,
            spine: find(&definition.spine)?,
            legs: [legs[0], legs[1]],
            parents: model.bones.iter().map(|b| b.parent).collect(),
            bind,
            basis: Mat4::from_cols_array(&skin.basis),
        };
        for [hip, knee, foot] in rig.legs {
            if model.bones[knee].parent != hip as i16
                || model.bones[foot].parent != knee as i16
                || !rig.descendant(hip, rig.root)
                || !rig.descendant(rig.spine, rig.root)
                || (rig.bind[hip].w_axis.truncate() - rig.bind[knee].w_axis.truncate()).length()
                    < 0.001
                || (rig.bind[knee].w_axis.truncate() - rig.bind[foot].w_axis.truncate()).length()
                    < 0.001
            {
                return Err(
                    "Locomotion legs must be nondegenerate direct chains under the root".into(),
                );
            }
        }
        Ok(rig)
    }
    fn descendant(&self, mut joint: usize, ancestor: usize) -> bool {
        loop {
            if joint == ancestor {
                return true;
            }
            let parent = self.parents[joint];
            if parent < 0 {
                return false;
            }
            joint = parent as usize;
        }
    }
    fn world(&self, body: Mat4, palette: &[Mat4], joint: usize) -> Mat4 {
        body * palette[joint] * self.basis * self.bind[joint]
    }
    fn apply(&self, body: Mat4, palette: &mut [Mat4], joint: usize, delta: Mat4) {
        let adjustment = body.inverse() * delta * body;
        for (i, matrix) in palette.iter_mut().enumerate() {
            if self.descendant(i, joint) {
                *matrix = adjustment * *matrix;
            }
        }
    }
    pub fn foot_positions(&self, body: Mat4, palette: &[Mat4]) -> Result<[Vec3; 2], String> {
        self.validate_palette(body, palette)?;
        Ok(self
            .legs
            .map(|leg| self.world(body, palette, leg[2]).w_axis.truncate()))
    }
    fn validate_palette(&self, body: Mat4, palette: &[Mat4]) -> Result<(), String> {
        if palette.len() != self.parents.len()
            || !body.is_finite()
            || body.determinant().abs() < 1e-8
            || palette.iter().any(|m| !crate::sockets::affine(*m))
        {
            return Err("Locomotion pose or body is invalid".into());
        }
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct Playback {
    life: Option<LifeId>,
    phase_owner: Option<u64>,
    position: Vec3,
    forward: Vec3,
    clock: f64,
    pub inputs: Inputs,
    distance: f32,
    phase: f32,
    planted: [Option<Vec3>; 2],
}
impl Playback {
    pub fn controls(
        &mut self,
        life: LifeId,
        body: Mat4,
        clock: f64,
        owner: Option<u64>,
        aim: Aim,
    ) -> Result<Inputs, String> {
        aim.validate()?;
        if !body.is_finite()
            || body.determinant().abs() < 1e-8
            || !clock.is_finite()
            || !(0. ..=1_000_000.).contains(&clock)
        {
            return Err("Invalid locomotion placement or clock".into());
        }
        let position = body.w_axis.truncate();
        let forward = body.transform_vector3(Vec3::X).normalize();
        let reset = self.life != Some(life)
            || self.phase_owner != owner
            || clock < self.clock
            || clock - self.clock > 0.5
            || position.distance(self.position) > 2.;
        let dt = (clock - self.clock) as f32;
        self.distance = if reset {
            0.
        } else {
            let d = position - self.position;
            Vec3::new(d.x, 0., d.z).length()
        };
        let mut inputs = if !reset && dt <= 0.00001 {
            Inputs {
                aim,
                reset,
                ..self.inputs
            }
        } else {
            Inputs {
                aim,
                reset,
                ..Default::default()
            }
        };
        if !reset && dt > 0.00001 && dt < 0.5 {
            let delta = position - self.position;
            let speed = Vec3::new(delta.x, 0., delta.z).length() / dt;
            let old = Vec3::new(self.forward.x, 0., self.forward.z).normalize_or_zero();
            let new = Vec3::new(forward.x, 0., forward.z).normalize_or_zero();
            inputs.speed = speed.min(20.);
            inputs.turn_rate = old.cross(new).y.atan2(old.dot(new)) / dt;
            inputs.turn_rate = inputs.turn_rate.clamp(-4., 4.);
        }
        if reset {
            self.planted = [None; 2];
            self.phase = 0.;
        }
        self.life = Some(life);
        self.phase_owner = owner;
        self.position = position;
        self.forward = forward;
        self.clock = clock;
        self.inputs = inputs;
        Ok(inputs)
    }
    /// Advance visual gait by presented travel; casts retain their authored clock.
    pub fn gait_phase(&mut self, rig: &Rig, state: State, authored: f32) -> f32 {
        if matches!(state, State::Walk | State::Run) {
            let d = &rig.definition;
            let weight =
                ((self.inputs.speed - d.walk_speed) / (d.run_speed - d.walk_speed)).clamp(0., 1.);
            self.phase += self.distance / (d.walk_stride + (d.run_stride - d.walk_stride) * weight);
            self.distance = 0.;
            self.phase
        } else if state == State::Idle {
            self.phase
        } else {
            authored
        }
    }
    pub fn adjust(
        &mut self,
        rig: &Rig,
        life: LifeId,
        body: Mat4,
        state: State,
        phase: f32,
        palette: &mut [Mat4],
        support: Option<&dyn Support>,
    ) -> Result<Report, String> {
        rig.validate_palette(body, palette)?;
        if !phase.is_finite() || phase < 0. || self.life != Some(life) {
            return Err(
                "Locomotion adjustment requires the current admitted life and phase".into(),
            );
        }
        let mut report = Report {
            inputs: self.inputs,
            ..Default::default()
        };
        let moving = matches!(
            state,
            State::Walk | State::Run | State::Backpedal | State::StrafeLeft | State::StrafeRight
        );
        if moving {
            let animated = rig.world(body, palette, rig.root);
            let rest = body * rig.basis * rig.bind[rig.root];
            let difference = animated.w_axis.truncate() - rest.w_axis.truncate();
            let translation = Vec3::new(-difference.x, 0., -difference.z);
            report.root_horizontal_removed = translation.length();
            let relative = rest.to_scale_rotation_translation().1.inverse()
                * animated.to_scale_rotation_translation().1;
            let yaw = relative.to_euler(glam::EulerRot::YXZ).0;
            report.root_yaw_removed = yaw;
            let point = animated.w_axis.truncate();
            rig.apply(
                body,
                palette,
                rig.root,
                Mat4::from_translation(translation)
                    * around(point, Quat::from_axis_angle(Vec3::Y, -yaw)),
            );
        }
        if matches!(state, State::Death | State::Prone | State::Airborne) {
            self.planted = [None; 2];
            return Ok(report);
        }
        let spine = rig.world(body, palette, rig.spine).w_axis.truncate();
        let right = Vec3::new(self.forward.x, 0., self.forward.z)
            .cross(Vec3::Y)
            .normalize_or_zero();
        let turn = (self.inputs.turn_rate * 0.06).clamp(-0.2, 0.2);
        let rotation = Quat::from_axis_angle(Vec3::Y, self.inputs.aim.yaw + turn)
            * if right.length_squared() > 0.5 {
                Quat::from_axis_angle(right, self.inputs.aim.pitch)
            } else {
                Quat::IDENTITY
            };
        rig.apply(body, palette, rig.spine, around(spine, rotation));
        let Some(support) = support else {
            self.planted = [None; 2];
            return Ok(report);
        };
        for side in 0..2 {
            let [hip, knee, foot] = rig.legs[side];
            let foot_position = rig.world(body, palette, foot).w_axis.truncate();
            let rest = (body * rig.basis * rig.bind[foot]).w_axis.truncate();
            let lift = foot_position.y - rest.y;
            let contact = !moving
                || lift
                    <= if self.planted[side].is_some() {
                        0.06
                    } else {
                        0.03
                    };
            if !contact {
                self.planted[side] = None;
                continue;
            }
            let requested = self.planted[side].unwrap_or(foot_position);
            report.probes += 1;
            let Some(ground) = support
                .sample(life, requested, rig.definition.max_adjustment)?
                .map(Ground::validate)
                .transpose()?
            else {
                self.planted[side] = None;
                continue;
            };
            if ground.normal.y < rig.definition.minimum_normal_y {
                self.planted[side] = None;
                continue;
            }
            report.contacts[side] = Some(ground);
            let target = ground.position + ground.normal * rig.definition.sole;
            if foot_position.distance(target) > rig.definition.max_adjustment {
                self.planted[side] = None;
                continue;
            }
            let origin = rig.world(body, palette, hip).w_axis.truncate();
            let elbow = rig.world(body, palette, knee).w_axis.truncate();
            let a = origin.distance(elbow);
            let b = elbow.distance(foot_position);
            let direction = (target - origin).normalize_or_zero();
            if direction.length_squared() < 0.5 || a < 0.001 || b < 0.001 {
                self.planted[side] = None;
                continue;
            }
            let distance = origin
                .distance(target)
                .clamp((a - b).abs() + 0.0001, a + b - 0.0001);
            if (distance - origin.distance(target)).abs() > 0.001 {
                report.reach_clamps += 1;
            }
            let reached = origin + direction * distance;
            let bend = elbow - origin - direction * (elbow - origin).dot(direction);
            let bend = if bend.length_squared() > 1e-8 {
                bend.normalize()
            } else {
                direction
                    .cross(if direction.x.abs() > 0.9 {
                        Vec3::Z
                    } else {
                        Vec3::X
                    })
                    .normalize()
            };
            let x = (a * a - b * b + distance * distance) / (2. * distance);
            let new_elbow = origin + direction * x + bend * (a * a - x * x).max(0.).sqrt();
            rig.apply(
                body,
                palette,
                hip,
                around(
                    origin,
                    Quat::from_rotation_arc(
                        (elbow - origin).normalize(),
                        (new_elbow - origin).normalize(),
                    ),
                ),
            );
            let current_foot = rig.world(body, palette, foot).w_axis.truncate();
            rig.apply(
                body,
                palette,
                knee,
                around(
                    new_elbow,
                    Quat::from_rotation_arc(
                        (current_foot - new_elbow).normalize(),
                        (reached - new_elbow).normalize(),
                    ),
                ),
            );
            let current = rig.world(body, palette, foot);
            let rest_rotation = (body * rig.basis * rig.bind[foot])
                .to_scale_rotation_translation()
                .1;
            let up =
                (current.to_scale_rotation_translation().1 * rest_rotation.inverse() * Vec3::Y)
                    .normalize();
            rig.apply(
                body,
                palette,
                foot,
                around(
                    current.w_axis.truncate(),
                    Quat::from_rotation_arc(up, ground.normal),
                ),
            );
            let final_position = rig.world(body, palette, foot).w_axis.truncate();
            report.foot_error[side] = final_position.distance(target);
            if report.foot_error[side] < 0.005 {
                self.planted[side] = Some(ground.position);
                report.planted[side] = true;
            } else {
                self.planted[side] = None;
            }
        }
        rig.validate_palette(body, palette)?;
        Ok(report)
    }
}
fn around(point: Vec3, rotation: Quat) -> Mat4 {
    Mat4::from_translation(point) * Mat4::from_quat(rotation) * Mat4::from_translation(-point)
}

#[cfg(test)]
#[path = "locomotion/tests.rs"]
mod tests;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Diagnostic {
    pub life: LifeId,
    pub model: String,
    pub semantic: State,
    pub graph_state: usize,
    pub selection_epoch: u64,
    pub phase: f32,
    #[serde(default)]
    pub source_epoch: u64,
    #[serde(default)]
    pub phase_owner: Option<u64>,
    pub parameters: Vec<crate::animation_graph::Value>,
    pub tier: Tier,
    pub evaluated: bool,
    pub bones_sampled: usize,
    pub ground_available: bool,
    pub adjustment: Option<Report>,
}
/// One atomic pose path shared by rendering and authoring previews.
#[derive(Clone, Default)]
pub struct Controller {
    graph: crate::animation_graph::Playback,
    motion: Playback,
    admission: Option<u64>,
    frame: u64,
    source_epoch: u64,
    source: Option<(State, f32)>,
}
impl Controller {
    pub fn update(
        &mut self,
        graph: &crate::animation_graph::Semantic,
        instance: &crate::presentation::Instance,
        clock: f64,
        aim: Aim,
        tier: Tier,
        support: Option<&dyn Support>,
    ) -> Result<(crate::animation_graph::Frame, Diagnostic), String> {
        let life = instance
            .actor
            .ok_or("Animation controller requires an actor life")?;
        let crate::motion::Selection::Named(state) = instance.animation else {
            return Err("Animation graph requires a semantic selection".into());
        };
        let mut candidate = self.clone();
        if candidate.admission != Some(graph.admitted().identity())
            || candidate
                .source
                .is_some_and(|(old_state, time)| old_state == state && instance.time < time)
        {
            candidate.motion = Playback::default();
        }
        candidate.admission = Some(graph.admitted().identity());
        candidate.source = Some((state, instance.time));
        let input = candidate.motion.controls(
            life,
            instance.transform,
            clock,
            instance.animation_epoch,
            aim,
        )?;
        if input.reset {
            candidate.frame = 0;
            candidate.source_epoch = candidate
                .source_epoch
                .checked_add(1)
                .ok_or("Animation source epoch exhausted")?;
            if graph.rig().is_some() && matches!(state, State::Walk | State::Run) {
                candidate.motion.phase = graph.phase(state, instance.time)?;
            }
        }
        let values = graph.motion_values(state, input)?;
        let phase = if state == State::Idle {
            graph.phase(state, instance.time)?
        } else {
            graph.rig().map_or(instance.time, |rig| {
                candidate.motion.gait_phase(rig, state, instance.time)
            })
        };
        let mut frame = candidate.graph.update_quality(
            graph.admitted(),
            life,
            &values,
            f64::from(phase),
            clock,
            Some(candidate.source_epoch),
            input.reset || tier.sample(life, candidate.frame),
        )?;
        candidate.frame = candidate.frame.wrapping_add(1);
        let adjustment = graph
            .rig()
            .map(|rig| {
                candidate.motion.adjust(
                    rig,
                    life,
                    instance.transform,
                    state,
                    phase,
                    &mut frame.matrices,
                    support,
                )
            })
            .transpose()?;
        let diagnostic = Diagnostic {
            life,
            model: instance.model.clone(),
            semantic: state,
            graph_state: frame.state,
            selection_epoch: frame.selection_epoch,
            phase,
            source_epoch: candidate.source_epoch,
            phase_owner: instance.animation_epoch,
            parameters: values,
            tier,
            evaluated: frame.evaluated,
            bones_sampled: if frame.evaluated {
                frame.matrices.len()
            } else {
                0
            },
            ground_available: support.is_some(),
            adjustment,
        };
        *self = candidate;
        Ok((frame, diagnostic))
    }
}
