//! Viewing state never starts a controller or physical simulation.

use glam::{DVec3, Mat4, Vec3};
use robot_lab::document::{Document, Pose, Reference, Scene, Step};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub use robot_lab::view::Bundle;

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Orbit {
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    pub center: [f32; 3],
}
impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.48,
            pitch: 0.32,
            distance: 1.3,
            center: [0.1, 0.9, 0.0],
        }
    }
}
impl Orbit {
    pub fn turn(&mut self, dx: f32, dy: f32) {
        if dx.is_finite() && dy.is_finite() {
            self.yaw = (self.yaw + dx * 0.006).rem_euclid(std::f32::consts::TAU);
            self.pitch = (self.pitch + dy * 0.006).clamp(-1.3, 1.3);
        }
    }
    pub fn zoom(&mut self, delta: f32) {
        if delta.is_finite() {
            self.distance =
                (self.distance * (delta * 0.001).clamp(-0.5, 0.5).exp()).clamp(0.15, 12.0);
        }
    }
    pub fn matrices(&self, width: u32, height: u32) -> (Mat4, Vec3) {
        let center = Vec3::from_array(self.center);
        let eye = center
            + Vec3::new(
                self.yaw.sin() * self.pitch.cos(),
                self.pitch.sin(),
                self.yaw.cos() * self.pitch.cos(),
            ) * self.distance;
        let aspect = width.max(1) as f32 / height.max(1) as f32;
        (
            Mat4::perspective_rh(0.75, aspect, 0.02, 100.0)
                * Mat4::look_at_rh(eye, center, Vec3::Y),
            eye,
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "intent", rename_all = "snake_case", deny_unknown_fields)]
pub enum Intent {
    Activate,
    ResetView,
    Expanded,
    Close,
    Play,
    Pause,
    Seek,
    ChooseJoint { joint: String },
    ChooseLink { link: String },
    ChooseCamera { camera: String },
    JointValue { joint: String },
    ApplyPose,
    Compare,
    NoteDraft,
    AddNote,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Draw,
    SavePose {
        base: Reference,
        joints: BTreeMap<String, f64>,
    },
    ChooseComparison {
        subject: Reference,
    },
    SaveNote {
        subject: Reference,
        frame: Option<u32>,
        text: String,
    },
}

pub struct State {
    bundle: Bundle,
    pub orbit: Orbit,
    pub joint: String,
    pub link: Option<String>,
    pub camera: String,
    pub frame: usize,
    pub expanded: bool,
    pub draft: BTreeMap<String, f64>,
    visible: bool,
    selected: bool,
    mounted: bool,
    generation: u64,
    playing: bool,
    last_time: Option<f64>,
    playback_time: f64,
    note: String,
}

impl State {
    pub fn new(bundle: Bundle) -> Result<Self, String> {
        bundle.validate()?;
        let joint = bundle
            .robot
            .joints
            .iter()
            .find(|j| j.kind != robot_lab::document::JointKind::Fixed)
            .map(|j| j.name.clone())
            .unwrap_or_default();
        Ok(Self {
            bundle,
            orbit: Orbit::default(),
            joint,
            link: None,
            camera: "orbit".into(),
            frame: 0,
            expanded: false,
            draft: BTreeMap::new(),
            visible: true,
            selected: false,
            mounted: false,
            generation: 1,
            playing: false,
            last_time: None,
            playback_time: 0.0,
            note: String::new(),
        })
    }
    pub fn bundle(&self) -> &Bundle {
        &self.bundle
    }
    pub fn note(&self) -> &str {
        &self.note
    }
    pub fn apply(&mut self, intent: Intent, value: Option<&str>) -> Result<Effect, String> {
        match intent {
            Intent::Activate => {
                self.lifecycle(true, true);
            }
            Intent::ResetView => {
                self.orbit = Orbit::default();
                self.camera = "orbit".into();
            }
            Intent::Expanded => self.expanded = !self.expanded,
            Intent::Close => self.close(),
            Intent::Play => self.play()?,
            Intent::Pause => self.pause(),
            Intent::Seek => self.seek(
                value
                    .ok_or("Choose a frame")?
                    .parse()
                    .map_err(|_| "Choose a frame number")?,
            )?,
            Intent::ChooseJoint { joint } => {
                if !self
                    .bundle
                    .robot
                    .joints
                    .iter()
                    .any(|j| j.name == joint && j.kind != robot_lab::document::JointKind::Fixed)
                {
                    return Err("Choose a movable joint".into());
                }
                self.joint = joint;
            }
            Intent::ChooseLink { link } => self.choose_link(&link)?,
            Intent::ChooseCamera { camera } => self.choose_camera(&camera)?,
            Intent::JointValue { joint } => self.pose_value(
                &joint,
                value
                    .ok_or("Choose a joint position")?
                    .parse()
                    .map_err(|_| "Use a number for the joint position")?,
            )?,
            Intent::ApplyPose => {
                if self.draft.is_empty() {
                    return Err("Move a joint before saving a pose".into());
                }
                return Ok(Effect::SavePose {
                    base: self.bundle.scene_reference.clone(),
                    joints: self.draft.clone(),
                });
            }
            Intent::Compare => {
                return Ok(Effect::ChooseComparison {
                    subject: self.bundle.subject.clone(),
                });
            }
            Intent::NoteDraft => {
                let value = value.ok_or("Write a note")?;
                if value.len() > 1024 || value.chars().any(char::is_control) {
                    return Err("Keep this note on one short line".into());
                }
                self.note = value.into();
            }
            Intent::AddNote => {
                if self.note.trim().is_empty() {
                    return Err("Write a note before saving it".into());
                }
                return Ok(Effect::SaveNote {
                    subject: self.bundle.subject.clone(),
                    frame: self.bundle.episode.as_ref().map(|_| self.frame as u32),
                    text: self.note.clone(),
                });
            }
        }
        Ok(Effect::Draw)
    }
    pub fn active(&self) -> bool {
        self.visible && self.selected
    }
    pub fn generation(&self) -> u64 {
        self.generation
    }
    pub fn mounted(&self) -> bool {
        self.mounted
    }
    pub fn remount(&mut self) {
        self.generation = self.generation.saturating_add(1);
        self.mounted = false;
    }
    /// A new visibility/selection lifetime invalidates pending GPU opens and
    /// callbacks. The adapter drops its GPU and subscriptions on `false`.
    pub fn lifecycle(&mut self, visible: bool, selected: bool) -> bool {
        let was = self.active();
        self.visible = visible;
        self.selected = selected;
        if was != self.active() {
            self.generation = self.generation.saturating_add(1);
            self.mounted = false;
            self.last_time = None;
            if !self.active() {
                self.playing = false;
            }
        }
        self.active()
    }
    pub fn mount(&mut self, generation: u64) -> bool {
        if generation == self.generation && self.active() {
            self.mounted = true;
            true
        } else {
            false
        }
    }
    pub fn close(&mut self) {
        self.lifecycle(false, false);
    }
    pub fn play(&mut self) -> Result<(), String> {
        if !self.active() || self.bundle.episode.is_none() {
            return Err("Open a saved recording to play it".into());
        }
        if let Some(episode) = &self.bundle.episode {
            if self.frame + 1 == episode.steps.len() && self.frame > 0 {
                self.frame = 0;
                self.playback_time = episode.steps[0].time_seconds;
            }
        }
        self.playing = true;
        self.last_time = None;
        Ok(())
    }
    pub fn pause(&mut self) {
        self.playing = false;
        self.last_time = None;
    }
    pub fn playing(&self) -> bool {
        self.playing
    }
    pub fn seek(&mut self, frame: usize) -> Result<(), String> {
        let episode = self
            .bundle
            .episode
            .as_ref()
            .ok_or("Open a recording before choosing a frame")?;
        let step = episode
            .steps
            .get(frame)
            .ok_or("Choose a frame in this recording")?;
        self.frame = frame;
        self.playback_time = step.time_seconds;
        self.last_time = None;
        Ok(())
    }
    pub fn tick(&mut self, now_seconds: f64) -> bool {
        if !self.active() || !self.playing || !now_seconds.is_finite() {
            self.last_time = None;
            return false;
        }
        let Some(previous) = self.last_time.replace(now_seconds) else {
            return false;
        };
        self.playback_time += (now_seconds - previous).clamp(0.0, 0.1);
        let episode = self.bundle.episode.as_ref().unwrap();
        let frame = episode
            .steps
            .partition_point(|s| s.time_seconds <= self.playback_time)
            .saturating_sub(1);
        let changed = frame != self.frame;
        self.frame = frame;
        if frame + 1 == episode.steps.len() {
            self.playing = false;
        }
        changed
    }
    pub fn step(&self) -> Option<&Step> {
        self.bundle.episode.as_ref().map(|e| &e.steps[self.frame])
    }
    pub fn recorded_camera(&self) -> bool {
        self.camera != "orbit"
            && self.bundle.episode.as_ref().is_some_and(|episode| {
                episode
                    .steps
                    .iter()
                    .any(|step| step.sensors.contains_key(&self.camera))
            })
    }
    pub fn sensor(&self) -> Option<&robot_lab::document::Sensor> {
        let episode = self.bundle.episode.as_ref()?;
        if episode.mode == robot_lab::document::EpisodeMode::Demonstration {
            return episode.steps[self.frame].sensors.get(&self.camera);
        }
        episode.steps[..=self.frame]
            .iter()
            .rev()
            .find_map(|step| step.sensors.get(&self.camera))
    }
    pub fn displayed_scene(&self) -> Scene {
        let mut scene = self.bundle.scene.clone();
        if self.bundle.episode.is_none() {
            scene.robots[0].joints.extend(self.draft.clone());
        }
        scene
    }
    pub fn pose_value(&mut self, joint: &str, value: f64) -> Result<(), String> {
        if self.bundle.episode.is_some() {
            return Err("Open the scene to edit a joint".into());
        }
        let mut pose = self.bundle.scene.robots[0].joints.clone();
        pose.extend(self.draft.clone());
        pose.insert(joint.into(), value);
        self.bundle
            .robot
            .validate_pose(&pose)
            .map_err(|e| e.to_string())?;
        self.draft.insert(joint.into(), value);
        Ok(())
    }
    pub fn choose_link(&mut self, link: &str) -> Result<(), String> {
        if !self.bundle.robot.links.iter().any(|l| l.name == link) {
            return Err("Choose a link in this robot".into());
        }
        self.link = Some(link.into());
        if let Some(joint) = self.bundle.robot.joints.iter().find(|joint| {
            joint.child == link && joint.kind != robot_lab::document::JointKind::Fixed
        }) {
            self.joint = joint.name.clone();
        }
        Ok(())
    }
    pub fn choose_camera(&mut self, camera: &str) -> Result<(), String> {
        if camera != "orbit" && !self.bundle.scene.cameras.iter().any(|c| c.id == camera) {
            return Err("Choose a camera in this scene".into());
        }
        self.camera = camera.into();
        Ok(())
    }
    pub fn readout(&self) -> String {
        let source = self
            .step()
            .map_or(&self.bundle.scene.robots[0].joints, |s| &s.joints);
        let value = self
            .draft
            .get(&self.joint)
            .or_else(|| source.get(&self.joint))
            .copied()
            .unwrap_or(0.0);
        let unit = self
            .bundle
            .robot
            .joints
            .iter()
            .find(|j| j.name == self.joint)
            .map_or("rad", |j| {
                if j.kind == robot_lab::document::JointKind::Prismatic {
                    "m"
                } else {
                    "rad"
                }
            });
        let warning = self
            .step()
            .is_some_and(|s| s.limit_violations.contains(&self.joint));
        let limits = self
            .bundle
            .robot
            .joints
            .iter()
            .find(|j| j.name == self.joint)
            .and_then(|j| j.limits)
            .map_or(String::new(), |l| {
                format!(" · Limits {:.3} to {:.3} {unit}", l.lower, l.upper)
            });
        let link = self
            .link
            .as_ref()
            .map_or(String::new(), |link| format!(" · Link: {link}"));
        format!(
            "{}: {value:.3} {unit}{}",
            self.joint,
            format!(
                "{limits}{link}{}",
                if warning { " · Outside limits" } else { "" }
            )
        )
    }
}

/// Reject a corrupt private asset before passing it to the description reader.
pub fn verify_asset(asset: &robot_lab::document::Asset, bytes: &[u8]) -> Result<(), String> {
    use sha2::{Digest, Sha256};
    if bytes.len() as u64 != asset.bytes || format!("{:x}", Sha256::digest(bytes)) != asset.sha256 {
        return Err("The robot file changed. Import it again".into());
    }
    Ok(())
}

pub fn camera_pose(pose: &Pose) -> (DVec3, DVec3, DVec3) {
    let rotation = glam::DQuat::from_array(pose.rotation);
    let eye = DVec3::from_array(pose.position);
    // Camera frames use the conventional right/down/forward optical axes.
    (eye, eye + rotation * DVec3::Z, rotation * (-DVec3::Y))
}

pub fn document_kind(document: &Document) -> &'static str {
    document.kind()
}
