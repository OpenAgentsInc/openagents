//! Manual simulation recording with separately configured, saved cameras.
use crate::host::Host;
use robot_lab::document::{
    self, Build, Camera, Experiment, JointKind, Outcome, Scene, Sensor, Step,
};
use robot_lab::{Document, Reference, Store};
use robot_sim::{
    jobs::{Book, Worker},
    mujoco::{LibraryPin, Model, Runtime, Servo},
    runner::{ActionChunk, Backend, Controller, Mode, Observation, Runner, Snapshot},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{cell::RefCell, collections::BTreeMap, path::Path, rc::Rc, sync::Arc};

fn error(message: impl Into<String>) -> robot_sim::Error {
    robot_sim::Error::Invalid(message.into())
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub struct Manual {
    initial: BTreeMap<String, f64>,
    target: Rc<RefCell<BTreeMap<String, f64>>>,
}
impl Controller for Manual {
    fn build(&self) -> Build {
        Build {
            name: "robot-manual-v1".into(),
            sha256: robot_lab::store::digest(include_bytes!("record.rs")),
        }
    }
    fn reset(&mut self, _seed: u64) -> robot_sim::Result<()> {
        *self.target.borrow_mut() = self.initial.clone();
        Ok(())
    }
    fn infer(&mut self, observation: Observation<'_>) -> robot_sim::Result<ActionChunk> {
        Ok(ActionChunk {
            observation_step: observation.step,
            expires_step: observation.step,
            actions: vec![self.target.borrow().clone()],
        })
    }
}

/// Captures happen on the simulation's sensor schedule, using only the
/// scene's calibrated cameras. Inspection orbit state is never an input.
pub struct Cameras<B> {
    inner: B,
    build: Build,
    store: Store,
    scene: Scene,
    robot: Arc<robot_description::Robot>,
    renderers: BTreeMap<String, verse::render::Offscreen>,
}
impl<B: Backend> Cameras<B> {
    pub fn new(
        inner: B,
        store: Store,
        scene: Scene,
        robot: Arc<robot_description::Robot>,
    ) -> robot_sim::Result<Self> {
        scene.validate()?;
        if scene.robots.len() != 1 {
            return Err(error("Choose one robot for this recording"));
        }
        if scene.cameras.is_empty()
            || scene.cameras.len() > 4
            || scene
                .cameras
                .iter()
                .any(|c| c.width > 2048 || c.height > 2048)
        {
            return Err(error(
                "Choose 1 to 4 recording cameras, at most 2048 pixels per side",
            ));
        }
        let build = Build {
            name: "robot-verse-camera-v1".into(),
            sha256: robot_lab::store::digest(&serde_json::to_vec(&(
                inner.build(),
                &scene,
                env!("ROBOT_RENDER_BUILD_SHA"),
                "colored-triangles-v1",
            ))?),
        };
        let mut renderers = BTreeMap::new();
        for camera in &scene.cameras {
            let renderer = verse::render::Offscreen::new(
                camera.width,
                camera.height,
                &verse::mesh::Mesh::default(),
                &verse::ui::Atlas::new(16.),
                verse::zones::atmosphere(verse::zones::ZoneId::Plaza),
            )
            .map_err(error)?;
            renderers.insert(camera.id.clone(), renderer);
        }
        Ok(Self {
            inner,
            build,
            store,
            scene,
            robot,
            renderers,
        })
    }
}
impl<B: Backend> Backend for Cameras<B> {
    fn build(&self) -> Build {
        self.build.clone()
    }
    fn timestep(&self) -> f64 {
        self.inner.timestep()
    }
    fn reset(&mut self, seed: u64) -> robot_sim::Result<Snapshot> {
        self.inner.reset(seed)
    }
    fn snapshot(&self) -> robot_sim::Result<Snapshot> {
        self.inner.snapshot()
    }
    fn validate_action(&self, values: &BTreeMap<String, f64>) -> robot_sim::Result<()> {
        self.inner.validate_action(values)
    }
    fn apply(&mut self, values: &BTreeMap<String, f64>) -> robot_sim::Result<()> {
        self.inner.apply(values)
    }
    fn step(&mut self) -> robot_sim::Result<Snapshot> {
        self.inner.step()
    }
    fn sensors(&mut self, index: u32) -> robot_sim::Result<BTreeMap<String, Sensor>> {
        let snapshot = self.inner.snapshot()?;
        let time = f64::from(index) * self.inner.timestep();
        let step = Step {
            index,
            time_seconds: time,
            joints: snapshot.joints,
            actions: BTreeMap::new(),
            observation_step: None,
            objects: snapshot.objects,
            contacts: snapshot.contacts,
            sensors: BTreeMap::new(),
            inference_ms: None,
            limit_violations: snapshot.limit_violations,
        };
        let visual = verse::robot_lab::document_scene(&self.robot, &self.scene, Some(&step))
            .map_err(error)?;
        let mut vertices = visual.props;
        vertices.extend(visual.robot);
        let mesh = verse::robot_lab::shaded_mesh(&vertices);
        let mut sensors = self.inner.sensors(index)?;
        for camera in &self.scene.cameras {
            let (view_proj, eye) =
                camera_view(&self.robot, &self.scene, &step, camera).map_err(error)?;
            let pixels = self
                .renderers
                .get_mut(&camera.id)
                .unwrap()
                .render(
                    verse::render::View { view_proj, eye },
                    &mesh,
                    &verse::ui::UiBatch::default(),
                )
                .map_err(error)?;
            let mut bytes = Vec::new();
            {
                let mut encoder = png::Encoder::new(&mut bytes, camera.width, camera.height);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                encoder
                    .write_header()
                    .and_then(|mut writer| writer.write_image_data(&pixels))
                    .map_err(|_| error("The camera frame couldn't be saved"))?;
            }
            let asset = self
                .store
                .put_asset(std::io::Cursor::new(bytes), "image/png", 16 << 20)?;
            sensors.insert(
                camera.id.clone(),
                Sensor {
                    asset,
                    timestamp_seconds: time,
                    frame: u64::from(index),
                },
            );
        }
        Ok(sensors)
    }
}

/// Pinhole projection preserves focal lengths and the principal point. A
/// mounted camera follows its named link in the same checked source frame.
pub fn camera_view(
    robot: &robot_description::Robot,
    scene: &Scene,
    step: &Step,
    camera: &Camera,
) -> Result<(glam::Mat4, glam::Vec3), String> {
    let (mut eye, mut target, mut up) = crate::view::camera_pose(&camera.pose);
    if let Some((instance, link)) = &camera.robot_link {
        let mounted = scene
            .robots
            .first()
            .ok_or("Choose a scene with a robot for this camera")?;
        if &mounted.id != instance {
            return Err("Choose a camera on this robot".into());
        }
        let index = robot.link(link).ok_or("Choose an existing camera link")?;
        let values = robot
            .movable()
            .map(|j| {
                step.joints
                    .get(&j.name)
                    .copied()
                    .ok_or("Record every camera joint")
            })
            .collect::<Result<Vec<_>, _>>()?;
        let frames = robot.forward_observed(&values).map_err(|e| e.to_string())?;
        let place = &mounted.pose;
        let transform = glam::DMat4::from_rotation_translation(
            glam::DQuat::from_array(place.rotation),
            glam::DVec3::from_array(place.position),
        ) * robot_description::z_up_to_y_up()
            * frames[index];
        eye = transform.transform_point3(eye);
        target = transform.transform_point3(target);
        up = transform.transform_vector3(up);
    }
    let [fx, fy, cx, cy] = camera.intrinsics.map(|n| n as f32);
    let w = camera.width as f32;
    let h = camera.height as f32;
    let near = 0.02;
    let far = 100.;
    let projection = glam::Mat4::from_cols_array(&[
        2. * fx / w,
        0.,
        0.,
        0.,
        0.,
        2. * fy / h,
        0.,
        0.,
        1. - 2. * cx / w,
        2. * cy / h - 1.,
        far / (near - far),
        -1.,
        0.,
        0.,
        near * far / (near - far),
        0.,
    ]);
    let view = glam::Mat4::look_at_rh(eye.as_vec3(), target.as_vec3(), up.as_vec3());
    Ok((projection * view, eye.as_vec3()))
}

pub struct Session {
    host: Host,
    robot: document::Robot,
    target: Rc<RefCell<BTreeMap<String, f64>>>,
    runner: Runner<Cameras<Model>, Manual>,
    worker: Worker,
}
impl Session {
    pub fn start(
        host: Host,
        subject: Reference,
        job: &str,
        pin: &LibraryPin,
        seed: u64,
        max_steps: u32,
    ) -> Result<Self, String> {
        if !(9..=100_000).contains(&max_steps) {
            return Err("Record between 9 and 100000 simulation steps".into());
        }
        let bundle = host.bundle(&subject)?;
        if bundle.episode.is_some() {
            return Err("Choose a scene to start recording".into());
        }
        let checked = robot_sim::import::pinned_urdf(&host.store, &host.catalog)
            .map_err(|e| e.to_string())?;
        if bundle.robot != checked {
            return Err("Import the checked robot again before recording".into());
        }
        let (source, robot) =
            robot_description::pin::load_pinned(&host.catalog).map_err(|e| e.to_string())?;
        if bundle.robot.source.revision != source.revision
            || bundle.robot.source.repository != source.repository
            || bundle.robot.description != source.description
            || source.files.iter().any(|file| {
                bundle
                    .robot
                    .source
                    .files
                    .get(&file.path)
                    .is_none_or(|asset| asset.sha256 != file.sha256)
            })
        {
            return Err("Use this host's checked robot description for recording".into());
        }
        let runtime = Runtime::open(pin, host.store.root()).map_err(|e| e.to_string())?;
        let backend = runtime
            .arm_scene(
                &host.catalog,
                &source.description,
                &bundle.scene,
                1. / 240.,
                Servo {
                    kp: 100.,
                    kd: 4.,
                    effort: 2.4,
                    speed: 2.,
                },
            )
            .map_err(|e| e.to_string())?;
        let backend = Cameras::new(
            backend,
            host.store.clone(),
            bundle.scene.clone(),
            Arc::new(robot),
        )
        .map_err(|e| e.to_string())?;
        let initial = bundle.scene.robots[0].joints.clone();
        let target = Rc::new(RefCell::new(initial.clone()));
        let controller = Manual {
            initial,
            target: target.clone(),
        };
        let task = document::CubeTask {
            version: Build {
                name: "cube-placement-v1".into(),
                sha256: robot_lab::store::digest(b"cube-placement-v1"),
            },
            object: "cube".into(),
            target_min: [0.3, 0.75, -0.15],
            target_max: [0.4, 0.85, -0.05],
            stable_seconds: 0.2,
            max_linear_speed: 0.01,
            max_angular_speed: 0.01,
        };
        let experiment = Experiment {
            scene: subject,
            task,
            controller: document::Controller::Manual {
                build: controller.build(),
                robot: bundle.scene.robots[0].id.clone(),
                joints: controller.initial.keys().cloned().collect(),
            },
            backend: backend.build(),
            timestep_seconds: 1. / 240.,
            control_every_steps: 8,
            sensor_every_steps: 8,
            seeds: vec![seed],
            limits: document::ResourceLimits {
                max_steps,
                wall_seconds: 3600,
                max_artifact_bytes: 128 << 20,
                inference_timeout_ms: 1000,
            },
        };
        let reference = host
            .store
            .save(
                &format!("recording-{job}"),
                None,
                Document::Experiment(experiment.clone()),
                &host.author,
                "Started manual recording",
                now(),
            )
            .map_err(|e| e.to_string())?;
        let runner = Runner::new(
            backend,
            controller,
            reference.clone(),
            experiment,
            seed,
            Mode::Interactive,
        )
        .map_err(|e| e.to_string())?;
        let book = Book::open(host.store.clone()).map_err(|e| e.to_string())?;
        book.create(job, reference, seed, Mode::Interactive)
            .map_err(|e| e.to_string())?;
        let worker = book.start(job, &runner).map_err(|e| e.to_string())?;
        Ok(Self {
            host,
            robot: bundle.robot,
            target,
            runner,
            worker,
        })
    }
    pub fn pose(&mut self, joints: &BTreeMap<String, f64>) -> Result<(), String> {
        if self.runner.finished() {
            return Err("This recording has ended".into());
        }
        let mut target = self.target.borrow().clone();
        target.extend(joints.clone());
        self.robot
            .validate_pose(&target)
            .map_err(|e| e.to_string())?;
        *self.target.borrow_mut() = target;
        Ok(())
    }
    pub fn step(&mut self, steps: u32) -> Result<Value, String> {
        if !(1..=240).contains(&steps) {
            return Err("Advance between 1 and 240 simulation steps".into());
        }
        for _ in 0..steps {
            if self.runner.finished() {
                break;
            }
            self.worker
                .tick(&mut self.runner, true, &self.host.author, now())
                .map_err(|e| e.to_string())?;
        }
        Ok(
            json!({"step":self.runner.recording().episode.steps.last(),"finished":self.runner.finished(),"episode":self.worker.job().episode}),
        )
    }
    pub fn finish(mut self) -> Result<Reference, String> {
        self.runner.stop();
        self.worker
            .tick(&mut self.runner, false, &self.host.author, now())
            .map_err(|e| e.to_string())?;
        self.worker
            .job()
            .episode
            .clone()
            .ok_or("The recording couldn't be saved".into())
    }
    pub fn interrupt(self) -> Result<Reference, String> {
        let book = Book::open(self.host.store.clone()).map_err(|e| e.to_string())?;
        let id = self.worker.job().id.clone();
        let author = self.host.author.clone();
        drop(self);
        book.recover(&id, &author, now())
            .map_err(|e| e.to_string())?
            .episode
            .ok_or("The partial recording couldn't be saved".into())
    }
}

/// Align each observed state and fresh image with the command applied on the
/// following physical step. The final state without a next command is excluded.
pub fn export_episode(
    host: &Host,
    subject: &Reference,
    destination: &Path,
) -> Result<robot_data::export::Export, String> {
    use robot_data::export::{Episode as DataEpisode, Frame, Spec, Termination, Unit};
    let bundle = host.bundle(subject)?;
    let episode = bundle
        .episode
        .as_ref()
        .ok_or("Choose a saved recording to export")?;
    if episode.mode != document::EpisodeMode::Simulation
        || matches!(episode.outcome, Outcome::InProgress)
    {
        return Err("Finish or stop the simulated recording before exporting".into());
    }
    let Document::Experiment(experiment) = host
        .store
        .get(&episode.experiment)
        .map_err(|e| e.to_string())?
        .content
    else {
        return Err("This recording is missing its experiment".into());
    };
    let interval = experiment.control_every_steps as usize;
    let fps = 1. / (experiment.timestep_seconds * interval as f64);
    let rate = fps.round();
    if (fps - rate).abs() > 1e-6 || !(1.0..=1000.).contains(&rate) {
        return Err("Export a recording with a whole-number control frame rate".into());
    }
    let joints: Vec<_> = bundle
        .robot
        .joints
        .iter()
        .filter(|j| j.kind != JointKind::Fixed)
        .collect();
    let mut frames = Vec::new();
    for origin in (0..episode.steps.len().saturating_sub(1)).step_by(interval) {
        let state = &episode.steps[origin];
        let command = &episode.steps[origin + 1];
        let mut cameras = BTreeMap::new();
        for camera in &bundle.scene.cameras {
            let sensor = state
                .sensors
                .get(&camera.id)
                .ok_or("A required recording camera is missing")?;
            if (sensor.timestamp_seconds - state.time_seconds).abs()
                > experiment.timestep_seconds * 0.5
            {
                return Err("A required camera frame is missing at this control step".into());
            }
            cameras.insert(
                format!("observation.images.{}", camera.id),
                host.store
                    .asset_bytes(&sensor.asset, 16 << 20)
                    .map_err(|e| e.to_string())?,
            );
        }
        frames.push(Frame {
            state: joints
                .iter()
                .map(|j| {
                    state
                        .joints
                        .get(&j.name)
                        .copied()
                        .map(|v| v as f32)
                        .ok_or("Record every joint before exporting")
                })
                .collect::<Result<_, _>>()?,
            action: joints
                .iter()
                .map(|j| {
                    command
                        .actions
                        .get(&j.name)
                        .copied()
                        .map(|v| v as f32)
                        .ok_or("Record every commanded joint before exporting")
                })
                .collect::<Result<_, _>>()?,
            cameras,
        });
    }
    let license = bundle
        .robot
        .source
        .files
        .get("LICENSE")
        .ok_or("Include the source robot's license before exporting")?;
    let license_text = String::from_utf8(
        host.store
            .asset_bytes(license, 1 << 20)
            .map_err(|e| e.to_string())?,
    )
    .map_err(|_| "Include a text license")?;
    let job = subject
        .document
        .strip_prefix("run-")
        .and_then(|id| Book::open(host.store.clone()).ok()?.get(id).ok())
        .filter(|job| job.episode.as_ref() == Some(subject));
    let termination = if job
        .as_ref()
        .is_some_and(|job| job.status == robot_sim::jobs::Status::Interrupted)
    {
        Termination::Interrupted
    } else {
        match &episode.outcome {
            Outcome::Complete | Outcome::Success => Termination::Complete,
            Outcome::Stopped { .. } => Termination::Stopped,
            Outcome::Failed { reason } => Termination::Failed {
                reason: reason.clone(),
            },
            Outcome::InProgress => unreachable!(),
        }
    };
    let spec = Spec {
        robot_type: "so101_follower".into(),
        joints: joints.iter().map(|j| format!("{}.pos", j.name)).collect(),
        units: joints
            .iter()
            .map(|j| {
                if j.kind == JointKind::Prismatic {
                    Unit::Meters
                } else {
                    Unit::Radians
                }
            })
            .collect(),
        fps: rate as u32,
        task: "Move the cube into the target".into(),
        cameras: bundle
            .scene
            .cameras
            .iter()
            .map(|c| (format!("observation.images.{}", c.id), [c.width, c.height]))
            .collect(),
        license: bundle.robot.source.license.to_lowercase(),
        license_text,
        notice: format!(
            "Robot Lab simulation recording. Robot description and visual meshes: {} at {}, {}. The source license is included in LICENSE. Camera profile: colored-triangles-v1. No physical robot was controlled.",
            bundle.robot.source.repository,
            bundle.robot.source.revision,
            bundle.robot.source.license
        ),
        source: json!({"episode":subject,"experiment":episode.experiment,"scene":bundle.scene_reference,"robot":bundle.robot_reference,"backend":experiment.backend,"controller":experiment.controller,"joint_units":"radians-or-meters","state_action_alignment":"observed-state-and-command-on-following-physical-step","original_timestep_seconds":experiment.timestep_seconds,"control_every_steps":interval,"image_conversion":"saved-PNG-to-RGB-PNG"}),
    };
    robot_data::export::write(
        destination,
        &spec,
        &[DataEpisode {
            frames,
            termination,
        }],
    )
    .map_err(|e| e.to_string())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Pose {
        joints: BTreeMap<String, f64>,
        steps: u32,
    },
    Step {
        steps: u32,
    },
    Finish,
}

pub async fn publish_cli(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut options = BTreeMap::new();
    let mut allow_public = false;
    let mut args = args.into_iter();
    while let Some(key) = args.next() {
        if key == "--allow-public" {
            allow_public = true;
            continue;
        }
        let value = args
            .next()
            .ok_or("Choose a value for this publication option")?;
        if !["--dataset", "--repo", "--parent", "--manifest-sha"].contains(&key.as_str())
            || options.insert(key, value).is_some()
        {
            return Err("Choose supported publication options once each".into());
        }
    }
    let required = |name: &str| {
        options
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Choose {name} before publishing"))
    };
    let snapshot = robot_data::publish::prepare(
        Path::new(&required("--dataset")?),
        &required("--repo")?,
        &required("--parent")?,
        &required("--manifest-sha")?,
    )?;
    let token = std::env::var("HF_TOKEN")
        .map_err(|_| "Set HF_TOKEN before publishing the reviewed dataset")?;
    let reply = robot_data::publish::Hub::new()?
        .publish(&snapshot, &token, allow_public)
        .await?;
    println!("{}", serde_json::to_string(&reply)?);
    Ok(())
}

pub fn export_cli(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut options = BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(key) = args.next() {
        let value = args.next().ok_or("Choose a value for this export option")?;
        if !["--root", "--project", "--episode", "--output"].contains(&key.as_str())
            || options.insert(key, value).is_some()
        {
            return Err("Choose supported export options once each".into());
        }
    }
    let required = |name: &str| {
        options
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Choose {name} before exporting"))
    };
    let store = Store::open(Path::new(&required("--root")?), &required("--project")?)?;
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let host = Host::open(store, &catalog, "http://127.0.0.1:8794")?;
    let episode = required("--episode")?;
    let (document, version) = episode
        .split_once('@')
        .ok_or("Choose the exact episode as DOCUMENT@VERSION")?;
    let subject = Reference {
        document: document.into(),
        version: version.into(),
    };
    subject.validate()?;
    let output = required("--output")?;
    let manifest = export_episode(&host, &subject, Path::new(&output))?;
    println!(
        "{}",
        serde_json::to_string(
            &json!({"schema":manifest.schema,"episode":subject,"folder":output})
        )?
    );
    Ok(())
}

pub fn cli(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::{BufRead, Read, Write};
    let mut options = BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(key) = args.next() {
        let value = args
            .next()
            .ok_or("Choose a value for this recording option")?;
        if ![
            "--root",
            "--project",
            "--scene",
            "--job",
            "--library",
            "--library-sha",
            "--seed",
            "--max-steps",
            "--export",
            "--player",
        ]
        .contains(&key.as_str())
            || options.insert(key, value).is_some()
        {
            return Err("Choose supported recording options once each".into());
        }
    }
    let required = |name: &str| {
        options
            .get(name)
            .cloned()
            .ok_or_else(|| format!("Choose {name} before recording"))
    };
    let store = Store::open(Path::new(&required("--root")?), &required("--project")?)?;
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let host = Host::open(
        store,
        &catalog,
        options
            .get("--player")
            .map_or("http://127.0.0.1:8794", String::as_str),
    )?;
    let scene = required("--scene")?;
    let (document, version) = scene
        .split_once('@')
        .ok_or("Choose the exact scene as DOCUMENT@VERSION")?;
    let subject = Reference {
        document: document.into(),
        version: version.into(),
    };
    subject.validate()?;
    let pin = LibraryPin {
        path: required("--library")?.into(),
        sha256: required("--library-sha")?,
    };
    let seed = options.get("--seed").map_or(Ok(7), |v| v.parse::<u64>())?;
    let max = options
        .get("--max-steps")
        .map_or(Ok(2401), |v| v.parse::<u32>())?;
    let mut session = Session::start(host.clone(), subject, &required("--job")?, &pin, seed, max)?;
    let mut input = std::io::BufReader::new(std::io::stdin());
    let mut line = Vec::new();
    let reference = loop {
        line.clear();
        if input.by_ref().take(65537).read_until(b'\n', &mut line)? == 0 {
            break session.interrupt()?;
        }
        if line.len() > 65536 {
            return Err("Keep recording commands smaller than 64 KB".into());
        }
        let command: Command = serde_json::from_slice(&line)?;
        let reply = match command {
            Command::Finish => break session.finish()?,
            Command::Pose { joints, steps } => {
                session.pose(&joints)?;
                session.step(steps)?
            }
            Command::Step { steps } => session.step(steps)?,
        };
        println!("{}", serde_json::to_string(&reply)?);
        std::io::stdout().flush()?;
        if reply["finished"] == true {
            break session.finish()?;
        }
    };
    if let Some(destination) = options.get("--export") {
        let export = export_episode(&host, &reference, Path::new(destination))?;
        println!(
            "{}",
            serde_json::to_string(&json!({"export":export.schema,"folder":destination}))?
        );
    }
    println!(
        "{}",
        serde_json::to_string(&json!({"episode":reference,"card":host.card(&reference,None)?}))?
    );
    Ok(())
}
