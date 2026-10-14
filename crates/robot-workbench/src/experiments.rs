//! Execute an immutable experiment on the configured native host. The
//! runner owns all physical bodies, control, cameras, and durable stop state.
use crate::{host::Host, record::Cameras};
use robot_lab::{
    Document, Reference,
    document::{Build, Experiment},
};
use robot_policy::{artifacts::Artifact, controller::NativeController};
use robot_sim::{
    jobs::{Book, Worker},
    mujoco::{LibraryPin, Model, Runtime, Servo},
    runner::{ActionChunk, Backend, Controller, Mode, Observation, Progress, Runner, Scripted},
};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub library: LibraryPin,
    pub policy: Option<Policy>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub id: String,
    pub root: PathBuf,
    pub sha256: String,
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
pub fn backend(
    host: &Host,
    scene: &Reference,
    timestep: f64,
    config: &Config,
) -> Result<Cameras<Model>, String> {
    let model = physical_model(host, scene, timestep, config)?;
    let bundle = host.bundle(scene)?;
    let (_, robot) =
        robot_description::pin::load_pinned(&host.catalog).map_err(|e| e.to_string())?;
    Cameras::new(model, host.store.clone(), bundle.scene, Arc::new(robot))
        .map_err(|e| e.to_string())
}
fn physical_model(
    host: &Host,
    scene: &Reference,
    timestep: f64,
    config: &Config,
) -> Result<Model, String> {
    let bundle = host.bundle(scene)?;
    if bundle.episode.is_some() {
        return Err("Choose a saved scene for this experiment".into());
    }
    let checked =
        robot_sim::import::pinned_urdf(&host.store, &host.catalog).map_err(|e| e.to_string())?;
    if checked != bundle.robot {
        return Err("Import the checked robot before running this experiment".into());
    }
    let runtime = Runtime::open(&config.library, host.store.root()).map_err(|e| e.to_string())?;
    let model = runtime
        .arm_scene(
            &host.catalog,
            &bundle.robot.description,
            &bundle.scene,
            timestep,
            Servo {
                kp: 100.,
                kd: 4.,
                effort: 2.4,
                speed: 2.,
            },
        )
        .map_err(|e| e.to_string())?;
    Ok(model)
}
fn backend_build(
    host: &Host,
    scene: &Reference,
    timestep: f64,
    config: &Config,
) -> Result<Build, String> {
    let model = physical_model(host, scene, timestep, config)?;
    crate::record::camera_build(model.build(), &host.bundle(scene)?.scene)
        .map_err(|e| e.to_string())
}
pub enum Control {
    Scripted(Scripted),
    Act(NativeController),
}
impl Controller for Control {
    fn build(&self) -> Build {
        match self {
            Self::Scripted(c) => c.build(),
            Self::Act(c) => c.build(),
        }
    }
    fn reset(&mut self, seed: u64) -> robot_sim::Result<()> {
        match self {
            Self::Scripted(c) => c.reset(seed),
            Self::Act(c) => c.reset(seed),
        }
    }
    fn infer(&mut self, observation: Observation<'_>) -> robot_sim::Result<ActionChunk> {
        match self {
            Self::Scripted(c) => c.infer(observation),
            Self::Act(c) => c.infer(observation),
        }
    }
}
pub struct Session {
    host: Host,
    pub runner: Runner<Cameras<Model>, Control>,
    pub worker: Worker,
    book: Book,
}
impl Session {
    pub fn start(
        host: Host,
        reference: Reference,
        seed: u64,
        job: &str,
        mode: Mode,
        config: &Config,
    ) -> Result<Self, String> {
        let Document::Experiment(experiment) = host
            .store
            .get(&reference)
            .map_err(|e| e.to_string())?
            .content
        else {
            return Err("Choose a saved robot experiment".into());
        };
        let bundle = host.bundle(&experiment.scene)?;
        let backend = backend(
            &host,
            &experiment.scene,
            experiment.timestep_seconds,
            config,
        )?;
        let controller = match &experiment.controller {
            robot_lab::document::Controller::Scripted { build, .. } => {
                Control::Scripted(Scripted::open(&host.store, build).map_err(|e| e.to_string())?)
            }
            robot_lab::document::Controller::Policy { .. }
            | robot_lab::document::Controller::Service { .. } => {
                let configured = config
                    .policy
                    .as_ref()
                    .ok_or("Configure a policy before starting this experiment")?;
                let artifact = SavedPolicy::open(configured)?;
                Control::Act(artifact.controller(
                    host.store.clone(),
                    &bundle.robot,
                    &bundle.scene,
                    &experiment,
                )?)
            }
            robot_lab::document::Controller::Manual { .. } => {
                return Err("Use the manual recorder for this experiment".into());
            }
        };
        let runner = Runner::new(
            backend,
            controller,
            reference.clone(),
            experiment,
            seed,
            mode,
        )
        .map_err(|e| e.to_string())?;
        let book = Book::open(host.store.clone()).map_err(|e| e.to_string())?;
        book.create(job, reference, seed, mode)
            .map_err(|e| e.to_string())?;
        let worker = book.start(job, &runner).map_err(|e| e.to_string())?;
        Ok(Self {
            host,
            runner,
            worker,
            book,
        })
    }
    pub fn tick(&mut self, single_step: bool) -> Result<Progress, String> {
        self.worker
            .tick(&mut self.runner, single_step, &self.host.author, now())
            .map_err(|e| e.to_string())
    }
    pub fn stop(&mut self) -> Result<Reference, String> {
        self.runner.stop();
        self.tick(false)?;
        self.worker
            .job()
            .episode
            .clone()
            .ok_or("The stopped attempt couldn't be saved".into())
    }
    pub fn reset(&mut self, job: &str, seed: u64) -> Result<Reference, String> {
        let previous = self.stop()?;
        self.runner.reset(seed).map_err(|e| e.to_string())?;
        self.book
            .create(
                job,
                self.runner.recording().episode.experiment.clone(),
                seed,
                self.runner.mode(),
            )
            .map_err(|e| e.to_string())?;
        self.worker = self
            .book
            .start(job, &self.runner)
            .map_err(|e| e.to_string())?;
        Ok(previous)
    }
    pub fn run(&mut self) -> Result<Reference, String> {
        self.runner.resume();
        while !self.runner.finished() {
            self.tick(false)?;
        }
        self.worker
            .job()
            .episode
            .clone()
            .ok_or("The attempt couldn't be saved".into())
    }
}

/// A baseline sequence is saved under the same content hash its controller
/// build names, so executing the document can recover its exact inputs.
pub fn scripted_experiment(
    host: &Host,
    scene: &Reference,
    seeds: Vec<u64>,
    max_steps: u32,
    frames: Vec<std::collections::BTreeMap<String, f64>>,
    hold: u32,
    config: &Config,
    document: &str,
) -> Result<Reference, String> {
    let bundle = host.bundle(scene)?;
    let controller = Scripted::new(frames, hold).map_err(|e| e.to_string())?;
    controller.save(&host.store).map_err(|e| e.to_string())?;
    let backend = backend_build(host, scene, 1. / 240., config)?;
    let experiment = Experiment {
        scene: scene.clone(),
        task: cube_task(),
        controller: robot_lab::document::Controller::Scripted {
            build: controller.build(),
            robot: bundle.scene.robots[0].id.clone(),
            joints: bundle.scene.robots[0].joints.keys().cloned().collect(),
        },
        backend,
        timestep_seconds: 1. / 240.,
        control_every_steps: 8,
        sensor_every_steps: 8,
        seeds,
        limits: limits(max_steps),
    };
    host.store
        .save(
            document,
            None,
            Document::Experiment(experiment),
            &host.author,
            "Saved scripted experiment",
            now(),
        )
        .map_err(|e| e.to_string())
}
enum SavedPolicy {
    Act(Artifact),
    Smol(robot_policy::smol_artifacts::Artifact),
    Service(robot_policy::service::Configured),
}
impl SavedPolicy {
    fn open(config: &Policy) -> Result<Self, String> {
        Ok(match config.id.as_str() {
            "act" => {
                Self::Act(Artifact::open(&config.root, &config.sha256).map_err(|e| e.to_string())?)
            }
            "smolvla" => Self::Smol(
                robot_policy::smol_artifacts::Artifact::open(&config.root, &config.sha256)
                    .map_err(|e| e.to_string())?,
            ),
            "service" => Self::Service(
                robot_policy::service::Configured::open(&config.root, &config.sha256)
                    .map_err(|e| e.to_string())?,
            ),
            _ => return Err("Choose act, smolvla, or service for this policy".into()),
        })
    }
    fn binding(&self) -> &robot_policy::artifacts::Binding {
        match self {
            Self::Act(a) => &a.pin.binding,
            Self::Smol(a) => &a.pin.binding,
            Self::Service(a) => &a.manifest.binding,
        }
    }
    fn build(&self) -> Build {
        match self {
            Self::Act(a) => a.build.clone(),
            Self::Smol(a) => a.build.clone(),
            Self::Service(a) => a.manifest.build.clone(),
        }
    }
    fn controller(
        self,
        store: robot_lab::Store,
        robot: &robot_lab::document::Robot,
        scene: &robot_lab::document::Scene,
        experiment: &Experiment,
    ) -> Result<NativeController, String> {
        match self {
            Self::Act(a) => NativeController::new(a, store, robot, scene, experiment),
            Self::Smol(a) => NativeController::new_smol(a, store, robot, scene, experiment),
            Self::Service(a) => a.controller(store, robot, scene, experiment),
        }
        .map_err(|e| e.to_string())
    }
}
pub fn policy_experiment(
    host: &Host,
    scene: &Reference,
    seeds: Vec<u64>,
    max_steps: u32,
    config: &Config,
    document: &str,
) -> Result<Reference, String> {
    let configured = config
        .policy
        .as_ref()
        .ok_or("Configure a policy before creating this experiment")?;
    let artifact = SavedPolicy::open(configured)?;
    let binding = artifact.binding();
    let bundle = host.bundle(scene)?;
    let backend = backend_build(host, scene, binding.timestep_seconds, config)?;
    let robot = bundle.scene.robots[0].id.clone();
    let joints = binding
        .actions
        .joints
        .iter()
        .map(|j| j.joint.clone())
        .collect();
    let cameras = binding.cameras.iter().map(|c| c.id.clone()).collect();
    let instruction = binding.instruction.clone();
    let controller = if let SavedPolicy::Service(service) = &artifact {
        let manifest = host
            .store
            .put_asset(
                std::io::Cursor::new(&service.manifest_bytes),
                "application/json",
                2 << 20,
            )
            .map_err(|e| e.to_string())?;
        robot_lab::document::Controller::Service {
            build: artifact.build(),
            manifest,
            robot,
            joints,
            cameras,
            instruction,
        }
    } else {
        let (weights, normalization) = match &artifact {
            SavedPolicy::Act(a) => (&a.pin.weights, &a.pin.normalization),
            SavedPolicy::Smol(a) => (&a.pin.weights, &a.pin.normalization),
            _ => unreachable!(),
        };
        let weights = host
            .store
            .put_asset(
                std::fs::File::open(configured.root.join(weights)).map_err(|e| e.to_string())?,
                "application/x-safetensors",
                2 << 30,
            )
            .map_err(|e| e.to_string())?;
        let normalization = host
            .store
            .put_asset(
                std::fs::File::open(configured.root.join(normalization))
                    .map_err(|e| e.to_string())?,
                "application/json",
                1 << 20,
            )
            .map_err(|e| e.to_string())?;
        robot_lab::document::Controller::Policy {
            build: artifact.build(),
            weights,
            normalization,
            robot,
            joints,
            cameras,
            instruction,
        }
    };
    let experiment = Experiment {
        scene: scene.clone(),
        task: cube_task(),
        controller,
        backend,
        timestep_seconds: binding.timestep_seconds,
        control_every_steps: binding.control_every_steps,
        sensor_every_steps: binding.control_every_steps,
        seeds,
        limits: limits(max_steps),
    };
    binding
        .validate_scene(&bundle.robot, &bundle.scene, &experiment)
        .map_err(|e| e.to_string())?;
    host.store
        .save(
            document,
            None,
            Document::Experiment(experiment),
            &host.author,
            "Saved policy experiment",
            now(),
        )
        .map_err(|e| e.to_string())
}
pub fn cube_task() -> robot_lab::document::CubeTask {
    robot_lab::document::CubeTask {
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
    }
}

/// Freeze per-seed scene versions before either controller sees a case.
/// Both sides use the same placements, physical build, and sampling rates.
pub fn study(
    host: &Host,
    scene: &Reference,
    name: &str,
    config: &Config,
    output: &Path,
) -> Result<serde_json::Value, String> {
    use gym::robot::{Case, Partition, Suite};
    if !output.is_absolute() || output.exists() {
        return Err("Choose a new absolute folder for this study".into());
    }
    let parent = std::fs::canonicalize(output.parent().ok_or("Choose a study folder")?)
        .map_err(|e| e.to_string())?;
    if parent.ancestors().any(|p| p.join(".git").exists()) {
        return Err("Keep robot studies outside the Git checkout".into());
    }
    if name.is_empty()
        || name.len() > 24
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err("Use a study name of up to 24 letters, numbers, dashes, or underscores".into());
    }
    let template = host.bundle(scene)?;
    let mut cases = std::collections::BTreeMap::new();
    for (partition, seeds) in [
        (Partition::Training, vec![7]),
        (Partition::Validation, (11..=20).collect()),
        (Partition::HeldOut, (31..=40).collect()),
    ] {
        let mut pinned = Vec::new();
        for seed in seeds {
            let reference = if seed == 7 {
                scene.clone()
            } else {
                let mut varied = template.scene.clone();
                let cube = varied
                    .objects
                    .iter_mut()
                    .find(|o| o.id == "cube")
                    .ok_or("This study needs its cube")?;
                let mut state: u64 = seed ^ 0x6a09_e667_f3bc_c909;
                let mut random = || {
                    state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
                    let mut value = state;
                    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
                    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
                    value ^= value >> 31;
                    ((value >> 11) as f64 / (1_u64 << 53) as f64 * 2. - 1.) * 0.015
                };
                cube.geometry.pose.position[0] += random();
                cube.geometry.pose.position[2] += random();
                host.store
                    .save(
                        &format!("{name}-case-{seed}"),
                        None,
                        Document::Scene(varied),
                        &host.author,
                        "Saved test placement",
                        now(),
                    )
                    .map_err(|e| e.to_string())?
            };
            let backend = backend_build(host, &reference, 1. / 240., config)?;
            pinned.push(Case {
                seed,
                scene: reference,
                backend,
            });
        }
        cases.insert(partition, pinned);
    }
    let configured = config
        .policy
        .as_ref()
        .ok_or("Choose the policy checkpoint for this comparison")?;
    let artifact = SavedPolicy::open(configured)?;
    let mut suite = Suite {
        schema: gym::robot::SUITE_SCHEMA.into(),
        name: name.into(),
        scene_template: scene.clone(),
        task: cube_task(),
        metric: gym::robot::metric_build(),
        timestep_seconds: 1. / 240.,
        control_every_steps: 8,
        sensor_every_steps: 8,
        max_steps: 241,
        cases,
        exposure: std::collections::BTreeMap::from([(artifact.build().sha256, vec![7])]),
        digest: String::new(),
    };
    suite.digest = suite.compute_digest()?;
    suite.validate()?;
    std::fs::create_dir(output).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(output, std::fs::Permissions::from_mode(0o700))
            .map_err(|e| e.to_string())?;
    }
    std::fs::write(
        output.join("suite.json"),
        serde_json::to_vec_pretty(&suite).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    let store = gym::store::Store::at(output.join("results.jsonl"));
    let mut controllers = Vec::new();
    for case in &suite.cases[&Partition::Validation] {
        let home = host.bundle(&case.scene)?.scene.robots[0].joints.clone();
        let mut frames = Vec::new();
        for angle in [0., 0.1, 0.2, 0.3, 0.4, 0.3, 0.2, 0.1, 0., -0.1] {
            let mut pose = home.clone();
            pose.insert("shoulder_pan".into(), angle);
            frames.push(pose);
        }
        let scripted = scripted_experiment(
            host,
            &case.scene,
            vec![case.seed],
            241,
            frames,
            3,
            config,
            &format!("{name}-script-{}", case.seed),
        )?;
        let policy = policy_experiment(
            host,
            &case.scene,
            vec![case.seed],
            241,
            config,
            &format!("{name}-act-{}", case.seed),
        )?;
        for (side, experiment) in [("script", scripted), ("act", policy)] {
            let mut session = Session::start(
                host.clone(),
                experiment,
                case.seed,
                &format!("{name}-{side}-{}", case.seed),
                Mode::Batch,
                config,
            )?;
            let episode = session.run()?;
            let row = gym::robot::row(&host.store, &suite, Partition::Validation, &episode)?;
            if !controllers.contains(&row.controller) {
                controllers.push(row.controller.clone());
            }
            store.append(&row).map_err(|e| e.to_string())?;
        }
    }
    let rows = gym::robot::verify_rows(&host.store, &suite, &output.join("results.jsonl"))?;
    if controllers.len() != 2 {
        return Err("This study needs two fixed controller builds".into());
    }
    let comparison = gym::robot::compare(
        &suite,
        Partition::Validation,
        &controllers[0],
        &controllers[1],
        &rows,
    )?;
    let result = serde_json::to_value(&comparison).map_err(|e| e.to_string())?;
    std::fs::write(
        output.join("comparison.json"),
        serde_json::to_vec_pretty(&result).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(result)
}
fn limits(max_steps: u32) -> robot_lab::document::ResourceLimits {
    robot_lab::document::ResourceLimits {
        max_steps,
        wall_seconds: 300,
        max_artifact_bytes: 128 << 20,
        inference_timeout_ms: 1000,
    }
}

pub fn cli(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut values = std::collections::BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or("Choose a value for this experiment option")?;
        if ![
            "--root",
            "--project",
            "--experiment",
            "--job",
            "--seed",
            "--library",
            "--library-sha",
            "--policy",
            "--policy-sha",
            "--policy-kind",
        ]
        .contains(&flag.as_str())
            || values.insert(flag, value).is_some()
        {
            return Err("Choose supported experiment options once each".into());
        }
    }
    let value = |key: &str| {
        values
            .get(key)
            .cloned()
            .ok_or_else(|| format!("Choose {key} for this experiment"))
    };
    let store = robot_lab::Store::open(Path::new(&value("--root")?), &value("--project")?)?;
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let host = Host::open(store, &catalog, "http://127.0.0.1:8794")?;
    let config = Config {
        library: LibraryPin {
            path: value("--library")?.into(),
            sha256: value("--library-sha")?,
        },
        policy: if let Some(root) = values.get("--policy") {
            Some(Policy {
                id: values
                    .get("--policy-kind")
                    .cloned()
                    .unwrap_or_else(|| "act".into()),
                root: root.into(),
                sha256: value("--policy-sha")?,
            })
        } else {
            None
        },
    };
    let reference = value("--experiment")?;
    let (document, version) = reference
        .split_once('@')
        .ok_or("Choose the exact experiment as DOCUMENT@VERSION")?;
    let reference = Reference {
        document: document.into(),
        version: version.into(),
    };
    let seed = value("--seed")?.parse::<u64>()?;
    let mut session = Session::start(
        host,
        reference,
        seed,
        &value("--job")?,
        Mode::Batch,
        &config,
    )?;
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({"episode":session.run()?}))?
    );
    Ok(())
}

pub fn recover_cli(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut values = std::collections::BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or("Choose a value for this recovery option")?;
        if !["--root", "--project", "--job"].contains(&flag.as_str())
            || values.insert(flag, value).is_some()
        {
            return Err("Choose supported recovery options once each".into());
        }
    }
    let value = |key: &str| {
        values
            .get(key)
            .ok_or_else(|| format!("Choose {key} for this interrupted run"))
    };
    let store = robot_lab::Store::open(Path::new(value("--root")?), value("--project")?)?;
    let job = Book::open(store)?.recover(value("--job")?, "Local user", now())?;
    println!(
        "{}",
        serde_json::json!({"job":job.id,"status":job.status,"episode":job.episode})
    );
    Ok(())
}

pub async fn service_cli(args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let mut values = std::collections::BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or("Choose a value for this policy-service option")?;
        if ![
            "--root",
            "--project",
            "--experiment",
            "--policy",
            "--policy-sha",
            "--policy-kind",
            "--port",
            "--manifest-output",
        ]
        .contains(&flag.as_str())
            || values.insert(flag, value).is_some()
        {
            return Err("Choose supported policy-service options once each".into());
        }
    }
    let value = |key: &str| {
        values
            .get(key)
            .cloned()
            .ok_or_else(|| format!("Choose {key} for this policy service"))
    };
    let store = robot_lab::Store::open(Path::new(&value("--root")?), &value("--project")?)?;
    let catalog = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
    let host = Host::open(store.clone(), &catalog, "http://127.0.0.1:8794")?;
    let reference = value("--experiment")?;
    let (document, version) = reference
        .split_once('@')
        .ok_or("Choose the exact experiment as DOCUMENT@VERSION")?;
    let Document::Experiment(experiment) = store
        .get(&Reference {
            document: document.into(),
            version: version.into(),
        })?
        .content
    else {
        return Err("Choose a saved policy experiment".into());
    };
    let bundle = host.bundle(&experiment.scene)?;
    let token = std::env::var("ROBOT_POLICY_TOKEN")
        .map_err(|_| "Set ROBOT_POLICY_TOKEN before serving this policy")?;
    let root = value("--policy")?;
    let expected = value("--policy-sha")?;
    let server = match values
        .get("--policy-kind")
        .map(String::as_str)
        .unwrap_or("act")
    {
        "act" => robot_policy::service::Server::act(
            Artifact::open(Path::new(&root), &expected)?,
            store,
            &bundle.robot,
            &bundle.scene,
            &experiment,
            token,
        )?,
        "smolvla" => robot_policy::service::Server::smol(
            robot_policy::smol_artifacts::Artifact::open(Path::new(&root), &expected)?,
            store,
            &bundle.robot,
            &bundle.scene,
            &experiment,
            token,
        )?,
        _ => return Err("Choose act or smolvla for this policy service".into()),
    };
    let path = PathBuf::from(value("--manifest-output")?);
    if !path.is_absolute()
        || path
            .parent()
            .ok_or("Choose a manifest folder")?
            .canonicalize()?
            .ancestors()
            .any(|p| p.join(".git").exists())
    {
        return Err("Save the policy-service manifest outside the Git checkout".into());
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options.open(path)?;
    file.write_all(server.manifest_bytes())?;
    file.sync_all()?;
    let port = values
        .get("--port")
        .map(|p| p.parse::<u16>())
        .transpose()?
        .unwrap_or(0);
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port)).await?;
    eprintln!(
        "{}",
        serde_json::json!({"url":format!("http://127.0.0.1:{}",listener.local_addr()?.port()),"manifest_sha256":robot_policy::artifacts::digest(server.manifest_bytes())})
    );
    axum::serve(listener, Arc::new(server).router()).await?;
    Ok(())
}
