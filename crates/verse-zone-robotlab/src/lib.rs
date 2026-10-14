//! A local Robot Lab room over the same scenes and recordings as chat.
//! Rendering does not start a controller. A separately configured host owns
//! simulation, while this room reads checked poses and sends explicit controls.
pub mod draw;
#[cfg(not(target_arch = "wasm32"))]
pub mod local;
use glam::{Mat4, Vec3};
use robot_description::Robot;
use robot_lab::{
    Reference,
    document::{Camera, Geometry, Object, Pose, RobotInstance, Scene, Shape, Step},
    view::Bundle,
};
use serde::Serialize;
use std::{collections::BTreeMap, sync::Arc};
use verse_core::{world::World, zone::Intent};
use verse_pbr::{
    mesh::{Mesh, Vertex},
    pbr,
};
pub const HALF_EXTENT: f32 = 20.;
pub const STAGE: Vec3 = Vec3::new(-0.8, 0., 3.);
pub const RETURN_PORTAL: Vec3 = Vec3::new(-8., 0., -6.);
pub const SPAWN: Vec3 = Vec3::new(0., 0., 0.5);
pub const SPAWN_YAW: f32 = 0.;
const SPEEDS: [f64; 5] = [0.25, 0.5, 1., 2., 4.];
#[derive(Clone, Debug, Serialize)]
pub struct JointReadout {
    pub name: String,
    pub position: f64,
    pub degrees: Option<f64>,
}
#[derive(Clone, Debug, Serialize)]
pub struct Snapshot {
    pub title: String,
    pub mode: String,
    pub subject: Option<Reference>,
    pub frame: u32,
    pub frames: u32,
    pub time_seconds: f64,
    pub paused: bool,
    pub speed: f64,
    pub active: bool,
    pub selected: String,
    pub joints: Vec<JointReadout>,
    pub cameras: Vec<String>,
    pub job: Option<String>,
    pub loading: bool,
    pub error: Option<String>,
}
pub struct Room {
    robot: Arc<Robot>,
    models: draw::RobotMeshes,
    scene: Scene,
    bundle: Option<Bundle>,
    live: Option<Step>,
    frame: usize,
    time: f64,
    speed: usize,
    knob: usize,
    joint: usize,
    playing: bool,
    active: bool,
    mesh: Mesh,
    pub error: Option<String>,
    #[cfg(not(target_arch = "wasm32"))]
    pub source: Option<local::Source>,
}
impl Room {
    pub fn new(robot: Arc<Robot>, scene: Scene) -> Result<Self, String> {
        scene.validate().map_err(|e| e.to_string())?;
        if robot.movable().count() == 0 {
            return Err("Choose an arm with movable joints".into());
        }
        let mut room = Self {
            models: draw::RobotMeshes::new(&robot)?,
            robot,
            scene,
            bundle: None,
            live: None,
            frame: 0,
            time: 0.,
            speed: 2,
            knob: 0,
            joint: 0,
            playing: false,
            active: true,
            mesh: Mesh::default(),
            error: None,
            #[cfg(not(target_arch = "wasm32"))]
            source: None,
        };
        room.redraw()?;
        Ok(room)
    }
    pub fn open(&mut self, robot: Arc<Robot>, bundle: Bundle) -> Result<(), String> {
        bundle.validate()?;
        // Check drawing before replacing an existing room selection.
        draw::document_scene(
            &robot,
            &bundle.scene,
            bundle.episode.as_ref().map(|e| &e.steps[0]),
        )?;
        if !Arc::ptr_eq(&self.robot, &robot) {
            self.models = draw::RobotMeshes::new(&robot)?;
        }
        self.robot = robot;
        self.scene = bundle.scene.clone();
        self.bundle = Some(bundle);
        self.live = None;
        self.frame = 0;
        self.time = 0.;
        self.playing = false;
        self.joint = 0;
        self.error = None;
        self.redraw()
    }
    fn step(&self) -> Option<&Step> {
        self.live.as_ref().or_else(|| {
            self.bundle
                .as_ref()?
                .episode
                .as_ref()?
                .steps
                .get(self.frame)
        })
    }
    fn frame_count(&self) -> usize {
        self.bundle
            .as_ref()
            .and_then(|b| b.episode.as_ref())
            .map_or(1, |e| e.steps.len())
    }
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.playing = false;
        }
    }
    pub fn tick(&mut self, dt: f32) {
        #[cfg(not(target_arch = "wasm32"))]
        self.poll_source(dt);
        if self.active && self.playing && self.live.is_none() && dt.is_finite() && dt > 0. {
            if let Some(episode) = self.bundle.as_ref().and_then(|b| b.episode.as_ref()) {
                self.time = (self.time + f64::from(dt.min(0.25)) * SPEEDS[self.speed])
                    .min(episode.steps.last().unwrap().time_seconds);
                let frame = episode
                    .steps
                    .partition_point(|s| s.time_seconds <= self.time)
                    .saturating_sub(1);
                if frame != self.frame {
                    self.frame = frame;
                    if let Err(e) = self.redraw() {
                        self.error = Some(e);
                        self.playing = false;
                    }
                }
                if self.frame + 1 == self.frame_count() {
                    self.playing = false;
                }
            }
        }
    }
    pub fn intent(&mut self, intent: Intent) -> Result<(), String> {
        self.error = None;
        match intent {
            Intent::KnobPrev => self.knob = self.knob.saturating_sub(1),
            Intent::KnobNext => self.knob = (self.knob + 1).min(self.knobs() - 1),
            Intent::Increase | Intent::Decrease => self.adjust(intent == Intent::Increase)?,
            Intent::Pause => {
                #[cfg(not(target_arch = "wasm32"))]
                if self.live.is_some() {
                    return self.control(if self.source.as_ref().is_some_and(|s| s.paused) {
                        "resume"
                    } else {
                        "pause"
                    });
                }
                if self.frame_count() > 1 {
                    if self.frame + 1 == self.frame_count() {
                        self.frame = 0;
                        self.time = 0.;
                        self.redraw()?;
                    }
                    self.playing = !self.playing;
                }
            }
            Intent::Step => {
                #[cfg(not(target_arch = "wasm32"))]
                if self.live.is_some() {
                    return self.control("step");
                }
                self.playing = false;
                self.frame = (self.frame + 1).min(self.frame_count() - 1);
                self.time = self.step().map_or(0., |s| s.time_seconds);
                self.redraw()?;
            }
            Intent::Reset => {
                #[cfg(not(target_arch = "wasm32"))]
                if self.live.is_some() {
                    return self.control("reset");
                }
                if self.bundle.as_ref().is_none_or(|b| b.episode.is_none()) {
                    #[cfg(not(target_arch = "wasm32"))]
                    if let (Some(source), Some(bundle)) = (&mut self.source, &self.bundle) {
                        return source.reset_pose(bundle.subject.clone());
                    }
                    self.scene.robots[0].joints = self
                        .robot
                        .movable()
                        .map(|j| {
                            (
                                j.name.clone(),
                                j.limit.map_or(0., |l| 0_f64.clamp(l.lower, l.upper)),
                            )
                        })
                        .collect();
                }
                self.playing = false;
                self.frame = 0;
                self.time = 0.;
                if let Some(b) = &self.bundle {
                    self.scene = b.scene.clone();
                }
                self.redraw()?;
            }
            Intent::Grab => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    return self.start();
                }
                #[cfg(target_arch = "wasm32")]
                {
                    return Err("Connect the lab's computer before starting a run".into());
                }
            }
            Intent::Release => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    return self.control("stop");
                }
                #[cfg(target_arch = "wasm32")]
                {
                    return Err("Connect the lab's computer before stopping a run".into());
                }
            }
            _ => return Err("Choose a Robot Lab control".into()),
        }
        Ok(())
    }
    fn knobs(&self) -> usize {
        let base = if self.step().is_some() { 3 } else { 4 };
        #[cfg(not(target_arch = "wasm32"))]
        if self.source.as_ref().is_some_and(|s| s.can_run()) {
            return base + 1;
        }
        base
    }
    fn adjust(&mut self, up: bool) -> Result<(), String> {
        #[cfg(not(target_arch = "wasm32"))]
        if self.knob == self.knobs() - 1 && self.source.as_ref().is_some_and(|s| s.can_run()) {
            if let Some(source) = &mut self.source {
                return source.pick_experiment(up);
            }
        }
        match self.knob {
            0 =>
            {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(source) = &mut self.source {
                    return source.pick(up);
                }
            }
            1 => {
                self.speed = if up {
                    (self.speed + 1).min(SPEEDS.len() - 1)
                } else {
                    self.speed.saturating_sub(1)
                }
            }
            2 => {
                let n = self.robot.movable().count();
                self.joint = if up {
                    (self.joint + 1) % n
                } else {
                    (self.joint + n - 1) % n
                };
            }
            3 => {
                if self.step().is_some() {
                    return Err("Open a scene to change a joint pose".into());
                }
                let joint = self
                    .robot
                    .movable()
                    .nth(self.joint)
                    .ok_or("Choose a robot joint")?;
                let limits = joint.limit.ok_or("Choose a joint with position limits")?;
                let current = self.scene.robots[0].joints[&joint.name];
                let value =
                    (current + if up { 0.05 } else { -0.05 }).clamp(limits.lower, limits.upper);
                #[cfg(not(target_arch = "wasm32"))]
                if let (Some(source), Some(bundle)) = (&mut self.source, &self.bundle) {
                    return source.pose(bundle.subject.clone(), joint.name.clone(), value);
                }
                self.scene.robots[0]
                    .joints
                    .insert(joint.name.clone(), value);
                self.redraw()?;
            }
            4 =>
            {
                #[cfg(not(target_arch = "wasm32"))]
                if let Some(source) = &mut self.source {
                    return source.pick_experiment(up);
                }
            }
            _ => (),
        }
        Ok(())
    }
    pub fn dynamic(&self) -> &Mesh {
        &self.mesh
    }
    fn redraw(&mut self) -> Result<(), String> {
        let scene = draw::props_scene(&self.robot, &self.scene, self.step())?;
        let mut vertices = scene.props;
        let models = self
            .models
            .frame(&self.robot, &self.scene, self.step(), STAGE)?;
        for vertex in &mut vertices {
            vertex.pos = (Vec3::from_array(vertex.pos) + STAGE).to_array();
        }
        self.mesh = Mesh {
            lit: vertices,
            instances: vec![models],
            ..Default::default()
        };
        let mut light = pbr::Neon::neutral(0.);
        light.key = Some(pbr::Key {
            dir: Vec3::new(0.4, 0.8, -0.5).normalize(),
            illuminance: 5000.,
            angular_radius: 0.035,
            rim_dir: Vec3::new(-0.5, 0.5, 0.5).normalize(),
            rim_illuminance: 1500.,
            rim_angular_radius: 0.08,
            sky: 420.,
            ground: 60.,
            ev100: 10.,
            shadow_center: STAGE + Vec3::Y,
            shadow_half: 4.,
            shadow_distance: None,
            cache_far_shadows: false,
        });
        self.mesh.neon = Some(light);
        Ok(())
    }
    pub fn snapshot(&self) -> Snapshot {
        let step = self.step();
        let positions = step.map_or(&self.scene.robots[0].joints, |s| &s.joints);
        let selected = match self.knob {
            0 => "Scene or attempt".to_owned(),
            1 => format!("Playback {}×", SPEEDS[self.speed]),
            2 => format!(
                "Joint {}",
                self.robot
                    .movable()
                    .nth(self.joint)
                    .map_or("", |j| j.name.as_str())
            ),
            3 if self.step().is_none() => "Joint position".into(),
            _ => {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.source
                        .as_ref()
                        .map_or("Experiment".into(), |s| s.experiment_label())
                }
                #[cfg(target_arch = "wasm32")]
                {
                    "Experiment".into()
                }
            }
        };
        let mode = if self.live.is_some() {
            "Simulation"
        } else if self
            .bundle
            .as_ref()
            .and_then(|b| b.episode.as_ref())
            .is_some_and(|e| e.mode == robot_lab::document::EpisodeMode::Demonstration)
        {
            "Recorded demonstration"
        } else if self.bundle.as_ref().is_some_and(|b| b.episode.is_some()) {
            "Saved simulation"
        } else {
            "Inspection"
        };
        Snapshot {
            title: self
                .bundle
                .as_ref()
                .and_then(|b| b.episode.as_ref())
                .map_or_else(
                    || self.scene.name.clone(),
                    |e| format!("{} · attempt {}", self.scene.name, e.seed),
                ),
            mode: mode.into(),
            subject: self.bundle.as_ref().map(|b| b.subject.clone()),
            frame: step.map_or(self.frame as u32, |s| s.index),
            frames: self.frame_count() as u32,
            time_seconds: step.map_or(0., |s| s.time_seconds),
            paused: {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    if self.live.is_some() {
                        self.source.as_ref().is_none_or(|s| s.paused)
                    } else {
                        !self.playing
                    }
                }
                #[cfg(target_arch = "wasm32")]
                {
                    !self.playing
                }
            },
            speed: SPEEDS[self.speed],
            active: self.active,
            selected,
            joints: self
                .robot
                .movable()
                .map(|j| JointReadout {
                    name: j.name.clone(),
                    position: positions[&j.name],
                    degrees: if matches!(j.kind, robot_description::JointKind::Prismatic) {
                        None
                    } else {
                        Some(positions[&j.name].to_degrees())
                    },
                })
                .collect(),
            cameras: step.map_or(Vec::new(), |s| s.sensors.keys().cloned().collect()),
            job: {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.source.as_ref().and_then(|s| s.job.clone())
                }
                #[cfg(target_arch = "wasm32")]
                {
                    None
                }
            },
            loading: {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.source.as_ref().is_some_and(|s| s.pending)
                }
                #[cfg(target_arch = "wasm32")]
                {
                    false
                }
            },
            error: self.error.clone(),
        }
    }
    pub fn caption(&self) -> String {
        let s = self.snapshot();
        let joint = s.joints.get(self.joint);
        let angle = joint.map_or(String::new(), |j| {
            format!(
                "{} {:.1}{}",
                j.name.replace('_', " "),
                j.degrees.unwrap_or(j.position),
                if j.degrees.is_some() { "°" } else { " m" }
            )
        });
        format!(
            "{} · {}\n{} · {:.3} s\n{}",
            s.title, s.mode, s.selected, s.time_seconds, angle
        )
    }
    pub fn controls(&self) -> Vec<(&'static str, Intent)> {
        let mut controls = vec![
            ("Prev", Intent::KnobPrev),
            ("Next", Intent::KnobNext),
            ("-", Intent::Decrease),
            ("+", Intent::Increase),
            ("Reset", Intent::Reset),
        ];
        if self.frame_count() > 1 {
            controls.push((if self.playing { "Pause" } else { "Play" }, Intent::Pause));
            controls.push(("Step", Intent::Step));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if self.source.as_ref().is_some_and(|s| s.can_run()) {
            if self.live.is_some() {
                controls.push((
                    if self.source.as_ref().is_some_and(|s| s.paused) {
                        "Run"
                    } else {
                        "Pause"
                    },
                    Intent::Pause,
                ));
                controls.push(("Step", Intent::Step));
                controls.push(("Stop", Intent::Release));
            } else if self
                .source
                .as_ref()
                .is_some_and(|s| s.selected_experiment().is_some())
            {
                controls.push(("Start", Intent::Grab));
            }
        }
        controls.push(("Plaza", Intent::Return));
        controls
    }
    pub fn world_scene(&self) -> Result<World, String> {
        Self::world(&self.robot)
    }
    pub fn world(robot: &Robot) -> Result<World, String> {
        let mut mesh = Mesh::default();
        for n in -20..=20 {
            let t = n as f32;
            for (a, b) in [
                (Vec3::new(t, 0., -20.), Vec3::new(t, 0., 20.)),
                (Vec3::new(-20., 0., t), Vec3::new(20., 0., t)),
            ] {
                line(&mut mesh, a, b, [0.045, 0.10, 0.14]);
            }
        }
        let a = Vec3::new(-20., -0.004, -20.);
        let b = Vec3::new(20., -0.004, -20.);
        let c = Vec3::new(20., -0.004, 20.);
        let d = Vec3::new(-20., -0.004, 20.);
        for p in [a, c, b, a, d, c] {
            mesh.faces.push(Vertex {
                pos: p.to_array(),
                color: [0.015, 0.023, 0.035],
                fog: 1.,
            });
        }
        let values = robot
            .movable()
            .map(|j| j.limit.map_or(0., |l| 0_f64.clamp(l.lower, l.upper)))
            .collect::<Vec<_>>();
        let second = draw::scene(robot, &values).map_err(|e| e.to_string())?;
        for mut v in second.props.into_iter().chain(second.robot) {
            v.pos = (Vec3::from_array(v.pos) + STAGE + Vec3::new(2.3, 0., 0.)).to_array();
            mesh.lit.push(v);
        }
        // Colored blocks and hollow cups are inspection scenery, outside the
        // task's physical scene. They cannot affect a scored cube placement.
        for (x, color) in [(-0.1, [0.2, 0.6, 1.]), (0.1, [0.25, 0.8, 0.4])] {
            let at = STAGE + Vec3::new(2.3 + x, 0.79, 0.23);
            for triangle in draw::cuboid(Mat4::from_scale(Vec3::splat(0.08))) {
                for p in triangle {
                    mesh.faces.push(Vertex {
                        pos: (at + p).to_array(),
                        color,
                        fog: 1.,
                    });
                }
            }
        }
        for x in [1.6, 1.8] {
            cup(&mut mesh, STAGE + Vec3::new(x, 0.75, -0.23));
        }
        for (a, b) in [
            (
                RETURN_PORTAL + Vec3::new(-1., 0., 0.),
                RETURN_PORTAL + Vec3::new(-1., 2.4, 0.),
            ),
            (
                RETURN_PORTAL + Vec3::new(1., 0., 0.),
                RETURN_PORTAL + Vec3::new(1., 2.4, 0.),
            ),
            (
                RETURN_PORTAL + Vec3::new(-1., 2.4, 0.),
                RETURN_PORTAL + Vec3::new(1., 2.4, 0.),
            ),
        ] {
            line(&mut mesh, a, b, [0.3, 0.7, 1.]);
        }
        Ok(World {
            mesh,
            blockers: vec![
                verse_world::social::controller::Footprint {
                    min: [STAGE.x - 0.6, STAGE.z - 0.38],
                    max: [STAGE.x + 0.6, STAGE.z + 0.38],
                },
                verse_world::social::controller::Footprint {
                    min: [STAGE.x + 1.7, STAGE.z - 0.38],
                    max: [STAGE.x + 2.9, STAGE.z + 0.38],
                },
            ],
        })
    }
}
fn line(mesh: &mut Mesh, a: Vec3, b: Vec3, color: [f32; 3]) {
    for p in [a, b] {
        mesh.lines.push(Vertex {
            pos: p.to_array(),
            color,
            fog: 1.,
        });
    }
}
fn cup(mesh: &mut Mesh, at: Vec3) {
    for i in 0..24 {
        let a = i as f32 * std::f32::consts::TAU / 24.;
        let b = (i + 1) as f32 * std::f32::consts::TAU / 24.;
        let point =
            |angle: f32, y: f32, r: f32| at + Vec3::new(angle.cos() * r, y, angle.sin() * r);
        for quad in [
            [
                point(a, 0., 0.035),
                point(b, 0., 0.035),
                point(b, 0.09, 0.04),
                point(a, 0.09, 0.04),
            ],
            [
                point(a, 0.09, 0.04),
                point(b, 0.09, 0.04),
                point(b, 0.09, 0.032),
                point(a, 0.09, 0.032),
            ],
            [
                point(a, 0.09, 0.032),
                point(b, 0.09, 0.032),
                point(b, 0.01, 0.028),
                point(a, 0.01, 0.028),
            ],
        ] {
            for p in [quad[0], quad[1], quad[2], quad[0], quad[2], quad[3]] {
                mesh.faces.push(Vertex {
                    pos: p.to_array(),
                    color: [0.65, 0.7, 0.76],
                    fog: 1.,
                });
            }
        }
    }
}
pub fn inspection_scene(robot: &Robot, description_hash: &str) -> Scene {
    Scene {
        name: "SO-101 inspection".into(),
        robots: vec![RobotInstance {
            id: "so101".into(),
            robot: Reference {
                document: "so101-description".into(),
                version: description_hash.into(),
            },
            pose: Pose {
                position: [0., 0.75, 0.],
                ..Default::default()
            },
            joints: robot
                .movable()
                .map(|j| {
                    (
                        j.name.clone(),
                        j.limit.map_or(0., |l| 0_f64.clamp(l.lower, l.upper)),
                    )
                })
                .collect::<BTreeMap<_, _>>(),
        }],
        objects: vec![
            Object {
                id: "table".into(),
                geometry: Geometry {
                    pose: Pose {
                        position: [0., 0.725, 0.],
                        ..Default::default()
                    },
                    shape: Shape::Box {
                        size: [1.2, 0.05, 0.75],
                    },
                    rgba: [0.25, 0.3, 0.35, 1.],
                },
                mesh: None,
                collider: Some(Shape::Box {
                    size: [1.2, 0.05, 0.75],
                }),
                mass: 0.,
                friction: 0.6,
            },
            Object {
                id: "cube".into(),
                geometry: Geometry {
                    pose: Pose {
                        position: [0.22, 0.775, -0.1],
                        ..Default::default()
                    },
                    shape: Shape::Box { size: [0.05; 3] },
                    rgba: [1., 0.15, 0.1, 1.],
                },
                mesh: None,
                collider: Some(Shape::Box { size: [0.05; 3] }),
                mass: 0.05,
                friction: 0.6,
            },
        ],
        lights: vec![],
        cameras: vec![Camera {
            id: "front".into(),
            pose: Pose::default(),
            width: 640,
            height: 400,
            intrinsics: [500., 500., 320., 200.],
            robot_link: None,
        }],
    }
}
