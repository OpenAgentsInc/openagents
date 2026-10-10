//! An offline service for tests and simulator or emulator checks.
//!
//! It contacts no host, relay, or SSH server. Each host's transport status
//! comes from a real `coder-link` supervisor driven by scripted reports, so
//! the screens see the same phases a live client would. Activity summaries
//! pass through `nostr::activity_summary::encode`, so the disclosure rules
//! apply. The invitation it creates is a placeholder that no host accepts.
use crate::model::{
    Compatibility, CreatedInvitation, DeviceList, DeviceRow, Enrollment, HostRecord, LocalHost,
    PendingEnrollment, Platform, ServiceState, Snapshot,
};
use crate::service::{ComputersService, Result};
use coder_access::protocol::{
    CommandAction, DeviceState, INVITATION_PREFIX, OriginKind, QueueEdit, QueueItem, QueueLease,
    TaskCommand, TaskCreate, TaskQueue,
};
use coder_access::{Code, Error, Right, Rights};
use coder_link::{
    AttemptId, BlockReason, Command, ConnectionId, Failure, Moment, Policy, Report, Signal,
    Supervisor,
};
use coder_reach::hints::Class;
use nostr::activity_summary::{
    ActivitySummary, Attention, Phase, SubjectKind, SummaryDraft, encode,
};
use secp256k1::{Keypair, Secp256k1, SecretKey};
use std::collections::BTreeMap;
use std::str::FromStr;

/// The fixture key for `tag`: the x-only public key of the secret key whose
/// 32 bytes all equal `tag`. Activity summaries need real public keys.
pub fn key(tag: u8) -> String {
    SecretKey::from_byte_array([tag; 32]).map_or_else(
        |_| format!("{tag:02x}").repeat(32),
        |secret| {
            Keypair::from_secret_key(&Secp256k1::new(), &secret)
                .x_only_public_key()
                .0
                .to_string()
        },
    )
}

/// This device's key in the fixture.
pub fn device() -> String {
    key(0x1d)
}
/// The owner key the fixture's grants name: the secret key whose 32 bytes
/// all equal `0x0e`, entered as 64 hex characters. A public test value, not
/// a credential.
pub fn owner_secret_hex() -> String {
    "0e".repeat(32)
}
/// The code the fixture's headless host shows.
pub const APPROVAL_CODE: &str = "7K4M-9QXZ";
/// An invitation string the fixture refuses as expired.
pub const EXPIRED_INVITATION: &str = "coder-host:expired";
/// The bytes every fixture screenshot answers: a PNG's signature and
/// header.
pub const SCREENSHOT: &[u8] = &[
    0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 13, b'I', b'H', b'D', b'R', 0, 0,
    0, 1, 0, 0, 0, 1, 8, 6, 0, 0, 0,
];

struct Host {
    record: HostRecord,
    supervisor: Option<Supervisor>,
}

/// Script a supervisor into a phase through its public signals and reports.
enum Script {
    Idle,
    Connected { data_current: bool },
    Connecting,
    Backoff(Failure),
    Blocked(BlockReason),
    SwitchedOff,
}

fn supervise(script: Script) -> Option<Supervisor> {
    let mut supervisor = Supervisor::new(Policy::default()).ok()?;
    let now = Moment(1_000);
    let open = |commands: Vec<Command>| {
        commands.into_iter().find_map(|command| match command {
            Command::Open { attempt, .. } => Some(attempt),
            _ => None,
        })
    };
    let report = |supervisor: &mut Supervisor, report: Report| {
        let _ = supervisor.report(now, report);
    };
    match script {
        Script::Idle => {}
        Script::Connecting => {
            supervisor.signal(now, Signal::Connect);
        }
        Script::Connected { data_current } => {
            let AttemptId(id) = open(supervisor.signal(now, Signal::Connect))?;
            report(&mut supervisor, Report::Established(AttemptId(id)));
            if data_current {
                report(&mut supervisor, Report::DataCurrent(ConnectionId(id)));
            }
        }
        Script::Backoff(failure) => {
            let attempt = open(supervisor.signal(now, Signal::Connect))?;
            report(&mut supervisor, Report::Failed(attempt, failure));
        }
        Script::Blocked(reason) => {
            let attempt = open(supervisor.signal(now, Signal::Connect))?;
            report(
                &mut supervisor,
                Report::Failed(attempt, Failure::Blocked(reason)),
            );
        }
        Script::SwitchedOff => {
            let AttemptId(id) = open(supervisor.signal(now, Signal::Connect))?;
            report(&mut supervisor, Report::Established(AttemptId(id)));
            supervisor.switch_off();
        }
    }
    Some(supervisor)
}

/// The offline fixture service.
pub struct Synthetic {
    platform: Platform,
    now: fn() -> u64,
    hosts: Vec<Host>,
    activity: Vec<ActivitySummary>,
    local_host: LocalHost,
    first_run_complete: bool,
    created: u64,
    tasks: u64,
    ssh: Option<crate::model::SshAttempt>,
    /// Whether this device holds the fixture's owner key.
    owner_key: bool,
    /// Every effect the screens requested, in order. Tests read it.
    pub calls: Vec<String>,
    /// Each task's queued messages and edit lease holder, as a host keeps
    /// them.
    pub queues: BTreeMap<String, (Option<String>, Vec<QueueItem>)>,
    /// Hosts this device nudged, in order.
    pub nudged: Vec<String>,
    /// Image bytes each host holds, by digest, as `artifact.put` left them.
    pub artifacts: BTreeMap<(String, String), Vec<u8>>,
    /// The images each created task named, by task, with their bytes.
    pub task_images: BTreeMap<String, Vec<(coder_access::media::ImageRef, Vec<u8>)>>,
}

impl Synthetic {
    /// Seven hosts: online, connecting, offline and retrying, out of date,
    /// not enrolled with a waiting request, revoked, and switched off.
    pub fn fixture(platform: Platform, now: fn() -> u64) -> Self {
        let at = now();
        let standard = Rights::standard();
        let enrolled = |rights: Rights| Enrollment::Enrolled {
            grant: "synthetic-grant".into(),
            rights,
            epoch: 1,
            expires_at: at + 7 * 86_400,
        };
        let record = |tag: u8, label: &str, enrollment: Enrollment| HostRecord {
            key: key(tag),
            label: label.into(),
            enrollment,
            link: None,
            route: None,
            compatibility: Compatibility::Compatible,
            listing: None,
            delisted: false,
            ssh: None,
            tunnel: None,
            presence: None,
            devices: DeviceList::NotLoaded,
            enrollments: Vec::new(),
            workspaces: None,
            watchers: None,
            background: None,
        };
        let mut studio = record(0xa1, "Studio Mac", enrolled(Rights::all()));
        studio.route = Some(Class::Lan);
        studio.workspaces = Some(vec!["openagents".into(), "scratch".into()]);
        studio.watchers = Some(vec!["disk cleanup".into()]);
        studio.background = Some((at - 600, "Freed 41 GB: 2 old build folders.".into()));
        studio.devices = DeviceList::Loaded {
            as_of: at - 90,
            devices: vec![
                DeviceRow {
                    device: device(),
                    label: None,
                    rights: Rights::all(),
                    state: DeviceState::Active,
                    origin: OriginKind::Invitation,
                    expires_at: at + 7 * 86_400,
                    last_seen: Some(at - 20),
                },
                DeviceRow {
                    device: key(0x2e),
                    label: Some("Laptop".into()),
                    rights: standard.clone(),
                    state: DeviceState::Active,
                    origin: OriginKind::Approval,
                    expires_at: at + 5 * 86_400,
                    last_seen: Some(at - 7_200),
                },
                DeviceRow {
                    device: key(0x3f),
                    label: None,
                    rights: Rights::new([Right::Observe]).unwrap_or_else(|_| standard.clone()),
                    state: DeviceState::Revoked,
                    origin: OriginKind::Invitation,
                    expires_at: at + 86_400,
                    last_seen: None,
                },
            ],
        };
        let mut old = record(0xa4, "Old laptop", enrolled(standard.clone()));
        old.compatibility = Compatibility::HostOutOfDate;
        // Read while it was online; it is offline now.
        let mut nas = record(0xa3, "Home NAS", enrolled(standard.clone()));
        nas.watchers = Some(vec!["disk cleanup".into()]);
        let mut lab = record(0xa5, "Lab box", Enrollment::NotEnrolled);
        lab.enrollments.push(PendingEnrollment {
            enrollment: "synthetic-enrollment".into(),
            rights: standard.clone(),
            expires_at: at + 240,
        });
        let hosts = vec![
            Host {
                record: studio,
                supervisor: supervise(Script::Connected { data_current: true }),
            },
            Host {
                record: record(0xa2, "Build server", enrolled(standard.clone())),
                supervisor: supervise(Script::Connecting),
            },
            Host {
                record: nas,
                supervisor: supervise(Script::Backoff(Failure::Unreachable)),
            },
            Host {
                record: old,
                supervisor: supervise(Script::Blocked(BlockReason::Incompatible)),
            },
            Host {
                record: lab,
                supervisor: None,
            },
            Host {
                record: record(0xa6, "Former work PC", Enrollment::Revoked),
                supervisor: supervise(Script::Blocked(BlockReason::Revoked)),
            },
            Host {
                record: record(0xa7, "Travel mini", enrolled(standard)),
                supervisor: supervise(Script::SwitchedOff),
            },
        ];
        let summary =
            |host: u8, subject: u8, kind, sequence, phase, headline: &str, attention, ago: u64| {
                encode(&SummaryDraft {
                    host: &key(host),
                    subject_kind: kind,
                    subject: &key(subject),
                    sequence,
                    phase,
                    headline,
                    attention,
                    updated_at: at - ago,
                })
                .ok()
            };
        let activity = [
            summary(
                0xa2,
                0xb1,
                SubjectKind::Task,
                2,
                Phase::Running,
                "Deploy preview",
                Attention::None,
                900,
            ),
            summary(
                0xa2,
                0xb1,
                SubjectKind::Task,
                3,
                Phase::Waiting,
                "Deploy preview",
                Attention::Approval,
                120,
            ),
            summary(
                0xa1,
                0xb2,
                SubjectKind::Session,
                7,
                Phase::Failed,
                "private detail",
                Attention::Failed,
                600,
            ),
            summary(
                0xa1,
                0xb3,
                SubjectKind::Task,
                4,
                Phase::Completed,
                "Refactor parser",
                Attention::Completed,
                3_000,
            ),
            summary(
                0xa3,
                0xb4,
                SubjectKind::Task,
                1,
                Phase::Running,
                "Back up /Users/me/notes.txt",
                Attention::None,
                5_000,
            ),
        ]
        .into_iter()
        .flatten()
        .collect();
        Self {
            platform,
            now,
            hosts,
            activity,
            local_host: if platform == Platform::Phone {
                LocalHost::NotSupported
            } else {
                LocalHost::Undecided
            },
            first_run_complete: false,
            created: 0,
            tasks: 0,
            ssh: None,
            owner_key: false,
            calls: Vec::new(),
            queues: BTreeMap::new(),
            nudged: Vec::new(),
            artifacts: BTreeMap::new(),
            task_images: BTreeMap::new(),
        }
    }

    /// No hosts and no activity: the start of first run.
    pub fn empty(platform: Platform, now: fn() -> u64) -> Self {
        let mut service = Self::fixture(platform, now);
        service.hosts.clear();
        service.activity.clear();
        service
    }

    fn host(&mut self, key: &str) -> Result<&mut Host> {
        self.hosts
            .iter_mut()
            .find(|host| host.record.key == key)
            .ok_or_else(|| Error::new(Code::Unavailable, "unknown synthetic host"))
    }

    /// Record a task summary as a host would publish it.
    fn summarize(
        &mut self,
        host: &str,
        subject: &str,
        sequence: u64,
        phase: Phase,
        headline: &str,
    ) -> Result<()> {
        let summary = encode(&SummaryDraft {
            host,
            subject_kind: SubjectKind::Task,
            subject,
            sequence,
            phase,
            headline,
            attention: Attention::None,
            updated_at: (self.now)(),
        })
        .map_err(|_| Error::new(Code::Malformed, "synthetic summary refused"))?;
        self.activity.push(summary);
        Ok(())
    }

    /// The newest summary of a task, which must be at `revision`.
    fn latest(&self, host: &str, task: &str, revision: u64) -> Result<(u64, Phase, String)> {
        let latest = self
            .activity
            .iter()
            .filter(|summary| summary.host == host && summary.subject == task)
            .max_by_key(|summary| summary.sequence)
            .ok_or_else(|| Error::new(Code::Stale, "no such synthetic task"))?;
        if latest.sequence != revision {
            return Err(Error::new(Code::Stale, "the task moved on"));
        }
        Ok((latest.sequence, latest.phase, latest.headline.clone()))
    }

    fn add(&mut self, label: String) -> String {
        self.created += 1;
        let record = HostRecord {
            key: format!("{:064x}", 0xc0de_0000_u64 + self.created),
            label,
            enrollment: Enrollment::Enrolled {
                grant: "synthetic-grant".into(),
                rights: Rights::standard(),
                epoch: 1,
                expires_at: (self.now)() + 7 * 86_400,
            },
            link: None,
            route: None,
            compatibility: Compatibility::Compatible,
            listing: None,
            delisted: false,
            ssh: None,
            tunnel: None,
            presence: None,
            devices: DeviceList::NotLoaded,
            enrollments: Vec::new(),
            workspaces: None,
            watchers: None,
            background: None,
        };
        let key = record.key.clone();
        self.hosts.push(Host {
            record,
            supervisor: supervise(Script::Connecting),
        });
        key
    }
}

impl ComputersService for Synthetic {
    fn snapshot(&mut self) -> Result<Snapshot> {
        Ok(Snapshot {
            now: (self.now)(),
            device: device(),
            owner: true,
            service: ServiceState::Ready,
            local_host: self.local_host.clone(),
            first_run_complete: self.first_run_complete,
            hosts: self
                .hosts
                .iter()
                .map(|host| HostRecord {
                    link: host.supervisor.as_ref().map(Supervisor::status),
                    ..host.record.clone()
                })
                .collect(),
            activity: self.activity.clone(),
            // Holding the owner key reads an empty directory; the fixture
            // contacts no relay.
            directory: if self.owner_key {
                crate::model::DirectoryState::Current {
                    revision: None,
                    as_of: (self.now)(),
                }
            } else {
                crate::model::DirectoryState::NoOwnerKey
            },
            ssh_ready: self.platform != Platform::Phone,
            ssh: self.ssh.clone(),
        })
    }
    fn set_enabled(&mut self, host: &str, enabled: bool) -> Result<()> {
        self.calls.push(format!("set_enabled {host} {enabled}"));
        let host = self.host(host)?;
        if host.supervisor.is_none() {
            host.supervisor = supervise(Script::Idle);
        }
        let Some(supervisor) = &mut host.supervisor else {
            return Err(Error::new(Code::Unavailable, "no synthetic supervisor"));
        };
        if enabled {
            supervisor.switch_on(Moment(2_000));
        } else {
            supervisor.switch_off();
        }
        Ok(())
    }
    fn retry_now(&mut self, host: &str) -> Result<()> {
        self.calls.push(format!("retry_now {host}"));
        if let Some(supervisor) = &mut self.host(host)?.supervisor {
            supervisor.signal(Moment(2_000), Signal::RetryNow);
        }
        Ok(())
    }
    fn forget(&mut self, host: &str) -> Result<()> {
        self.calls.push(format!("forget {host}"));
        self.host(host)?;
        self.hosts.retain(|record| record.record.key != host);
        self.activity.retain(|summary| summary.host != host);
        Ok(())
    }
    fn redeem_invitation(&mut self, invitation: &str) -> Result<String> {
        self.calls.push("redeem_invitation".into());
        if invitation == EXPIRED_INVITATION {
            return Err(Error::new(Code::Expired, "synthetic invitation expired"));
        }
        if !invitation.starts_with(INVITATION_PREFIX) {
            return Err(Error::new(Code::Malformed, "not a host invitation"));
        }
        Ok(self.add("New computer".into()))
    }
    fn approve_enrollment(
        &mut self,
        host: &str,
        enrollment: &str,
        code: &str,
        rights: &Rights,
        grant_expires_at: u64,
    ) -> Result<()> {
        self.calls
            .push(format!("approve {host} {enrollment} {}", rights.to_list()));
        let expected = coder_access::protocol::normalize_code(APPROVAL_CODE)?;
        let record = self.host(host)?;
        if !record
            .record
            .enrollments
            .iter()
            .any(|request| request.enrollment == enrollment)
        {
            return Err(Error::new(Code::Stale, "no such request"));
        }
        if code != expected {
            return Err(Error::new(Code::WrongCode, "code digest differs"));
        }
        record.record.enrollments.clear();
        record.record.enrollment = Enrollment::Enrolled {
            grant: "synthetic-approved".into(),
            rights: rights.clone(),
            epoch: 1,
            expires_at: grant_expires_at,
        };
        record.supervisor = supervise(Script::Connecting);
        Ok(())
    }
    fn deny_enrollment(&mut self, host: &str, enrollment: &str) -> Result<()> {
        self.calls.push(format!("deny {host} {enrollment}"));
        self.host(host)?
            .record
            .enrollments
            .retain(|request| request.enrollment != enrollment);
        Ok(())
    }
    fn connect_ssh(&mut self, destination: &str) -> Result<()> {
        self.calls.push(format!("connect_ssh {destination}"));
        if self.platform == Platform::Phone {
            return Err(Error::new(Code::Unsupported, "phones never start SSH"));
        }
        let label: String = destination.chars().take(48).collect();
        let host = self.add(label);
        if let Some(added) = self.hosts.last_mut() {
            added.record.ssh = Some(destination.to_owned());
        }
        self.ssh = Some(crate::model::SshAttempt {
            destination: destination.to_owned(),
            stage: crate::model::SshStage::Added { host },
        });
        Ok(())
    }
    fn run_without_local_host(&mut self) -> Result<()> {
        self.calls.push("run_without_local_host".into());
        self.local_host = LocalHost::ClientOnly;
        Ok(())
    }
    fn refresh_devices(&mut self, host: &str) -> Result<()> {
        self.calls.push(format!("refresh_devices {host}"));
        let at = (self.now)();
        let record = &mut self.host(host)?.record;
        record.devices = match std::mem::replace(&mut record.devices, DeviceList::NotLoaded) {
            DeviceList::Loaded { devices, .. } => DeviceList::Loaded { devices, as_of: at },
            DeviceList::NotLoaded => DeviceList::Loaded {
                as_of: at,
                devices: vec![DeviceRow {
                    device: device(),
                    label: None,
                    rights: Rights::standard(),
                    state: DeviceState::Active,
                    origin: OriginKind::Invitation,
                    expires_at: at + 86_400,
                    last_seen: Some(at),
                }],
            },
        };
        Ok(())
    }
    fn create_invitation(
        &mut self,
        host: &str,
        rights: &Rights,
        grant_expires_at: u64,
    ) -> Result<CreatedInvitation> {
        self.calls.push(format!(
            "create_invitation {host} {} {grant_expires_at}",
            rights.to_list()
        ));
        self.host(host)?;
        Ok(CreatedInvitation {
            invitation: "synthetic-invitation".into(),
            code: format!("{INVITATION_PREFIX}SYNTHETIC-PLACEHOLDER-NOT-REDEEMABLE"),
            rights: rights.clone(),
            expires_at: (self.now)() + 600,
        })
    }
    fn cancel_invitation(&mut self, host: &str, invitation: &str) -> Result<()> {
        self.calls
            .push(format!("cancel_invitation {host} {invitation}"));
        self.host(host).map(|_| ())
    }
    fn revoke(&mut self, host: &str, device: &str) -> Result<()> {
        self.calls.push(format!("revoke {host} {device}"));
        let record = &mut self.host(host)?.record;
        if let DeviceList::Loaded { devices, .. } = &mut record.devices
            && let Some(row) = devices.iter_mut().find(|row| row.device == device)
        {
            row.state = DeviceState::Revoked;
            return Ok(());
        }
        Err(Error::new(Code::Stale, "no such device"))
    }
    fn import_owner_key(&mut self, secret: &str) -> Result<()> {
        // Record the call, never the key.
        self.calls.push("import_owner_key".into());
        let owner = key(0x0e);
        let named = SecretKey::from_str(secret)
            .ok()
            .map(|secret| {
                Keypair::from_secret_key(&Secp256k1::new(), &secret)
                    .x_only_public_key()
                    .0
                    .to_string()
            })
            .is_some_and(|key| key == owner);
        if !named {
            return Err(Error::new(
                Code::Malformed,
                "no held grant names this key as the owner",
            ));
        }
        self.owner_key = true;
        Ok(())
    }
    fn refresh_workspaces(&mut self, host: &str) -> Result<()> {
        self.calls.push(format!("refresh_workspaces {host}"));
        let record = &mut self.host(host)?.record;
        if record.workspaces.is_none() {
            record.workspaces = Some(Vec::new());
        }
        Ok(())
    }
    /// A fixed picture: the eight-byte PNG signature and a one-pixel
    /// header are enough for a screen that only shows it.
    fn screenshot(&mut self, host: &str) -> Result<Vec<u8>> {
        self.calls.push(format!("screenshot {host}"));
        self.host(host)?;
        Ok(SCREENSHOT.to_vec())
    }
    /// `~/notes.txt` is a short text file, `~/photo.png` the screenshot,
    /// and `~/data.bin` bytes that are neither; anything else is missing.
    fn pull_file(&mut self, host: &str, path: &str, limit: u64) -> Result<(String, Vec<u8>)> {
        self.calls.push(format!("pull_file {host} {path}"));
        self.host(host)?;
        let bytes = match path {
            "~/notes.txt" => b"Buy milk.\nCall the shop.\n".to_vec(),
            "~/photo.png" => SCREENSHOT.to_vec(),
            "~/data.bin" => vec![0, 159, 146, 150, 255],
            _ => {
                return Err(Error::new(
                    Code::Unavailable,
                    format!("there is no file at {path}"),
                ));
            }
        };
        if bytes.len() as u64 > limit {
            return Err(Error::new(Code::Bounds, "over the limit"));
        }
        Ok((path.replacen('~', "/home/synthetic", 1), bytes))
    }
    /// Keep a chunk as a host does: in order, and checked against its
    /// digest once whole.
    fn put_artifact(
        &mut self,
        host: &str,
        put: &coder_access::media::ArtifactPut,
    ) -> Result<coder_access::media::ArtifactState> {
        self.calls
            .push(format!("put_artifact {host} {} {}", put.digest, put.offset));
        self.host(host)?;
        let chunk = put.bytes()?;
        let held = self
            .artifacts
            .entry((host.to_owned(), put.digest.clone()))
            .or_default();
        if put.offset == held.len() as u64 {
            held.extend(chunk);
        }
        let received = held.len() as u64;
        let complete = received == put.size && coder_access::media::digest(held) == put.digest;
        Ok(coder_access::media::ArtifactState {
            digest: put.digest.clone(),
            received,
            complete,
        })
    }
    fn create_task(&mut self, host: &str, task: &TaskCreate) -> Result<String> {
        // Record the workspace and title, never the prompt.
        self.calls.push(format!(
            "create_task {host} {} {}",
            task.workspace, task.title
        ));
        let record = &self.host(host)?.record;
        if record
            .workspaces
            .as_ref()
            .is_some_and(|list| !list.contains(&task.workspace))
        {
            return Err(Error::new(Code::Forbidden, "unknown synthetic workspace"));
        }
        let mut images = Vec::new();
        for image in &task.images {
            let bytes = self
                .artifacts
                .get(&(host.to_owned(), image.digest.clone()))
                .filter(|bytes| coder_access::media::digest(bytes) == image.digest)
                .ok_or_else(|| Error::new(Code::Conflict, "the host holds no such image"))?;
            images.push((image.clone(), bytes.clone()));
        }
        self.tasks += 1;
        let subject = format!("{:064x}", 0x7a5c_0000_u64 + self.tasks);
        if !images.is_empty() {
            self.task_images.insert(subject.clone(), images);
        }
        self.summarize(host, &subject, 1, Phase::Queued, &task.title)?;
        Ok(subject)
    }
    fn steer_task(&mut self, host: &str, task: &str, revision: u64, _: &str) -> Result<()> {
        self.calls
            .push(format!("steer_task {host} {task} {revision}"));
        let (sequence, phase, headline) = self.latest(host, task, revision)?;
        self.summarize(host, task, sequence + 1, phase, &headline)
    }
    fn command_task(&mut self, host: &str, command: &TaskCommand) -> Result<()> {
        // Record the action and identities, never the text.
        self.calls.push(format!(
            "command_task {host} {} {:?} {}",
            command.task, command.action, command.command
        ));
        self.host(host)?;
        if command.action == CommandAction::Queue {
            let (_, items) = self.queues.entry(command.task.clone()).or_default();
            if !items.iter().any(|item| item.command == command.command) {
                items.push(QueueItem {
                    command: command.command.clone(),
                    device: device(),
                    text: Some(command.text.clone()),
                    priority: false,
                });
            }
        }
        Ok(())
    }
    fn queue_task(&mut self, host: &str, task: &str, edit: &QueueEdit) -> Result<TaskQueue> {
        self.calls
            .push(format!("queue_task {host} {task} {}", queue_action(edit)));
        self.host(host)?;
        let me = device();
        let now = (self.now)();
        let (lease, items) = self.queues.entry(task.to_owned()).or_default();
        let conflict = || Error::new(Code::Conflict, "another device holds the queue");
        let holds = lease.as_deref() == Some(me.as_str());
        let position = |items: &[QueueItem], command: &str| {
            items
                .iter()
                .position(|item| item.command == command)
                .ok_or_else(|| Error::new(Code::Forbidden, "no such queued message"))
        };
        match edit {
            QueueEdit::List {} => {}
            QueueEdit::Lease {} if lease.is_some() && !holds => return Err(conflict()),
            QueueEdit::Lease {} => *lease = Some(me.clone()),
            QueueEdit::Release {} => {
                if holds {
                    *lease = None;
                }
            }
            _ if !holds => return Err(conflict()),
            QueueEdit::Edit { command, text } => {
                let at = position(items, command)?;
                items[at].text = Some(text.clone());
            }
            QueueEdit::Remove { command } => {
                if let Ok(at) = position(items, command) {
                    items.remove(at);
                }
            }
            QueueEdit::SendNow { command } => {
                let at = position(items, command)?;
                items.remove(at);
            }
            QueueEdit::Reorder { commands } => {
                let mut reordered = Vec::new();
                for command in commands {
                    reordered.push(items[position(items, command)?].clone());
                }
                if reordered.len() != items.len() {
                    return Err(conflict());
                }
                *items = reordered;
            }
        }
        Ok(TaskQueue {
            task: task.to_owned(),
            revision: 1,
            lease: lease.clone().map(|device| QueueLease {
                device,
                expires_at: now + 60,
            }),
            items: items.clone(),
        })
    }
    fn nudge_host(&mut self, host: &str) -> Result<()> {
        self.calls.push(format!("nudge_host {host}"));
        self.nudged.push(host.to_owned());
        Ok(())
    }
    fn cancel_task(&mut self, host: &str, task: &str, revision: u64, _: &str) -> Result<()> {
        self.calls
            .push(format!("cancel_task {host} {task} {revision}"));
        let (sequence, _, headline) = self.latest(host, task, revision)?;
        self.summarize(host, task, sequence + 1, Phase::Cancelled, &headline)
    }
    fn complete_first_run(&mut self) -> Result<()> {
        self.calls.push("complete_first_run".into());
        self.first_run_complete = true;
        Ok(())
    }
    fn application(&mut self, active: bool) -> Result<()> {
        self.calls.push(format!("application {active}"));
        let at = Moment(if active { 3_000 } else { 2_500 });
        let signal = if active {
            Signal::ApplicationActive
        } else {
            Signal::ApplicationBackground
        };
        for host in &mut self.hosts {
            // Scripted phases stay as scripted: only a connected host sees
            // the signal. The fixture has no transport, so the probe or
            // replacement it asks for succeeds at once.
            let Some(supervisor) = host
                .supervisor
                .as_mut()
                .filter(|supervisor| supervisor.status().phase == coder_link::Phase::Connected)
            else {
                continue;
            };
            for command in supervisor.signal(at, signal) {
                if let Command::Probe { attempt, .. } | Command::Open { attempt, .. } = command {
                    let _ = supervisor.report(at, Report::Established(attempt));
                }
            }
        }
        Ok(())
    }
}

/// A queue edit's action name, for the call log. Never a message's text.
fn queue_action(edit: &QueueEdit) -> &'static str {
    match edit {
        QueueEdit::List {} => "list",
        QueueEdit::Lease {} => "lease",
        QueueEdit::Release {} => "release",
        QueueEdit::Edit { .. } => "edit",
        QueueEdit::Remove { .. } => "remove",
        QueueEdit::Reorder { .. } => "reorder",
        QueueEdit::SendNow { .. } => "send_now",
    }
}
