//! Explicit, protected observation of retained project supervisor evidence.

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::fs::{File, Metadata};
use std::io::Read;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

use base64::{Engine, engine::general_purpose::STANDARD};
use coder_access::{Code, project as wire};
use coder_scheduler::{ledger, plan::Status, resources::InUse};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::controller::Configuration;

const CONFIG_BOUND: usize = 4 * 1024 * 1024;
const SNAPSHOT_BOUND: usize = 64 * 1024 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    schema: String,
    projects: Vec<Binding>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    id: String,
    workspace: String,
    label: String,
    configuration: PathBuf,
    state: PathBuf,
    devices: Vec<String>,
}
struct Admitted {
    binding: Binding,
    state_identity: Identity,
    repository: PathBuf,
}

/// The configured observer holds no credentials or writer handles.
pub struct Observer {
    policy_path: PathBuf,
    policy_identity: Identity,
    policy_digest: String,
    admitted: Vec<Admitted>,
}
impl std::fmt::Debug for Observer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectObserver")
            .field("projects", &self.admitted.len())
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Identity {
    device: u64,
    inode: u64,
}
fn identity(metadata: &Metadata) -> Identity {
    Identity {
        device: metadata.dev(),
        inode: metadata.ino(),
    }
}
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn malformed() -> Code {
    Code::Malformed
}

/// Traverse every path component with `openat` and `O_NOFOLLOW`. A renamed
/// ancestor cannot redirect an already opened directory handle.
fn open(path: &Path, directory: bool) -> std::io::Result<File> {
    if !path.is_absolute() {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
    }
    let components = path.components().collect::<Vec<_>>();
    let mut file = File::open("/")?;
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            if index == 0 && *component == Component::RootDir {
                continue;
            }
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput));
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| std::io::Error::from(std::io::ErrorKind::InvalidInput))?;
        let is_directory = directory || index + 1 != components.len();
        let flags = libc::O_RDONLY
            | libc::O_CLOEXEC
            | libc::O_NOFOLLOW
            | libc::O_NONBLOCK
            | if is_directory { libc::O_DIRECTORY } else { 0 };
        // SAFETY: the directory descriptor and NUL-terminated name stay live.
        let descriptor = unsafe { libc::openat(file.as_raw_fd(), name.as_ptr(), flags) };
        if descriptor < 0 {
            return Err(std::io::Error::last_os_error());
        }
        // SAFETY: openat returned a new owned descriptor.
        file = unsafe { File::from_raw_fd(descriptor) };
    }
    let metadata = file.metadata()?;
    // SAFETY: geteuid has no arguments or memory preconditions.
    let private = metadata.uid() == unsafe { libc::geteuid() }
        && metadata.mode() & 0o077 == 0
        && if directory {
            metadata.is_dir()
        } else {
            metadata.is_file() && metadata.nlink() == 1
        };
    if !private {
        return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
    }
    Ok(file)
}
fn read(path: &Path, bound: usize) -> std::io::Result<(Vec<u8>, Identity)> {
    let _parent = open(
        path.parent()
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::InvalidInput))?,
        true,
    )?;
    let file = open(path, false)?;
    let before = file.metadata()?;
    if before.len() > bound as u64 {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
    }
    let mut bytes = Vec::new();
    (&file).take(bound as u64 + 1).read_to_end(&mut bytes)?;
    let after = file.metadata()?;
    if bytes.len() > bound
        || before.len() != after.len()
        || before.mtime() != after.mtime()
        || before.mtime_nsec() != after.mtime_nsec()
        || identity(&before) != identity(&open(path, false)?.metadata()?)
    {
        return Err(std::io::Error::from(std::io::ErrorKind::InvalidData));
    }
    Ok((bytes, identity(&before)))
}
fn children(path: &Path) -> Result<Vec<PathBuf>, Code> {
    let opened = match open(path, true) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(Code::Unavailable),
    };
    let before = identity(&opened.metadata().map_err(|_| Code::Unavailable)?);
    let mut paths = Vec::new();
    for (index, entry) in std::fs::read_dir(path)
        .map_err(|_| Code::Unavailable)?
        .enumerate()
    {
        if index >= 4096 {
            return Err(Code::Unavailable);
        }
        let entry = entry.map_err(|_| Code::Unavailable)?;
        let name = entry.file_name();
        if name
            .to_str()
            .is_some_and(|name| name.len() <= 256 && name.ends_with(".json"))
        {
            paths.push(entry.path());
        }
        if paths.len() > 4096 {
            return Err(Code::Unavailable);
        }
    }
    if before
        != identity(
            &open(path, true)
                .map_err(|_| Code::Unavailable)?
                .metadata()
                .map_err(|_| Code::Unavailable)?,
        )
    {
        return Err(Code::Unavailable);
    }
    paths.sort();
    Ok(paths)
}

impl Observer {
    /// Load an explicit owner policy without creating supervisor state.
    pub fn load(path: &Path, workspaces: &BTreeMap<String, PathBuf>) -> Result<Self, String> {
        Self::load_inner(path, workspaces)
            .map_err(|_| "project observer configuration is unavailable or invalid".into())
    }
    fn load_inner(path: &Path, workspaces: &BTreeMap<String, PathBuf>) -> Result<Self, Code> {
        let (bytes, policy_identity) = read(path, CONFIG_BOUND).map_err(|_| Code::Unavailable)?;
        let policy: Policy = serde_json::from_slice(&bytes).map_err(|_| malformed())?;
        if policy.schema != "openagents.project.observe.v1"
            || policy.projects.is_empty()
            || policy.projects.len() > 64
        {
            return Err(malformed());
        }
        let mut seen = BTreeSet::new();
        let mut admitted = Vec::new();
        for binding in policy.projects {
            if !wire::alias(&binding.id)
                || !wire::alias(&binding.workspace)
                || binding.label.len() > 256
                || binding.label.chars().any(char::is_control)
                || binding.devices.is_empty()
                || binding.devices.len() > 128
                || !seen.insert((binding.workspace.clone(), binding.id.clone()))
                || binding.devices.iter().any(|device| {
                    device.len() != 64
                        || !device
                            .bytes()
                            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                })
            {
                return Err(malformed());
            }
            let root = workspaces.get(&binding.workspace).ok_or(Code::Denied)?;
            let repository = root.canonicalize().map_err(|_| Code::Unavailable)?;
            let state_identity = identity(
                &open(&binding.state, true)
                    .map_err(|_| Code::Unavailable)?
                    .metadata()
                    .map_err(|_| Code::Unavailable)?,
            );
            let (source, _) =
                read(&binding.configuration, CONFIG_BOUND).map_err(|_| Code::Unavailable)?;
            let configuration: Configuration =
                serde_json::from_slice(&source).map_err(|_| malformed())?;
            configuration.validate().map_err(|_| malformed())?;
            if configuration
                .repository
                .canonicalize()
                .map_err(|_| Code::Unavailable)?
                != repository
                || binding.state.starts_with(&repository)
                || binding.configuration.starts_with(&repository)
                || path.starts_with(&repository)
            {
                return Err(Code::Denied);
            }
            admitted.push(Admitted {
                binding,
                state_identity,
                repository,
            });
        }
        Ok(Self {
            policy_path: path.into(),
            policy_identity,
            policy_digest: digest(&bytes),
            admitted,
        })
    }
    fn ready(&self) -> Result<(), Code> {
        let (bytes, current) =
            read(&self.policy_path, CONFIG_BOUND).map_err(|_| Code::Unavailable)?;
        if current != self.policy_identity || digest(&bytes) != self.policy_digest {
            return Err(Code::Stale);
        }
        Ok(())
    }
    fn binding(&self, device: &str, workspace: &str, project: &str) -> Result<&Admitted, Code> {
        self.ready()?;
        self.admitted
            .iter()
            .find(|entry| {
                entry.binding.id == project
                    && entry.binding.workspace == workspace
                    && entry.binding.devices.iter().any(|id| id == device)
            })
            .ok_or(Code::Denied)
    }
    fn capture(&self, entry: &Admitted) -> Result<Snapshot, Code> {
        if identity(
            &open(&entry.binding.state, true)
                .map_err(|_| Code::Unavailable)?
                .metadata()
                .map_err(|_| Code::Unavailable)?,
        ) != entry.state_identity
        {
            return Err(Code::Stale);
        }
        let (configuration_bytes, _) =
            read(&entry.binding.configuration, CONFIG_BOUND).map_err(|_| Code::Unavailable)?;
        let configuration: Configuration =
            serde_json::from_slice(&configuration_bytes).map_err(|_| malformed())?;
        configuration.validate().map_err(|_| malformed())?;
        if configuration
            .repository
            .canonicalize()
            .map_err(|_| Code::Unavailable)?
            != entry.repository
        {
            return Err(Code::Stale);
        }
        let mut sources = BTreeMap::new();
        sources.insert(
            "configuration".into(),
            Evidence::valid(configuration_bytes.clone()),
        );
        let ledger_path = entry.binding.state.join("scheduler-ledger.json");
        let ledger_source = Evidence::read(&ledger_path, wire::MAX_SOURCE_BYTES as usize);
        let retained = ledger_source
            .bytes
            .as_deref()
            .and_then(|bytes| ledger::retained(bytes).ok());
        let ledger_digest = ledger_source.digest();
        sources.insert("ledger".into(), ledger_source);
        let rounds = children(&entry.binding.state.join("snapshots"))?;
        let latest = rounds.iter().max_by_key(|path| {
            std::fs::symlink_metadata(path)
                .ok()
                .map(|m| (m.mtime(), m.mtime_nsec(), (*path).clone()))
        });
        let round_source =
            latest.map_or_else(Evidence::missing, |path| Evidence::read(path, CONFIG_BOUND));
        let round: Value = round_source
            .bytes
            .as_deref()
            .and_then(|bytes| serde_json::from_slice(bytes).ok())
            .unwrap_or(Value::Null);
        sources.insert("round".into(), round_source);
        let records = retained.as_ref().map(|retained| &retained.records);
        let mut source_bytes = sources
            .values()
            .filter_map(|source| source.bytes.as_ref())
            .map(Vec::len)
            .sum::<usize>();
        let mut tasks = BTreeMap::new();
        let mut occupied = InUse::default();
        if configuration.external_reservation != coder_scheduler::resources::Resources::default() {
            occupied.add(&configuration.external_reservation);
        }
        for prepared in &configuration.tasks {
            let task = &prepared.scheduling;
            let record = records.and_then(|records| records.get(&task.id));
            let status = record.map_or(
                if retained.is_some() {
                    Status::Queued
                } else {
                    Status::Unknown
                },
                |record| record.status,
            );
            if matches!(status, Status::Active | Status::Review | Status::Unknown) {
                occupied.add(&task.resources);
            }
            let blockers = round
                .get("blocked")
                .and_then(|blocked| blocked.get(&task.id))
                .and_then(Value::as_str)
                .map(|reason| vec![reason.into()])
                .unwrap_or_default();
            tasks.insert(
                task.id.clone(),
                wire::Task {
                    id: task.id.clone(),
                    issue: task.issue,
                    title: format!("Issue {}", task.issue),
                    dependencies: task.depends_on.clone(),
                    footprint: json!(task.footprint),
                    resources: json!(task.resources),
                    status: status_name(status).into(),
                    content_digest: task.digest(),
                    claim: record.map(claim),
                    blockers,
                    worktree_state: "unknown".into(),
                    inline_state: "complete".into(),
                    worktree: None,
                },
            );
        }
        if let Some(records) = records {
            for (id, record) in records {
                if !wire::alias(id) {
                    return Err(malformed());
                }
                tasks.entry(id.clone()).or_insert_with(|| wire::Task {
                    id: id.clone(),
                    issue: 0,
                    title: "Retained claim outside current catalog".into(),
                    dependencies: vec![],
                    footprint: Value::Null,
                    resources: Value::Null,
                    status: status_name(record.status).into(),
                    content_digest: if wire::digest(&record.task_digest) {
                        record.task_digest.clone()
                    } else {
                        format!("sha256:{}", atif::digest(&json!(record)))
                    },
                    claim: Some(claim(record)),
                    blockers: vec!["Current prepared task evidence is unavailable".into()],
                    worktree_state: "unknown".into(),
                    inline_state: "complete".into(),
                    worktree: None,
                });
                if !record.attempt.is_empty() {
                    if !wire::alias(&record.attempt) {
                        return Err(malformed());
                    }
                    let source_id = format!(
                        "attempt-{}",
                        digest(record.attempt.as_bytes()).trim_start_matches("sha256:")
                    );
                    let evidence = Evidence::read(
                        &entry
                            .binding
                            .state
                            .join("attempts")
                            .join(&record.attempt)
                            .join("result.json"),
                        CONFIG_BOUND,
                    );
                    source_bytes =
                        source_bytes.saturating_add(evidence.bytes.as_ref().map_or(0, Vec::len));
                    if source_bytes > SNAPSHOT_BOUND {
                        return Err(Code::Unavailable);
                    }
                    if let Some(bytes) = evidence.bytes.as_deref()
                        && let Ok(report) = serde_json::from_slice::<crate::DispatchReport>(bytes)
                        && report.schema == "openagents.project-dispatch.v1"
                        && report.task_id == *id
                        && record.result_digest.as_ref().is_some_and(|expected| {
                            serde_json::from_slice::<Value>(bytes)
                                .is_ok_and(|value| expected == &atif::digest(&value))
                        })
                    {
                        let task = tasks.get_mut(id).ok_or(Code::Unavailable)?;
                        task.worktree_state = if report
                            .execution_status
                            .as_ref()
                            .is_some_and(|value| value.len() > 256)
                        {
                            "oversized-evidence"
                        } else {
                            "retained-evidence"
                        }
                        .into();
                        task.worktree = Some(wire::Worktree {
                            owner: record.owner.clone(),
                            attempt: record.attempt.clone(),
                            retained: report.retained_worktree.is_some(),
                            artifact_verified: report.artifact_verified,
                            execution_status: report
                                .execution_status
                                .filter(|value| value.len() <= 256),
                        });
                    }
                    sources.insert(source_id, evidence);
                }
            }
        }
        // Reviewed decisions remain original evidence; this reader never moves a
        // control record or applies a decision to the ledger.
        let reviews = children(&entry.binding.state.join("reviewed"))?;
        for path in &reviews {
            let source = Evidence::read(&path, CONFIG_BOUND);
            source_bytes = source_bytes.saturating_add(source.bytes.as_ref().map_or(0, Vec::len));
            if source_bytes > SNAPSHOT_BOUND {
                return Err(Code::Unavailable);
            }
            let id = format!(
                "review-{}",
                digest(path.file_name().ok_or(Code::Unavailable)?.as_bytes())
                    .trim_start_matches("sha256:")
            );
            sources.insert(id, source);
        }
        // A bounded derived index preserves the exact original source pins when
        // there are more retained reports than one inline page can display.
        let index = serde_json::to_vec(
            &sources
                .iter()
                .filter(|(id, _)| !["configuration", "ledger", "round"].contains(&id.as_str()))
                .map(|(id, source)| source.pin(id))
                .collect::<Vec<_>>(),
        )
        .map_err(|_| Code::Unavailable)?;
        if index.len() > wire::MAX_SOURCE_BYTES as usize
            || source_bytes.saturating_add(index.len()) > SNAPSHOT_BOUND
        {
            return Err(Code::Unavailable);
        }
        sources.insert(
            "evidence-index".into(),
            Evidence {
                state: "derived-index",
                bytes: Some(index),
                path: None,
            },
        );
        if children(&entry.binding.state.join("snapshots"))? != rounds
            || children(&entry.binding.state.join("reviewed"))? != reviews
            || digest(
                &read(&entry.binding.configuration, CONFIG_BOUND)
                    .map_err(|_| Code::Unavailable)?
                    .0,
            ) != digest(&configuration_bytes)
            || Evidence::read(&ledger_path, wire::MAX_SOURCE_BYTES as usize).digest()
                != ledger_digest
        {
            return Err(Code::Stale);
        }
        for (id, source) in &sources {
            if let Some((path, bound)) = &source.path
                && source.pin(id) != Evidence::read(path, *bound).pin(id)
            {
                return Err(Code::Stale);
            }
        }
        self.ready()?;
        if identity(
            &open(&entry.binding.state, true)
                .map_err(|_| Code::Unavailable)?
                .metadata()
                .map_err(|_| Code::Unavailable)?,
        ) != entry.state_identity
        {
            return Err(Code::Stale);
        }
        let pins = sources
            .iter()
            .map(|(id, source)| (id, source.pin(id)))
            .collect::<BTreeMap<_, _>>();
        let snapshot_digest = format!(
            "sha256:{}",
            atif::digest(
                &json!({"policy":self.policy_digest,"workspace":entry.binding.workspace,"project":entry.binding.id,"sources":pins}),
            )
        );
        Ok(Snapshot {
            configuration,
            retained,
            round,
            tasks: tasks.into_values().collect(),
            occupied,
            sources,
            digest: snapshot_digest,
        })
    }
}
fn status_name(status: Status) -> &'static str {
    match status {
        Status::Queued => "queued",
        Status::Active => "active",
        Status::Review => "review",
        Status::Completed => "completed",
        Status::Rejected => "rejected",
        Status::Unknown => "unknown",
    }
}
fn claim(record: &ledger::TaskRecord) -> wire::Claim {
    wire::Claim {
        owner: record.owner.clone(),
        attempt: record.attempt.clone(),
        task_digest: record.task_digest.clone(),
        result_digest: record.result_digest.clone(),
        cause: record.cause.clone(),
        backoff_until: record.backoff_until,
        attempts: record.attempts,
        updated_unix: record.updated_unix,
    }
}
struct Evidence {
    state: &'static str,
    bytes: Option<Vec<u8>>,
    path: Option<(PathBuf, usize)>,
}
impl Evidence {
    fn valid(bytes: Vec<u8>) -> Self {
        Self {
            state: "retained",
            bytes: Some(bytes),
            path: None,
        }
    }
    fn missing() -> Self {
        Self {
            state: "unknown",
            bytes: None,
            path: None,
        }
    }
    fn read(path: &Path, bound: usize) -> Self {
        let mut source = match read(path, bound) {
            Ok((bytes, _)) if serde_json::from_slice::<Value>(&bytes).is_ok() => Self::valid(bytes),
            Ok(_) => Self {
                state: "invalid",
                bytes: None,
                path: None,
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::missing(),
            Err(_) => Self {
                state: "unavailable",
                bytes: None,
                path: None,
            },
        };
        source.path = Some((path.into(), bound));
        source
    }
    fn digest(&self) -> Option<String> {
        self.bytes.as_deref().map(digest)
    }
    fn pin(&self, id: &str) -> wire::Source {
        wire::Source {
            id: id.into(),
            state: self.state.into(),
            original: self.bytes.as_ref().map(|bytes| wire::Original {
                id: id.into(),
                digest: digest(bytes),
                bytes: bytes.len() as u64,
                media_type: "application/json".into(),
            }),
        }
    }
}
struct Snapshot {
    configuration: Configuration,
    retained: Option<ledger::Retained>,
    round: Value,
    tasks: Vec<wire::Task>,
    occupied: InUse,
    sources: BTreeMap<String, Evidence>,
    digest: String,
}
impl coder_host::projects::Projects for Observer {
    fn list(&self, device: &str, workspace: &str) -> Result<wire::List, Code> {
        self.ready()?;
        if !wire::alias(workspace) {
            return Err(malformed());
        }
        let mut rows = Vec::new();
        for entry in self.admitted.iter().filter(|entry| {
            entry.binding.workspace == workspace
                && entry.binding.devices.iter().any(|id| id == device)
        }) {
            let snapshot = self.capture(entry)?;
            rows.push(wire::Row {
                id: entry.binding.id.clone(),
                label: entry.binding.label.clone(),
                snapshot_digest: snapshot.digest,
                goals_state: "unknown".into(),
            });
        }
        if rows.is_empty() {
            return Err(Code::Denied);
        }
        let list = wire::List {
            workspace: workspace.into(),
            rows,
        };
        list.validate().map_err(|_| malformed())?;
        Ok(list)
    }
    fn read(&self, device: &str, query: &wire::Query) -> Result<wire::Page, Code> {
        query.validate().map_err(|_| malformed())?;
        let entry = self.binding(device, &query.workspace, &query.project)?;
        let snapshot = self.capture(entry)?;
        if query
            .snapshot
            .as_ref()
            .is_some_and(|pin| pin != &snapshot.digest)
        {
            return Err(Code::Stale);
        }
        let offset = query
            .cursor
            .as_ref()
            .map_or(0, |cursor| cursor.offset as usize);
        if offset > snapshot.tasks.len() {
            return Err(malformed());
        }
        let retained = snapshot.retained.as_ref();
        let pending = snapshot
            .tasks
            .iter()
            .filter(|task| task.status == "review")
            .count() as u32;
        let occupied = snapshot.occupied;
        let configuration = &snapshot.configuration;
        let mut page = wire::Page {
            workspace: query.workspace.clone(),
            project: query.project.clone(),
            label: entry.binding.label.clone(),
            snapshot_digest: snapshot.digest.clone(),
            sequence: retained.map_or(0, |ledger| ledger.sequence),
            goals_state: "unknown".into(),
            tracker_state: if snapshot
                .round
                .get("snapshot")
                .is_some_and(|value| !value.is_null())
            {
                "retained"
            } else {
                "unknown"
            }
            .into(),
            tracker: json!({"round":snapshot.round.get("round"),"unprepared_issues":snapshot.round.get("unprepared_issues"),"source_state":"retained-observation"}),
            supervisor: json!({"run":retained.map(|ledger| &ledger.run),"ledger_state":if retained.is_some(){"retained"}else{"unknown"},"dispatch_limit":configuration.dispatch_limit,"admission_minutes":configuration.admission_minutes,"poll_seconds":configuration.poll_seconds,"quota_backoff_seconds":configuration.quota_backoff_seconds,"lease_state":"unknown","admission_state":"retained-observation"}),
            tasks: vec![],
            capacity: wire::Capacity {
                declared: json!(configuration.capacity),
                external: json!(configuration.external_reservation),
                occupied: json!({"executor_slots":occupied.executor_slots,"cpu_units":occupied.cpu_units,"memory_mib":occupied.memory_mib,"integration":occupied.integration,"quiet":occupied.quiet,"unknown_catalog_claims":snapshot.tasks.iter().filter(|task|task.resources.is_null()).count()}),
            },
            review: wire::Review {
                limit: configuration.review_cap,
                pending,
                state: if retained.is_none() {
                    "unknown"
                } else if pending >= configuration.review_cap {
                    "backpressure"
                } else {
                    "below-cap"
                }
                .into(),
            },
            external_owners: json!(configuration.external_owners),
            excluded_issues: configuration.excluded_issues.iter().copied().collect(),
            accepted_closed_issues: configuration
                .accepted_closed_issues
                .iter()
                .copied()
                .collect(),
            sources: vec![],
            next: None,
            remaining: (snapshot.tasks.len() - offset) as u32,
        };
        // Fixed metadata that cannot fit is refused with no partial claim. Task
        // summaries have an explicit gap and their source remains reconstructable.
        let fixed = ["configuration", "ledger", "round", "evidence-index"];
        page.sources = fixed
            .iter()
            .map(|id| snapshot.sources[*id].pin(id))
            .collect();
        if serde_json::to_vec(&page)
            .map_err(|_| Code::Unavailable)?
            .len()
            > 24 * 1024
        {
            return Err(Code::Unavailable);
        }
        for source in snapshot
            .tasks
            .iter()
            .skip(offset)
            .take(query.limit as usize)
        {
            let mut task = source.clone();
            if task.claim.as_ref().is_some_and(|claim| {
                claim.owner.len() > 256
                    || claim.attempt.len() > 256
                    || claim.cause.as_ref().is_some_and(|value| value.len() > 4096)
            }) || task.blockers.iter().any(|value| value.len() > 4096)
                || serde_json::to_vec(&task)
                    .map_err(|_| Code::Unavailable)?
                    .len()
                    > 12 * 1024
            {
                task.inline_state = "oversized".into();
                task.footprint = Value::Null;
                task.resources = Value::Null;
                task.dependencies.clear();
                task.blockers =
                    vec!["Task summary exceeds the inline bound; read the pinned originals".into()];
                task.claim = None;
                task.worktree = None;
            }
            let source_count = page.sources.len();
            if let Some(claim) = &task.claim
                && !claim.attempt.is_empty()
            {
                let id = format!(
                    "attempt-{}",
                    digest(claim.attempt.as_bytes()).trim_start_matches("sha256:")
                );
                if let Some(source) = snapshot.sources.get(&id) {
                    page.sources.push(source.pin(&id));
                }
            }
            page.tasks.push(task);
            let delivered = offset + page.tasks.len();
            page.remaining = (snapshot.tasks.len() - delivered) as u32;
            page.next = (page.remaining > 0).then(|| wire::Cursor {
                snapshot_digest: snapshot.digest.clone(),
                offset: delivered as u32,
            });
            if serde_json::to_vec(&page)
                .map_err(|_| Code::Unavailable)?
                .len()
                > wire::MAX_REPLY_BYTES
            {
                page.tasks.pop();
                page.sources.truncate(source_count);
                break;
            }
        }
        let delivered = offset + page.tasks.len();
        page.remaining = (snapshot.tasks.len() - delivered) as u32;
        page.next = (page.remaining > 0).then(|| wire::Cursor {
            snapshot_digest: snapshot.digest.clone(),
            offset: delivered as u32,
        });
        if page.remaining > 0 && page.tasks.is_empty() {
            return Err(Code::Unavailable);
        }
        page.validate().map_err(|_| Code::Unavailable)?;
        Ok(page)
    }
    fn original(&self, device: &str, query: &wire::OriginalQuery) -> Result<wire::Chunk, Code> {
        query.validate().map_err(|_| malformed())?;
        let entry = self.binding(device, &query.workspace, &query.project)?;
        let snapshot = self.capture(entry)?;
        if snapshot.digest != query.snapshot_digest {
            return Err(Code::Stale);
        }
        let source = snapshot
            .sources
            .get(&query.original.id)
            .ok_or(Code::Denied)?;
        if source.pin(&query.original.id).original.as_ref() != Some(&query.original) {
            return Err(Code::Stale);
        }
        let bytes = source.bytes.as_ref().ok_or(Code::Unavailable)?;
        let offset = query.cursor.as_ref().map_or(0, |cursor| cursor.offset) as usize;
        let end = offset.saturating_add(query.limit as usize).min(bytes.len());
        let chunk = wire::Chunk {
            workspace: query.workspace.clone(),
            project: query.project.clone(),
            snapshot_digest: snapshot.digest.clone(),
            original: query.original.clone(),
            offset: offset as u64,
            data_base64: STANDARD.encode(&bytes[offset..end]),
            next: (end < bytes.len()).then(|| wire::OriginalCursor {
                snapshot_digest: snapshot.digest,
                source_digest: query.original.digest.clone(),
                offset: end as u64,
            }),
        };
        chunk.validate().map_err(|_| Code::Unavailable)?;
        Ok(chunk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use coder_host::projects::Projects;
    use coder_scheduler::{catalog, resources};
    use std::os::unix::fs::{PermissionsExt, symlink};

    struct Fixture {
        root: tempfile::TempDir,
        state: PathBuf,
        policy: PathBuf,
        workspaces: BTreeMap<String, PathBuf>,
    }
    fn write(path: &Path, value: &Value) {
        std::fs::write(path, serde_json::to_vec(value).unwrap()).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    fn mkdir(path: &Path) {
        std::fs::create_dir(path).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    impl Fixture {
        fn new() -> Self {
            let root = tempfile::tempdir_in(std::env::temp_dir().canonicalize().unwrap()).unwrap();
            std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
            let repository = root.path().join("checkout");
            mkdir(&repository);
            let state = root.path().join("supervisor");
            mkdir(&state);
            mkdir(&state.join("snapshots"));
            mkdir(&state.join("attempts"));
            mkdir(&state.join("reviewed"));
            let base = "a".repeat(40);
            let mut tasks = Vec::new();
            for (id, issue, dependencies) in
                [("first", 1, vec![]), ("second", 2, vec!["first".into()])]
            {
                let mut prepared = crate::controller::Prepared {
                    scheduling: catalog::Task {
                        id: id.into(),
                        issue,
                        base: base.clone(),
                        input: String::new(),
                        depends_on: dependencies,
                        footprint: catalog::Footprint::Declared {
                            reads: vec!["src/lib.rs".into()],
                            writes: vec![],
                        },
                        priority: 0,
                        resources: resources::Resources::default(),
                        estimate_ticks: 1,
                    },
                    assignment: crate::Assignment {
                        id: id.into(),
                        base: base.clone(),
                        prompt: "Inspect the admitted task".into(),
                        writes: false,
                        expected_text: None,
                        minutes: 1,
                    },
                    issue_updated: "2026-10-08T00:00:00Z".into(),
                    issue_body_digest: "sha256:retained-issue".into(),
                    tracker_base: base.clone(),
                };
                prepared.scheduling.input = prepared.input_digest();
                tasks.push(prepared);
            }
            let configuration = Configuration {
                v: 1,
                repository: repository.clone(),
                project: crate::github::Scope {
                    owner: "OpenAgentsInc".into(),
                    repository: "synthetic".into(),
                    project: 19,
                },
                capacity: resources::Capacity {
                    executor_slots: 2,
                    cpu_units: 4,
                    memory_mib: 1024,
                    integration_lanes: 1,
                },
                external_reservation: resources::Resources::default(),
                excluded_issues: BTreeSet::from([77]),
                external_owners: vec![coder_scheduler::plan::Exclusion {
                    owner: "external-review".into(),
                    writes: vec!["docs".into()],
                }],
                accepted_closed_issues: BTreeSet::from([66]),
                review_cap: 1,
                dispatch_limit: 2,
                admission_minutes: 10,
                poll_seconds: 10,
                quota_backoff_seconds: 300,
                tasks,
            };
            configuration.validate().unwrap();
            let config = root.path().join("configuration.json");
            write(&config, &json!(configuration));
            write(
                &state.join("scheduler-ledger.json"),
                &json!({"v":ledger::SCHEMA,"run":"fixture-run","sequence":7,"next_attempt":4,"tasks":{
                    "first":{"status":"active","task_digest":configuration.tasks[0].scheduling.digest(),"attempt":"fixture-run-1","owner":"supervisor-a","attempts":1,"updated_unix":10},
                    "second":{"status":"review","task_digest":configuration.tasks[1].scheduling.digest(),"attempt":"fixture-run-2","owner":"supervisor-a","attempts":1,"updated_unix":11},
                    "orphan":{"status":"unknown","task_digest":digest(b"previous task"),"attempt":"fixture-run-3","owner":"previous-supervisor","cause":"Recovered by writer before observation","attempts":1,"updated_unix":9}
                }}),
            );
            write(
                &state.join("snapshots/owner-000001.json"),
                &json!({"round":1,"snapshot":{"issues":{"1":{"number":1,"closed":false}},"source_digest":digest(b"tracker")},"blocked":{"first":"External owner retains the footprint"},"unprepared_issues":[88]}),
            );
            let policy = root.path().join("observer.json");
            write(
                &policy,
                &json!({"schema":"openagents.project.observe.v1","projects":[{"id":"project","workspace":"repo","label":"Synthetic project","configuration":config,"state":state,"devices":["a".repeat(64)]}]}),
            );
            Self {
                root,
                state,
                policy,
                workspaces: BTreeMap::from([("repo".into(), repository)]),
            }
        }
        fn observer(&self) -> Observer {
            Observer::load(&self.policy, &self.workspaces).unwrap()
        }
        fn query(&self) -> wire::Query {
            wire::Query {
                workspace: "repo".into(),
                project: "project".into(),
                snapshot: None,
                cursor: None,
                limit: 64,
            }
        }
    }
    #[test]
    fn observation_preserves_active_claims_and_reports_dependencies_capacity_and_backpressure() {
        let fixture = Fixture::new();
        let observer = fixture.observer();
        let ledger_path = fixture.state.join("scheduler-ledger.json");
        let before = std::fs::read(&ledger_path).unwrap();
        let page = observer.read(&"a".repeat(64), &fixture.query()).unwrap();
        assert_eq!(page.sequence, 7);
        assert_eq!(page.tasks.len(), 3);
        assert_eq!(page.tasks[0].status, "active");
        assert_eq!(page.tasks[0].claim.as_ref().unwrap().owner, "supervisor-a");
        assert_eq!(page.tasks[2].dependencies, vec!["first"]);
        assert_eq!(page.review.state, "backpressure");
        assert_eq!(page.capacity.occupied["executor_slots"], 2);
        assert_eq!(page.capacity.occupied["unknown_catalog_claims"], 1);
        assert_eq!(page.goals_state, "unknown");
        assert_eq!(page.supervisor["lease_state"], "unknown");
        assert_eq!(page.tracker_state, "retained");
        assert_eq!(page.excluded_issues, vec![77]);
        assert_eq!(std::fs::read(&ledger_path).unwrap(), before);
        assert!(!fixture.state.join("scheduler-ledger.lock").exists());
        assert!(
            !serde_json::to_string(&page)
                .unwrap()
                .contains(fixture.root.path().to_str().unwrap())
        );
    }
    #[test]
    fn device_workspace_and_snapshot_fences_apply_to_pages_and_originals() {
        let fixture = Fixture::new();
        let observer = fixture.observer();
        let query = fixture.query();
        assert_eq!(observer.read(&"b".repeat(64), &query), Err(Code::Denied));
        let mut wrong = query.clone();
        wrong.workspace = "other".into();
        assert_eq!(observer.read(&"a".repeat(64), &wrong), Err(Code::Denied));
        let page = observer.read(&"a".repeat(64), &query).unwrap();
        let original = page
            .sources
            .iter()
            .find(|source| source.id == "ledger")
            .unwrap()
            .original
            .clone()
            .unwrap();
        let mut read = wire::OriginalQuery {
            workspace: "repo".into(),
            project: "project".into(),
            snapshot_digest: page.snapshot_digest.clone(),
            original,
            cursor: None,
            limit: 31,
        };
        let mut reconstructed = Vec::new();
        loop {
            let chunk = observer.original(&"a".repeat(64), &read).unwrap();
            reconstructed.extend(STANDARD.decode(chunk.data_base64).unwrap());
            if chunk.next.is_none() {
                break;
            }
            read.cursor = chunk.next;
        }
        assert_eq!(
            reconstructed,
            std::fs::read(fixture.state.join("scheduler-ledger.json")).unwrap()
        );
        let ledger_path = fixture.state.join("scheduler-ledger.json");
        let mut value: Value = serde_json::from_slice(&reconstructed).unwrap();
        value["sequence"] = json!(8);
        write(&ledger_path, &value);
        let mut pinned = query;
        pinned.snapshot = Some(page.snapshot_digest);
        assert_eq!(observer.read(&"a".repeat(64), &pinned), Err(Code::Stale));
        assert_eq!(observer.original(&"a".repeat(64), &read), Err(Code::Stale));
    }
    #[test]
    fn policy_replacement_and_symlink_sources_cannot_redirect_a_loaded_observer() {
        let fixture = Fixture::new();
        let observer = fixture.observer();
        let replacement = fixture.root.path().join("replacement.json");
        std::fs::copy(&fixture.policy, &replacement).unwrap();
        std::fs::rename(replacement, &fixture.policy).unwrap();
        assert_eq!(observer.list(&"a".repeat(64), "repo"), Err(Code::Stale));
        let fixture = Fixture::new();
        let observer = fixture.observer();
        let ledger = fixture.state.join("scheduler-ledger.json");
        let other = fixture.root.path().join("unadmitted.json");
        std::fs::rename(&ledger, &other).unwrap();
        symlink(other, &ledger).unwrap();
        let page = observer.read(&"a".repeat(64), &fixture.query()).unwrap();
        assert_eq!(page.supervisor["ledger_state"], "unknown");
        assert_eq!(
            page.sources
                .iter()
                .find(|source| source.id == "ledger")
                .unwrap()
                .state,
            "unavailable"
        );
        assert!(page.tasks.iter().all(|task| task.status == "unknown"));
    }
    #[test]
    fn missing_ledger_stays_unknown_and_never_creates_or_recovers_state() {
        let fixture = Fixture::new();
        let observer = fixture.observer();
        let ledger = fixture.state.join("scheduler-ledger.json");
        std::fs::remove_file(&ledger).unwrap();
        let page = observer.read(&"a".repeat(64), &fixture.query()).unwrap();
        assert_eq!(page.sequence, 0);
        assert!(page.tasks.iter().all(|task| task.status == "unknown"));
        assert!(!ledger.exists());
    }

    #[test]
    fn worktree_evidence_requires_the_exact_native_result_and_keeps_review_originals() {
        let fixture = Fixture::new();
        let observer = fixture.observer();
        let attempt = fixture.state.join("attempts/fixture-run-2");
        mkdir(&attempt);
        let report = crate::DispatchReport {
            schema: "openagents.project-dispatch.v1".into(),
            task_id: "second".into(),
            input_digest: atif::digest(&json!("input")),
            base: "a".repeat(40),
            program_digest: atif::digest(&json!("program")),
            trace: attempt.join("trace.atif.jsonl"),
            answered: true,
            execution_status: Some("completed".into()),
            text_matched: Some(true),
            refusal: None,
            retained_worktree: Some(fixture.root.path().join("retained-worktree")),
            elapsed_ms: 10,
            executor_cost_usd: None,
            artifact_verified: true,
        };
        write(&attempt.join("result.json"), &json!(report));
        let ledger = fixture.state.join("scheduler-ledger.json");
        let mut value: Value = serde_json::from_slice(&std::fs::read(&ledger).unwrap()).unwrap();
        value["tasks"]["second"]["result_digest"] = json!(atif::digest(&json!(report)));
        write(&ledger, &value);
        write(
            &fixture.state.join("reviewed/decision.json"),
            &json!({"task":"second","attempt":"fixture-run-2","accepted":false,"evidence":"Retained independent review"}),
        );
        let page = observer.read(&"a".repeat(64), &fixture.query()).unwrap();
        let task = page.tasks.iter().find(|task| task.id == "second").unwrap();
        let worktree = task.worktree.as_ref().unwrap();
        assert_eq!(worktree.owner, "supervisor-a");
        assert_eq!(worktree.attempt, "fixture-run-2");
        assert!(worktree.retained && worktree.artifact_verified);
        let index = page
            .sources
            .iter()
            .find(|source| source.id == "evidence-index")
            .unwrap()
            .original
            .clone()
            .unwrap();
        let chunk = observer
            .original(
                &"a".repeat(64),
                &wire::OriginalQuery {
                    workspace: "repo".into(),
                    project: "project".into(),
                    snapshot_digest: page.snapshot_digest,
                    original: index,
                    cursor: None,
                    limit: 16384,
                },
            )
            .unwrap();
        let sources: Vec<wire::Source> =
            serde_json::from_slice(&STANDARD.decode(chunk.data_base64).unwrap()).unwrap();
        assert!(
            sources
                .iter()
                .any(|source| source.id.starts_with("review-") && source.original.is_some())
        );
        value["tasks"]["second"]["result_digest"] = json!("unproven");
        write(&ledger, &value);
        let page = observer.read(&"a".repeat(64), &fixture.query()).unwrap();
        assert!(
            page.tasks
                .iter()
                .find(|task| task.id == "second")
                .unwrap()
                .worktree
                .is_none()
        );
    }
}
