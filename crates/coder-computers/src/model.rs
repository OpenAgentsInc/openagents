//! Projection inputs: what a client knows about its computers.
//!
//! These types are the application-level snapshot the screens draw from. A
//! client service fills them from verified records: grants from
//! `coder-access`, transport health from a `coder-link` supervisor,
//! compatibility and the route in use from `coder-reach`, and activity from
//! `nostr::activity_summary`. Nothing here is a credential or a grant.
use coder_access::protocol::{DeviceState, OriginKind};
use coder_access::{Right, Rights};
use coder_link::{BlockReason, Failure, Freshness, Phase, Stage, Status as LinkStatus};
use coder_reach::hints::Class;
use coder_reach::placement::{self, Assessment, Candidate, Limits};
use coder_reach::presence::{ClientProfile, Freshness as PresenceFreshness, Presence, Received};
use nostr::activity_summary::ActivitySummary;

/// Where the client runs. It decides which ways to add a computer apply.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// iPhone or Android. A phone never runs a host and never starts SSH.
    Phone,
    /// The desktop client.
    Desktop,
    /// The terminal client.
    Terminal,
}

/// What this client's platform adapter can do.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Capabilities {
    pub platform: Platform,
    /// The adapter can scan a QR code.
    pub camera: bool,
}

impl Capabilities {
    /// SSH hosts start only from the desktop and terminal clients.
    pub fn ssh(self) -> bool {
        matches!(self.platform, Platform::Desktop | Platform::Terminal)
    }
}

/// Whether the client service can act at all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServiceState {
    Ready,
    /// Every effect is unavailable. The reason is user-facing copy.
    Unavailable {
        reason: String,
    },
}

/// Whether this machine runs a host of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LocalHost {
    /// A phone. Phones connect to computers and never run a host.
    NotSupported,
    /// No choice recorded yet.
    Undecided,
    /// A host runs on this machine under this key.
    Running { host: String },
    /// The person chose to run this client with no local host.
    ClientOnly,
}

/// This device's access to one host, as the client last verified it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Enrollment {
    /// The host's current grant for this device.
    Enrolled {
        grant: String,
        rights: Rights,
        epoch: u64,
        expires_at: u64,
    },
    /// The owner directory lists the host, and this device holds no grant.
    NotEnrolled,
    /// The grant expired at `at`.
    Expired { at: u64 },
    /// The host revoked this device.
    Revoked,
}

impl Enrollment {
    /// The rights this device holds on the host right now, if any.
    pub fn rights(&self, now: u64) -> Option<&Rights> {
        match self {
            Self::Enrolled {
                rights, expires_at, ..
            } if *expires_at > now => Some(rights),
            _ => None,
        }
    }
}

/// Which side of a protocol mismatch needs an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compatibility {
    /// No presence sample yet.
    Unknown,
    Compatible,
    /// The host speaks a version older than this client accepts.
    HostOutOfDate,
    /// This client speaks a version older than the host accepts.
    ClientOutOfDate,
}

impl Compatibility {
    /// Apply the NIP-REACH compatibility rule and say which side is behind.
    pub fn judge(presence: &Presence, client: &ClientProfile) -> Self {
        if presence.compatible(client).is_ok() {
            Self::Compatible
        } else if presence.protocol < client.accepts.min
            || client.protocol > presence.compatibility.max
        {
            Self::HostOutOfDate
        } else {
            Self::ClientOutOfDate
        }
    }
}

/// One enrolled device, as the host's `device.list` reported it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRow {
    /// The device's public key in hex.
    pub device: String,
    /// A local label this client knows for the key, if any.
    pub label: Option<String>,
    pub rights: Rights,
    pub state: DeviceState,
    pub origin: OriginKind,
    pub expires_at: u64,
    /// Unix seconds of the host's last authenticated request from the
    /// device. `None` when the host does not report it.
    pub last_seen: Option<u64>,
}

/// The enrolled devices of one host, when this device may read them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceList {
    /// Not fetched yet.
    NotLoaded,
    Loaded {
        devices: Vec<DeviceRow>,
        as_of: u64,
    },
}

/// A headless host's enrollment request addressed to this device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingEnrollment {
    pub enrollment: String,
    /// The most the host asks to grant.
    pub rights: Rights,
    pub expires_at: u64,
}

/// An invitation this device created. It holds a single-use capability:
/// show it to the person once and never log it.
#[derive(Clone, PartialEq, Eq)]
pub struct CreatedInvitation {
    pub invitation: String,
    /// The complete `coder-host:` string.
    pub code: String,
    pub rights: Rights,
    pub expires_at: u64,
}

impl std::fmt::Debug for CreatedInvitation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CreatedInvitation")
            .field("invitation", &self.invitation)
            .field("code", &"[redacted]")
            .field("rights", &self.rights)
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// The owner directory's entry for a host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Listing {
    /// The owner's placement weight, 0 to 1,000. Zero keeps the host listed
    /// and excludes it from placement.
    pub weight: u32,
    pub added_at: u64,
}

/// The weight placement gives a host the owner directory does not list.
/// NIP-REACH lets a client apply a local weight; this is that default. It is
/// also the weight a host gets when this device adds it to the directory.
pub const LOCAL_WEIGHT: u32 = 100;

/// What this client knows about the owner host directory.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectoryState {
    /// This device holds no owner key. It lists only the hosts it enrolled
    /// with.
    NoOwnerKey,
    /// This device holds the owner key and has not read the directory yet.
    Loading,
    /// The last read succeeded. `revision` is `None` when the owner has not
    /// published a directory yet.
    Current { revision: Option<u64>, as_of: u64 },
    /// Two different directories share the highest revision. The client
    /// keeps the last revision it trusted and waits for a higher one.
    Conflict { revision: u64 },
    /// The last read failed. The list shows the last revision read, if any.
    Failed { revision: Option<u64> },
}

impl DirectoryState {
    /// Whether this device may publish the next revision: it holds the owner
    /// key and its last read succeeded.
    pub fn writable(self) -> bool {
        matches!(self, Self::Current { .. })
    }

    /// The revision the screens show. An owner edit carries it, so an edit
    /// made against an older revision is refused as stale.
    pub fn revision(self) -> Option<u64> {
        match self {
            Self::Current { revision, .. } | Self::Failed { revision } => revision,
            Self::Conflict { revision } => Some(revision),
            Self::NoOwnerKey | Self::Loading => None,
        }
    }
}

/// A change to one listed host's directory entry. Each publishes the next
/// directory revision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ListingChange {
    /// New display text, 1 to 64 bytes without control characters.
    Label(String),
    /// A new placement weight, 0 to 1,000. Zero keeps the host listed and
    /// excludes it from placement.
    Weight(u32),
}

/// What an explicit SSH remove did on the remote machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SshRemoval {
    /// This app's setup started the host, and the remove stopped it.
    Stopped,
    /// The host was already running before the setup. It keeps running;
    /// this device detached from it.
    Detached,
    /// No host was running there.
    Absent,
}

/// This client's SSH tunnel to a host it set up over SSH. The tunnel is
/// only a route: when it closes, the host keeps running and the client uses
/// the relay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tunnel {
    /// The tunnel's forwarded loopback port accepts connections.
    pub open: bool,
    /// The live connection runs through the tunnel.
    pub in_use: bool,
}

/// A host SSH setup in progress or just finished.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SshAttempt {
    /// The destination as the person entered it.
    pub destination: String,
    pub stage: SshStage,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SshStage {
    /// Installing, then starting or adopting the host.
    Starting,
    /// `ssh` asks a question, such as a password. `id` names this prompt;
    /// `text` is what `ssh` printed, with control characters removed.
    Prompt { id: u64, text: String },
    /// Redeeming the host's invitation with this device's key.
    Enrolling,
    /// Enrolled. The host is in the list under `host`.
    Added { host: String },
    /// The setup stopped. `reason` is user-facing copy.
    Failed { reason: String },
    /// Running `coder-ssh`'s explicit remove on the destination.
    Removing { label: String },
    /// The remove finished and this device forgot the computer.
    Removed { label: String, removal: SshRemoval },
    /// The remove failed. The computer stays in the list. `reason` is
    /// user-facing copy.
    RemoveFailed { label: String, reason: String },
}

impl SshStage {
    /// Whether the setup or remove still runs.
    pub fn running(&self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Prompt { .. } | Self::Enrolling | Self::Removing { .. }
        )
    }

    /// Whether this stage belongs to an explicit remove.
    pub fn removal(&self) -> bool {
        matches!(
            self,
            Self::Removing { .. } | Self::Removed { .. } | Self::RemoveFailed { .. }
        )
    }
}

/// Everything the client knows about one host.
#[derive(Clone, Debug, PartialEq)]
pub struct HostRecord {
    /// The host's public key in hex.
    pub key: String,
    /// The owner directory's label when the directory lists the host, and
    /// otherwise this device's own label for it.
    pub label: String,
    /// The owner directory's entry, when this device can read the directory
    /// and it lists the host.
    pub listing: Option<Listing>,
    /// The owner removed the host from the directory, and this device still
    /// holds its grant. It stays reachable and leaves placement. Adding it to
    /// the directory again clears this.
    pub delisted: bool,
    /// The SSH destination this device set the host up through, if any.
    pub ssh: Option<String>,
    /// This client's SSH tunnel to the host, when it opened one.
    pub tunnel: Option<Tunnel>,
    pub enrollment: Enrollment,
    /// The connection supervisor's status. `None` when no supervisor runs
    /// for this host, for example before enrollment.
    pub link: Option<LinkStatus>,
    /// The class of the route a live connection uses.
    pub route: Option<Class>,
    pub compatibility: Compatibility,
    /// The newest presence this device accepted, for placement.
    pub presence: Option<Received>,
    pub devices: DeviceList,
    pub enrollments: Vec<PendingEnrollment>,
    /// The workspace labels the host accepts in `task.create`, as its
    /// `workspace.list` answered. `None` until read, or when the host does
    /// not list them; ordering work then asks for a label instead.
    pub workspaces: Option<Vec<String>>,
    /// The background watchers the host runs, by name ("disk cleanup"), as
    /// its `background.list` answered (see [`watchers`]). `None` until read,
    /// or when the host runs no background runner.
    pub watchers: Option<Vec<String>>,
}

impl HostRecord {
    /// The weight placement uses: the owner directory's, zero for a host the
    /// owner removed from the directory, or [`LOCAL_WEIGHT`] for a host the
    /// directory never listed.
    pub fn weight(&self) -> u32 {
        match self.listing {
            Some(listing) => listing.weight,
            None if self.delisted => 0,
            None => LOCAL_WEIGHT,
        }
    }

    /// The owner directory lists the host and this device holds no grant
    /// for it. Such a row carries no connection to switch, retry, or forget.
    pub fn directory_only(&self) -> bool {
        self.listing.is_some() && self.enrollment == Enrollment::NotEnrolled
    }
}

/// The projection input for every Computers screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    /// Unix seconds when the service took the snapshot.
    pub now: u64,
    /// This device's public key in hex.
    pub device: String,
    /// Whether this device acts with the owner key. The owner may approve
    /// enrollment requests without a grant.
    pub owner: bool,
    pub service: ServiceState,
    pub local_host: LocalHost,
    pub first_run_complete: bool,
    pub hosts: Vec<HostRecord>,
    /// Verified activity summaries. Order does not matter; the projection
    /// keeps the newest per subject.
    pub activity: Vec<ActivitySummary>,
    pub directory: DirectoryState,
    /// This client can start hosts over SSH: it runs where `ssh` does and
    /// holds a pinned host release to install.
    pub ssh_ready: bool,
    /// The current or last SSH setup.
    pub ssh: Option<SshAttempt>,
}

impl Snapshot {
    pub fn host(&self, key: &str) -> Option<&HostRecord> {
        self.hosts.iter().find(|host| host.key == key)
    }

    /// First run is satisfied once any host has a current grant.
    pub fn has_enrolled_host(&self) -> bool {
        self.hosts
            .iter()
            .any(|host| host.enrollment.rights(self.now).is_some())
    }

    /// Assess every host for new work with the NIP-REACH placement rule.
    /// Weights come from the owner directory (see [`HostRecord::weight`]).
    /// A host is admitted only when it is online and this device may run
    /// tasks on it. Placement never substitutes for the host's admission.
    pub fn assess_placement(&self, client: &ClientProfile) -> Vec<Assessment<'_>> {
        placement::assess(
            &self.candidates(),
            client,
            self.now,
            PresenceFreshness::default(),
            Limits::default(),
        )
    }

    /// The host new work goes to, if any qualifies.
    pub fn place(&self, client: &ClientProfile) -> Option<&str> {
        placement::place(
            &self.candidates(),
            client,
            self.now,
            PresenceFreshness::default(),
            Limits::default(),
        )
    }

    fn candidates(&self) -> Vec<Candidate<'_>> {
        self.hosts
            .iter()
            .map(|host| Candidate {
                host: &host.key,
                weight: host.weight(),
                presence: host.presence.as_ref(),
                admitted: host
                    .enrollment
                    .rights(self.now)
                    .is_some_and(|rights| rights.contains(Right::Operate))
                    && HostStatus::derive(host, self.now).online(),
            })
            .collect()
    }
}

/// Whether the client's data for a connected host is current. Transport
/// health and data freshness are separate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataState {
    Current,
    CatchingUp,
    /// The connection is up and its subscription or catch-up failed.
    UpdatesFailed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OfflineCause {
    /// The person switched the host off. The client remembers it.
    SwitchedOff,
    /// No connection is wanted or running.
    NotConnected,
    /// This device has no usable network.
    NoNetwork,
    /// The last attempt failed. The supervisor retries on its own.
    Retrying { failure: Option<Failure> },
    /// Blocked: the host refused this device's key or proof.
    Refused,
    /// Blocked: this client's route or settings for the host are invalid.
    Misconfigured,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutOfDate {
    Host,
    ThisApp,
    /// The versions do not overlap and no presence sample says which side.
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotEnrolledCause {
    NoAccess,
    Expired,
}

/// The status a Computers row shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostStatus {
    Online { data: DataState },
    Connecting { stage: Stage },
    Offline { cause: OfflineCause },
    OutOfDate { side: OutOfDate },
    NotEnrolled { cause: NotEnrolledCause },
    Revoked,
}

impl HostStatus {
    /// Derive the status. Access outranks compatibility, which outranks
    /// transport: a revoked host never reads as merely offline.
    pub fn derive(host: &HostRecord, now: u64) -> Self {
        let phase = host.link.map(|status| status.phase);
        if host.enrollment == Enrollment::Revoked
            || phase == Some(Phase::Blocked(BlockReason::Revoked))
        {
            return Self::Revoked;
        }
        match &host.enrollment {
            Enrollment::NotEnrolled => {
                return Self::NotEnrolled {
                    cause: NotEnrolledCause::NoAccess,
                };
            }
            Enrollment::Expired { .. } => {
                return Self::NotEnrolled {
                    cause: NotEnrolledCause::Expired,
                };
            }
            Enrollment::Enrolled { expires_at, .. } if *expires_at <= now => {
                return Self::NotEnrolled {
                    cause: NotEnrolledCause::Expired,
                };
            }
            _ => {}
        }
        match host.compatibility {
            Compatibility::HostOutOfDate => {
                return Self::OutOfDate {
                    side: OutOfDate::Host,
                };
            }
            Compatibility::ClientOutOfDate => {
                return Self::OutOfDate {
                    side: OutOfDate::ThisApp,
                };
            }
            _ => {}
        }
        let Some(link) = host.link else {
            return Self::Offline {
                cause: OfflineCause::NotConnected,
            };
        };
        if link.phase == Phase::Blocked(BlockReason::Incompatible) {
            return Self::OutOfDate {
                side: OutOfDate::Unknown,
            };
        }
        if !link.enabled {
            return Self::Offline {
                cause: OfflineCause::SwitchedOff,
            };
        }
        let cause = match link.phase {
            Phase::Connected => {
                let data = match link.freshness {
                    Freshness::Current { .. } => DataState::Current,
                    Freshness::Stale {
                        cause: coder_link::StaleCause::SubscriptionFailed,
                        ..
                    } => DataState::UpdatesFailed,
                    Freshness::Stale { .. } | Freshness::Unknown => DataState::CatchingUp,
                };
                return Self::Online { data };
            }
            Phase::Connecting(stage) => return Self::Connecting { stage },
            Phase::Available => OfflineCause::NotConnected,
            Phase::Offline => OfflineCause::NoNetwork,
            Phase::Backoff { .. } => OfflineCause::Retrying {
                failure: link.last_failure,
            },
            Phase::Blocked(BlockReason::Authentication) => OfflineCause::Refused,
            Phase::Blocked(BlockReason::Configuration) => OfflineCause::Misconfigured,
            // Checked first above; kept total so a refactor can't drop them.
            Phase::Blocked(BlockReason::Revoked) => return Self::Revoked,
            Phase::Blocked(BlockReason::Incompatible) => {
                return Self::OutOfDate {
                    side: OutOfDate::Unknown,
                };
            }
        };
        Self::Offline { cause }
    }

    /// Whether the host answers live requests now.
    pub fn online(self) -> bool {
        matches!(self, Self::Online { .. })
    }
}

/// The user-facing name of a right.
pub fn right_label(right: Right) -> &'static str {
    match right {
        Right::Observe => "View sessions and tasks",
        Right::Operate => "Run and steer tasks",
        Right::Terminal => "Open terminals",
        Right::Review => "Write reviews",
        Right::AccessRead => "See who has access",
        Right::AccessAdmin => "Manage access",
    }
}

/// The background watchers in a host's `background.list` answer, by name:
/// the rules that are on, readable, and not paused at `now`, with the first
/// letter lowercased ("disk cleanup"). The host answers `background.list`
/// only while its runner runs, so these are the ones running. The same rule
/// as `background::view::watchers` on the computer itself.
#[must_use]
pub fn watchers(list: &serde_json::Value, now: u64) -> Option<Vec<String>> {
    #[derive(serde::Deserialize)]
    struct Row {
        name: String,
        enabled: bool,
        #[serde(default)]
        paused_until: Option<u64>,
        #[serde(default)]
        error: Option<String>,
    }
    let rows: Vec<Row> = serde_json::from_value(list.clone()).ok()?;
    Some(
        rows.into_iter()
            .filter(|row| {
                row.error.is_none()
                    && row.enabled
                    && row.paused_until.is_none_or(|until| until <= now)
            })
            .map(|row| {
                let mut chars = row.name.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_lowercase().chain(chars).collect()
                })
            })
            .collect(),
    )
}
