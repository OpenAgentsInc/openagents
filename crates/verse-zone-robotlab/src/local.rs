//! The explicitly selected private lab. File and HTTP work runs off the frame
//! thread; responses are applied only to the selection that requested them.
use super::*;
use robot_lab::{Document, Operation, Reply, Store};
use robot_sim::jobs::{Book, Status};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub root: PathBuf,
    pub project: String,
    pub subject: Option<Reference>,
    pub control_url: Option<String>,
    pub watch_job: Option<String>,
}
#[derive(Clone)]
pub struct Entry {
    pub reference: Reference,
    pub label: String,
}
#[derive(Clone)]
pub struct ExperimentEntry {
    pub reference: Reference,
    pub scene: Reference,
    pub seed: u64,
    pub label: String,
}
pub struct Source {
    pub entries: Vec<Entry>,
    pub experiments: Vec<ExperimentEntry>,
    picked: usize,
    experiment: usize,
    sender: mpsc::SyncSender<Request>,
    receiver: mpsc::Receiver<Response>,
    revision: u64,
    poll: f32,
    pub pending: bool,
    pub job: Option<String>,
    handle: Option<String>,
    pub paused: bool,
    pub owned_interactive: bool,
    control: bool,
}
enum Task {
    Open(Reference),
    Pose(Reference, String, f64),
    ResetPose(Reference),
    Watch(String),
    Control(String, serde_json::Value),
    Start(Reference, u64, String),
}
struct Request {
    revision: u64,
    task: Task,
}
struct Response {
    revision: u64,
    result: Result<Update, String>,
}
pub struct Loaded {
    pub robot: Arc<Robot>,
    pub bundle: Bundle,
}
enum Update {
    Loaded(Loaded),
    Live {
        loaded: Loaded,
        step: Step,
        paused: bool,
        finished: bool,
    },
    Started(String),
    Controlled(Option<String>),
}
struct Files {
    store: Store,
    robots: BTreeMap<String, Arc<Robot>>,
    control: Option<Control>,
}
struct Control {
    base: String,
    client: reqwest::blocking::Client,
    enabled: bool,
}
impl Control {
    fn new(url: &str, project: &str) -> Result<Self, String> {
        let parsed = url::Url::parse(url).map_err(|_| "Choose the local Robot Lab address")?;
        if parsed.scheme() != "http"
            || !matches!(parsed.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"))
            || parsed.path() != "/"
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err("Choose the local Robot Lab address".into());
        }
        let client = reqwest::blocking::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(std::time::Duration::from_secs(12))
            .build()
            .map_err(|_| "The lab connection couldn't be prepared")?;
        let base = url.trim_end_matches('/').to_owned();
        let response = client
            .get(format!("{base}/api/docs"))
            .send()
            .map_err(|_| "The local Robot Lab didn't answer")?;
        let mut bytes = Vec::new();
        use std::io::Read;
        response
            .take((1 << 20) + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "The lab list couldn't be read")?;
        if bytes.len() > 1 << 20 {
            return Err("Open a smaller lab list".into());
        }
        let docs: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| "The lab list couldn't be read")?;
        if docs["project"] != project {
            return Err("Choose this project's Robot Lab service".into());
        }
        let response = client
            .get(format!("{base}/api/tools"))
            .send()
            .map_err(|_| "The local Robot Lab didn't answer")?;
        let mut bytes = Vec::new();
        response
            .take((1 << 20) + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "The lab controls couldn't be read")?;
        if bytes.len() > 1 << 20 {
            return Err("The lab returned too much control data".into());
        }
        let tools: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| "The lab controls couldn't be read")?;
        let enabled = tools["tools"]
            .as_array()
            .is_some_and(|t| t.iter().any(|t| t["name"] == "robot_run"));
        Ok(Self {
            base,
            client,
            enabled,
        })
    }
    fn tool(&self, name: &str, value: serde_json::Value) -> Result<serde_json::Value, String> {
        let response = self
            .client
            .post(format!("{}/api/tool/{name}", self.base))
            .json(&value)
            .send()
            .map_err(|_| "The lab command didn't finish in time")?;
        let success = response.status().is_success();
        use std::io::Read;
        let mut bytes = Vec::new();
        response
            .take((1 << 20) + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| "The lab command couldn't be read")?;
        if bytes.len() > 1 << 20 {
            return Err("The lab command returned too much data".into());
        }
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| "The lab command couldn't be read")?;
        if !success {
            return Err(value["error"]
                .as_str()
                .unwrap_or("The lab couldn't finish this command")
                .chars()
                .take(512)
                .collect());
        }
        Ok(value)
    }
}
impl Files {
    fn load(&mut self, reference: &Reference) -> Result<Loaded, String> {
        let bundle = self.store.view(reference)?;
        let key = robot_lab::store::digest(
            &serde_json::to_vec(&bundle.robot.source.files).map_err(|e| e.to_string())?,
        );
        let robot = if let Some(robot) = self.robots.get(&key) {
            robot.clone()
        } else {
            let asset = bundle
                .robot
                .source
                .files
                .get(&bundle.robot.description)
                .ok_or("The robot description is missing")?;
            let bytes = self
                .store
                .asset_bytes(asset, 1 << 20)
                .map_err(|e| e.to_string())?;
            let text = std::str::from_utf8(&bytes).map_err(|_| "Use a UTF-8 robot description")?;
            let robot = robot_description::parse_with_reader(text, &|name| {
                let asset = bundle.robot.source.files.get(name).ok_or_else(|| {
                    robot_description::Error::Read {
                        path: name.into(),
                        reason: "This mesh isn't saved with the robot".into(),
                    }
                })?;
                self.store
                    .asset_bytes(asset, robot_description::MAX_MESH_BYTES as usize)
                    .map_err(|e| robot_description::Error::Read {
                        path: name.into(),
                        reason: e.to_string(),
                    })
            })
            .map_err(|e| e.to_string())?;
            if self.robots.len() >= 4 {
                self.robots.clear();
            }
            let robot = Arc::new(robot);
            self.robots.insert(key, robot.clone());
            robot
        };
        Ok(Loaded { robot, bundle })
    }
    fn execute(&mut self, task: Task) -> Result<Update, String> {
        match task {
            Task::Open(reference) => Ok(Update::Loaded(self.load(&reference)?)),
            Task::Pose(base, joint, value) => {
                let loaded = self.load(&base)?;
                if loaded.bundle.episode.is_some() {
                    return Err("Open a scene to change its joint pose".into());
                }
                let result = self
                    .store
                    .execute(
                        Operation::Pose {
                            base,
                            robot: loaded.bundle.scene.robots[0].id.clone(),
                            joints: BTreeMap::from([(joint.clone(), value)]),
                            author: "Local user".into(),
                            summary: format!("Moved {}", joint.replace('_', " ")),
                        },
                        now(),
                    )
                    .map_err(|e| e.to_string())?;
                let Reply::Saved { reference, .. } = result else {
                    return Err("The joint pose couldn't be saved".into());
                };
                Ok(Update::Loaded(self.load(&reference)?))
            }
            Task::ResetPose(base) => {
                let loaded = self.load(&base)?;
                if loaded.bundle.episode.is_some() {
                    return Err("Open a scene to change its joint pose".into());
                }
                let joints = loaded
                    .robot
                    .movable()
                    .map(|j| {
                        (
                            j.name.clone(),
                            j.limit.map_or(0., |l| 0_f64.clamp(l.lower, l.upper)),
                        )
                    })
                    .collect();
                let result = self
                    .store
                    .execute(
                        Operation::Pose {
                            base,
                            robot: loaded.bundle.scene.robots[0].id.clone(),
                            joints,
                            author: "Local user".into(),
                            summary: "Reset joint pose".into(),
                        },
                        now(),
                    )
                    .map_err(|e| e.to_string())?;
                let Reply::Saved { reference, .. } = result else {
                    return Err("The joint pose couldn't be saved".into());
                };
                Ok(Update::Loaded(self.load(&reference)?))
            }
            Task::Watch(id) => {
                let job = Book::open(self.store.clone())
                    .and_then(|b| b.get(&id))
                    .map_err(|e| e.to_string())?;
                let Document::Experiment(experiment) = self
                    .store
                    .get(&job.experiment)
                    .map_err(|e| e.to_string())?
                    .content
                else {
                    return Err("The run's experiment is missing".into());
                };
                let episode = job
                    .recording
                    .as_ref()
                    .ok_or("This run hasn't started yet")?;
                episode.episode.validate().map_err(|e| e.to_string())?;
                let step = episode
                    .episode
                    .steps
                    .last()
                    .ok_or("This run has no saved poses yet")?
                    .clone();
                let finished = matches!(job.status, Status::Finished | Status::Interrupted);
                let reference = job.episode.as_ref().unwrap_or(&experiment.scene);
                Ok(Update::Live {
                    loaded: self.load(reference)?,
                    step,
                    paused: job.status == Status::Paused,
                    finished,
                })
            }
            Task::Control(job, value) => {
                if let Some(control) = &self.control {
                    let reply=control.tool("robot_run_control",serde_json::json!({"job":job,"command_json":serde_json::to_string(&value).map_err(|e|e.to_string())?}))?;
                    return Ok(Update::Controlled(reply["job"].as_str().map(str::to_owned)));
                } else if value["action"] == "stop" {
                    Book::open(self.store.clone())
                        .and_then(|b| b.stop(&job))
                        .map_err(|e| e.to_string())?;
                } else {
                    return Err("Connect the lab service before controlling a run".into());
                }
                Ok(Update::Controlled(None))
            }
            Task::Start(experiment, seed, job) => {
                let control = self
                    .control
                    .as_ref()
                    .ok_or("Connect the lab service before starting a run")?;
                control.tool("robot_run",serde_json::json!({"experiment":experiment.document,"version":experiment.version,"seed":seed,"job":job,"batch":false}))?;
                Ok(Update::Started(job))
            }
        }
    }
}
impl Source {
    fn request(&mut self, task: Task, selection: bool) -> Result<(), String> {
        if self.pending {
            return Err("Wait for the current lab command to finish".into());
        }
        if selection {
            self.revision = self.revision.wrapping_add(1);
        }
        self.sender
            .try_send(Request {
                revision: self.revision,
                task,
            })
            .map_err(|_| "The lab reader is busy")?;
        self.pending = true;
        Ok(())
    }
    pub fn pick(&mut self, up: bool) -> Result<(), String> {
        if self.pending {
            return Err("Wait for the current lab command to finish".into());
        }
        self.leave_run();
        if self.entries.is_empty() {
            return Err("Import a robot scene in chat first".into());
        }
        let n = self.entries.len();
        self.picked = if up {
            (self.picked + 1) % n
        } else {
            (self.picked + n - 1) % n
        };
        self.job = None;
        self.request(
            Task::Open(self.entries[self.picked].reference.clone()),
            true,
        )
    }
    pub fn pose(&mut self, base: Reference, joint: String, value: f64) -> Result<(), String> {
        self.request(Task::Pose(base, joint, value), true)
    }
    pub fn reset_pose(&mut self, base: Reference) -> Result<(), String> {
        self.request(Task::ResetPose(base), true)
    }
    pub fn pick_experiment(&mut self, up: bool) -> Result<(), String> {
        if self.pending {
            return Err("Wait for the current lab command to finish".into());
        }
        self.leave_run();
        if self.experiments.is_empty() {
            return Err("Create a saved robot experiment in chat first".into());
        }
        let n = self.experiments.len();
        self.experiment = if up {
            (self.experiment + 1) % n
        } else {
            (self.experiment + n - 1) % n
        };
        self.job = None;
        self.request(
            Task::Open(self.experiments[self.experiment].scene.clone()),
            true,
        )
    }
    fn leave_run(&mut self) {
        if self.owned_interactive {
            if let Some(handle) = self.handle.take() {
                let _ = self.sender.try_send(Request {
                    revision: self.revision,
                    task: Task::Control(handle, serde_json::json!({"action":"pause"})),
                });
            }
        }
        self.owned_interactive = false;
        self.job = None;
    }
    pub fn selected_experiment(&self) -> Option<&ExperimentEntry> {
        self.experiments.get(self.experiment)
    }
    pub fn experiment_label(&self) -> String {
        self.selected_experiment()
            .map_or("Experiment".into(), |e| e.label.clone())
    }
    pub fn can_run(&self) -> bool {
        self.control && !self.experiments.is_empty()
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        if self.owned_interactive {
            if let Some(job) = &self.job {
                let _ = self.sender.try_send(Request {
                    revision: self.revision,
                    task: Task::Control(job.clone(), serde_json::json!({"action":"pause"})),
                });
            }
        }
    }
}
impl Room {
    pub fn local(config: &Config) -> Result<Self, String> {
        let store = Store::open(&config.root, &config.project).map_err(|e| e.to_string())?;
        let mut entries = Vec::new();
        let mut experiments = Vec::new();
        for reference in store.list().map_err(|e| e.to_string())? {
            let version = store.get(&reference).map_err(|e| e.to_string())?;
            match &version.content {
                Document::Scene(scene) => entries.push(Entry {
                    reference,
                    label: scene.name.clone(),
                }),
                Document::Episode(episode) => entries.push(Entry {
                    reference,
                    label: format!(
                        "{} {}",
                        if episode.mode == robot_lab::document::EpisodeMode::Demonstration {
                            "Demonstration"
                        } else {
                            "Attempt"
                        },
                        episode.seed
                    ),
                }),
                Document::Experiment(experiment) => {
                    if matches!(
                        experiment.controller,
                        robot_lab::document::Controller::Manual { .. }
                    ) {
                        continue;
                    }
                    let label = match &experiment.controller {
                        robot_lab::document::Controller::Scripted { .. } => "Scripted",
                        robot_lab::document::Controller::Manual { .. } => "Manual",
                        robot_lab::document::Controller::Service { .. } => "Policy service",
                        robot_lab::document::Controller::Policy { build, .. }
                            if build.name.contains("smolvla") =>
                        {
                            "SmolVLA"
                        }
                        _ => "ACT",
                    };
                    experiments.push(ExperimentEntry {
                        reference,
                        scene: experiment.scene.clone(),
                        seed: experiment.seeds[0],
                        label: format!("{} · attempt {}", label, experiment.seeds[0]),
                    });
                }
                _ => (),
            }
            if entries.len() > 512 || experiments.len() > 256 {
                return Err(
                    "Choose a lab with fewer than 512 scenes and attempts and 256 experiments"
                        .into(),
                );
            }
        }
        let reference = config
            .subject
            .as_ref()
            .or_else(|| entries.first().map(|e| &e.reference))
            .ok_or("Import a robot scene in chat first")?;
        let control = config
            .control_url
            .as_ref()
            .map(|url| Control::new(url, &config.project))
            .transpose()?;
        let enabled = control.as_ref().is_some_and(|c| c.enabled);
        let mut files = Files {
            store,
            robots: BTreeMap::new(),
            control,
        };
        let loaded = files.load(reference)?;
        let mut room = Room::new(loaded.robot.clone(), loaded.bundle.scene.clone())?;
        room.open(loaded.robot, loaded.bundle)?;
        let (sender, receive) = mpsc::sync_channel::<Request>(4);
        let (reply, receiver) = mpsc::channel::<Response>();
        std::thread::Builder::new()
            .name("robot-room-files".into())
            .spawn(move || {
                while let Ok(request) = receive.recv() {
                    let result = files.execute(request.task);
                    let _ = reply.send(Response {
                        revision: request.revision,
                        result,
                    });
                }
            })
            .map_err(|_| "The lab reader couldn't start")?;
        let picked = entries
            .iter()
            .position(|e| &e.reference == reference)
            .unwrap_or(0);
        let experiment = experiments
            .iter()
            .position(|e| e.scene == room.bundle.as_ref().unwrap().scene_reference)
            .unwrap_or(0);
        room.source = Some(Source {
            entries,
            experiments,
            picked,
            experiment,
            sender,
            receiver,
            revision: 0,
            poll: 0.,
            pending: false,
            job: config.watch_job.clone(),
            handle: config.watch_job.clone(),
            paused: true,
            owned_interactive: false,
            control: enabled,
        });
        Ok(room)
    }
    pub fn catalog() -> Result<Self, String> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../robot-description/assets/so101");
        let (pin, robot) = robot_description::pin::load_pinned(&root).map_err(|e| e.to_string())?;
        let scene = super::inspection_scene(
            &robot,
            &pin.files
                .iter()
                .find(|f| f.path == pin.description)
                .ok_or("The checked description is missing")?
                .sha256,
        );
        Room::new(Arc::new(robot), scene)
    }
    pub(super) fn poll_source(&mut self, dt: f32) {
        let Some(source) = &mut self.source else {
            return;
        };
        let result = source.receiver.try_recv().ok();
        if let Some(response) = result {
            if response.revision != source.revision {
                return;
            }
            source.pending = false;
            match response.result {
                Err(error) => {
                    self.error = Some(error);
                }
                Ok(Update::Loaded(loaded)) => {
                    if let Some(index) = source
                        .entries
                        .iter()
                        .position(|e| e.reference.document == loaded.bundle.subject.document)
                    {
                        source.entries[index].reference = loaded.bundle.subject.clone();
                        source.picked = index;
                    }
                    if let Some(index) = source
                        .experiments
                        .iter()
                        .position(|e| e.scene == loaded.bundle.scene_reference)
                    {
                        source.experiment = index;
                    }
                    if let Err(e) = self.open(loaded.robot, loaded.bundle) {
                        self.error = Some(e);
                    }
                }
                Ok(Update::Started(job)) => {
                    source.handle = Some(job.clone());
                    source.job = Some(job);
                    source.owned_interactive = true;
                }
                Ok(Update::Controlled(job)) => {
                    if let Some(job) = job {
                        source.job = Some(job);
                    }
                }
                Ok(Update::Live {
                    loaded,
                    step,
                    paused,
                    finished,
                }) => {
                    if self
                        .bundle
                        .as_ref()
                        .is_none_or(|b| b.scene_reference != loaded.bundle.scene_reference)
                        || finished
                    {
                        if let Err(e) = self.open(loaded.robot, loaded.bundle) {
                            self.error = Some(e);
                            return;
                        }
                    }
                    if finished {
                        self.frame = self.frame_count() - 1;
                        self.time = step.time_seconds;
                        self.live = None;
                        if let Some(source) = &mut self.source {
                            source.job = None;
                            source.owned_interactive = false;
                        }
                    } else {
                        self.live = Some(step);
                        if let Some(source) = &mut self.source {
                            source.paused = paused;
                        }
                    }
                    if let Err(e) = self.redraw() {
                        self.error = Some(e);
                    }
                }
            }
        }
        if self.active {
            if let Some(source) = &mut self.source {
                source.poll += dt.max(0.).min(0.25);
                if source.poll >= 0.2 && !source.pending {
                    source.poll = 0.;
                    if let Some(job) = source.job.clone() {
                        let _ = source.request(Task::Watch(job), false);
                    }
                }
            }
        }
    }
    pub(super) fn start(&mut self) -> Result<(), String> {
        let source = self
            .source
            .as_mut()
            .ok_or("Connect the lab service before starting a run")?;
        let experiment = source
            .selected_experiment()
            .ok_or("Choose a saved experiment")?
            .clone();
        if self
            .bundle
            .as_ref()
            .is_none_or(|b| b.scene_reference != experiment.scene)
        {
            return Err("Open this experiment's scene before starting it".into());
        }
        let job = format!("room-{}", now());
        source.request(
            Task::Start(experiment.reference, experiment.seed, job),
            false,
        )
    }
    pub(super) fn control(&mut self, action: &str) -> Result<(), String> {
        let source = self
            .source
            .as_mut()
            .ok_or("Connect the lab service before controlling a run")?;
        let job = source
            .handle
            .clone()
            .or_else(|| source.job.clone())
            .ok_or("Choose a running attempt")?;
        let value = if action == "reset" {
            serde_json::json!({"action":"reset","next_job":format!("room-{}",now()),"seed":source.selected_experiment().ok_or("Choose the run's experiment")?.seed})
        } else {
            serde_json::json!({"action":action})
        };
        source.request(Task::Control(job, value), false)
    }
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
