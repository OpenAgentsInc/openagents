//! Core operations on an explicitly authorized workspace on an enrolled host.
//! Detach only removes logical access. It never deletes files or stops the host.

use base64::Engine as _;
use coder_access::enrolled::{self, Action, Answer, Call, Command, Progress, Root};
use coder_access::{Code, Error, Operation, Right};
use coder_host::client::{Device, Link};
use computer_driver::{
    Capabilities, CommandCursor, CommandProgress, CommandRead, CommandSpec, CreateRequest, Driver,
    Exec, ExecOutput, Meter, Observation, Outcome, State,
};
use computer_files::{FileError, FileInfo};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A verified NIP-HOST exchange. Implementations must authenticate the host,
/// check the reply's binding, and return uncertainty when a reply is lost.
#[allow(async_fn_in_trait)]
pub trait Transport: Clone {
    fn device(&self) -> &Device;
    async fn call(&self, call: Call) -> coder_access::Result<Answer>;
}

/// Existing signed NIP-HOST transport. A relay call opens a fresh exchange;
/// a lost connection never causes an automatic retry of an effect.
#[derive(Clone)]
pub struct HostTransport {
    device: Arc<Device>,
    link: Option<Arc<Link>>,
}
impl HostTransport {
    pub fn relay(device: Arc<Device>) -> Self {
        Self { device, link: None }
    }
    pub fn linked(link: Arc<Link>) -> Self {
        Self {
            device: link.device().clone(),
            link: Some(link),
        }
    }
}
impl Transport for HostTransport {
    fn device(&self) -> &Device {
        &self.device
    }
    async fn call(&self, call: Call) -> coder_access::Result<Answer> {
        let expected = call.clone();
        let relay;
        let link = match &self.link {
            Some(link) => &**link,
            None => {
                relay = Link::relay(self.device.clone(), self.device.relay().into());
                &relay
            }
        };
        let outcome = link
            .call(Operation::Enrolled { computer: call })
            .await
            .map_err(|error| match error {
                coder_host::Error::Access(error) => error,
                _ => Error::new(
                    Code::Transport,
                    "The computer did not answer. Its work may still be running.",
                ),
            })?;
        match outcome {
            coder_access::Outcome::Enrolled { computer } if computer.answers(&expected) => {
                Ok(computer)
            }
            _ => Err(Error::new(
                Code::Transport,
                "The computer's reply could not be checked.",
            )),
        }
    }
}

#[derive(Clone)]
pub struct Enrolled<T: Transport = HostTransport> {
    transport: T,
    account: String,
    host: String,
    root: Root,
}
impl<T: Transport> Enrolled<T> {
    /// Bind a service-authenticated account to a verified device grant. The
    /// account, host, and workspace come from trusted enrollment configuration.
    pub fn new(
        transport: T,
        account: &str,
        host: &str,
        workspace: &str,
    ) -> coder_access::Result<Self> {
        let device = transport.device();
        let grant = &device.access().grant;
        let scope = grant
            .scope
            .as_ref()
            .ok_or_else(|| Error::new(Code::Forbidden, "Choose a workspace connection."))?;
        if device.host() != host
            || scope.account != account
            || !grant.rights.contains(Right::Observe)
        {
            return Err(Error::new(
                Code::Forbidden,
                "This workspace is not available for your account.",
            ));
        }
        let root = scope
            .roots
            .iter()
            .find(|root| root.workspace == workspace)
            .cloned()
            .ok_or_else(|| Error::new(Code::Forbidden, "This workspace is not shared."))?;
        Ok(Self {
            transport,
            account: account.into(),
            host: host.into(),
            root,
        })
    }
    pub fn account(&self) -> &str {
        &self.account
    }
    pub fn host(&self) -> &str {
        &self.host
    }
    pub fn workspace(&self) -> &str {
        &self.root.workspace
    }
    /// Connection identity changes when the scoped device, epoch, or root changes.
    pub fn fingerprint(&self) -> String {
        enrolled::hash(
            &serde_json::to_vec(&(
                &self.account,
                &self.host,
                self.transport.device().key(),
                self.transport.device().epoch(),
                &self.root,
            ))
            .expect("connection encodes"),
        )
    }
    async fn call(&self, action: Action) -> coder_access::Result<Answer> {
        if !computer_files::valid_digest(action.computer()) {
            return Err(Error::new(Code::Malformed, "The computer ID is invalid."));
        }
        let call = Call {
            account: self.account.clone(),
            action,
        };
        call.validate()?;
        let answer = self.transport.call(call.clone()).await?;
        if !answer.answers(&call) {
            return Err(Error::new(
                Code::Transport,
                "The computer's reply could not be checked.",
            ));
        }
        answer.validate()?;
        Ok(answer)
    }
    fn file_path(&self, path: &str) -> Result<String, FileError> {
        computer_files::validate_path(path)?;
        Ok(format!("{}/{path}", self.root.path))
    }
    async fn file(
        &self,
        computer: &str,
        file: coder_access::computer::Request,
    ) -> Result<coder_access::computer::Answer, FileError> {
        match self
            .call(Action::File {
                computer: computer.into(),
                file,
            })
            .await
            .map_err(file_error)?
        {
            Answer::File { file } => file.able().map_err(file_error),
            _ => Err(FileError::Unavailable),
        }
    }
    fn command(&self, spec: &CommandSpec) -> coder_access::Result<Command> {
        if !spec.credential_names.is_empty() || spec.id.is_empty() || spec.id.len() > 256 {
            return Err(Error::new(
                Code::Unsupported,
                "Credentials are not supported on this workspace connection.",
            ));
        }
        let mut command = Command {
            id: enrolled::hash(spec.id.as_bytes()),
            script: spec.command.clone(),
            cwd: spec.cwd.clone(),
            env: spec.env.clone(),
            timeout_seconds: spec.timeout_seconds,
            digest: String::new(),
            source_digest: spec.digest.clone(),
        };
        command.digest = command.content_hash();
        command.validate()?;
        Ok(command)
    }
}
fn outcome<T>(error: Error) -> Outcome<T> {
    if matches!(
        error.code,
        Code::Transport | Code::Unavailable | Code::Stale
    ) {
        Outcome::unknown("The computer did not confirm the result. Its work may still be running.")
    } else {
        Outcome::failed(error.message)
    }
}
fn file_error(error: Error) -> FileError {
    match error.code {
        Code::Forbidden | Code::MissingRight | Code::Revoked | Code::Expired => FileError::Escape,
        Code::Bounds => FileError::TooLarge,
        Code::Conflict => FileError::Exists,
        Code::Unsupported => FileError::Unsupported,
        _ => FileError::Unavailable,
    }
}
impl<T: Transport> Driver for Enrolled<T> {
    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new("enrolled", [], &["workspace"]);
        caps.command_sessions = self
            .transport
            .device()
            .access()
            .grant
            .rights
            .contains(Right::Operate);
        caps.workspace_attachment = true;
        caps.workspace_file_bytes = Some(computer_files::MAX_BYTES);
        caps
    }
    async fn create(&self, request: &CreateRequest) -> Outcome<String> {
        if request.operation.is_empty()
            || request.operation.len() > 1024
            || request.size.as_deref().is_some_and(|s| s != "workspace")
            || request.image.is_some()
            || request.ttl_seconds.is_some()
        {
            return Outcome::failed(
                "unsupported: This connection attaches an existing workspace without an image, size, or lifetime setting.",
            );
        }
        let id = enrolled::hash(
            &serde_json::to_vec(&(self.fingerprint(), &request.operation))
                .expect("identity encodes"),
        );
        match self
            .call(Action::Attach {
                computer: id.clone(),
                workspace: self.root.workspace.clone(),
            })
            .await
        {
            Ok(Answer::Computer { attached: true, .. }) => Outcome::done(id),
            Ok(_) => Outcome::failed(
                "This workspace connection was detached. Use a new request to attach it again.",
            ),
            Err(error) => outcome(error),
        }
    }
    async fn observe(&self, computer: &str) -> Outcome<Observation> {
        match self
            .call(Action::Observe {
                computer: computer.into(),
            })
            .await
        {
            Ok(Answer::Computer { attached, .. }) => Outcome::done(Observation {
                state: if attached {
                    State::Running
                } else {
                    State::Gone
                },
                detail: None,
            }),
            Ok(_) => Outcome::unknown("The computer's status could not be checked."),
            Err(error) => outcome(error),
        }
    }
    async fn start_command(&self, computer: &str, spec: &CommandSpec) -> Outcome<String> {
        let command = match self.command(spec) {
            Ok(command) => command,
            Err(error) => return outcome(error),
        };
        let id = spec.id.clone();
        match self
            .call(Action::Start {
                computer: computer.into(),
                command,
            })
            .await
        {
            Ok(Answer::Command {
                progress: Progress::Lost,
                ..
            }) => Outcome::unknown(
                "The command's result is unavailable. It will not be started again.",
            ),
            Ok(Answer::Command { .. }) => Outcome::done(id),
            Ok(_) => Outcome::unknown("The computer did not confirm the command."),
            Err(error) => outcome(error),
        }
    }
    async fn read_command(
        &self,
        computer: &str,
        id: &str,
        cursor: CommandCursor,
        max_bytes: u64,
    ) -> Outcome<CommandRead> {
        if id.is_empty() || id.len() > 256 || max_bytes == 0 {
            return Outcome::failed("The command read settings are invalid.");
        }
        let max = (max_bytes.min(enrolled::MAX_OUTPUT as u64)) as usize;
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut digest = None;
        loop {
            let answer = self
                .call(Action::Read {
                    computer: computer.into(),
                    command: enrolled::hash(id.as_bytes()),
                    stdout: cursor.stdout + stdout.len() as u64,
                    stderr: cursor.stderr + stderr.len() as u64,
                    max_bytes: enrolled::MAX_READ
                        .min(max - stdout.len())
                        .min(max - stderr.len()),
                })
                .await;
            let (progress, source, out, err) = match answer {
                Ok(Answer::Command {
                    progress,
                    digest,
                    stdout,
                    stderr,
                    ..
                }) => (progress, digest, stdout, stderr),
                Ok(_) => return Outcome::unknown("The command's output could not be checked."),
                Err(error) => return outcome(error),
            };
            if digest.as_ref().is_some_and(|old| old != &source) {
                return Outcome::unknown("The command's output changed during the read.");
            }
            digest = Some(source);
            let drained = out.is_empty() && err.is_empty();
            stdout.extend(out);
            stderr.extend(err);
            if drained
                || stdout.len() >= max
                || stderr.len() >= max
                || matches!(progress, Progress::Running | Progress::Lost)
            {
                let (progress, timed_out) = match progress {
                    Progress::Running => (CommandProgress::Running, false),
                    Progress::Exited { code, timed_out } => {
                        (CommandProgress::Exited { code }, timed_out)
                    }
                    Progress::Lost => (CommandProgress::Lost, false),
                };
                return Outcome::done(CommandRead {
                    progress,
                    timed_out,
                    digest,
                    stdout,
                    stderr,
                });
            }
        }
    }
    async fn stop_command(&self, computer: &str, id: &str) -> Outcome<String> {
        if let Err(error) = self
            .call(Action::Stop {
                computer: computer.into(),
                command: enrolled::hash(id.as_bytes()),
            })
            .await
        {
            return outcome(error);
        }
        let until = Instant::now() + Duration::from_secs(5);
        while Instant::now() < until {
            match self
                .read_command(computer, id, CommandCursor::default(), 1)
                .await
            {
                Outcome::Done {
                    value:
                        CommandRead {
                            progress: CommandProgress::Exited { .. },
                            ..
                        },
                } => return Outcome::done(id.into()),
                Outcome::Done {
                    value:
                        CommandRead {
                            progress: CommandProgress::Running,
                            ..
                        },
                } => tokio::time::sleep(Duration::from_millis(100)).await,
                _ => {
                    return Outcome::unknown(
                        "The computer did not confirm that the command stopped.",
                    );
                }
            }
        }
        Outcome::unknown("The computer did not confirm that the command stopped.")
    }
    async fn exec(&self, computer: &str, exec: &Exec) -> Outcome<ExecOutput> {
        if !exec.secret_env.is_empty() {
            return Outcome::failed(
                "unsupported: Credentials are not supported on this workspace connection.",
            );
        }
        let spec = CommandSpec {
            id: coder_access::protocol::random_id(),
            command: exec.command.clone(),
            cwd: exec.cwd.clone().unwrap_or_default(),
            credential_names: Default::default(),
            env: Default::default(),
            timeout_seconds: exec.timeout_seconds,
            digest: enrolled::hash(exec.command.as_bytes()),
        };
        let id = match self.start_command(computer, &spec).await {
            Outcome::Done { value } => value,
            Outcome::Failed { reason } => return Outcome::failed(reason),
            Outcome::Unknown { reason } => return Outcome::unknown(reason),
        };
        let mut cursor = CommandCursor::default();
        let until = Instant::now() + Duration::from_secs(exec.timeout_seconds + 15);
        let mut out = Vec::new();
        let mut err = Vec::new();
        while Instant::now() < until {
            let read = match self
                .read_command(computer, &id, cursor, enrolled::MAX_READ as u64)
                .await
            {
                Outcome::Done { value } => value,
                Outcome::Failed { reason } => return Outcome::failed(reason),
                Outcome::Unknown { reason } => return Outcome::unknown(reason),
            };
            cursor.stdout += read.stdout.len() as u64;
            cursor.stderr += read.stderr.len() as u64;
            exec.output.append(false, &read.stdout);
            exec.output.append(true, &read.stderr);
            let drained = read.stdout.is_empty() && read.stderr.is_empty();
            out.extend(read.stdout);
            err.extend(read.stderr);
            match read.progress {
                CommandProgress::Exited { code } if drained => {
                    return Outcome::done(ExecOutput {
                        code,
                        stdout: String::from_utf8_lossy(&out).into_owned(),
                        stderr: String::from_utf8_lossy(&err).into_owned(),
                    });
                }
                CommandProgress::Lost | CommandProgress::Absent => {
                    return Outcome::unknown(
                        "The command's result is unavailable. It will not be started again.",
                    );
                }
                _ => tokio::time::sleep(Duration::from_millis(100)).await,
            }
        }
        Outcome::unknown("The computer did not confirm the command's result.")
    }
    async fn read_workspace_file(&self, computer: &str, path: &str) -> Result<Vec<u8>, FileError> {
        use coder_access::computer::{Answer as A, Request as R};
        let path = self.file_path(path)?;
        let A::File { file } = self.file(computer, R::Stat { path: path.clone() }).await? else {
            return Err(FileError::Unavailable);
        };
        if file.path != path || file.size > computer_files::MAX_BYTES as u64 {
            return Err(FileError::TooLarge);
        }
        let mut bytes = Vec::with_capacity(file.size as usize);
        while bytes.len() < (file.size as usize) {
            let offset = bytes.len() as u64;
            let length = coder_access::computer::CHUNK_BYTES.min(file.size - offset);
            let A::Chunk { offset: got, data } = self
                .file(
                    computer,
                    R::Read {
                        path: path.clone(),
                        offset,
                        length,
                    },
                )
                .await?
            else {
                return Err(FileError::Unavailable);
            };
            let chunk = base64::engine::general_purpose::STANDARD
                .decode(data)
                .map_err(|_| FileError::Changed)?;
            if got != offset || chunk.len() as u64 != length {
                return Err(FileError::Changed);
            }
            bytes.extend(chunk);
        }
        if coder_access::computer::digest(&bytes) != file.digest {
            return Err(FileError::Changed);
        }
        Ok(bytes)
    }
    async fn write_workspace_file(
        &self,
        computer: &str,
        path: &str,
        bytes: &[u8],
        overwrite: bool,
    ) -> Result<FileInfo, FileError> {
        use coder_access::computer::{Answer as A, FilePut, Request as R};
        let path = self.file_path(path)?;
        if bytes.len() > computer_files::MAX_BYTES {
            return Err(FileError::TooLarge);
        }
        let digest = coder_access::computer::digest(bytes);
        let chunks = bytes
            .chunks(coder_access::computer::CHUNK_BYTES as usize)
            .collect::<Vec<_>>();
        let chunks = if chunks.is_empty() {
            vec![&[][..]]
        } else {
            chunks
        };
        for (index, chunk) in chunks.iter().enumerate() {
            let offset = index as u64 * coder_access::computer::CHUNK_BYTES;
            let result = self
                .file(
                    computer,
                    R::Write {
                        put: FilePut {
                            path: path.clone(),
                            size: bytes.len() as u64,
                            digest: digest.clone(),
                            offset,
                            data: base64::engine::general_purpose::STANDARD.encode(chunk),
                            overwrite,
                        },
                    },
                )
                .await;
            let A::Written { received, complete } = result.map_err(|e| {
                if e == FileError::Unavailable {
                    FileError::WriteUncertain
                } else {
                    e
                }
            })?
            else {
                return Err(FileError::WriteUncertain);
            };
            if received < offset + chunk.len() as u64
                || received > bytes.len() as u64
                || index + 1 == chunks.len() && !complete
            {
                return Err(FileError::WriteUncertain);
            }
        }
        Ok(FileInfo {
            size: bytes.len(),
            sha256: computer_files::digest(bytes),
        })
    }
    async fn put_file(&self, computer: &str, path: &str, bytes: &[u8]) -> Outcome<()> {
        match self.write_workspace_file(computer, path, bytes, true).await {
            Ok(_) => Outcome::done(()),
            Err(FileError::WriteUncertain) => Outcome::unknown(FileError::WriteUncertain.message()),
            Err(error) => Outcome::failed(error.message()),
        }
    }
    async fn get_file(&self, computer: &str, path: &str) -> Outcome<Vec<u8>> {
        match self.read_workspace_file(computer, path).await {
            Ok(bytes) => Outcome::done(bytes),
            Err(error) => Outcome::failed(error.message()),
        }
    }
    async fn meter(&self, computer: &str) -> Outcome<Meter> {
        match self.observe(computer).await {
            Outcome::Done {value}=>Outcome::done(Meter {running:value.state==State::Running,evidence:"An attached workspace uses your hardware. Compute, electricity, and network costs are not measured by this connection.".into(),seconds:None}),
            Outcome::Failed {reason}=>Outcome::failed(reason),Outcome::Unknown {reason}=>Outcome::unknown(reason),
        }
    }
    async fn delete(&self, computer: &str) -> Outcome<()> {
        match self
            .call(Action::Detach {
                computer: computer.into(),
            })
            .await
        {
            Ok(Answer::Computer {
                attached: false, ..
            }) => Outcome::done(()),
            Ok(_) => Outcome::unknown("The computer did not confirm the disconnect."),
            Err(error) => outcome(error),
        }
    }
}
