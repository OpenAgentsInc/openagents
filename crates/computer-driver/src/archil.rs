//! Archil sandbox driver (#12138), over our port of Archil's SDK
//! (`crates/archil`).
//!
//! One computer is one Archil sandbox on its own root disk. Attaching a
//! separate Archil disk failed at boot on Archil's side in the 2026-10-18
//! trial, so the driver never mounts one.
//!
//! - **Core.** Create, observe, exec, identified commands, files, meter,
//!   delete. Archil takes no idempotency key on sandbox create, so the
//!   driver names the sandbox after a digest of the create operation and
//!   looks that name up before creating. A retried create after a lost reply
//!   returns the same sandbox. Two creates racing under one operation can
//!   still both reach Archil; the lookup then settles on the oldest.
//! - **Suspend.** Archil pause keeps memory and processes; resume continues
//!   them. Resume also cold-starts a stopped sandbox. `with_stop_suspend`
//!   switches suspend to Archil stop, which keeps only the disk.
//! - **Fork.** Archil fork of a running, paused, or stopped sandbox, named
//!   after the source and the fork operation, so a repeated fork returns
//!   the same child.
//! - **Snapshot** is not declared: Archil has no named disk snapshot that a
//!   new sandbox can be created from.
//!
//! Commands run through Archil's process API. A background process started
//! from a shell command dies when that command returns, so an identified
//! command's wrapper is itself the process, started and then disconnected.
//! Its spool directory claim (`mkdir`) makes a retried start a no-op.
//! Secrets travel over the process's stdin, never in its start request,
//! because Archil may keep a process's start environment.

use crate::*;
use ::archil::{
    Archil, ClientOptions, CreateSandboxRequest, ListSandboxesOptions, ProcessStartOptions,
    ProcessStatus, Sandbox, SandboxFileUploadOptions, SandboxForkOptions, SandboxStatus,
    SandboxTimeoutOptions, WaitOptions,
};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use sha2::{Digest, Sha256};
use std::time::Duration;

const MAX_FILE: usize = 16 * 1024 * 1024;
const MAX_OUTPUT: usize = 16 * 1024 * 1024;
const COMMANDS: &str = "/tmp/oa-public-commands";
/// Archil's refusal to delete a sandbox that forks still depend on.
const HAS_FORKS: &str = "sandbox_has_forks";
const LIFECYCLE_DEADLINE: Duration = Duration::from_secs(300);
/// The reference that means "Archil's default image".
pub const DEFAULT_IMAGE: &str = "archil:default";
/// A reference to an image built with Archil's image API.
pub const IMAGE_ID_PREFIX: &str = "archil-image:";
/// Size classes: name, vCPUs, memory in MiB.
const SIZES: [(&str, u32, u32); 3] = [("small", 2, 4096), ("medium", 4, 8192), ("large", 8, 16384)];

/// The control-plane origin of a known Archil region.
pub fn region_url(region: &str) -> Option<String> {
    ::archil::resolve_base_url(region).ok()
}

#[derive(Clone)]
pub struct ArchilDriver {
    client: Archil,
    /// Region (and any other non-secret scope) that separates sandbox names
    /// of two deployments on one account.
    scope: String,
    image: Option<String>,
    home: String,
    memory_suspend: bool,
}

fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\"'\"'"))
}

fn short_code(error: &::archil::Error) -> String {
    // A server code is a short token; never echo a server message, which can
    // repeat command input.
    error
        .code()
        .filter(|c| {
            c.len() <= 64
                && c.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
        .map(|c| format!(", {c}"))
        .unwrap_or_default()
}

/// Map an SDK error. A refused request (4xx other than 408 and 429) did not
/// happen; anything else might have.
fn outcome<T>(what: &str, error: &::archil::Error) -> Outcome<T> {
    match error {
        ::archil::Error::InvalidArgument(_) | ::archil::Error::Config(_) => {
            return Outcome::failed(format!("Archil refused the {what} request."));
        }
        ::archil::Error::SandboxPause(_) => {
            return Outcome::failed("Archil couldn't pause the computer.");
        }
        _ => {}
    }
    match error.status() {
        Some(404) => Outcome::failed(format!("Archil: the computer doesn't exist ({what}).")),
        Some(s) if (400..500).contains(&s) && s != 408 && s != 429 => Outcome::failed(format!(
            "Archil refused the {what} (HTTP {s}{}).",
            short_code(error)
        )),
        Some(s) => Outcome::unknown(format!(
            "Archil: the {what} could not be confirmed (HTTP {s}{}).",
            short_code(error)
        )),
        None => Outcome::unknown(format!("Archil: the {what} could not be confirmed.")),
    }
}

fn fail<T>(o: Outcome<()>) -> Outcome<T> {
    match o {
        Outcome::Unknown { reason } => Outcome::unknown(reason),
        Outcome::Failed { reason } => Outcome::failed(reason),
        Outcome::Done { .. } => Outcome::unknown("Archil: unexpected state."),
    }
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn valid_operation(op: &str) -> bool {
    !op.is_empty() && op.len() <= 128 && op.bytes().all(|b| b.is_ascii_graphic())
}

fn state(status: SandboxStatus) -> State {
    match status {
        SandboxStatus::Running => State::Running,
        SandboxStatus::Pausing | SandboxStatus::Stopping | SandboxStatus::Deleting => {
            State::Stopping
        }
        SandboxStatus::Paused | SandboxStatus::Stopped | SandboxStatus::Exited => State::Stopped,
        SandboxStatus::Failed => State::Failed,
        SandboxStatus::Deleted => State::Gone,
        SandboxStatus::Pending | SandboxStatus::Unknown => State::Starting,
    }
}

/// Read `NAME=base64` lines from stdin until a blank line and export them.
const SECRET_READER: &str = "while IFS= read -r oa_line; do [ -n \"$oa_line\" ] || break; \
oa_name=${oa_line%%=*}; oa_value=$(printf %s \"${oa_line#*=}\" | base64 -d) || exit 125; \
export \"$oa_name=$oa_value\"; done\nunset oa_line oa_name oa_value\n";

fn secret_input<'a>(secrets: impl Iterator<Item = (&'a String, &'a str)>) -> String {
    let mut input = String::new();
    for (name, value) in secrets {
        input.push_str(name);
        input.push('=');
        input.push_str(&BASE64.encode(value));
        input.push('\n');
    }
    input.push('\n');
    input
}

fn valid_spec(spec: &CommandSpec) -> bool {
    let id = &spec.id;
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
        && !spec.command.is_empty()
        && spec.command.len() <= 65536
        && !spec.command.contains('\0')
        && !spec.cwd.is_empty()
        && !spec.cwd.starts_with('/')
        && !spec.cwd.chars().any(char::is_control)
        && (spec.cwd == "." || spec.cwd.split('/').all(|p| !p.is_empty() && p != ".."))
        && (1..=86400).contains(&spec.timeout_seconds)
        && spec.digest.len() == 64
        && spec.digest.bytes().all(|b| b.is_ascii_hexdigit())
        && spec.env.len() <= 64
        && spec
            .env
            .iter()
            .all(|(k, v)| env_name_ok(k) && v.len() <= 8192 && !v.contains('\0'))
        && spec
            .credential_names
            .iter()
            .all(|n| env_name_ok(n) && spec.env.contains_key(n))
}

fn id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && !id.starts_with('.')
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}

impl ArchilDriver {
    /// A driver for one region. The key stays inside the SDK client.
    pub fn new(api_key: Secret, region: &str) -> std::result::Result<Self, String> {
        Self::build(api_key, region, None)
    }

    /// Against an explicit control-plane origin (tests, private endpoints).
    pub fn with_base_url(
        api_key: Secret,
        region: &str,
        base_url: &str,
    ) -> std::result::Result<Self, String> {
        Self::build(api_key, region, Some(base_url))
    }

    fn build(
        api_key: Secret,
        region: &str,
        base_url: Option<&str>,
    ) -> std::result::Result<Self, String> {
        if api_key.expose().trim().is_empty() || region.trim().is_empty() {
            return Err("Archil needs an API key and a region.".into());
        }
        let mut options = ClientOptions::new()
            .api_key(api_key.expose())
            .region(region)
            .poll_interval(Duration::from_millis(250));
        if let Some(url) = base_url {
            options = options.base_url(url).s3_base_url(url);
        }
        let client = Archil::new(options).map_err(|_| "Couldn't configure the Archil client.")?;
        Ok(Self {
            client,
            scope: format!("{region}\0{}", base_url.unwrap_or("")),
            image: None,
            home: "/root".into(),
            memory_suspend: true,
        })
    }

    /// Reads `ARCHIL_API_KEY` and `ARCHIL_REGION`, and optionally
    /// `ARCHIL_IMAGE` (an OCI reference or `archil-image:ID`).
    pub fn from_env() -> std::result::Result<Self, String> {
        let required = |name| {
            std::env::var(name)
                .ok()
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| format!("Set {name} to use Archil."))
        };
        let driver = Self::new(
            Secret::new(required("ARCHIL_API_KEY")?),
            &required("ARCHIL_REGION")?,
        )?;
        match std::env::var("ARCHIL_IMAGE").ok().filter(|v| !v.is_empty()) {
            Some(image) => driver.with_image(&image),
            None => Ok(driver),
        }
    }

    /// The base image new computers start from.
    pub fn with_image(mut self, image: &str) -> std::result::Result<Self, String> {
        if !valid_image(image) {
            return Err("The Archil image reference is invalid.".into());
        }
        self.image = (image != DEFAULT_IMAGE).then(|| image.to_owned());
        Ok(self)
    }

    /// The directory relative paths and commands start in (default `/root`).
    pub fn with_home(mut self, home: &str) -> std::result::Result<Self, String> {
        if !home.starts_with('/') || home.contains(['\0', '\n', '\'']) || home.len() > 256 {
            return Err("The home directory must be an absolute path.".into());
        }
        self.home = home.trim_end_matches('/').to_owned();
        if self.home.is_empty() {
            self.home = "/".into();
        }
        Ok(self)
    }

    /// The SDK client, for operator tools (listing, cleanup).
    pub fn client(&self) -> &::archil::Archil {
        &self.client
    }

    /// Suspend with Archil stop (disk only) instead of pause (memory too).
    pub fn with_stop_suspend(mut self) -> Self {
        self.memory_suspend = false;
        self
    }

    fn name(&self, kind: &str, parts: &[&str]) -> String {
        let mut digest = Sha256::new();
        digest.update(b"openagents.computer-driver.archil.v1\0");
        digest.update(self.scope.as_bytes());
        for part in parts {
            digest.update([0]);
            digest.update(part.as_bytes());
        }
        let hex = format!("{:x}", digest.finalize());
        format!("oa-{kind}-{}", &hex[..32])
    }

    /// The oldest sandbox with this name, in any state.
    async fn find_named(&self, name: &str) -> ::archil::Result<Option<String>> {
        let all = self
            .client
            .sandboxes()
            .list(ListSandboxesOptions::default())
            .await?;
        Ok(all.into_iter().find(|s| s.name == name).map(|s| s.id))
    }

    async fn lookup(&self, computer: &str) -> std::result::Result<Option<Sandbox>, Outcome<()>> {
        if !valid_id(computer) {
            return Err(Outcome::failed("Invalid Archil computer ID."));
        }
        match tokio::time::timeout(
            Duration::from_secs(30),
            self.client.sandboxes().get(computer),
        )
        .await
        {
            Ok(Ok(sandbox)) => Ok(Some(sandbox)),
            Ok(Err(e)) if e.status() == Some(404) => Ok(None),
            Ok(Err(e)) => Err(outcome("lookup", &e)),
            Err(_) => Err(Outcome::unknown(
                "Archil: the lookup exceeded its deadline.",
            )),
        }
    }

    async fn get(&self, computer: &str) -> std::result::Result<Sandbox, Outcome<()>> {
        self.lookup(computer)
            .await?
            .ok_or_else(|| Outcome::failed("The computer doesn't exist."))
    }

    async fn running(&self, computer: &str) -> std::result::Result<Sandbox, Outcome<()>> {
        let sandbox = self.get(computer).await?;
        if sandbox.status != SandboxStatus::Running {
            return Err(Outcome::failed(format!(
                "The computer isn't running ({}).",
                sandbox.status
            )));
        }
        Ok(sandbox)
    }

    fn absolute(&self, path: &str) -> Option<String> {
        if path.is_empty()
            || path.len() > 4096
            || path.contains('\0')
            || path.split('/').any(|p| p == "..")
        {
            return None;
        }
        Some(if path.starts_with('/') {
            path.to_owned()
        } else {
            format!("{}/{path}", self.home)
        })
    }

    /// Run `script` as one process, feed `input` on stdin, wait.
    async fn process(
        &self,
        sandbox: &Sandbox,
        script: &str,
        input: Option<String>,
        timeout_seconds: u64,
        capture: &OutputCapture,
    ) -> ::archil::Result<::archil::ProcessResult> {
        let capture = capture.clone();
        let options = ProcessStartOptions::new()
            .timeout_seconds(timeout_seconds)
            .on_output(move |chunk| {
                capture.append(chunk.stream == ::archil::ProcessStream::Stderr, &chunk.data)
            });
        let process = sandbox.run(script, options).await?;
        let result = async {
            if let Some(input) = input {
                process.send_input(input.as_bytes()).await?;
            }
            process.close_stdin().await?;
            process.wait().await
        }
        .await;
        process.disconnect().await;
        result
    }
}

fn valid_image(image: &str) -> bool {
    !image.is_empty()
        && image.len() <= 512
        && !image
            .chars()
            .any(|c| c.is_control() || c.is_whitespace() || c == '?' || c == '#')
}

impl Driver for ArchilDriver {
    fn capabilities(&self) -> Capabilities {
        let mut caps = Capabilities::new(
            "archil",
            [Profile::Core, Profile::Suspend, Profile::Fork],
            &SIZES.map(|(name, _, _)| name),
        );
        caps.memory_suspend = self.memory_suspend;
        caps.command_sessions = true;
        caps.default_artifact = Some(Artifact {
            kind: ArtifactKind::ArchilImage,
            reference: self.image.clone().unwrap_or_else(|| DEFAULT_IMAGE.into()),
            provider_identity: None,
            disk_gib: None,
        });
        caps.shapes = SIZES
            .iter()
            .map(|(name, vcpus, memory)| {
                (
                    (*name).to_owned(),
                    Shape {
                        vcpus: *vcpus,
                        memory_mib: u64::from(*memory),
                        min_disk_gib: None,
                    },
                )
            })
            .collect();
        caps
    }

    async fn create(&self, r: &CreateRequest) -> Outcome<String> {
        if !valid_operation(&r.operation) {
            return Outcome::failed(
                "An Archil create needs an operation ID of up to 128 printable ASCII characters.",
            );
        }
        let size = r.size.as_deref().unwrap_or(SIZES[0].0);
        let Some((_, vcpus, memory)) = SIZES.iter().find(|(name, _, _)| *name == size) else {
            return Outcome::failed("Choose a supported Archil computer size.");
        };
        let image = match r.image.as_deref().or(self.image.as_deref()) {
            Some(image) if !valid_image(image) => {
                return Outcome::failed("The Archil image reference is invalid.");
            }
            Some(DEFAULT_IMAGE) | None => None,
            Some(image) => Some(image),
        };
        let name = self.name("c", &[&r.operation]);
        let mut request = CreateSandboxRequest::new()
            .name(&name)
            .vcpu_count(*vcpus)
            .mem_size_mib(*memory)
            // Our lifecycle owns stopping; a disconnected command must not
            // pause its own computer.
            .idle_ttl_seconds(0);
        if let Some(ttl) = r.ttl_seconds {
            request = request.max_ttl_seconds(ttl);
        }
        match image {
            Some(image) => match image.strip_prefix(IMAGE_ID_PREFIX) {
                Some(id) => request = request.image_id(id),
                None => request = request.base_image(image),
            },
            None => {}
        }
        let create = async {
            if let Some(id) = self.find_named(&name).await? {
                return Ok(id);
            }
            match self
                .client
                .sandboxes()
                .create_with(request, WaitOptions::no_wait())
                .await
            {
                Ok(sandbox) => Ok(sandbox.id),
                // A name conflict means an earlier attempt landed.
                Err(e) if e.status() == Some(409) => match self.find_named(&name).await? {
                    Some(id) => Ok(id),
                    None => Err(e),
                },
                Err(e) => Err(e),
            }
        };
        match tokio::time::timeout(Duration::from_secs(180), create).await {
            Ok(Ok(id)) if valid_id(&id) => Outcome::done(id),
            Ok(Ok(_)) => Outcome::unknown("Archil returned an invalid computer ID."),
            Ok(Err(e)) => outcome("create", &e),
            Err(_) => Outcome::unknown(
                "The Archil create exceeded its deadline. Repeat the same create operation to check it.",
            ),
        }
    }

    async fn observe(&self, computer: &str) -> Outcome<Observation> {
        match self.lookup(computer).await {
            Ok(sandbox) => Outcome::done(Observation {
                state: sandbox.map_or(State::Gone, |s| state(s.status)),
                detail: None,
            }),
            Err(e) => fail(e),
        }
    }

    async fn exec(&self, computer: &str, exec: &Exec) -> Outcome<ExecOutput> {
        if exec.secret_env.len() > 256
            || exec
                .secret_env
                .iter()
                .map(|(k, v)| k.len() + v.expose().len())
                .sum::<usize>()
                > 65536
            || exec.secret_env.iter().any(|(name, value)| {
                !env_name_ok(name) || value.expose().contains('\0') || value.expose().len() > 8192
            })
            || exec.command.contains('\0')
        {
            return Outcome::failed(
                "Use valid environment variable names and values without NUL bytes.",
            );
        }
        let cwd = match exec.cwd.as_deref() {
            None => self.home.clone(),
            Some(cwd) => match self.absolute(cwd) {
                Some(cwd) => cwd,
                None => return Outcome::failed("The working directory is invalid."),
            },
        };
        let sandbox = match self.running(computer).await {
            Ok(sandbox) => sandbox,
            Err(e) => return fail(e),
        };
        let secrets = !exec.secret_env.is_empty();
        let script = format!(
            "cd {} || exit 126\n{}exec sh -c {} </dev/null",
            quote(&cwd),
            if secrets { SECRET_READER } else { "" },
            quote(&exec.command)
        );
        let input = secrets.then(|| {
            secret_input(
                exec.secret_env
                    .iter()
                    .map(|(name, value)| (name, value.expose())),
            )
        });
        let seconds = exec.timeout_seconds.clamp(1, 86400);
        let run = self.process(&sandbox, &script, input, seconds, &exec.output);
        match tokio::time::timeout(Duration::from_secs(seconds + 45), run).await {
            Ok(Ok(result)) => {
                let code = match (result.status, result.exit_code) {
                    (_, Some(code)) => code,
                    (ProcessStatus::TimedOut, None) => 124,
                    _ => {
                        return Outcome::unknown(
                            "Archil did not report how the command ended. Check its effects before running it again.",
                        );
                    }
                };
                let mut stdout = result.stdout;
                let mut stderr = result.stderr;
                if stdout.len() > MAX_OUTPUT || stderr.len() > MAX_OUTPUT {
                    stdout.truncate(stdout.floor_char_boundary(MAX_OUTPUT));
                    stderr.truncate(stderr.floor_char_boundary(MAX_OUTPUT));
                }
                Outcome::done(ExecOutput {
                    code,
                    stdout,
                    stderr,
                })
            }
            Ok(Err(e)) => outcome("command", &e),
            Err(_) => Outcome::unknown(
                "The Archil command exceeded its deadline. Check its outcome before running it again.",
            ),
        }
    }

    async fn start_command(&self, computer: &str, spec: &CommandSpec) -> Outcome<String> {
        if !valid_spec(spec) {
            return Outcome::failed("Use valid command settings.");
        }
        let sandbox = match self.running(computer).await {
            Ok(sandbox) => sandbox,
            Err(e) => return fail(e),
        };
        let mut public = spec.clone();
        let mut credentials = BTreeMap::new();
        for name in &spec.credential_names {
            if let Some(value) = public.env.remove(name) {
                credentials.insert(name.clone(), value);
            }
        }
        let workspace = format!("{}/workspace", self.home);
        let script = format!(
            "cd {} || exit 125\n{SECRET_READER}{}",
            quote(&self.home),
            identified::bounded_command_script(COMMANDS, &workspace, &public)
        );
        let input = secret_input(credentials.iter().map(|(k, v)| (k, v.as_str())));
        // The wrapper is the process: started, fed its credentials, then
        // left running with no connection.
        let started = async {
            let process = sandbox
                .run(
                    &script,
                    ProcessStartOptions::new()
                        .timeout_seconds(spec.timeout_seconds + 300)
                        .collect_output(false),
                )
                .await?;
            let fed = async {
                process.send_input(input.as_bytes()).await?;
                process.close_stdin().await
            }
            .await;
            process.disconnect().await;
            fed
        };
        let started = tokio::time::timeout(Duration::from_secs(60), started).await;
        // Confirm by the spool record, whichever attempt wrote it.
        let directory = quote(&format!("{COMMANDS}/{}", spec.id));
        let confirm = format!(
            "for attempt in $(seq 1 100); do\n if [ -f {directory}/spec ]; then [ \"$(cat {directory}/spec)\" = {} ] && exit 0; exit 98; fi\n sleep 0.05\ndone\nexit 125",
            quote(&spec.digest)
        );
        let mut probe = Exec::new(confirm);
        probe.timeout_seconds = 30;
        match self.exec(computer, &probe).await {
            Outcome::Done { value } if value.code == 0 => {
                Outcome::done(format!("archil-command:{}", spec.id))
            }
            Outcome::Done { value } if value.code == 98 => {
                Outcome::failed("That command identity already has different settings.")
            }
            _ => match started {
                Ok(Err(e)) if matches!(outcome::<()>("command", &e), Outcome::Failed { .. }) => {
                    Outcome::failed("The command could not start.")
                }
                _ => Outcome::unknown(
                    "The command start could not be confirmed. Check the same command before retrying.",
                ),
            },
        }
    }

    async fn read_command(
        &self,
        computer: &str,
        id: &str,
        cursor: CommandCursor,
        max_bytes: u64,
    ) -> Outcome<CommandRead> {
        if !id_ok(id) {
            return Outcome::failed("This command identity is invalid.");
        }
        let mut exec = Exec::new(identified::read_script(
            COMMANDS,
            id,
            cursor,
            max_bytes.clamp(1, 65536),
        ));
        exec.timeout_seconds = 30;
        match self.exec(computer, &exec).await {
            Outcome::Done { value } if value.code == 0 => {
                match identified::parse_read(&value.stdout) {
                    Ok(read) => Outcome::done(read),
                    Err(e) => Outcome::unknown(e),
                }
            }
            Outcome::Failed { reason } => Outcome::failed(reason),
            _ => Outcome::unknown("The command output is unavailable."),
        }
    }

    async fn stop_command(&self, computer: &str, id: &str) -> Outcome<String> {
        if !id_ok(id) {
            return Outcome::failed("This command identity is invalid.");
        }
        let mut exec = Exec::new(identified::bounded_stop_script(COMMANDS, id));
        exec.timeout_seconds = 30;
        match self.exec(computer, &exec).await {
            Outcome::Done { value } if value.code == 0 => Outcome::done(value.stdout),
            Outcome::Failed { reason } => Outcome::failed(reason),
            _ => Outcome::unknown(
                "The command stop could not be confirmed. Read its status before continuing.",
            ),
        }
    }

    async fn put_file(&self, computer: &str, path: &str, bytes: &[u8]) -> Outcome<()> {
        if bytes.len() > MAX_FILE {
            return Outcome::failed("Archil computers accept files up to 16 MiB.");
        }
        let Some(path) = self.absolute(path) else {
            return Outcome::failed("The file path is invalid.");
        };
        let sandbox = match self.running(computer).await {
            Ok(sandbox) => sandbox,
            Err(e) => return e,
        };
        let files = sandbox.files();
        let upload = files.upload_file(bytes.to_vec(), &path, SandboxFileUploadOptions::default());
        match tokio::time::timeout(Duration::from_secs(120), upload).await {
            Ok(Ok(())) => Outcome::done(()),
            Ok(Err(e)) => outcome("file write", &e),
            Err(_) => Outcome::unknown("The Archil file write exceeded its deadline."),
        }
    }

    async fn get_file(&self, computer: &str, path: &str) -> Outcome<Vec<u8>> {
        let Some(path) = self.absolute(path) else {
            return Outcome::failed("The file path is invalid.");
        };
        let sandbox = match self.running(computer).await {
            Ok(sandbox) => sandbox,
            Err(e) => return fail(e),
        };
        let read = async {
            let mut download = sandbox.files().download(&path).await?;
            let mut bytes = Vec::new();
            while let Some(chunk) = download.next_chunk().await? {
                if bytes.len() + chunk.len() > MAX_FILE {
                    return Ok(None);
                }
                bytes.extend_from_slice(&chunk);
            }
            Ok::<_, ::archil::Error>(Some(bytes))
        };
        match tokio::time::timeout(Duration::from_secs(120), read).await {
            Ok(Ok(Some(bytes))) => Outcome::done(bytes),
            Ok(Ok(None)) => Outcome::failed("The file is larger than 16 MiB."),
            Ok(Err(e)) => outcome("file read", &e),
            Err(_) => Outcome::unknown("The Archil file read exceeded its deadline."),
        }
    }

    async fn meter(&self, computer: &str) -> Outcome<Meter> {
        self.observe(computer).await.map(|v| Meter {
            running: v.state == State::Running,
            evidence: format!("archil-observe:{computer}:{:?}", v.state),
            // Archil's SDK exposes no billed-seconds counter; never
            // substitute wall time.
            seconds: None,
        })
    }

    async fn delete(&self, computer: &str) -> Outcome<()> {
        let mut sandbox = match self.lookup(computer).await {
            Ok(Some(sandbox)) => sandbox,
            Ok(None) => return Outcome::done(()),
            Err(e) => return e,
        };
        if matches!(
            sandbox.status,
            SandboxStatus::Deleted | SandboxStatus::Deleting
        ) {
            return Outcome::done(());
        }
        // Archil refuses to delete a sandbox while forks of it exist. Ask
        // before stopping anything, so that refusal changes nothing.
        let work = async {
            let mut last = None;
            for attempt in 0..6 {
                if attempt > 0 {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    match sandbox.refresh().await {
                        Ok(_) => {}
                        Err(e) if e.status() == Some(404) => return Ok(true),
                        Err(e) => {
                            last = Some(e);
                            continue;
                        }
                    }
                }
                if matches!(
                    sandbox.status,
                    SandboxStatus::Deleted | SandboxStatus::Deleting
                ) {
                    return Ok(true);
                }
                if sandbox.status == SandboxStatus::Pending {
                    let _ = sandbox.wait_for_start().await;
                }
                match sandbox.delete().await {
                    Ok(()) => return Ok(true),
                    Err(e) if e.status() == Some(404) => return Ok(true),
                    Err(e) if e.code() == Some(HAS_FORKS) => return Ok(false),
                    Err(e) => last = Some(e),
                }
                // Otherwise stop it first, then try again.
                if matches!(
                    sandbox.status,
                    SandboxStatus::Running
                        | SandboxStatus::Paused
                        | SandboxStatus::Pausing
                        | SandboxStatus::Stopping
                ) && let Err(e) = sandbox.stop().await
                {
                    last = Some(e);
                }
            }
            Err(last.expect("an attempt failed"))
        };
        match tokio::time::timeout(LIFECYCLE_DEADLINE, work).await {
            Ok(Ok(true)) => Outcome::done(()),
            Ok(Ok(false)) => Outcome::failed(
                "This computer has copies. Delete the copies first, then delete this computer.",
            ),
            Ok(Err(e)) => outcome("delete", &e),
            Err(_) => Outcome::unknown("The Archil delete exceeded its deadline."),
        }
    }

    async fn suspend(&self, computer: &str) -> Outcome<String> {
        let mut sandbox = match self.get(computer).await {
            Ok(sandbox) => sandbox,
            Err(e) => return fail(e),
        };
        match sandbox.status {
            SandboxStatus::Paused | SandboxStatus::Stopped | SandboxStatus::Exited => {
                return Outcome::done(format!("archil-{}:{computer}", sandbox.status));
            }
            SandboxStatus::Running | SandboxStatus::Pausing | SandboxStatus::Stopping => {}
            other => return Outcome::failed(format!("The computer can't be stopped ({other}).")),
        }
        let pause = self.memory_suspend && sandbox.status != SandboxStatus::Stopping;
        let work = async {
            if pause {
                sandbox.pause().await.map(|_| ())
            } else {
                sandbox.stop().await.map(|_| ())
            }
        };
        match tokio::time::timeout(LIFECYCLE_DEADLINE, work).await {
            Ok(Ok(())) => Outcome::done(format!(
                "archil-{}:{computer}:{}",
                sandbox.status,
                sandbox.checkpoint.as_deref().unwrap_or("-")
            )),
            Ok(Err(e)) => outcome("suspend", &e),
            Err(_) => Outcome::unknown("The Archil suspend exceeded its deadline."),
        }
    }

    async fn resume(&self, computer: &str) -> Outcome<String> {
        let mut sandbox = match self.get(computer).await {
            Ok(sandbox) => sandbox,
            Err(e) => return fail(e),
        };
        let work = async {
            match sandbox.status {
                SandboxStatus::Running => Ok(true),
                SandboxStatus::Pending => sandbox.wait_for_start().await.map(|_| true),
                SandboxStatus::Paused | SandboxStatus::Pausing => {
                    sandbox.resume().await.map(|_| true)
                }
                SandboxStatus::Stopped | SandboxStatus::Exited | SandboxStatus::Stopping => {
                    sandbox.start().await.map(|_| true)
                }
                _ => Ok(false),
            }
        };
        match tokio::time::timeout(LIFECYCLE_DEADLINE, work).await {
            Ok(Ok(true)) => Outcome::done(format!("archil-{}:{computer}", sandbox.status)),
            Ok(Ok(false)) => Outcome::failed(format!(
                "The computer can't be resumed ({}).",
                sandbox.status
            )),
            Ok(Err(e)) => outcome("resume", &e),
            Err(_) => Outcome::unknown("The Archil resume exceeded its deadline."),
        }
    }

    async fn fork(&self, computer: &str, operation: &str) -> Outcome<String> {
        if !valid_operation(operation) {
            return Outcome::failed("A fork needs an operation ID.");
        }
        let name = self.name("f", &[computer, operation]);
        match self.find_named(&name).await {
            Ok(Some(id)) => return Outcome::done(id),
            Ok(None) => {}
            Err(e) => return outcome("fork", &e),
        }
        let mut source = match self.get(computer).await {
            Ok(sandbox) => sandbox,
            Err(e) => return fail(e),
        };
        if !matches!(
            source.status,
            SandboxStatus::Running
                | SandboxStatus::Paused
                | SandboxStatus::Stopped
                | SandboxStatus::Exited
        ) {
            return Outcome::failed(format!(
                "The computer can't be copied now ({}).",
                source.status
            ));
        }
        let work = async {
            let child = match source
                .fork_with(SandboxForkOptions {
                    wait: true,
                    name: Some(name.clone()),
                })
                .await
            {
                Ok(child) => child,
                Err(e) if e.status() == Some(409) => match self.find_named(&name).await? {
                    Some(id) => return Ok(id),
                    None => return Err(e),
                },
                Err(e) => return Err(e),
            };
            let mut child = child;
            // Best effort: keep a disconnected command from pausing the copy.
            let _ = child
                .set_timeout(SandboxTimeoutOptions {
                    timeout_seconds: None,
                    idle_ttl_seconds: Some(0),
                })
                .await;
            Ok(child.id)
        };
        match tokio::time::timeout(LIFECYCLE_DEADLINE, work).await {
            Ok(Ok(id)) if valid_id(&id) => Outcome::done(id),
            Ok(Ok(_)) => Outcome::unknown("Archil returned an invalid computer ID."),
            Ok(Err(e)) => outcome("fork", &e),
            Err(_) => Outcome::unknown(
                "The Archil fork exceeded its deadline. Repeat the same fork operation to check it.",
            ),
        }
    }
}

#[cfg(test)]
#[path = "archil/tests.rs"]
mod tests;
