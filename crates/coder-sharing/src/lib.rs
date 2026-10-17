//! Project sharing tied to one authenticated account and one private device key.
//! The application supplies the current account; this crate reads no saved login.
pub mod model;
mod store;
use base64::Engine as _;
use coder_access::enrolled::hash;
use coder_access::{Access, Code, Operation, Outcome, RelayPolicy, Right, Rights};
use coder_host::client::{Device, Link};
use computer_driver::{
    CommandCursor, CommandProgress, CommandRead, CommandSpec, CreateRequest, Driver,
    Outcome as DriverOutcome,
};
use computer_enrolled::{Enrolled, HostTransport};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::Arc,
};

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: Code,
    pub message: String,
}
pub(crate) fn failed(message: &str) -> Error {
    Error {
        code: Code::Unavailable,
        message: message.into(),
    }
}
fn access(error: coder_access::Error) -> Error {
    let message = match error.code {
        Code::Forbidden | Code::MissingRight | Code::Denied => {
            "This computer has not allowed that action."
        }
        Code::Revoked => "The owner removed this device's access.",
        Code::Expired => "This invitation or permission expired. Ask for a new one.",
        Code::Unsupported => "Update this computer to use project sharing.",
        Code::Conflict => "This request was already used with different settings.",
        Code::Bounds => "This computer cannot accept more work right now.",
        Code::Malformed => "Check the request and try again.",
        _ => "Could not reach this computer. Try again later.",
    };
    Error {
        code: error.code,
        message: message.into(),
    }
}
fn transport(error: coder_host::Error) -> Error {
    match error {
        coder_host::Error::Access(error) => access(error),
        _ => failed("The computer did not confirm the result. Check it before trying again."),
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Connection {
    access: Access,
    label: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pending {
    connection: String,
    packet: coder_access::Pending,
    reply: Option<Outcome>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Profile {
    v: String,
    account: String,
    connections: BTreeMap<String, Connection>,
    pending: BTreeMap<String, Pending>,
    #[serde(default)]
    attachments: BTreeMap<String, String>,
    #[serde(default)]
    pub(crate) jobs: BTreeMap<String, Job>,
    #[serde(default)]
    contacts: BTreeMap<String, String>,
}
impl Profile {
    fn new(account: &str) -> Self {
        Self {
            v: "openagents.project-sharing.v1".into(),
            account: account.into(),
            connections: BTreeMap::new(),
            pending: BTreeMap::new(),
            attachments: BTreeMap::new(),
            jobs: BTreeMap::new(),
            contacts: BTreeMap::new(),
        }
    }
}

/// A code the recipient copies from their signed-in account. The owner
/// selects this account/device pair explicitly; a code grants no host access.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipient {
    pub account: String,
    pub label: String,
    pub device: String,
}
impl Recipient {
    pub fn encode(&self) -> String {
        format!(
            "openagents-share-to:{}",
            base64::engine::general_purpose::URL_SAFE_NO_PAD
                .encode(serde_json::to_vec(self).expect("recipient encodes"))
        )
    }
    pub fn parse(code: &str) -> Result<Self> {
        if code.len() > 2048 {
            return Err(failed("The recipient code is too long."));
        }
        let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
            .decode(
                code.trim()
                    .strip_prefix("openagents-share-to:")
                    .ok_or_else(|| {
                        failed("Copy the recipient's device code from Project sharing.")
                    })?,
            )
            .map_err(|_| failed("The recipient code could not be read."))?;
        let recipient: Self = serde_json::from_slice(&bytes)
            .map_err(|_| failed("The recipient code could not be read."))?;
        if recipient.account.is_empty()
            || recipient.account.len() > 128
            || recipient.label.is_empty()
            || recipient.label.len() > 128
            || recipient
                .account
                .chars()
                .chain(recipient.label.chars())
                .any(char::is_control)
            || !computer_files_digest(&recipient.device)
        {
            return Err(failed("The recipient code could not be read."));
        }
        Ok(recipient)
    }
}
fn computer_files_digest(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[derive(Clone, Debug)]
pub struct Computer {
    pub id: String,
    pub label: String,
    pub shared: bool,
    pub projects: Vec<String>,
    pub expires_at: u64,
    pub can_run: bool,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Job {
    pub id: String,
    pub connection: String,
    pub project: String,
    pub computer: String,
    pub spec: CommandSpec,
    pub finished: bool,
}
fn driver_result<T>(result: DriverOutcome<T>) -> Result<T> {
    match result {
        DriverOutcome::Done { value } => Ok(value),
        DriverOutcome::Failed { reason } => Err(Error {
            code: Code::Denied,
            message: reason,
        }),
        DriverOutcome::Unknown { reason } => Err(Error {
            code: Code::Unavailable,
            message: reason,
        }),
    }
}

#[derive(Clone)]
pub struct Client {
    directory: PathBuf,
    account: String,
    label: String,
    secret: secp256k1::SecretKey,
    policy: RelayPolicy,
}
impl Client {
    pub fn open(directory: &Path, account: &str, label: &str, policy: RelayPolicy) -> Result<Self> {
        if account.is_empty()
            || account.len() > 128
            || label.is_empty()
            || label.len() > 128
            || account.chars().chain(label.chars()).any(char::is_control)
        {
            return Err(failed("Sign in to manage project sharing."));
        }
        let store = store::Store::open(directory)?;
        let profile = store.profile(account)?;
        let secret = store.key(account)?;
        for connection in profile.connections.values() {
            let grant = &connection.access.grant;
            if connection.label.is_empty()
                || connection.label.len() > 128
                || connection.label.chars().any(char::is_control)
            {
                return Err(failed("The saved computer name could not be read."));
            }
            if let Err(error) = connection.access.verify(
                &secret,
                coder_access::unix_time().map_err(access)?,
                policy,
            ) {
                if error.code != Code::Expired {
                    return Err(access(error));
                }
            }

            if grant
                .scope
                .as_ref()
                .is_some_and(|scope| scope.account != account)
                || grant.device != coder_host::reach::pubkey(&secret)
            {
                return Err(failed(
                    "This computer list belongs to another account or device.",
                ));
            }
        }
        store.save(&profile)?;
        Ok(Self {
            directory: directory.into(),
            account: account.into(),
            label: label.into(),
            secret,
            policy,
        })
    }
    pub fn recipient(&self) -> Recipient {
        Recipient {
            account: self.account.clone(),
            label: self.label.clone(),
            device: coder_host::reach::pubkey(&self.secret),
        }
    }
    pub fn computers(&self) -> Result<Vec<Computer>> {
        let store = store::Store::open(&self.directory)?;
        let profile = store.profile(&self.account)?;
        Ok(profile
            .connections
            .iter()
            .map(|(id, connection)| {
                let grant = &connection.access.grant;
                Computer {
                    id: id.clone(),
                    label: connection.label.clone(),
                    shared: grant.scope.is_some(),
                    projects: grant
                        .scope
                        .as_ref()
                        .map(|scope| {
                            scope
                                .roots
                                .iter()
                                .map(|root| root.workspace.clone())
                                .collect()
                        })
                        .unwrap_or_default(),
                    expires_at: grant.expires_at,
                    can_run: grant.rights.contains(Right::Operate),
                }
            })
            .collect())
    }
    fn device(&self, id: &str) -> Result<Arc<Device>> {
        let store = store::Store::open(&self.directory)?;
        let profile = store.profile(&self.account)?;
        let connection = profile
            .connections
            .get(id)
            .ok_or_else(|| failed("Choose a connected computer."))?;
        Ok(Arc::new(
            Device::new(connection.access.clone(), self.secret, self.policy).map_err(transport)?,
        ))
    }
    pub async fn connect(&self, code: &str, shared: bool, label: &str) -> Result<String> {
        if label.is_empty() || label.len() > 128 || label.chars().any(char::is_control) {
            return Err(failed("Choose a computer name."));
        }
        let access = if shared {
            coder_access::client::redeem_project(code, &self.secret, &self.account, self.policy)
                .await
        } else {
            coder_access::client::redeem(code, &self.secret, self.policy).await
        }
        .map_err(access)?;
        if shared {
            if access
                .grant
                .scope
                .as_ref()
                .is_none_or(|scope| scope.account != self.account)
            {
                return Err(failed(
                    "This invitation does not share projects with your account.",
                ));
            }
        } else if access.grant.scope.is_some()
            || !access.grant.rights.contains(Right::AccessAdmin)
            || !access.grant.rights.contains(Right::Observe)
            || !access.grant.rights.contains(Right::Operate)
        {
            return Err(failed(
                "Use a computer owner's connect code to manage sharing.",
            ));
        }
        let id = hash(
            &serde_json::to_vec(&(&access.grant.host, &access.grant.scope))
                .expect("identity encodes"),
        );
        let store = store::Store::open(&self.directory)?;
        let mut profile = store.profile(&self.account)?;
        if profile.connections.len() >= 64 && !profile.connections.contains_key(&id) {
            return Err(failed("Remove an unused computer before adding another."));
        }
        profile.connections.insert(
            id.clone(),
            Connection {
                access,
                label: label.into(),
            },
        );
        store.save(&profile)?;
        Ok(id)
    }
    async fn read(&self, id: &str, operation: Operation) -> Result<Outcome> {
        let device = self.device(id)?;
        Link::relay(device.clone(), device.relay().into())
            .call(operation)
            .await
            .map_err(transport)
    }
    pub async fn projects(&self, id: &str) -> Result<Vec<String>> {
        let device = self.device(id)?;
        if device.access().grant.scope.is_some() {
            return Err(failed("Choose a computer you can manage."));
        }
        // A newly enrolled device can precede the host's next presence tick.
        // Retry only this read; never turn a missing capability into permission.
        let reach = tokio::time::timeout(std::time::Duration::from_secs(4), async {
            loop {
                match coder_host::client::fetch_reach(&device, device.relay()).await {
                    Ok(reach) => return Ok(reach),
                    Err(coder_host::Error::Reach(_)) => {
                        tokio::time::sleep(std::time::Duration::from_millis(250)).await
                    }
                    Err(error) => return Err(transport(error)),
                }
            }
        })
        .await
        .map_err(|_| failed("This computer has not replied yet. Try again."))??;
        if !reach
            .presence
            .presence
            .capabilities
            .iter()
            .any(|capability| capability == coder_access::sharing::CAPABILITY)
        {
            return Err(Error {
                code: Code::Unsupported,
                message: "Update this computer before sharing projects.".into(),
            });
        }
        match self.read(id, Operation::ListWorkspaces {}).await? {
            Outcome::Workspaces { workspaces } => Ok(workspaces),
            _ => Err(failed("The project list could not be read.")),
        }
    }
    /// Save the exact signed request before sending it. A lost reply can be
    /// retried under the same operation ID without issuing another invitation.
    async fn effect(&self, id: &str, operation_id: &str, operation: Operation) -> Result<Outcome> {
        if !computer_files_digest(operation_id) {
            return Err(failed("The request ID is invalid."));
        }
        let device = self.device(id)?;
        let packet = {
            let store = store::Store::open(&self.directory)?;
            let mut profile = store.profile(&self.account)?;
            if let Some(pending) = profile.pending.get(operation_id) {
                if pending.connection != id || pending.packet.request.op != operation {
                    return Err(Error {
                        code: Code::Conflict,
                        message: "This request was already used with different settings.".into(),
                    });
                }
                if let Some(reply) = &pending.reply {
                    return Ok(reply.clone());
                }
                pending.packet.clone()
            } else {
                if profile.pending.len() >= 128 {
                    return Err(failed("This device has too many saved sharing requests."));
                }
                let packet = device
                    .prepare_operation(operation, operation_id.into())
                    .map_err(transport)?;
                profile.pending.insert(
                    operation_id.into(),
                    Pending {
                        connection: id.into(),
                        packet: packet.clone(),
                        reply: None,
                    },
                );
                store.save(&profile)?;
                packet
            }
        };
        let reply = Link::relay(device.clone(), device.relay().into())
            .send_pending(&packet)
            .await
            .map_err(transport)?;
        let store = store::Store::open(&self.directory)?;
        let mut profile = store.profile(&self.account)?;
        let pending = profile
            .pending
            .get_mut(operation_id)
            .ok_or_else(|| failed("The saved sharing request is unavailable."))?;
        pending.reply = Some(reply.clone());
        store.save(&profile)?;
        Ok(reply)
    }
    pub async fn invite(
        &self,
        id: &str,
        operation_id: &str,
        recipient: &Recipient,
        mut projects: Vec<String>,
        can_run: bool,
        expires_at: u64,
    ) -> Result<(String, u64)> {
        projects.sort();
        projects.dedup();
        // Verify support on this enrolled route before sending an effect.
        let available = self.projects(id).await?;
        if projects.iter().any(|project| !available.contains(project)) {
            return Err(failed("Choose projects from this computer."));
        }
        Recipient::parse(&recipient.encode())?;
        {
            let store = store::Store::open(&self.directory)?;
            let mut profile = store.profile(&self.account)?;
            if profile.contacts.len() < 64 || profile.contacts.contains_key(&recipient.account) {
                profile
                    .contacts
                    .insert(recipient.account.clone(), recipient.label.clone());
                store.save(&profile)?;
            }
        }
        let rights = if can_run {
            Rights::parse_list("observe,operate")
        } else {
            Rights::parse_list("observe")
        }
        .map_err(access)?;
        let share = coder_access::sharing::Invite {
            account: recipient.account.clone(),
            device: recipient.device.clone(),
            workspaces: projects,
            rights,
            expires_at,
        };
        share.validate().map_err(access)?;
        match self
            .effect(id, operation_id, Operation::InviteProject { share })
            .await?
        {
            Outcome::Invitation {
                code, expires_at, ..
            } => Ok((code, expires_at)),
            _ => Err(failed("The invitation could not be read.")),
        }
    }
    pub async fn shares(&self, id: &str) -> Result<Vec<coder_access::protocol::DeviceEntry>> {
        match self.read(id, Operation::ListDevices {}).await? {
            Outcome::Devices { devices } => Ok(devices
                .into_iter()
                .filter(|device| device.scope.is_some())
                .collect()),
            _ => Err(failed("The shared project list could not be read.")),
        }
    }
    pub async fn revoke(&self, id: &str, operation_id: &str, grant: &str) -> Result<()> {
        match self
            .effect(
                id,
                operation_id,
                Operation::RevokeProject {
                    grant: grant.into(),
                },
            )
            .await?
        {
            Outcome::ProjectRevoked { .. } => Ok(()),
            _ => Err(failed(
                "The computer did not confirm that access was removed.",
            )),
        }
    }
    pub fn workspace(&self, id: &str, project: &str) -> Result<Enrolled> {
        let device = self.device(id)?;
        let host = device.host().to_owned();
        Enrolled::new(HostTransport::relay(device), &self.account, &host, project).map_err(access)
    }
    pub fn contacts(&self) -> Result<BTreeMap<String, String>> {
        let store = store::Store::open(&self.directory)?;
        Ok(store.profile(&self.account)?.contacts)
    }
    pub async fn workspace_computer(&self, id: &str, project: &str) -> Result<(Enrolled, String)> {
        let driver = self.workspace(id, project)?;
        let key = hash(&serde_json::to_vec(&(id, project)).expect("workspace encodes"));
        let operation = {
            let store = store::Store::open(&self.directory)?;
            let mut profile = store.profile(&self.account)?;
            if let Some(operation) = profile.attachments.get(&key) {
                operation.clone()
            } else {
                if profile.attachments.len() >= 128 {
                    return Err(failed("Remove an unused workspace before opening another."));
                }
                let operation = coder_access::protocol::random_id();
                profile.attachments.insert(key, operation.clone());
                store.save(&profile)?;
                operation
            }
        };
        let computer = driver_result(
            driver
                .create(&CreateRequest {
                    operation,
                    ..Default::default()
                })
                .await,
        )?;
        Ok((driver, computer))
    }
    pub async fn read_file(&self, id: &str, project: &str, path: &str) -> Result<Vec<u8>> {
        computer_files::validate_path(path).map_err(|error| failed(error.message()))?;
        let (driver, computer) = self.workspace_computer(id, project).await?;
        driver
            .read_workspace_file(&computer, path)
            .await
            .map_err(|error| failed(error.message()))
    }
    pub async fn write_file(
        &self,
        id: &str,
        project: &str,
        path: &str,
        bytes: &[u8],
    ) -> Result<()> {
        computer_files::validate_path(path).map_err(|error| failed(error.message()))?;
        if bytes.len() > computer_files::MAX_BYTES {
            return Err(failed("This file is too large to send."));
        }
        let (driver, computer) = self.workspace_computer(id, project).await?;
        driver
            .write_workspace_file(&computer, path, bytes, true)
            .await
            .map_err(|error| failed(error.message()))?;
        Ok(())
    }
    pub fn jobs(&self) -> Result<Vec<Job>> {
        let store = store::Store::open(&self.directory)?;
        Ok(store.profile(&self.account)?.jobs.into_values().collect())
    }
    pub async fn start(
        &self,
        id: &str,
        project: &str,
        job_id: &str,
        script: &str,
        timeout: u64,
    ) -> Result<Job> {
        if !computer_files_digest(job_id)
            || script.is_empty()
            || script.len() > 32 * 1024
            || script.contains('\0')
            || !(1..=coder_access::enrolled::MAX_TIMEOUT).contains(&timeout)
        {
            return Err(failed(
                "Choose a command of at most 32 KiB and a timeout from 1 to 300 seconds.",
            ));
        }
        let (driver, computer) = self.workspace_computer(id, project).await?;
        let spec = CommandSpec {
            id: job_id.into(),
            command: script.into(),
            cwd: ".".into(),
            credential_names: Default::default(),
            env: Default::default(),
            timeout_seconds: timeout,
            digest: hash(&serde_json::to_vec(&(job_id, script, timeout)).expect("command encodes")),
        };
        let job = Job {
            id: job_id.into(),
            connection: id.into(),
            project: project.into(),
            computer,
            spec,
            finished: false,
        };
        {
            let store = store::Store::open(&self.directory)?;
            let mut profile = store.profile(&self.account)?;
            if let Some(old) = profile.jobs.get(job_id) {
                if old.connection != job.connection
                    || old.project != job.project
                    || old.computer != job.computer
                    || old.spec != job.spec
                {
                    return Err(Error {
                        code: Code::Conflict,
                        message: "This command was already used with different settings.".into(),
                    });
                }
            } else {
                while profile.jobs.len() >= 32 {
                    let Some(old) = profile
                        .jobs
                        .values()
                        .find(|job| job.finished)
                        .map(|job| job.id.clone())
                    else {
                        return Err(failed("Finish an earlier command before starting another."));
                    };
                    profile.jobs.remove(&old);
                }
                profile.jobs.insert(job_id.into(), job.clone());
                store.save(&profile)?;
            }
        }
        driver_result(driver.start_command(&job.computer, &job.spec).await)?;
        Ok(job)
    }
    pub async fn read_job(&self, id: &str, cursor: CommandCursor) -> Result<CommandRead> {
        let job = self
            .jobs()?
            .into_iter()
            .find(|job| job.id == id)
            .ok_or_else(|| failed("This command is no longer saved on this device."))?;
        let driver = self.workspace(&job.connection, &job.project)?;
        let read = driver_result(
            driver
                .read_command(&job.computer, &job.id, cursor, 8192)
                .await,
        )?;
        if matches!(
            read.progress,
            CommandProgress::Exited { .. } | CommandProgress::Lost
        ) {
            let store = store::Store::open(&self.directory)?;
            let mut profile = store.profile(&self.account)?;
            if let Some(job) = profile.jobs.get_mut(id) {
                job.finished = true;
            }
            store.save(&profile)?;
        }
        Ok(read)
    }
    pub async fn stop_job(&self, id: &str) -> Result<()> {
        let job = self
            .jobs()?
            .into_iter()
            .find(|job| job.id == id)
            .ok_or_else(|| failed("This command is no longer saved on this device."))?;
        let driver = self.workspace(&job.connection, &job.project)?;
        driver_result(driver.stop_command(&job.computer, &job.id).await)?;
        Ok(())
    }
    pub fn forget(&self, id: &str) -> Result<()> {
        let store = store::Store::open(&self.directory)?;
        let mut profile = store.profile(&self.account)?;
        profile.connections.remove(id);
        store.save(&profile)
    }
}
