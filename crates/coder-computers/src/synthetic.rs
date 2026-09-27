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
use coder_access::protocol::{DeviceState, INVITATION_PREFIX, OriginKind};
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
    ssh: Option<crate::model::SshAttempt>,
    /// Whether this device holds the fixture's owner key.
    owner_key: bool,
    /// Every effect the screens requested, in order. Tests read it.
    pub calls: Vec<String>,
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
            ssh: None,
            presence: None,
            devices: DeviceList::NotLoaded,
            enrollments: Vec::new(),
        };
        let mut studio = record(0xa1, "Studio Mac", enrolled(Rights::all()));
        studio.route = Some(Class::Lan);
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
                record: record(0xa3, "Home NAS", enrolled(standard.clone())),
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
            ssh: None,
            owner_key: false,
            calls: Vec::new(),
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
            ssh: None,
            presence: None,
            devices: DeviceList::NotLoaded,
            enrollments: Vec::new(),
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
