//! Phone-owned Robot Lab viewing, bounded transfer, and paired-host controls.
use coder_host::access::protocol::{Operation, Outcome};
use robot_lab::{
    Reference,
    remote::{Answer, CHUNK_BYTES, Request},
};
use robot_workbench::{
    device::{self, Intent},
    view::{Bundle, State},
};
use rust_native::{Activation, ValidatedView};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    sync::{Arc, mpsc},
    time::{Duration, Instant},
};

pub trait Transport: Send + Sync {
    fn call(&self, request: Request) -> Result<Answer, String>;
    fn operate(&self) -> bool;
}
struct Live {
    links: coder_computers::terminal::session::Links,
    handle: tokio::runtime::Handle,
}
impl Transport for Live {
    fn call(&self, request: Request) -> Result<Answer, String> {
        request.validate()?;
        let link = (self.links)().map_err(|_| "Reconnect to this computer and choose Refresh")?;
        let answer = match self.handle.block_on(link.call(Operation::RobotLab {
            robot: request.clone(),
        })) {
            Ok(Outcome::RobotLab { robot }) => robot,
            _ => {
                return Err(
                    "This computer couldn't answer. Check its connection and Robot Lab access"
                        .into(),
                );
            }
        };
        answer.validate()?;
        if !answer.answers(&request) {
            return Err("The computer answered another robot request".into());
        }
        Ok(answer)
    }
    fn operate(&self) -> bool {
        (self.links)().is_ok_and(|link| {
            link.device()
                .access()
                .grant
                .rights
                .iter()
                .any(|r| r.as_str() == "operate")
        })
    }
}
fn data(transport: &dyn Transport, request: Request) -> Result<Value, String> {
    let answer = transport.call(request.clone())?;
    answer.validate()?;
    if !answer.answers(&request) {
        return Err("The computer answered another robot request".into());
    }
    match answer {
        Answer::Data { value, .. } => Ok(value),
        _ => Err("This robot reply couldn't be read".into()),
    }
}
fn download(
    transport: &dyn Transport,
    make: impl Fn(u64) -> Request,
    max: usize,
) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut expected = None;
    let started = Instant::now();
    loop {
        if started.elapsed() > Duration::from_secs(60) {
            return Err("The robot file took too long to open. Choose Refresh".into());
        }
        let offset = bytes.len() as u64;
        let request = make(offset);
        let answer = transport.call(request.clone())?;
        if !answer.answers(&request) {
            return Err("The computer answered another robot file request".into());
        }
        answer.validate()?;
        let Answer::Chunk {
            sha256,
            size,
            offset: actual,
            bytes: part,
            ..
        } = answer
        else {
            return Err("This robot file couldn't be read".into());
        };
        if size > max as u64
            || actual != offset
            || expected
                .as_ref()
                .is_some_and(|p| p != &(sha256.clone(), size))
        {
            return Err("This robot file changed while opening".into());
        }
        expected = Some((sha256.clone(), size));
        bytes.extend(robot_lab::remote::decode(&part)?);
        if bytes.len() as u64 == size {
            if format!("{:x}", Sha256::digest(&bytes)) != sha256 {
                return Err("This robot file couldn't be checked".into());
            }
            return Ok(bytes);
        }
        if bytes.len() as u64 > size || bytes.len() % CHUNK_BYTES != 0 {
            return Err("This robot file is incomplete".into());
        }
    }
}
fn preview(
    transport: &dyn Transport,
    reference: &Reference,
    frame: usize,
) -> Result<(String, Vec<u8>), String> {
    let value = data(
        transport,
        Request::Preview {
            reference: reference.clone(),
            frame: frame as u32,
        },
    )?;
    let asset: robot_lab::document::Asset =
        serde_json::from_value(value).map_err(|_| "This robot preview couldn't be opened")?;
    if asset.bytes == 0 || asset.bytes > 8 << 20 || asset.media_type != "image/png" {
        return Err("Choose a saved robot preview".into());
    }
    let bytes = download(
        transport,
        |offset| Request::Asset {
            sha256: asset.sha256.clone(),
            offset,
        },
        8 << 20,
    )?;
    if bytes.len() as u64 != asset.bytes {
        return Err("This robot preview is incomplete".into());
    }
    // Decode and bound pixels before exposing the image through the native bridge.
    let image = openagents_chat_app::attachments::Image::decode("robot.png", bytes.clone())
        .map_err(|_| "This robot preview couldn't be decoded")?;
    let _ = image;
    Ok((format!("image:robot-{}", asset.sha256), bytes))
}
enum Work {
    List(u32),
    Open(Reference),
    Frame(Reference, usize),
    Call(Request),
    Sensor(robot_lab::document::Asset),
}
enum ResultItem {
    Listed(u32, Value),
    Opened(Bundle, String, Vec<u8>),
    Frame(usize, String, Vec<u8>),
    Called(Request, Value),
    Sensor(String, Vec<u8>),
}
pub struct Lab {
    send: mpsc::SyncSender<Work>,
    receive: mpsc::Receiver<Result<ResultItem, String>>,
    transport: Arc<dyn Transport>,
    pub state: Option<State>,
    documents: Vec<(Reference, String)>,
    jobs: Vec<String>,
    next: Option<u32>,
    previous: Option<u32>,
    experiments: Vec<(Reference, u64)>,
    execution: bool,
    operate: bool,
    busy: bool,
    playing: bool,
    active: bool,
    image: Option<(String, Vec<u8>)>,
    sensor_image: Option<(String, Vec<u8>)>,
    job: Option<String>,
    control_job: Option<String>,
    status: String,
    saved: Option<Reference>,
    latest: Option<robot_lab::document::Step>,
    error: Option<String>,
    revision: u64,
    view: Option<ValidatedView<Intent>>,
    last_poll: Instant,
    last_play: Instant,
}
impl Lab {
    pub fn live(
        links: coder_computers::terminal::session::Links,
        handle: tokio::runtime::Handle,
    ) -> Self {
        Self::new(Arc::new(Live { links, handle }))
    }
    pub fn new(transport: Arc<dyn Transport>) -> Self {
        let (send, work) = mpsc::sync_channel(2);
        let (reply, receive) = mpsc::channel();
        let worker = transport.clone();
        std::thread::spawn(move || {
            while let Ok(work) = work.recv() {
                let result = (|| match work {
                    Work::List(offset) => data(worker.as_ref(), Request::List { offset })
                        .map(|value| ResultItem::Listed(offset, value)),
                    Work::Open(reference) => {
                        let bytes = download(
                            worker.as_ref(),
                            |offset| Request::View {
                                reference: reference.clone(),
                                offset,
                            },
                            robot_lab::remote::MAX_VIEW_BYTES,
                        )?;
                        let bundle: Bundle = serde_json::from_slice(&bytes)
                            .map_err(|_| "This robot view couldn't be opened")?;
                        bundle.validate()?;
                        if bundle.subject != reference {
                            return Err("The computer opened another robot view".into());
                        }
                        let (resource, image) = preview(worker.as_ref(), &reference, 0)?;
                        Ok(ResultItem::Opened(bundle, resource, image))
                    }
                    Work::Frame(reference, frame) => {
                        let (resource, image) = preview(worker.as_ref(), &reference, frame)?;
                        Ok(ResultItem::Frame(frame, resource, image))
                    }
                    Work::Sensor(asset) => {
                        if asset.bytes > 8 << 20 || asset.media_type != "image/png" {
                            return Err("Choose a saved robot camera image".into());
                        }
                        let bytes = download(
                            worker.as_ref(),
                            |offset| Request::Asset {
                                sha256: asset.sha256.clone(),
                                offset,
                            },
                            8 << 20,
                        )?;
                        if bytes.len() as u64 != asset.bytes {
                            return Err("This robot camera image is incomplete".into());
                        }
                        openagents_chat_app::attachments::Image::decode(
                            "robot-camera.png",
                            bytes.clone(),
                        )?;
                        Ok(ResultItem::Sensor(
                            format!("image:robot-{}", asset.sha256),
                            bytes,
                        ))
                    }
                    Work::Call(request) => data(worker.as_ref(), request.clone())
                        .map(|value| ResultItem::Called(request, value)),
                })();
                if reply.send(result).is_err() {
                    break;
                }
                crate::wake::ring();
            }
        });
        let mut lab = Self {
            send,
            receive,
            transport,
            state: None,
            documents: Vec::new(),
            jobs: Vec::new(),
            next: None,
            previous: None,
            experiments: Vec::new(),
            execution: false,
            operate: false,
            busy: false,
            playing: false,
            active: true,
            image: None,
            sensor_image: None,
            job: None,
            control_job: None,
            status: String::new(),
            saved: None,
            latest: None,
            error: None,
            revision: 0,
            view: None,
            last_poll: Instant::now(),
            last_play: Instant::now(),
        };
        lab.queue(Work::List(0));
        lab
    }
    fn queue(&mut self, work: Work) {
        if self.busy {
            return;
        }
        match self.send.try_send(work) {
            Ok(()) => {
                self.busy = true;
                self.error = None;
            }
            Err(_) => self.error = Some("Wait for this robot request to finish".into()),
        }
    }
    pub fn active(&mut self, active: bool) {
        self.active = active;
        if !active {
            self.playing = false;
        }
    }
    pub fn image(&self, resource: &str) -> Option<&[u8]> {
        self.image
            .as_ref()
            .filter(|(r, _)| r == resource)
            .or_else(|| self.sensor_image.as_ref().filter(|(r, _)| r == resource))
            .map(|(_, b)| b.as_slice())
    }
    pub fn activate(&mut self, event: Activation) -> bool {
        let Some(intent) = self
            .view
            .as_ref()
            .and_then(|v| v.activate(&event).ok())
            .cloned()
        else {
            return false;
        };
        match intent {
            Intent::Joint(joint) => {
                if let Some(state) = &mut self.state {
                    let _ = state.apply(robot_workbench::view::Intent::ChooseJoint { joint }, None);
                }
            }
            Intent::MoveJoint { increase } => {
                if let Some(state) = &self.state {
                    let bundle = state.bundle();
                    if bundle.episode.is_none() {
                        let joint = bundle
                            .robot
                            .joints
                            .iter()
                            .find(|joint| joint.name == state.joint);
                        if let Some(joint) = joint {
                            let change = if joint.kind == robot_lab::document::JointKind::Prismatic
                            {
                                0.01
                            } else {
                                0.05
                            };
                            let current = bundle.scene.robots[0].joints[&joint.name];
                            let value = current + if increase { change } else { -change };
                            let value = joint
                                .limits
                                .map_or(value, |limits| value.clamp(limits.lower, limits.upper));
                            self.queue(Work::Call(Request::Pose {
                                reference: bundle.subject.clone(),
                                joint: joint.name.clone(),
                                position: value.to_string(),
                            }));
                        }
                    }
                }
            }
            Intent::Close => return true,
            Intent::More(offset) => self.queue(Work::List(offset)),
            Intent::Refresh => {
                if let Some(job) = &self.job {
                    self.queue(Work::Call(Request::Status { job: job.clone() }));
                } else {
                    self.queue(Work::List(0));
                }
            }
            Intent::Open(reference) | Intent::OpenSaved(reference) => {
                self.playing = false;
                self.queue(Work::Open(reference));
            }
            Intent::Previous | Intent::Next => {
                if let Some(state) = &self.state {
                    if let Some(episode) = &state.bundle().episode {
                        let frame = if matches!(intent, Intent::Previous) {
                            state.frame.saturating_sub(1)
                        } else {
                            (state.frame + 1).min(episode.steps.len() - 1)
                        };
                        self.queue(Work::Frame(state.bundle().subject.clone(), frame));
                    }
                }
            }
            Intent::Play => {
                if let Some(state) = &self.state {
                    if state
                        .bundle()
                        .episode
                        .as_ref()
                        .is_some_and(|episode| state.frame + 1 == episode.steps.len())
                    {
                        self.queue(Work::Frame(state.bundle().subject.clone(), 0));
                    }
                }
                self.playing = true;
                self.last_play = Instant::now();
            }
            Intent::Pause => self.playing = false,
            Intent::Run { experiment, seed } => {
                let job = format!(
                    "phone-{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos()
                );
                // Keep the named run even if its start reply is lost. Reconnect reads it.
                self.job = Some(job.clone());
                self.control_job = Some(job.clone());
                self.status = "Starting".into();
                // A batch is explicitly named on the button and continues when this card closes.
                self.queue(Work::Call(Request::Run {
                    experiment,
                    seed,
                    job,
                    batch: true,
                }));
            }
            Intent::Watch(job) => {
                self.control_job = Some(job.clone());
                self.job = Some(job.clone());
                self.queue(Work::Call(Request::Status { job }));
            }
            Intent::NewRun => {
                self.job = None;
                self.control_job = None;
                self.saved = None;
                self.latest = None;
                self.sensor_image = None;
                self.status.clear();
            }
            Intent::Control(command) => {
                if let Some(job) = &self.control_job {
                    self.queue(Work::Call(Request::Control {
                        job: job.clone(),
                        command,
                    }));
                }
            }
        }
        false
    }
    pub fn packet(&mut self) -> Option<Value> {
        if let Ok(result) = self.receive.try_recv() {
            self.busy = false;
            match result {
                Err(error) => {
                    self.error = Some(error);
                    self.playing = false;
                }
                Ok(ResultItem::Listed(offset, value)) => {
                    self.documents.clear();
                    self.experiments.clear();
                    self.jobs.clear();
                    self.previous = (offset > 0).then(|| offset.saturating_sub(16));
                    self.next = value["next"]
                        .as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .filter(|n| *n <= 10000 && *n % 16 == 0);
                    self.documents.extend(
                        value["documents"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|d| {
                                Some((
                                    serde_json::from_value(d["reference"].clone()).ok()?,
                                    d["title"].as_str()?.to_owned(),
                                ))
                            })
                            .take(16)
                            .collect::<Vec<_>>(),
                    );
                    self.experiments.extend(
                        value["experiments"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|e| {
                                Some((
                                    serde_json::from_value(e["reference"].clone()).ok()?,
                                    e["seeds"].as_array()?.first()?.as_u64()?,
                                ))
                            })
                            .take(16)
                            .collect::<Vec<_>>(),
                    );
                    self.jobs.extend(
                        value["jobs"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .take(16)
                            .collect::<Vec<_>>(),
                    );
                    self.execution = value["execution"] == true;
                }
                Ok(ResultItem::Opened(bundle, resource, image)) => match State::new(bundle) {
                    Ok(mut state) => {
                        state.lifecycle(true, true);
                        self.state = Some(state);
                        self.image = Some((resource, image));
                    }
                    Err(error) => self.error = Some(error),
                },
                Ok(ResultItem::Sensor(resource, image)) => {
                    self.sensor_image = Some((resource, image));
                }
                Ok(ResultItem::Frame(frame, resource, image)) => {
                    if let Some(state) = &mut self.state {
                        if state.seek(frame).is_ok() {
                            self.image = Some((resource, image));
                        }
                    }
                }
                Ok(ResultItem::Called(request, value)) => {
                    if matches!(request, Request::Pose { .. }) {
                        if let Ok(reference) =
                            serde_json::from_value(value["card"]["reference"].clone())
                        {
                            self.queue(Work::Open(reference));
                        } else {
                            self.error = Some("The changed robot pose couldn't be opened".into());
                        }
                        self.playing = false;
                    } else {
                        if let Request::Run { job, .. } = request {
                            self.control_job = Some(job.clone());
                            self.job = Some(job);
                        }
                        if let Some(job) = value["job"].as_str() {
                            self.job = Some(job.into());
                        }
                        self.saved = serde_json::from_value(value["episode"].clone()).ok();
                        self.latest = serde_json::from_value(value["latest"].clone()).ok();
                        if let Some(sensor) = self
                            .latest
                            .as_ref()
                            .and_then(|step| step.sensors.values().next())
                        {
                            let resource = format!("image:robot-{}", sensor.asset.sha256);
                            if self
                                .sensor_image
                                .as_ref()
                                .is_none_or(|(r, _)| r != &resource)
                            {
                                self.queue(Work::Sensor(sensor.asset.clone()));
                            }
                        }
                        self.status = match value["status"].as_str() {
                            Some("running") => "Running",
                            Some("finished") => "Finished",
                            Some("stopped") => "Stopped",
                            Some("failed") => "Stopped with a problem",
                            Some("interrupted") => "Interrupted",
                            Some("paused") => "Paused",
                            Some("queued") => "Starting",
                            _ => "Run unavailable",
                        }
                        .into();
                    }
                }
            }
        }
        self.operate = self.transport.operate();
        if self.active && !self.busy {
            if self.playing && self.last_play.elapsed() > Duration::from_millis(500) {
                self.last_play = Instant::now();
                if let Some(state) = &self.state {
                    if let Some(episode) = &state.bundle().episode {
                        if state.frame + 1 < episode.steps.len() {
                            self.queue(Work::Frame(
                                state.bundle().subject.clone(),
                                state.frame + 1,
                            ));
                        } else {
                            self.playing = false;
                        }
                    }
                }
            } else if self.job.is_some()
                && self.saved.is_none()
                && self.last_poll.elapsed() > Duration::from_secs(2)
            {
                self.last_poll = Instant::now();
                self.queue(Work::Call(Request::Status {
                    job: self.job.clone()?,
                }));
            }
        }
        self.revision += 1;
        let status = if let Some(step) = &self.latest {
            format!(
                "{} · step {} · {:.3} s · {}",
                self.status,
                step.index,
                step.time_seconds,
                step.joints
                    .iter()
                    .map(|(name, value)| format!("{name}: {value:.4} rad"))
                    .collect::<Vec<_>>()
                    .join(" · ")
            )
        } else {
            self.status.clone()
        };
        self.view = device::card(
            device::Card {
                previous: self.previous,
                next: self.next,
                sensor_resource: self.sensor_image.as_ref().map(|(r, _)| r.as_str()),
                state: self.state.as_ref(),
                resource: self.image.as_ref().map(|(r, _)| r.as_str()),
                jobs: &self.jobs,
                documents: &self.documents,
                experiments: &self.experiments,
                operate: self.operate,
                controllable: matches!(self.status.as_str(), "Running" | "Paused"),
                execution: self.execution,
                playing: self.playing,
                busy: self.busy,
                job: self.job.as_deref(),
                status: &status,
                saved: self.saved.as_ref(),
                error: self.error.as_deref(),
            },
            "robot-lab-phone",
            self.revision,
        )
        .ok();
        self.view
            .as_ref()
            .and_then(|v| serde_json::to_value(v.view()).ok())
    }
    pub fn live_poll(&self) -> bool {
        self.active && (self.busy || self.playing || self.job.is_some() && self.saved.is_none())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    struct Fixture {
        bundle: Bundle,
        calls: AtomicUsize,
        runs: AtomicUsize,
        disconnected: AtomicBool,
        changed: AtomicBool,
    }
    impl Fixture {
        fn new() -> Self {
            Self {
                bundle: serde_json::from_str(include_str!(
                    "../../robot-workbench/fixtures/phone-demo.json"
                ))
                .unwrap(),
                calls: AtomicUsize::new(0),
                runs: AtomicUsize::new(0),
                disconnected: AtomicBool::new(false),
                changed: AtomicBool::new(false),
            }
        }
        fn png(&self) -> &'static [u8] {
            include_bytes!("../../robot-policy/fixtures/camera-pattern.png")
        }
    }
    impl Transport for Fixture {
        fn operate(&self) -> bool {
            true
        }
        fn call(&self, request: Request) -> Result<Answer, String> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.disconnected.load(Ordering::SeqCst) {
                return Err("Reconnect to this computer and choose Refresh".into());
            }
            let value = match &request {
                Request::List { .. } => {
                    serde_json::json!({"documents":[{"reference":self.bundle.subject,"title":"Recorded robot"}],"experiments":[{"reference":self.bundle.episode.as_ref().unwrap().experiment,"seeds":[7]}],"execution":true})
                }
                Request::View { offset, .. } => {
                    let bytes = serde_json::to_vec(&self.bundle).unwrap();
                    let mut answer = robot_lab::remote::chunk(request.clone(), &bytes, *offset)?;
                    if self.changed.load(Ordering::SeqCst) {
                        if let Answer::Chunk { sha256, .. } = &mut answer {
                            *sha256 = "b".repeat(64);
                        }
                    }
                    return Ok(answer);
                }
                Request::Preview { .. } => {
                    serde_json::json!({"sha256":format!("{:x}",Sha256::digest(self.png())),"bytes":self.png().len(),"media_type":"image/png"})
                }
                Request::Asset { offset, .. } => {
                    return robot_lab::remote::chunk(request.clone(), self.png(), *offset);
                }
                Request::Run { job, .. } => {
                    self.runs.fetch_add(1, Ordering::SeqCst);
                    serde_json::json!({"job":job,"status":"running","episode":null})
                }
                Request::Status { job } => {
                    serde_json::json!({"job":job,"status":"running","episode":null,"latest":self.bundle.episode.as_ref().unwrap().steps[1]})
                }
                Request::Control { job, .. } => {
                    serde_json::json!({"job":job,"status":"finished","episode":self.bundle.subject})
                }
                _ => return Err("This fixture doesn't answer that request".into()),
            };
            Ok(Answer::Data { request, value })
        }
    }
    fn settle(lab: &mut Lab) {
        let start = Instant::now();
        while lab.busy && start.elapsed() < Duration::from_secs(5) {
            lab.packet();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!lab.busy);
        assert!(lab.error.is_none(), "{:?}", lab.error);
        lab.packet();
    }
    fn press(lab: &mut Lab, node: &str) -> bool {
        let view = lab.view.as_ref().unwrap().view();
        lab.activate(Activation {
            instance: view.instance.clone(),
            revision: view.revision,
            node: node.into(),
        })
    }
    #[test]
    fn robot_phone_checks_files_replays_frames_and_reconnects_without_starting_twice() {
        let transport = Arc::new(Fixture::new());
        transport.bundle.validate().unwrap();
        let mut lab = Lab::new(transport.clone());
        settle(&mut lab);
        press(&mut lab, "document-0");
        settle(&mut lab);
        assert_eq!(lab.state.as_ref().unwrap().frame, 0);
        assert!(!lab.image.as_ref().unwrap().1.is_empty());
        press(&mut lab, "next");
        settle(&mut lab);
        assert_eq!(lab.state.as_ref().unwrap().frame, 1);
        assert_eq!(
            lab.state.as_ref().unwrap().step().unwrap().joints["shoulder_pan"],
            0.1
        );
        assert!(!lab.activate(Activation {
            instance: "robot-lab-phone".into(),
            revision: 0,
            node: "run-0".into()
        }));
        assert_eq!(transport.runs.load(Ordering::SeqCst), 0);
        press(&mut lab, "run-0");
        settle(&mut lab);
        let original = lab.job.clone().unwrap();
        assert_eq!(transport.runs.load(Ordering::SeqCst), 1);
        lab.active(false);
        lab.last_poll = Instant::now() - Duration::from_secs(3);
        let calls = transport.calls.load(Ordering::SeqCst);
        lab.packet();
        assert_eq!(transport.calls.load(Ordering::SeqCst), calls);
        lab.active(true);
        transport.disconnected.store(true, Ordering::SeqCst);
        press(&mut lab, "refresh");
        let started = Instant::now();
        while lab.busy && started.elapsed() < Duration::from_secs(5) {
            lab.packet();
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(lab.error.is_some());
        transport.disconnected.store(false, Ordering::SeqCst);
        press(&mut lab, "refresh");
        settle(&mut lab);
        assert_eq!(lab.job.as_deref(), Some(original.as_str()));
        assert_eq!(transport.runs.load(Ordering::SeqCst), 1);
        assert_eq!(lab.latest.as_ref().unwrap().index, 1);
        press(&mut lab, "stop");
        settle(&mut lab);
        assert!(lab.saved.is_some());
        assert!(press(&mut lab, "close"));
        lab.active(false);
        assert_eq!(transport.runs.load(Ordering::SeqCst), 1);
    }
    #[test]
    fn robot_phone_refuses_wrong_hash_and_oversized_download_before_mounting() {
        let fixture = Fixture::new();
        fixture.changed.store(true, Ordering::SeqCst);
        assert!(
            download(
                &fixture,
                |offset| Request::View {
                    reference: fixture.bundle.subject.clone(),
                    offset
                },
                robot_lab::remote::MAX_VIEW_BYTES
            )
            .is_err()
        );
        fixture.changed.store(false, Ordering::SeqCst);
        assert!(
            download(
                &fixture,
                |offset| Request::View {
                    reference: fixture.bundle.subject.clone(),
                    offset
                },
                1
            )
            .is_err()
        );
    }
}
