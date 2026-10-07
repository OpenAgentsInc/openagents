//! The live Boat binding for the retail seams (#10748).
//!
//! [`BoatAdapter`] implements [`Provider`], [`Sandbox`], [`TaskOwner`],
//! [`StopOwner`], and [`Artifacts`] over the Boat public API (`crates/boat`)
//! with a key the caller supplies. It never resolves `BOAT_API_KEY` or the
//! operator's Secret Manager entry itself, so the operator allowance that
//! `chat work --on boat` uses cannot reach a retail path through it.
//!
//! - **Provider.** A create carries an `Idempotency-Key` derived from the
//!   provisioning identity and a body derived only from the [`CreateSpec`],
//!   so repeating a create whose reply was lost returns the same sandbox
//!   (Boat keeps keys for 24 hours). The provisioning-to-sandbox index is
//!   kept in a file under the adapter's state directory, so `find` survives
//!   a restart. A deletion counts as done only when Boat's deletion
//!   operation reports `completed` or the sandbox answers 404.
//! - **Sandbox and task owner.** On first use the adapter writes a fixed
//!   POSIX `sh` program, [`OWNER_SCRIPT`], to [`OWNER_PATH`] through the
//!   files API, and runs each operation as `sh OWNER_PATH VERB ARG...`
//!   through the commands API. The customer's key travels only in a files
//!   API body; it never appears in a command line. The program writes plain
//!   status, event, manifest, and stop-receipt files that the adapter reads
//!   back through the files API.
//!
//! Every call is blocking: the adapter owns a current-thread Tokio runtime.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::Mutex;

use boat::models::{
    CommandParams, CommandRequest, CommandResponseBody, CreateParams, CreateSandboxRequest,
    DeleteSandboxParams, GetDeletionOperationParams, GetParams, UsageParams,
};
use boat::{ApiKey, Client, Nullable};
use serde::{Deserialize, Serialize};

use crate::authority::Source;
use crate::cancel::{StopEvidence, StopOwner};
use crate::dispatch::{
    CheckRun, DispatchSpec, ExecutorEnd, OwnerError, TaskEvent, TaskOwner, TaskStatus,
};
use crate::material::Sandbox;
use crate::provision::{
    CreateSpec, Provider, ProviderError, Resource, ResourceState, StartRefusal,
};
use crate::retain::{Artifact, Artifacts, Kind, Manifest};
use crate::{Error, Result, sha256_hex};

/// The owner program's text.
pub const OWNER_SCRIPT: &str = include_str!("owner-v1.sh");
/// Where the owner program lives in the sandbox.
pub const OWNER_PATH: &str = "/tmp/oa-retail/owner-v1.sh";
/// The one workspace a retail sandbox clones into.
pub const WORKSPACE: &str = "/tmp/oa-retail/src";
/// The admitted source the clone verb records.
pub const SOURCE_FILE: &str = "/tmp/oa-retail/source";
/// The index file under the adapter's state directory.
pub const INDEX_FILE: &str = "boat-index.json";
/// Original create bytes and time, synced before the provider sees them.
pub const INTENTS_FILE: &str = "boat-create-intents.json";
/// The bound on one owner command.
const COMMAND_TIMEOUT_SECS: i64 = 600;

/// One task's directory in the sandbox.
#[must_use]
pub fn task_dir(task: &str) -> String {
    format!("/tmp/oa-retail/task/{task}")
}

/// The `Idempotency-Key` for a provisioning identity: account-unique and
/// stable across restarts.
#[must_use]
pub fn idempotency_key(provisioning: &str) -> String {
    format!("oa-retail-{}", &sha256_hex(provisioning.as_bytes())[..40])
}

/// Where the adapter reaches Boat and keeps its index.
#[derive(Clone, Debug)]
pub struct BoatConfig {
    /// The API base, `https://boat.dev/api/v1` for the live service. Plain
    /// HTTP is accepted only on loopback, for the simulated server.
    pub base_url: String,
    /// The retail Boat organization, when the key's account has several.
    pub org: Option<String>,
    /// A private directory for the provisioning index.
    pub state_dir: PathBuf,
    /// Retries for reads and keyed creates; `None` keeps the SDK default.
    pub retry: Option<boat::RetryPolicy>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    id: String,
    account: String,
    /// Boat's deletion operation, once a delete was accepted.
    deletion: Option<String>,
    /// The deletion operation reported `completed`, or the sandbox is gone.
    deleted: bool,
}

/// The live Boat binding.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Creation {
    spec: CreateSpec,
    requested_at: i64,
}

pub struct BoatAdapter {
    client: Client,
    runtime: tokio::runtime::Runtime,
    index_path: PathBuf,
    intents_path: PathBuf,
    intents: Mutex<BTreeMap<String, Creation>>,
    /// Provisioning identity to sandbox.
    index: Mutex<BTreeMap<String, Entry>>,
    /// Sandboxes the owner program was written to in this process.
    installed: Mutex<BTreeSet<String>>,
    /// Each task's frozen check commands, read back from `spec.json`.
    specs: Mutex<BTreeMap<String, DispatchSpec>>,
}

fn remote(context: &str, error: &boat::Error) -> Error {
    Error::Remote(format!("{context}: {error}"))
}

fn status_of(error: &boat::Error) -> Option<u16> {
    match error {
        boat::Error::Api(api) => Some(api.status.as_u16()),
        _ => None,
    }
}

fn provider_error(error: &boat::Error) -> ProviderError {
    if let boat::Error::Api(api) = error {
        match (api.status.as_u16(), api.code()) {
            (_, Some("rate_limited" | "limit_reached" | "member_limit_reached")) | (402, _) => {
                return ProviderError::Refused(StartRefusal::PlanLimit);
            }
            (_, Some("no_ready_machine" | "out_of_capacity")) => {
                return ProviderError::Refused(StartRefusal::Capacity);
            }
            _ => {}
        }
    }
    ProviderError::Unknown(error.to_string())
}

impl BoatAdapter {
    /// Bind Boat with `key`, which the caller resolved from the retail
    /// account's own credential.
    ///
    /// # Errors
    ///
    /// A malformed base URL, an unreadable index, or a runtime failure.
    pub fn new(key: ApiKey, config: &BoatConfig) -> Result<Self> {
        let mut builder = Client::builder(key).base_url(config.base_url.clone());
        if let Some(org) = &config.org {
            builder = builder.org(org.clone());
        }
        if let Some(retry) = config.retry.clone() {
            builder = builder.retry(retry);
        }
        let client = builder.build().map_err(|e| remote("boat client", &e))?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| Error::Invalid("cannot start the Boat runtime"))?;
        std::fs::create_dir_all(&config.state_dir)
            .map_err(|_| Error::Invalid("cannot create the Boat state directory"))?;
        let index_path = config.state_dir.join(INDEX_FILE);
        let index = match std::fs::read_to_string(&index_path) {
            Ok(text) => serde_json::from_str(&text)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(_) => return Err(Error::Invalid("cannot read the Boat index")),
        };
        let intents_path = config.state_dir.join(INTENTS_FILE);
        let intents = match std::fs::read_to_string(&intents_path) {
            Ok(text) => serde_json::from_str(&text)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(_) => return Err(Error::Invalid("cannot read the Boat creation intents")),
        };
        Ok(Self {
            client,
            runtime,
            index_path,
            intents_path,
            intents: Mutex::new(intents),
            index: Mutex::new(index),
            installed: Mutex::new(BTreeSet::new()),
            specs: Mutex::new(BTreeMap::new()),
        })
    }

    fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
        mutex
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn save(&self, index: &BTreeMap<String, Entry>) -> Result<()> {
        save_private(&self.index_path, &serde_json::to_vec_pretty(index)?)
    }
    fn remember_creation(&self, spec: &CreateSpec, now: i64) -> Result<()> {
        let mut intents = Self::lock(&self.intents);
        if let Some(original) = intents.get(&spec.provisioning) {
            if original.spec != *spec
                || now < original.requested_at
                || now - original.requested_at > crate::provision::READY_DEADLINE_SECS
            {
                return Err(Error::Invalid(
                    "the original Boat create cannot be replayed",
                ));
            }
        } else {
            intents.insert(
                spec.provisioning.clone(),
                Creation {
                    spec: spec.clone(),
                    requested_at: now,
                },
            );
        }
        save_private(&self.intents_path, &serde_json::to_vec_pretty(&*intents)?)
    }

    fn update(&self, id: &str, change: impl FnOnce(&mut Entry)) -> Result<()> {
        let mut index = Self::lock(&self.index);
        if let Some(entry) = index.values_mut().find(|e| e.id == id) {
            change(entry);
            let copy = index.clone();
            drop(index);
            return self.save(&copy);
        }
        Ok(())
    }

    fn entry(&self, id: &str) -> Option<Entry> {
        Self::lock(&self.index)
            .values()
            .find(|e| e.id == id)
            .cloned()
    }

    /// Every sandbox this adapter created that is not known deleted.
    #[must_use]
    pub fn undeleted(&self) -> Vec<String> {
        Self::lock(&self.index)
            .values()
            .filter(|e| !e.deleted)
            .map(|e| e.id.clone())
            .collect()
    }

    fn read_optional(
        &self,
        resource: &str,
        path: &str,
    ) -> std::result::Result<Option<String>, boat::Error> {
        match self.runtime.block_on(self.client.read_text(resource, path)) {
            Ok(text) => Ok(Some(text)),
            Err(error) if status_of(&error) == Some(404) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn write(
        &self,
        resource: &str,
        path: &str,
        text: &str,
    ) -> std::result::Result<(), boat::Error> {
        self.runtime
            .block_on(self.client.write_text(resource, path, text))
            .map(|_| ())
    }

    fn install(&self, resource: &str) -> std::result::Result<(), String> {
        if Self::lock(&self.installed).contains(resource) {
            return Ok(());
        }
        self.write(resource, OWNER_PATH, OWNER_SCRIPT)
            .map_err(|e| format!("install the owner program: {e}"))?;
        Self::lock(&self.installed).insert(resource.to_owned());
        Ok(())
    }

    /// Run one owner verb and return its trimmed standard output.
    fn owner(&self, resource: &str, args: &[&str]) -> std::result::Result<String, String> {
        self.install(resource)?;
        let mut command = format!("sh {OWNER_PATH}");
        for arg in args {
            command.push(' ');
            command.push_str(&boat::shell_quote(arg));
        }
        let reply = self
            .runtime
            .block_on(self.client.command(&CommandParams {
                sandbox_id: resource.into(),
                body: CommandRequest {
                    command,
                    timeout_seconds: Some(COMMAND_TIMEOUT_SECS),
                    ..Default::default()
                },
                ..Default::default()
            }))
            .map_err(|e| format!("owner {}: {e}", args.first().unwrap_or(&"")))?;
        match reply {
            CommandResponseBody::Finished(done) if done.exit_code == Some(0) => {
                Ok(done.stdout.trim().to_owned())
            }
            CommandResponseBody::Finished(done) => Err(format!(
                "owner {} exited {:?}",
                args.first().unwrap_or(&""),
                done.exit_code
            )),
            CommandResponseBody::Started(_) => Err("the owner command detached".into()),
        }
    }

    fn spec(
        &self,
        resource: &str,
        task: &str,
    ) -> std::result::Result<Option<DispatchSpec>, String> {
        if let Some(spec) = Self::lock(&self.specs).get(task) {
            return Ok(Some(spec.clone()));
        }
        let Some(text) = self
            .read_optional(resource, &format!("{}/spec.json", task_dir(task)))
            .map_err(|e| format!("read the task specification: {e}"))?
        else {
            return Ok(None);
        };
        let spec: DispatchSpec =
            serde_json::from_str(&text).map_err(|_| "the task specification is malformed")?;
        Self::lock(&self.specs).insert(task.to_owned(), spec.clone());
        Ok(Some(spec))
    }
}

impl Provider for BoatAdapter {
    fn create(&self, spec: &CreateSpec) -> std::result::Result<Resource, ProviderError> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .and_then(|d| i64::try_from(d.as_secs()).ok())
            .ok_or_else(|| ProviderError::Unknown("Boat creation time is unavailable".into()))?;
        self.remember_creation(spec, now).map_err(|_| {
            ProviderError::Unknown(
                "the original Boat create intent cannot be retained or replayed".into(),
            )
        })?;
        let params = CreateParams {
            idempotency_key: Some(idempotency_key(&spec.provisioning)),
            body: Some(CreateSandboxRequest {
                type_: Some(spec.size.clone()),
                ttl_seconds: Nullable::Value(i64::try_from(spec.lifetime_secs).unwrap_or(i64::MAX)),
                no_env: Some(spec.no_env),
                from_: Some(spec.template.clone()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let created = self
            .runtime
            .block_on(self.client.create(&params))
            .map_err(|e| provider_error(&e))?;
        let entry = Entry {
            id: created.sandbox.id.clone(),
            account: spec.account.clone(),
            deletion: None,
            deleted: false,
        };
        let mut index = Self::lock(&self.index);
        index.insert(spec.provisioning.clone(), entry);
        let copy = index.clone();
        drop(index);
        // A sandbox Boat created but the index cannot name is still found
        // again: the next create with the same key returns it.
        self.save(&copy)
            .map_err(|e| ProviderError::Unknown(e.to_string()))?;
        Ok(Resource {
            id: created.sandbox.id,
            provisioning: spec.provisioning.clone(),
            account: spec.account.clone(),
        })
    }

    fn find(&self, provisioning: &str) -> std::result::Result<Option<Resource>, ProviderError> {
        Ok(Self::lock(&self.index)
            .get(provisioning)
            .map(|entry| Resource {
                id: entry.id.clone(),
                provisioning: provisioning.to_owned(),
                account: entry.account.clone(),
            }))
    }

    fn reconcile_creation(
        &self,
        spec: &CreateSpec,
        now: i64,
    ) -> std::result::Result<Option<Resource>, ProviderError> {
        if let Some(resource) = self.find(&spec.provisioning)? {
            return Ok(Some(resource));
        }
        let original = Self::lock(&self.intents).get(&spec.provisioning).cloned();
        let Some(original) = original else {
            return Ok(None);
        };
        // Ten minutes is conservative within Boat's 24-hour keyed-create
        // window. A late restart never treats missing index state as absence.
        if original.spec != *spec
            || now < original.requested_at
            || now - original.requested_at > crate::provision::READY_DEADLINE_SECS
        {
            return Ok(None);
        }
        self.create(spec).map(Some)
    }

    fn state(&self, id: &str) -> std::result::Result<ResourceState, ProviderError> {
        if let Some(entry) = self.entry(id) {
            if entry.deleted {
                return Ok(ResourceState::Deleted);
            }
            if let Some(operation) = entry.deletion {
                let found = self.runtime.block_on(self.client.get_deletion_operation(
                    &GetDeletionOperationParams {
                        operation_id: operation,
                        ..Default::default()
                    },
                ));
                if let Ok(found) = found
                    && found.operation.status == "completed"
                {
                    self.update(id, |e| e.deleted = true)
                        .map_err(|e| ProviderError::Unknown(e.to_string()))?;
                    return Ok(ResourceState::Deleted);
                }
            }
        }
        let info = match self.runtime.block_on(self.client.get(&GetParams {
            sandbox_id: id.into(),
            ..Default::default()
        })) {
            Ok(info) => info,
            Err(error) if status_of(&error) == Some(404) => {
                self.update(id, |e| e.deleted = true)
                    .map_err(|e| ProviderError::Unknown(e.to_string()))?;
                return Ok(ResourceState::Deleted);
            }
            Err(error) => return Err(ProviderError::Unknown(error.to_string())),
        };
        let sandbox = info.sandbox;
        Ok(match sandbox.state.as_str() {
            "init" | "provisioning" | "provisioned" | "cloning" => ResourceState::Starting,
            "ready" | "idle" | "running" => ResourceState::Ready {
                address: match sandbox.url {
                    Nullable::Value(url) => url,
                    _ => format!("boat:{}", sandbox.id),
                },
            },
            "error" => ResourceState::RestoreFailed,
            "cancelled" => ResourceState::Deleted,
            _ => ResourceState::Stopped,
        })
    }

    fn delete(&self, id: &str) -> std::result::Result<(), ProviderError> {
        if self.entry(id).is_some_and(|e| e.deleted) {
            return Ok(());
        }
        match self
            .runtime
            .block_on(self.client.delete_sandbox(&DeleteSandboxParams {
                sandbox_id: id.into(),
                x_ascii_confirm_delete: id.into(),
                ..Default::default()
            })) {
            Ok(accepted) => {
                let operation = accepted.operation;
                let done = operation.status == "completed";
                self.update(id, |e| {
                    e.deletion = Some(operation.id);
                    e.deleted |= done;
                })
                .map_err(|e| ProviderError::Unknown(e.to_string()))
            }
            Err(error) if status_of(&error) == Some(404) => self
                .update(id, |e| e.deleted = true)
                .map_err(|e| ProviderError::Unknown(e.to_string())),
            Err(error) => Err(ProviderError::Unknown(error.to_string())),
        }
    }

    fn usage_seconds(&self, id: &str) -> std::result::Result<Option<u64>, ProviderError> {
        match self.runtime.block_on(self.client.usage(&UsageParams {
            sandbox_id: id.into(),
            ..Default::default()
        })) {
            Ok(usage) => Ok(u64::try_from(usage.seconds).ok()),
            Err(error) if status_of(&error) == Some(404) => Ok(None),
            Err(error) => Err(ProviderError::Unknown(error.to_string())),
        }
    }
}

impl Sandbox for BoatAdapter {
    fn write_private(&self, resource: &str, path: &str, contents: &str) -> Result<bool> {
        let prepared = self
            .owner(resource, &["prepare", path])
            .map_err(Error::Remote)?;
        if prepared != "prepared" {
            return Ok(false);
        }
        self.write(resource, path, contents)
            .map_err(|e| remote("write the private file", &e))?;
        let mode = self
            .owner(resource, &["private", path])
            .map_err(Error::Remote)?;
        Ok(mode == "600")
    }

    fn clone_source(&self, resource: &str, source: &Source) -> Result<(String, bool)> {
        let out = self
            .owner(
                resource,
                &["clone", &source.repository, &source.commit, WORKSPACE],
            )
            .map_err(Error::Remote)?;
        match out.split_once(' ') {
            Some((head, state)) => Ok((head.to_owned(), state == "clean")),
            None => Err(Error::Invalid("the clone reported no HEAD")),
        }
    }

    fn remove(&self, resource: &str, path: &str) -> Result<()> {
        self.owner(resource, &["remove", path])
            .map(|_| ())
            .map_err(Error::Remote)
    }

    fn exists(&self, resource: &str, path: &str) -> Result<bool> {
        self.owner(resource, &["exists", path])
            .map(|out| out == "yes")
            .map_err(Error::Remote)
    }
}

/// Parse the owner's `status` file against the frozen checks.
#[must_use]
pub fn parse_status(text: &str, checks: &[String]) -> Option<TaskStatus> {
    let mut lines = text.lines();
    let first: Vec<&str> = lines.next()?.split_whitespace().collect();
    Some(match first.as_slice() {
        ["queued"] => TaskStatus::Queued,
        ["running"] => TaskStatus::Running,
        ["cancelled"] => TaskStatus::Cancelled,
        ["ended", end, patch] => {
            let end = match *end {
                "completed" => ExecutorEnd::Completed,
                "limited" => ExecutorEnd::Limited,
                "timed_out" => ExecutorEnd::TimedOut,
                _ => ExecutorEnd::Failed,
            };
            let patch = (*patch != "-").then(|| (*patch).to_owned());
            let mut runs = Vec::new();
            for line in lines {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let ["check", index, exit] = parts.as_slice() {
                    let command = checks.get(index.parse::<usize>().ok()?)?.clone();
                    runs.push(CheckRun {
                        command,
                        candidate: patch.clone().unwrap_or_default(),
                        exit_status: exit.parse().ok()?,
                    });
                }
            }
            TaskStatus::Ended {
                end,
                patch,
                checks: runs,
            }
        }
        _ => return None,
    })
}

/// Parse a stop receipt file.
#[must_use]
pub fn parse_stop(text: &str, checks: &[String]) -> Option<StopEvidence> {
    let (head, status) = text.split_once("\nstatus\n").unwrap_or((text, ""));
    let mut at = None;
    let mut started = None;
    let mut effects = Vec::new();
    for line in head.lines() {
        match line.split_once(' ') {
            Some(("at", v)) => at = v.parse().ok(),
            Some(("started", v)) => started = Some(v == "1"),
            Some(("effect", v)) => effects.push(v.to_owned()),
            _ => {}
        }
    }
    Some(StopEvidence {
        at: at?,
        started: started?,
        status: parse_status(status, checks).unwrap_or(TaskStatus::Cancelled),
        effects,
    })
}

/// A stop request's identity as a safe file name.
fn request_name(request: &str) -> String {
    if !request.is_empty()
        && request.len() <= 64
        && request
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        request.to_owned()
    } else {
        sha256_hex(request.as_bytes())
    }
}

impl TaskOwner for BoatAdapter {
    fn submit(&self, resource: &str, spec: &DispatchSpec) -> std::result::Result<(), OwnerError> {
        let dir = task_dir(&spec.task);
        let unknown = OwnerError::Unknown;
        self.owner(resource, &["prepare", &format!("{dir}/spec.json")])
            .map_err(unknown)?;
        self.owner(resource, &["prepare", &format!("{dir}/checks/count")])
            .map_err(unknown)?;
        let mut files = vec![
            (
                "spec.json".to_owned(),
                serde_json::to_string(spec).map_err(|e| OwnerError::Unknown(e.to_string()))?,
            ),
            ("prompt".to_owned(), spec.prompt.clone()),
            ("key".to_owned(), crate::material::key_path(&spec.execution)),
            ("workspace".to_owned(), WORKSPACE.to_owned()),
            ("max_seconds".to_owned(), spec.max_seconds.to_string()),
            ("checks/count".to_owned(), spec.checks.len().to_string()),
        ];
        for (n, check) in spec.checks.iter().enumerate() {
            files.push((format!("checks/{n}"), check.clone()));
        }
        for (name, text) in files {
            self.write(resource, &format!("{dir}/{name}"), &text)
                .map_err(|e| OwnerError::Unknown(format!("write {name}: {e}")))?;
        }
        Self::lock(&self.specs).insert(spec.task.clone(), spec.clone());
        match self
            .owner(resource, &["submit", &dir])
            .map_err(unknown)?
            .as_str()
        {
            "new" | "existing" => Ok(()),
            _ => Err(OwnerError::Unknown(
                "the owner did not accept the task".into(),
            )),
        }
    }

    fn status(
        &self,
        resource: &str,
        task: &str,
    ) -> std::result::Result<Option<TaskStatus>, OwnerError> {
        let Some(text) = self
            .read_optional(resource, &format!("{}/status", task_dir(task)))
            .map_err(|e| OwnerError::Unknown(e.to_string()))?
        else {
            return Ok(None);
        };
        let checks = self
            .spec(resource, task)
            .map_err(OwnerError::Unknown)?
            .map(|s| s.checks)
            .unwrap_or_default();
        parse_status(&text, &checks)
            .map(Some)
            .ok_or_else(|| OwnerError::Unknown("the task status is malformed".into()))
    }

    fn events(
        &self,
        resource: &str,
        task: &str,
        after: u64,
    ) -> std::result::Result<Vec<TaskEvent>, OwnerError> {
        let text = self
            .read_optional(resource, &format!("{}/events", task_dir(task)))
            .map_err(|e| OwnerError::Unknown(e.to_string()))?
            .unwrap_or_default();
        Ok(text
            .lines()
            .enumerate()
            .map(|(i, line)| TaskEvent {
                cursor: i as u64 + 1,
                text: line.to_owned(),
            })
            .filter(|e| e.cursor > after)
            .collect())
    }
}

impl StopOwner for BoatAdapter {
    fn stop(
        &self,
        resource: &str,
        task: &str,
        request: &str,
    ) -> std::result::Result<StopEvidence, OwnerError> {
        let dir = task_dir(task);
        self.owner(resource, &["stop", &dir, &request_name(request)])
            .map_err(OwnerError::Unknown)?;
        self.stopped(resource, task, request)?
            .ok_or_else(|| OwnerError::Unknown("the stop left no receipt".into()))
    }

    fn stopped(
        &self,
        resource: &str,
        task: &str,
        request: &str,
    ) -> std::result::Result<Option<StopEvidence>, OwnerError> {
        let path = format!("{}/stop/{}", task_dir(task), request_name(request));
        let Some(text) = self
            .read_optional(resource, &path)
            .map_err(|e| OwnerError::Unknown(e.to_string()))?
        else {
            return Ok(None);
        };
        let checks = self
            .spec(resource, task)
            .map_err(OwnerError::Unknown)?
            .map(|s| s.checks)
            .unwrap_or_default();
        parse_stop(&text, &checks)
            .map(Some)
            .ok_or_else(|| OwnerError::Unknown("the stop receipt is malformed".into()))
    }
}

impl Artifacts for BoatAdapter {
    fn manifest(&self, resource: &str, task: &str) -> Result<Manifest> {
        let dir = task_dir(task);
        let text = self
            .read_optional(resource, &format!("{dir}/manifest"))
            .map_err(|e| remote("read the manifest", &e))?
            .ok_or(Error::Invalid("no declared artifacts"))?;
        let spec = self
            .spec(resource, task)
            .map_err(Error::Remote)?
            .ok_or(Error::Invalid("no task specification"))?;
        let source = self
            .read_optional(resource, SOURCE_FILE)
            .map_err(|e| remote("read the source record", &e))?
            .ok_or(Error::Invalid("no source record"))?;
        let (repository, commit) = source
            .trim()
            .split_once(' ')
            .ok_or(Error::Invalid("the source record is malformed"))?;
        let mut artifacts = Vec::new();
        for line in text.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            let [name, kind, digest, size] = parts.as_slice() else {
                return Err(Error::Invalid("the manifest is malformed"));
            };
            artifacts.push(Artifact {
                name: (*name).to_owned(),
                kind: match *kind {
                    "patch" => Kind::Patch,
                    "checks" => Kind::Checks,
                    _ => Kind::Log,
                },
                digest: (*digest).to_owned(),
                size: size.parse().map_err(|_| Error::Invalid("artifact size"))?,
            });
        }
        Ok(Manifest {
            execution: spec.execution,
            task: task.to_owned(),
            resource: resource.to_owned(),
            source: Source {
                repository: repository.to_owned(),
                commit: commit.to_owned(),
            },
            engine: spec.engine,
            artifacts,
        })
    }

    fn read(&self, resource: &str, task: &str, name: &str, max_bytes: usize) -> Result<Vec<u8>> {
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric()) {
            return Err(Error::Invalid("artifact names are identifiers"));
        }
        let bytes = self
            .runtime
            .block_on(
                self.client
                    .read_bytes(resource, &format!("{}/artifacts/{name}", task_dir(task))),
            )
            .map_err(|e| remote("read an artifact", &e))?;
        if bytes.len() > max_bytes {
            return Err(Error::Invalid("artifact too large"));
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn statuses_and_stop_receipts_parse() {
        let checks = vec!["cargo test".to_owned()];
        assert_eq!(parse_status("queued\n", &checks), Some(TaskStatus::Queued));
        assert_eq!(
            parse_status("ended completed abc\ncheck 0 0\n", &checks),
            Some(TaskStatus::Ended {
                end: ExecutorEnd::Completed,
                patch: Some("abc".into()),
                checks: vec![CheckRun {
                    command: "cargo test".into(),
                    candidate: "abc".into(),
                    exit_status: 0,
                }],
            })
        );
        assert_eq!(
            parse_status("ended failed - \n", &checks)
                .map(|s| matches!(s, TaskStatus::Ended { patch: None, .. })),
            Some(true)
        );
        assert_eq!(parse_status("check 9 0\n", &checks), None);
        let stop = parse_stop("at 7\nstarted 1\neffect d1\nstatus\ncancelled\n", &checks).unwrap();
        assert_eq!(stop.at, 7);
        assert!(stop.started);
        assert_eq!(stop.effects, vec!["d1".to_owned()]);
        assert_eq!(stop.status, TaskStatus::Cancelled);
        assert_eq!(request_name("stop-1"), "stop-1");
        assert_eq!(request_name("a/b").len(), 64);
    }
}

fn save_private(path: &std::path::Path, bytes: &[u8]) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    if std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(Error::Invalid("Boat state cannot be a symlink"));
    }
    let tmp = path.with_extension("json.tmp");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&tmp)
        .or_else(|error| {
            if error.kind() == std::io::ErrorKind::AlreadyExists
                && std::fs::symlink_metadata(&tmp)
                    .is_ok_and(|m| m.is_file() && !m.file_type().is_symlink())
            {
                std::fs::remove_file(&tmp)?;
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .mode(0o600)
                    .open(&tmp)
            } else {
                Err(error)
            }
        })
        .map_err(|_| Error::Invalid("cannot create private Boat state"))?;
    file.set_permissions(std::fs::Permissions::from_mode(0o600))
        .map_err(|_| Error::Invalid("cannot protect Boat state"))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| std::fs::rename(&tmp, path))
        .and_then(|()| {
            std::fs::File::open(
                path.parent()
                    .ok_or_else(|| std::io::Error::other("Boat state parent"))?,
            )?
            .sync_all()
        })
        .map_err(|_| Error::Invalid("cannot commit private Boat state"))
}
