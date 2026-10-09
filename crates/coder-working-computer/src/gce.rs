//! The optional dedicated GCE image adapter (ENV-09).
//!
//! Each environment setup, builder, or verifier computer is its own GCE
//! instance, created for that computer alone. It is never a host of the
//! shared Coder pool (`coder_cloud::pool`): instances carry
//! `openagents-managed=coder-environment`, never the pool's label, run with
//! no service account and no API scopes, and are reached over the pool's
//! SSH key through IAP. A chat computer is refused.
//!
//! Identity:
//!
//! - A fresh setup or builder instance boots the configured base image by
//!   exact name, never by family, and its boot disk must report the pinned
//!   numeric image ID ([`GceImage`]). Drift deletes the instance and fails.
//! - An output image is a GCE image made from a stopped builder's boot
//!   disk. Its numeric ID is the immutable snapshot identity; a name is
//!   read first and never replaced, and an image that does not name the
//!   requested logical image in its description is refused.
//! - A verifier boots exactly that image, and its readiness
//!   ([`Images::hydration`]) requires the boot disk to report the sealed
//!   image ID and the guest to answer.
//!
//! Commands and evidence are the same identified, at-most-once wrapper the
//! Boat adapter uses ([`crate::boat::command_script`]), run over SSH, so
//! setup, build, and verify evidence has the same shape on both adapters.
//! Selected credentials are written per boot to a tmpfs file that no disk
//! image captures, uploaded through standard input, never on a command
//! line.
//!
//! A turn checkpoint stops the instance; its evidence names the boot disk
//! and GCE's last stop time, and a restore refuses a disk that booted since.
//!
//! Cleanup: [`Provider::delete`] deletes the instance with its disks and
//! confirms it is gone. [`GceProvider::reconcile`] lists every instance and
//! image this adapter owns against what the owners retain, with sizes and
//! run times, and [`GceProvider::sweep`] deletes only owned instances no
//! record holds. [`GceProvider::retire_image`] deletes an owned image only
//! when its numeric ID is the one expected.
//!
//! Errors: a definite GCE refusal is `Failed`; transport loss and timeouts
//! are `Unknown`; "not found" on a read means gone.

use crate::boat::{
    ALREADY_CLAIMED, COMMAND_DIR, command_script, parse_read, read_script, stop_script,
};
use crate::provider::{
    CheckpointEvidence, CommandCursor, CommandRead, CommandSpec, Commands, ImageRecord, ImageState,
    Images, Inspection, Meter, Outcome, Provider,
};
use crate::{Checkpoint, Computer, Health, Purpose, ServiceDecl};
use boat::shell_quote;
use coder_cloud::runtime::Credentials;
use coder_environment::{ImagePin, Provider as ProviderKind, digest};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Duration;

/// The label every instance and image of this adapter carries.
pub const MANAGED_KEY: &str = "openagents-managed";
pub const MANAGED_VALUE: &str = "coder-environment";
/// The shared pool's value of [`MANAGED_KEY`]; never touched here.
pub const POOL_VALUE: &str = "coder-pool";
/// Per-boot private files (credentials); tmpfs, so no image captures them.
pub const RUNTIME_DIR: &str = "/dev/shm/oa-env";
const SERVICE_DIR: &str = "/tmp/oa-services";
/// The backstop: an instance still running after its bound is deleted by GCE.
const MAX_RUN_SECONDS: u64 = 24 * 3600;

/// A GCE image pinned by its numeric ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GceImage {
    pub project: String,
    pub name: String,
    /// GCE's numeric image ID; a re-created image of the same name differs.
    pub id: String,
}
impl GceImage {
    pub fn path(&self) -> String {
        format!("projects/{}/global/images/{}", self.project, self.name)
    }
    /// The recipe base pin a GCE builder admits.
    pub fn pin(&self) -> ImagePin {
        ImagePin {
            provider: ProviderKind::Gce,
            image_id: self.path(),
            digest: digest(format!("gce-image\0{}\0{}", self.path(), self.id).as_bytes()),
        }
    }
}

/// The adapter's configuration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GceConfig {
    pub project: String,
    pub zone: String,
    pub machine: String,
    pub disk_gb: u64,
    /// The trusted base every setup and builder instance boots.
    pub base: GceImage,
}
fn gce_word(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && s.bytes()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
}
impl GceConfig {
    pub fn validate(&self) -> Result<(), String> {
        if !gce_word(&self.project, 63) || !gce_word(&self.base.project, 63) {
            return Err("The GCE project names are not valid.".into());
        }
        if !gce_word(&self.zone, 63) || !gce_word(&self.machine, 63) {
            return Err("The GCE zone and machine type are not valid.".into());
        }
        if !gce_name(&self.base.name) {
            return Err("The base image name is not valid.".into());
        }
        if self.base.id.is_empty() || !self.base.id.bytes().all(|c| c.is_ascii_digit()) {
            return Err("The base image needs its numeric GCE ID.".into());
        }
        if !(10..=2000).contains(&self.disk_gb) {
            return Err("disk_gb must be 10 to 2000.".into());
        }
        Ok(())
    }
}

/// A valid GCE resource name.
pub fn gce_name(s: &str) -> bool {
    gce_word(s, 63) && s.as_bytes()[0].is_ascii_lowercase() && !s.ends_with('-')
}
/// The GCE name an output image is kept under: the logical name when GCE
/// accepts it, else a digest of it. The image description holds the
/// logical name either way.
pub fn image_resource(name: &str) -> String {
    if gce_name(name) {
        name.to_owned()
    } else {
        format!("oaenv-image-{}", &digest(name.as_bytes())[..40])
    }
}
/// The instance a create operation makes: one per operation identity, so
/// a repeated create finds the same instance.
pub fn instance_name(operation: &str) -> String {
    format!("oaenv-{}", &digest(operation.as_bytes())[..24])
}
/// A GCE label value: lowercase letters, digits, `-`, `_`; at most 63.
pub fn label_value(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| match c.to_ascii_lowercase() {
            c @ ('a'..='z' | '0'..='9' | '-' | '_') => c,
            _ => '_',
        })
        .collect();
    out.truncate(63);
    out
}
fn basename(url: &str) -> String {
    url.rsplit('/').next().unwrap_or(url).to_owned()
}

/// One instance as GCE describes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstanceView {
    pub name: String,
    pub status: String,
    pub machine: String,
    pub labels: BTreeMap<String, String>,
    pub created: String,
    #[serde(default)]
    pub last_start: Option<String>,
    #[serde(default)]
    pub last_stop: Option<String>,
}
impl InstanceView {
    pub fn ours(&self) -> bool {
        self.labels.get(MANAGED_KEY).map(String::as_str) == Some(MANAGED_VALUE)
    }
    pub fn stopped(&self) -> bool {
        matches!(self.status.as_str(), "TERMINATED" | "STOPPED")
    }
    pub fn running(&self) -> bool {
        self.status == "RUNNING"
    }
    /// The disk state a stop left: the boot disk and GCE's last stop time.
    pub fn stop_marker(&self) -> String {
        format!(
            "gce-disk:{}@{}",
            self.name,
            self.last_stop.as_deref().unwrap_or("never")
        )
    }
    pub fn from_json(v: &Value) -> Self {
        let text = |k: &str| v[k].as_str().map(str::to_owned);
        Self {
            name: text("name").unwrap_or_default(),
            status: text("status").unwrap_or_default(),
            machine: basename(v["machineType"].as_str().unwrap_or_default()),
            labels: labels(&v["labels"]),
            created: text("creationTimestamp").unwrap_or_default(),
            last_start: text("lastStartTimestamp"),
            last_stop: text("lastStopTimestamp"),
        }
    }
}
fn labels(v: &Value) -> BTreeMap<String, String> {
    v.as_object()
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
                .collect()
        })
        .unwrap_or_default()
}
fn number(v: &Value) -> Option<u64> {
    v.as_u64()
        .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
}

/// One boot disk as GCE describes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskView {
    pub name: String,
    pub size_gb: u64,
    #[serde(default)]
    pub source_image: Option<String>,
    #[serde(default)]
    pub source_image_id: Option<String>,
}
impl DiskView {
    pub fn from_json(v: &Value) -> Self {
        Self {
            name: v["name"].as_str().unwrap_or_default().to_owned(),
            size_gb: number(&v["sizeGb"]).unwrap_or(0),
            source_image: v["sourceImage"].as_str().map(basename),
            source_image_id: v["sourceImageId"].as_str().map(str::to_owned),
        }
    }
}

/// One image as GCE describes it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageView {
    pub name: String,
    pub id: String,
    pub status: String,
    #[serde(default)]
    pub description: String,
    /// The source disk's name (an instance's boot disk has its name).
    #[serde(default)]
    pub source_disk: Option<String>,
    #[serde(default)]
    pub archive_bytes: Option<u64>,
    #[serde(default)]
    pub disk_gb: Option<u64>,
    pub labels: BTreeMap<String, String>,
}
impl ImageView {
    pub fn ours(&self) -> bool {
        self.labels.get(MANAGED_KEY).map(String::as_str) == Some(MANAGED_VALUE)
    }
    pub fn from_json(v: &Value) -> Self {
        Self {
            name: v["name"].as_str().unwrap_or_default().to_owned(),
            id: v["id"]
                .as_str()
                .map(str::to_owned)
                .or_else(|| v["id"].as_u64().map(|n| n.to_string()))
                .unwrap_or_default(),
            status: v["status"].as_str().unwrap_or_default().to_owned(),
            description: v["description"].as_str().unwrap_or_default().to_owned(),
            source_disk: v["sourceDisk"].as_str().map(basename),
            archive_bytes: number(&v["archiveSizeBytes"]),
            disk_gb: number(&v["diskSizeGb"]),
            labels: labels(&v["labels"]),
        }
    }
}

/// A GCE or SSH call that did not succeed. `definite` means GCE refused it
/// and nothing happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallError {
    pub definite: bool,
    pub message: String,
}
impl CallError {
    pub fn definite(m: impl Into<String>) -> Self {
        Self {
            definite: true,
            message: m.into(),
        }
    }
    pub fn unknown(m: impl Into<String>) -> Self {
        Self {
            definite: false,
            message: m.into(),
        }
    }
    fn outcome<T>(self, context: &str) -> Outcome<T> {
        if self.definite {
            Outcome::failed(format!("{context}: {}", self.message))
        } else {
            Outcome::unknown(format!("{context}: {}", self.message))
        }
    }
}
type Call<T> = Result<T, CallError>;

/// The output of one remote script.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RunOutput {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateInstance {
    pub name: String,
    pub image: GceImage,
    pub labels: BTreeMap<String, String>,
    pub max_run_seconds: u64,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreateImage {
    pub name: String,
    pub source_disk: String,
    /// The logical image name the owners asked for.
    pub description: String,
    pub labels: BTreeMap<String, String>,
}

/// GCE effects, one call each. [`Gcloud`] runs them; tests use [`fake`].
#[allow(async_fn_in_trait)]
pub trait Compute {
    /// `None` when GCE reports no such instance.
    async fn describe_instance(&self, name: &str) -> Call<Option<InstanceView>>;
    async fn describe_disk(&self, name: &str) -> Call<Option<DiskView>>;
    async fn create_instance(&self, request: &CreateInstance) -> Call<()>;
    async fn start_instance(&self, name: &str) -> Call<()>;
    async fn stop_instance(&self, name: &str) -> Call<()>;
    /// Delete with every disk; a missing instance is success.
    async fn delete_instance(&self, name: &str) -> Call<()>;
    async fn describe_image(&self, project: &str, name: &str) -> Call<Option<ImageView>>;
    async fn create_image(&self, request: &CreateImage) -> Call<()>;
    /// A missing image is success.
    async fn delete_image(&self, name: &str) -> Call<()>;
    /// Every instance with this adapter's label.
    async fn list_instances(&self) -> Call<Vec<InstanceView>>;
    /// Every image with this adapter's label.
    async fn list_images(&self) -> Call<Vec<ImageView>>;
    /// Run `script` with `sh -c` on a running instance.
    async fn run(
        &self,
        instance: &str,
        script: &str,
        input: Option<&[u8]>,
        timeout_seconds: u64,
    ) -> Call<RunOutput>;
}

// ---------------------------------------------------------------------------
// gcloud.

/// `gcloud compute instances create` arguments for one dedicated instance:
/// the exact image by name (never a family), no service account, no
/// scopes, no external address, and this adapter's labels only.
pub fn create_instance_args(
    config: &GceConfig,
    request: &CreateInstance,
    public_key: &str,
) -> Vec<String> {
    let labels = request
        .labels
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",");
    [
        "compute".to_owned(),
        "instances".into(),
        "create".into(),
        request.name.clone(),
        "--zone".into(),
        config.zone.clone(),
        "--machine-type".into(),
        config.machine.clone(),
        "--image".into(),
        request.image.name.clone(),
        "--image-project".into(),
        request.image.project.clone(),
        "--boot-disk-type".into(),
        "pd-balanced".into(),
        format!("--boot-disk-size={}GB", config.disk_gb),
        "--no-address".into(),
        "--no-service-account".into(),
        "--no-scopes".into(),
        "--shielded-secure-boot".into(),
        "--shielded-vtpm".into(),
        "--shielded-integrity-monitoring".into(),
        format!("--labels={labels}"),
        format!(
            "--metadata=^;^block-project-ssh-keys=TRUE;enable-oslogin=FALSE;ssh-keys=coder:{}",
            public_key.trim()
        ),
        format!("--max-run-duration={}s", request.max_run_seconds),
        "--instance-termination-action=DELETE".into(),
        "--format=json".into(),
    ]
    .into()
}

/// `gcloud compute images create` arguments: from one stopped boot disk.
pub fn create_image_args(config: &GceConfig, request: &CreateImage) -> Vec<String> {
    let labels = request
        .labels
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect::<Vec<_>>()
        .join(",");
    vec![
        "compute".into(),
        "images".into(),
        "create".into(),
        request.name.clone(),
        format!("--source-disk={}", request.source_disk),
        format!("--source-disk-zone={}", config.zone),
        format!("--description={}", request.description),
        format!("--labels={labels}"),
        "--format=json".into(),
    ]
}

/// Whether a gcloud error is a definite refusal (nothing happened).
pub fn definite_refusal(stderr: &str) -> bool {
    [
        "PERMISSION_DENIED",
        "Required '",
        "QUOTA",
        "Quota",
        "ZONE_RESOURCE_POOL_EXHAUSTED",
        "Invalid value",
        "INVALID_ARGUMENT",
        "is not ready",
        "resourceNotReady",
    ]
    .iter()
    .any(|n| stderr.contains(n))
}
pub fn not_found(stderr: &str) -> bool {
    stderr.contains("was not found") || stderr.contains("notFound")
}

/// The system transport: `gcloud` and the pool's SSH key through IAP.
pub struct Gcloud {
    pub config: GceConfig,
    pub public_key: String,
}
impl Gcloud {
    /// Uses the pool's SSH key on this computer (made once).
    pub fn new(config: GceConfig) -> Result<Self, String> {
        config.validate()?;
        let public_key = coder_cloud::pool::ensure_key()?;
        Ok(Self { config, public_key })
    }
    async fn call(&self, args: Vec<String>, seconds: u64) -> Call<String> {
        let child = tokio::process::Command::new("gcloud")
            .args(&args)
            .args(["--project", &self.config.project, "--quiet"])
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true)
            .output();
        let output = match tokio::time::timeout(Duration::from_secs(seconds), child).await {
            Err(_) => return Err(CallError::unknown("gcloud timed out")),
            Ok(Err(e)) => return Err(CallError::definite(format!("gcloud did not start: {e}"))),
            Ok(Ok(o)) => o,
        };
        if output.status.success() {
            return Ok(String::from_utf8_lossy(&output.stdout).into_owned());
        }
        let error = String::from_utf8_lossy(&output.stderr);
        let tail: Vec<&str> = error.lines().rev().take(4).collect();
        let message = tail.into_iter().rev().collect::<Vec<_>>().join(" ");
        Err(CallError {
            definite: definite_refusal(&message),
            message,
        })
    }
    async fn describe(&self, args: Vec<String>) -> Call<Option<Value>> {
        match self.call(args, 120).await {
            Ok(text) => serde_json::from_str(&text)
                .map(Some)
                .map_err(|e| CallError::unknown(format!("gcloud description: {e}"))),
            Err(e) if not_found(&e.message) => Ok(None),
            Err(e) => Err(e),
        }
    }
    fn zoned(&self, verb: &[&str], name: &str) -> Vec<String> {
        let mut args: Vec<String> = verb.iter().map(|s| (*s).to_owned()).collect();
        args.extend([name.to_owned(), "--zone".into(), self.config.zone.clone()]);
        args
    }
}
fn filter() -> String {
    format!("--filter=labels.{MANAGED_KEY}={MANAGED_VALUE}")
}
impl Compute for Gcloud {
    async fn describe_instance(&self, name: &str) -> Call<Option<InstanceView>> {
        let mut args = self.zoned(&["compute", "instances", "describe"], name);
        args.push("--format=json".into());
        Ok(self
            .describe(args)
            .await?
            .map(|v| InstanceView::from_json(&v)))
    }
    async fn describe_disk(&self, name: &str) -> Call<Option<DiskView>> {
        let mut args = self.zoned(&["compute", "disks", "describe"], name);
        args.push("--format=json".into());
        Ok(self.describe(args).await?.map(|v| DiskView::from_json(&v)))
    }
    async fn create_instance(&self, request: &CreateInstance) -> Call<()> {
        self.call(
            create_instance_args(&self.config, request, &self.public_key),
            600,
        )
        .await
        .map(|_| ())
    }
    async fn start_instance(&self, name: &str) -> Call<()> {
        let args = self.zoned(&["compute", "instances", "start"], name);
        self.call(args, 600).await.map(|_| ())
    }
    async fn stop_instance(&self, name: &str) -> Call<()> {
        let args = self.zoned(&["compute", "instances", "stop"], name);
        self.call(args, 600).await.map(|_| ())
    }
    async fn delete_instance(&self, name: &str) -> Call<()> {
        let mut args = self.zoned(&["compute", "instances", "delete"], name);
        args.push("--delete-disks=all".into());
        match self.call(args, 600).await {
            Err(e) if not_found(&e.message) => Ok(()),
            other => other.map(|_| ()),
        }
    }
    async fn describe_image(&self, project: &str, name: &str) -> Call<Option<ImageView>> {
        let args = vec![
            "compute".into(),
            "images".into(),
            "describe".into(),
            name.into(),
            format!("--project={project}"),
            "--format=json".into(),
        ];
        Ok(self.describe(args).await?.map(|v| ImageView::from_json(&v)))
    }
    async fn create_image(&self, request: &CreateImage) -> Call<()> {
        self.call(create_image_args(&self.config, request), 1800)
            .await
            .map(|_| ())
    }
    async fn delete_image(&self, name: &str) -> Call<()> {
        let args = vec![
            "compute".into(),
            "images".into(),
            "delete".into(),
            name.into(),
        ];
        match self.call(args, 600).await {
            Err(e) if not_found(&e.message) => Ok(()),
            other => other.map(|_| ()),
        }
    }
    async fn list_instances(&self) -> Call<Vec<InstanceView>> {
        let text = self
            .call(
                vec![
                    "compute".into(),
                    "instances".into(),
                    "list".into(),
                    filter(),
                    "--format=json".into(),
                ],
                120,
            )
            .await?;
        let v: Value = serde_json::from_str(&text)
            .map_err(|e| CallError::unknown(format!("gcloud listing: {e}")))?;
        Ok(v.as_array()
            .map(|a| a.iter().map(InstanceView::from_json).collect())
            .unwrap_or_default())
    }
    async fn list_images(&self) -> Call<Vec<ImageView>> {
        let text = self
            .call(
                vec![
                    "compute".into(),
                    "images".into(),
                    "list".into(),
                    filter(),
                    "--no-standard-images".into(),
                    "--format=json".into(),
                ],
                120,
            )
            .await?;
        let v: Value = serde_json::from_str(&text)
            .map_err(|e| CallError::unknown(format!("gcloud listing: {e}")))?;
        Ok(v.as_array()
            .map(|a| a.iter().map(ImageView::from_json).collect())
            .unwrap_or_default())
    }
    async fn run(
        &self,
        instance: &str,
        script: &str,
        input: Option<&[u8]>,
        timeout_seconds: u64,
    ) -> Call<RunOutput> {
        use tokio::io::AsyncWriteExt;
        let host = coder_cloud::pool::Host {
            name: instance.into(),
            zone: self.config.zone.clone(),
            status: "RUNNING".into(),
            machine: self.config.machine.clone(),
            spot: false,
            created: String::new(),
            address: None,
        };
        let remote = format!("timeout {timeout_seconds} sh -c {}", shell_quote(script));
        let command = coder_cloud::pool::ssh(&self.config.project, &host, &remote);
        let mut child = tokio::process::Command::from(command)
            .kill_on_drop(true)
            .stdin(if input.is_some() {
                std::process::Stdio::piped()
            } else {
                std::process::Stdio::null()
            })
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| CallError::unknown(format!("ssh did not start: {e}")))?;
        if let Some(bytes) = input {
            let mut stdin = child
                .stdin
                .take()
                .ok_or_else(|| CallError::unknown("no ssh input"))?;
            stdin
                .write_all(bytes)
                .await
                .map_err(|_| CallError::unknown("the ssh upload disconnected"))?;
        }
        let output = tokio::time::timeout(
            Duration::from_secs(timeout_seconds + 60),
            child.wait_with_output(),
        )
        .await
        .map_err(|_| CallError::unknown("ssh timed out"))?
        .map_err(|_| CallError::unknown("ssh disconnected"))?;
        match output.status.code() {
            // ssh's own failure; the remote side may or may not have run.
            None | Some(255) => Err(CallError::unknown(format!(
                "ssh to {instance} failed: {}",
                String::from_utf8_lossy(&output.stderr)
                    .lines()
                    .last()
                    .unwrap_or_default()
            ))),
            Some(code) => Ok(RunOutput {
                code,
                stdout: output.stdout,
                stderr: output.stderr,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// The provider.

/// Where the provider keeps its per-instance files. The defaults are the
/// real paths; tests root them in a scratch directory.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Paths {
    pub workdir: String,
    pub commands: String,
    pub runtime: String,
    pub services: String,
}
impl Paths {
    pub fn new(workdir: impl Into<String>) -> Self {
        Self {
            workdir: workdir.into(),
            commands: COMMAND_DIR.into(),
            runtime: RUNTIME_DIR.into(),
            services: SERVICE_DIR.into(),
        }
    }
    fn credentials(&self) -> String {
        format!("{}/credentials", self.runtime)
    }
}

pub struct GceProvider<T> {
    pub compute: T,
    pub config: GceConfig,
    pub credentials: Credentials,
    pub paths: Paths,
    /// Reachability probes after a boot, `ready_pause` apart.
    pub ready_attempts: u32,
    pub ready_pause: Duration,
}

impl GceProvider<Gcloud> {
    pub fn new(
        config: GceConfig,
        credentials: Credentials,
        workdir: String,
    ) -> Result<Self, String> {
        Ok(Self {
            compute: Gcloud::new(config.clone())?,
            config,
            credentials,
            paths: Paths::new(workdir),
            ready_attempts: 60,
            ready_pause: Duration::from_secs(5),
        })
    }
}

fn purpose_label(p: &Purpose) -> Option<&'static str> {
    match p {
        Purpose::Chat => None,
        Purpose::EnvironmentSetup { .. } => Some("setup"),
        Purpose::EnvironmentBuild { .. } => Some("build"),
        Purpose::EnvironmentVerify { .. } => Some("verify"),
    }
}
fn environment_of(p: &Purpose) -> &str {
    match p {
        Purpose::Chat => "",
        Purpose::EnvironmentSetup { environment }
        | Purpose::EnvironmentBuild { environment, .. }
        | Purpose::EnvironmentVerify { environment, .. } => environment,
    }
}

impl<T: Compute> GceProvider<T> {
    fn labels(&self, c: &Computer, purpose: &str) -> BTreeMap<String, String> {
        BTreeMap::from([
            (MANAGED_KEY.to_owned(), MANAGED_VALUE.to_owned()),
            ("oa-env-purpose".into(), purpose.into()),
            ("oa-env-computer".into(), label_value(&c.id)),
            (
                "oa-env-environment".into(),
                label_value(environment_of(&c.purpose)),
            ),
        ])
    }

    /// The image `c` must boot: the pinned base, or a verifier's sealed
    /// output image, with the ID its boot disk must report.
    async fn boot_image(&self, c: &Computer) -> Result<GceImage, Outcome<String>> {
        match c.purpose.verify_image() {
            None => match self
                .compute
                .describe_image(&self.config.base.project, &self.config.base.name)
                .await
            {
                Ok(Some(i)) if i.status == "READY" && i.id == self.config.base.id => {
                    Ok(self.config.base.clone())
                }
                Ok(Some(_)) => Err(Outcome::failed(
                    "The base image is not the pinned one or is not ready.",
                )),
                Ok(None) => Err(Outcome::failed("The pinned base image is gone.")),
                Err(e) => Err(e.outcome("read base image")),
            },
            Some(name) => match self.read_image(name).await {
                Outcome::Done {
                    value:
                        Some(ImageRecord {
                            state: ImageState::Ready,
                            snapshot: Some(id),
                            ..
                        }),
                } => Ok(GceImage {
                    project: self.config.project.clone(),
                    name: image_resource(name),
                    id,
                }),
                Outcome::Done { .. } => Err(Outcome::failed("No such ready image.")),
                Outcome::Failed { reason } => Err(Outcome::failed(reason)),
                Outcome::Unknown { reason } => Err(Outcome::unknown(reason)),
            },
        }
    }

    /// The boot disk reports exactly `id`.
    async fn booted_from(&self, name: &str, id: &str) -> Call<bool> {
        Ok(self
            .compute
            .describe_disk(name)
            .await?
            .is_some_and(|d| d.source_image_id.as_deref() == Some(id)))
    }

    /// Wait until the guest answers a trivial command.
    async fn reachable(&self, name: &str) -> bool {
        for attempt in 0..self.ready_attempts.max(1) {
            if attempt > 0 {
                tokio::time::sleep(self.ready_pause).await;
            }
            if matches!(self.compute.run(name, "true", None, 30).await, Ok(o) if o.code == 0) {
                return true;
            }
        }
        false
    }

    async fn instance(&self, name: &str) -> Result<InstanceView, Outcome<String>> {
        match self.compute.describe_instance(name).await {
            Ok(Some(i)) if i.ours() => Ok(i),
            Ok(Some(_)) => Err(Outcome::failed(
                "That instance is not a dedicated environment instance.",
            )),
            Ok(None) => Err(Outcome::failed("The instance is gone.")),
            Err(e) => Err(e.outcome("read instance")),
        }
    }

    /// One bounded command; `Ok(true)` when it exits 0.
    async fn exec(&self, name: &str, script: String, seconds: u64) -> Call<bool> {
        Ok(self.compute.run(name, &script, None, seconds).await?.code == 0)
    }

    /// Source the per-boot credentials file, if any.
    fn with_credentials(&self, script: &str) -> String {
        let f = shell_quote(&self.paths.credentials());
        format!("if [ -f {f} ]; then . {f}; fi\n{script}")
    }

    async fn stop_and_read(&self, name: &str) -> Outcome<InstanceView> {
        let i = match self.instance(name).await {
            Ok(i) => i,
            Err(o) => return map_outcome(o),
        };
        if !i.stopped() {
            if let Err(e) = self.compute.stop_instance(name).await {
                return e.outcome("stop");
            }
        }
        match self.instance(name).await {
            Ok(i) if i.stopped() => Outcome::done(i),
            Ok(i) => Outcome::unknown(format!("the instance is {}", i.status)),
            Err(o) => map_outcome(o),
        }
    }

    /// Every owned instance and image against what the owners retain.
    /// GCE is listed first and the records read after, so an instance
    /// whose create intent was retained before the call is never counted
    /// as an orphan.
    pub async fn reconcile(
        &self,
        retained: impl FnOnce() -> Result<Retained, String>,
    ) -> Result<Reconciliation, String> {
        let instances = self.compute.list_instances().await.map_err(|e| e.message)?;
        let images = self.compute.list_images().await.map_err(|e| e.message)?;
        let retained = retained()?;
        let mut out = Reconciliation::default();
        for i in instances.into_iter().filter(InstanceView::ours) {
            let disk = match self.compute.describe_disk(&i.name).await {
                Ok(d) => d.map(|d| d.size_gb),
                Err(e) => return Err(e.message),
            };
            out.disk_gb += disk.unwrap_or(0);
            out.instances.push(InstanceUsage {
                retained: retained.instances.contains(&i.name),
                name: i.name,
                status: i.status,
                machine: i.machine,
                disk_gb: disk,
                created: i.created,
                last_start: i.last_start,
                last_stop: i.last_stop,
            });
        }
        for i in images.into_iter().filter(ImageView::ours) {
            out.image_bytes += i.archive_bytes.unwrap_or(0);
            out.images.push(ImageUsage {
                referenced: retained.images.contains(&i.name),
                name: i.name,
                id: i.id,
                status: i.status,
                archive_bytes: i.archive_bytes,
                disk_gb: i.disk_gb,
            });
        }
        Ok(out)
    }

    /// Delete owned instances no record retains. Never touches another
    /// label, a retained instance, or an image.
    pub async fn sweep(&self, report: &Reconciliation) -> Vec<(String, Outcome<String>)> {
        let mut out = vec![];
        for i in report.instances.iter().filter(|i| !i.retained) {
            // Read the label again right before deleting.
            let o = match self.instance(&i.name).await {
                Ok(_) => match self.compute.delete_instance(&i.name).await {
                    Ok(()) => Outcome::done(format!("deleted:{}", i.name)),
                    Err(e) => e.outcome("delete"),
                },
                Err(o) => o,
            };
            out.push((i.name.clone(), o));
        }
        out
    }

    /// Delete an owned output image only when GCE still holds `expected_id`
    /// under its name. The caller decides that nothing references it.
    pub async fn retire_image(&self, name: &str, expected_id: &str) -> Outcome<String> {
        let resource = image_resource(name);
        match self
            .compute
            .describe_image(&self.config.project, &resource)
            .await
        {
            Ok(None) => return Outcome::done(format!("absent:{resource}")),
            Ok(Some(i)) if !i.ours() => {
                return Outcome::failed("That image is not an environment image.");
            }
            Ok(Some(i)) if i.id != expected_id => {
                return Outcome::failed("The image under that name is not the expected one.");
            }
            Ok(Some(_)) => {}
            Err(e) => return e.outcome("read image"),
        }
        if let Err(e) = self.compute.delete_image(&resource).await {
            return e.outcome("delete image");
        }
        match self
            .compute
            .describe_image(&self.config.project, &resource)
            .await
        {
            Ok(None) => Outcome::done(format!("deleted:{resource}")),
            Ok(Some(i)) => Outcome::unknown(format!("the image is still {}", i.status)),
            Err(e) => Outcome::unknown(format!("read after delete: {}", e.message)),
        }
    }
}

fn map_outcome<A, B>(o: Outcome<A>) -> Outcome<B> {
    match o {
        Outcome::Done { .. } => Outcome::unknown("unexpected"),
        Outcome::Failed { reason } => Outcome::failed(reason),
        Outcome::Unknown { reason } => Outcome::unknown(reason),
    }
}

/// What the owners hold: instance names of computers not confirmed
/// deleted, and image resource names a build or saved version references.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Retained {
    pub instances: BTreeSet<String>,
    pub images: BTreeSet<String>,
}
impl Retained {
    /// Hold every instance a GCE computer may own: each create attempt not
    /// confirmed deleted, by its operation (retained before the call) and
    /// by its recorded resource.
    pub fn hold(&mut self, c: &Computer) {
        if c.provider != ProviderKind::Gce {
            return;
        }
        for a in &c.creates {
            if a.deletion.as_ref().is_some_and(crate::Fact::is_done) {
                continue;
            }
            self.instances.insert(instance_name(&a.operation));
            if let Some(r) = &a.resource {
                self.instances.insert(r.clone());
            }
        }
    }
    /// Hold a sealed image a build or saved version names.
    pub fn hold_image(&mut self, image: &coder_environment::ImageIdentity) {
        if image.provider == ProviderKind::Gce {
            self.images.insert(image_resource(&image.image_id));
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct InstanceUsage {
    pub name: String,
    pub status: String,
    pub machine: String,
    pub disk_gb: Option<u64>,
    pub created: String,
    pub last_start: Option<String>,
    pub last_stop: Option<String>,
    /// A computer record still holds it.
    pub retained: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ImageUsage {
    pub name: String,
    pub id: String,
    pub status: String,
    pub archive_bytes: Option<u64>,
    pub disk_gb: Option<u64>,
    /// A build or saved version names it.
    pub referenced: bool,
}
/// Owned GCE resources with their sizes and run times. Charges are not
/// computed here; the billing owners keep them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Reconciliation {
    pub instances: Vec<InstanceUsage>,
    pub images: Vec<ImageUsage>,
    /// Boot disks of owned instances, running or stopped.
    pub disk_gb: u64,
    /// Stored bytes of owned images.
    pub image_bytes: u64,
}
impl Reconciliation {
    pub fn orphans(&self) -> Vec<&str> {
        self.instances
            .iter()
            .filter(|i| !i.retained)
            .map(|i| i.name.as_str())
            .collect()
    }
    pub fn unreferenced_images(&self) -> Vec<&str> {
        self.images
            .iter()
            .filter(|i| !i.referenced)
            .map(|i| i.name.as_str())
            .collect()
    }
}

impl<T: Compute> Provider for GceProvider<T> {
    fn kind(&self) -> ProviderKind {
        ProviderKind::Gce
    }
    fn admits_base(&self, base: &ImagePin) -> Result<(), &'static str> {
        if *base == self.config.base.pin() {
            Ok(())
        } else {
            Err("The recipe's base is not the pinned GCE base image.")
        }
    }

    async fn create(&self, c: &Computer, operation: &str) -> Outcome<String> {
        let Some(purpose) = purpose_label(&c.purpose) else {
            return Outcome::failed(
                "GCE computers serve environment setup, builds, and verifiers only.",
            );
        };
        let name = instance_name(operation);
        let image = match self.boot_image(c).await {
            Ok(i) => i,
            Err(o) => return o,
        };
        match self.compute.describe_instance(&name).await {
            Ok(Some(i)) if !i.ours() => {
                return Outcome::failed("The instance name belongs to another resource.");
            }
            Ok(Some(_)) => {}
            Ok(None) => {
                let request = CreateInstance {
                    name: name.clone(),
                    image: image.clone(),
                    labels: self.labels(c, purpose),
                    max_run_seconds: (c.bounds.absolute_ms / 1000).clamp(600, MAX_RUN_SECONDS),
                };
                if let Err(e) = self.compute.create_instance(&request).await {
                    return e.outcome("create");
                }
            }
            Err(e) => return e.outcome("read before create"),
        }
        match self.booted_from(&name, &image.id).await {
            Ok(true) => {}
            Ok(false) => {
                let _ = self.compute.delete_instance(&name).await;
                return Outcome::failed(
                    "The instance did not boot the pinned image; it was deleted.",
                );
            }
            Err(e) => return e.outcome("read boot disk"),
        }
        if !self.reachable(&name).await {
            return Outcome::unknown("The instance does not answer yet.");
        }
        Outcome::done(name)
    }

    async fn restore(
        &self,
        _c: &Computer,
        resource: &str,
        checkpoint: Option<&Checkpoint>,
    ) -> Outcome<String> {
        let i = match self.instance(resource).await {
            Ok(i) => i,
            Err(o) => return o,
        };
        if i.stopped() {
            if let Some(expected) = checkpoint.and_then(|k| k.fact.evidence())
                && i.stop_marker() != expected
            {
                return Outcome::failed("The disk changed since the retained checkpoint.");
            }
            if let Err(e) = self.compute.start_instance(resource).await {
                return e.outcome("start");
            }
        } else if i.status == "SUSPENDED" {
            return Outcome::failed("The instance is suspended.");
        }
        if !self.reachable(resource).await {
            return Outcome::unknown("The instance does not answer yet.");
        }
        Outcome::done(format!("resumed:{resource}"))
    }

    async fn apply_credentials(&self, c: &Computer, resource: &str) -> Outcome<String> {
        let names: Vec<&str> = c.credential_names.iter().map(String::as_str).collect();
        if names.is_empty() {
            return Outcome::done("applied:".into());
        }
        let dir = shell_quote(&self.paths.runtime);
        let file = shell_quote(&self.paths.credentials());
        // Values travel on standard input into tmpfs; presence is then
        // checked by name only.
        let check = names
            .iter()
            .map(|n| format!("[ -n \"${{{n}+x}}\" ] || exit 3"))
            .collect::<Vec<_>>()
            .join("; ");
        let script = format!(
            "umask 077; mkdir -p {dir} && cat > {file}.tmp && mv {file}.tmp {file} && . {file} && {check}"
        );
        let shell = self.credentials.shell();
        match self
            .compute
            .run(resource, &script, Some(shell.as_bytes()), 60)
            .await
        {
            Ok(o) if o.code == 0 => Outcome::done(format!("applied:{}", names.join(","))),
            Ok(_) => Outcome::failed("a selected credential is absent after boot"),
            Err(e) => e.outcome("credentials"),
        }
    }

    async fn start_service(
        &self,
        _c: &Computer,
        resource: &str,
        s: &ServiceDecl,
    ) -> Outcome<String> {
        let dir = if s.cwd == "." {
            self.paths.workdir.clone()
        } else {
            format!("{}/{}", self.paths.workdir, s.cwd)
        };
        let sd = &self.paths.services;
        let name = &s.name;
        let start = self.with_credentials(&format!(
            "mkdir -p {sd} && cd {} && (nohup sh -c {} >{sd}/{name}.log 2>&1 </dev/null & echo $! >{sd}/{name}.pid)",
            shell_quote(&dir),
            shell_quote(&s.command),
        ));
        match self.exec(resource, start, 60).await {
            Ok(true) => {}
            Ok(false) => return Outcome::failed("the service did not start"),
            Err(e) => return e.outcome("start service"),
        }
        let probe = match &s.health {
            Health::Http { port, path } => format!(
                "curl -fsS -o /dev/null {}",
                shell_quote(&format!("http://127.0.0.1:{port}{path}"))
            ),
            Health::Command { command } => format!("sh -c {}", shell_quote(command)),
        };
        let seconds = s.ready_within_seconds;
        let check = format!(
            "cd {} && i=0; while [ $i -lt {seconds} ]; do {probe} && exit 0; i=$((i+1)); sleep 1; done; exit 1",
            shell_quote(&dir)
        );
        match self.exec(resource, check, u64::from(seconds) + 30).await {
            Ok(true) => Outcome::done(format!("ready:{name}")),
            Ok(false) => Outcome::failed(format!("{name} did not pass its health rule")),
            Err(e) => e.outcome("readiness"),
        }
    }

    async fn checkpoint(
        &self,
        _c: &Computer,
        resource: &str,
        _generation: u64,
    ) -> Outcome<CheckpointEvidence> {
        match self.stop_and_read(resource).await {
            Outcome::Done { value } => Outcome::done(CheckpointEvidence {
                snapshot: value.stop_marker(),
                stopped: Some(format!("stop:{}", value.stop_marker())),
            }),
            Outcome::Failed { reason } => Outcome::failed(reason),
            Outcome::Unknown { reason } => Outcome::unknown(reason),
        }
    }

    async fn shutdown_processes(&self, _c: &Computer, resource: &str) -> Outcome<String> {
        let sd = &self.paths.services;
        let kill = format!(
            "for f in {sd}/*.pid; do [ -f \"$f\" ] && kill \"$(cat \"$f\")\" 2>/dev/null; rm -f \"$f\"; done; true"
        );
        match self.exec(resource, kill, 60).await {
            Ok(true) => Outcome::done("declared services stopped".into()),
            Ok(false) => Outcome::failed("the shutdown command failed"),
            Err(e) => e.outcome("shutdown"),
        }
    }

    async fn stop(&self, _c: &Computer, resource: &str) -> Outcome<String> {
        match self.compute.describe_instance(resource).await {
            Ok(None) => return Outcome::done("already gone".into()),
            Err(e) => return e.outcome("read before stop"),
            Ok(Some(_)) => {}
        }
        match self.stop_and_read(resource).await {
            Outcome::Done { value } => Outcome::done(format!("stop:{}", value.stop_marker())),
            Outcome::Failed { reason } => Outcome::failed(reason),
            Outcome::Unknown { reason } => Outcome::unknown(reason),
        }
    }

    async fn meter(&self, _c: &Computer, resource: &str) -> Outcome<Meter> {
        match self.compute.describe_instance(resource).await {
            Ok(None) => Outcome::done(Meter {
                running: false,
                evidence: "instance gone".into(),
            }),
            Ok(Some(i)) => {
                let disk = match self.compute.describe_disk(resource).await {
                    Ok(d) => d.map_or(0, |d| d.size_gb),
                    Err(e) => return e.outcome("read disk"),
                };
                Outcome::done(Meter {
                    running: !(i.stopped() || i.status == "SUSPENDED"),
                    evidence: format!(
                        "status={} machine={} disk_gb={disk} created={} last_start={} last_stop={}",
                        i.status,
                        i.machine,
                        i.created,
                        i.last_start.as_deref().unwrap_or("-"),
                        i.last_stop.as_deref().unwrap_or("-"),
                    ),
                })
            }
            Err(e) => Outcome::unknown(format!("usage: {}", e.message)),
        }
    }

    async fn delete(&self, _c: &Computer, resource: &str) -> Outcome<String> {
        match self.compute.describe_instance(resource).await {
            Ok(None) => return Outcome::done("already gone".into()),
            Ok(Some(i)) if !i.ours() => {
                return Outcome::failed("That instance is not a dedicated environment instance.");
            }
            Ok(Some(_)) => {}
            Err(e) => return e.outcome("read before delete"),
        }
        if let Err(e) = self.compute.delete_instance(resource).await {
            return e.outcome("delete");
        }
        match self.compute.describe_instance(resource).await {
            Ok(None) => Outcome::done(format!("deleted:{resource}")),
            Ok(Some(i)) => Outcome::unknown(format!("the instance is still {}", i.status)),
            Err(e) => Outcome::unknown(format!("read after delete: {}", e.message)),
        }
    }

    async fn inspect(&self, _c: &Computer, resource: &str) -> Outcome<Inspection> {
        match self.compute.describe_instance(resource).await {
            Ok(None) => Outcome::done(Inspection {
                running: None,
                stop: None,
                latest_snapshot: None,
            }),
            Ok(Some(i)) if i.stopped() => Outcome::done(Inspection {
                running: Some(false),
                stop: Some(format!("stop:{}", i.stop_marker())),
                latest_snapshot: Some(i.stop_marker()),
            }),
            Ok(Some(_)) => Outcome::done(Inspection {
                running: Some(true),
                stop: None,
                latest_snapshot: None,
            }),
            Err(e) => Outcome::unknown(format!("inspect: {}", e.message)),
        }
    }
}

impl<T: Compute> Commands for GceProvider<T> {
    async fn start_command(
        &self,
        c: &Computer,
        resource: &str,
        spec: &CommandSpec,
    ) -> Outcome<String> {
        let unset: Vec<&str> = c
            .credential_names
            .iter()
            .filter(|n| !spec.credential_names.contains(*n))
            .map(String::as_str)
            .collect();
        let wrapper = self.with_credentials(&command_script(
            &self.paths.commands,
            &self.paths.workdir,
            spec,
            &unset,
        ));
        // Detached: the wrapper keeps its own records, so the SSH session
        // can end while the command runs.
        let script = format!(
            "nohup sh -c {} >/dev/null 2>&1 </dev/null &\necho started",
            shell_quote(&wrapper)
        );
        match self.compute.run(resource, &script, None, 60).await {
            Ok(o) if o.code == 0 => Outcome::done(format!("gce-process:{}", spec.id)),
            Ok(o) if o.code == ALREADY_CLAIMED => Outcome::done(format!("gce-process:{}", spec.id)),
            Ok(o) => Outcome::failed(format!("the command did not start (exit {})", o.code)),
            Err(e) => Outcome::unknown(format!("start command: {}", e.message)),
        }
    }

    async fn read_command(
        &self,
        _c: &Computer,
        resource: &str,
        id: &str,
        cursor: CommandCursor,
        max_bytes: u64,
    ) -> Outcome<CommandRead> {
        let max = max_bytes.clamp(1, 1024 * 1024);
        match self
            .compute
            .run(
                resource,
                &read_script(&self.paths.commands, id, cursor, max),
                None,
                60,
            )
            .await
        {
            Ok(o) if o.code == 0 => match parse_read(&String::from_utf8_lossy(&o.stdout)) {
                Ok(read) => Outcome::done(read),
                Err(m) => Outcome::unknown(m),
            },
            Ok(_) => Outcome::unknown("the command read did not finish"),
            Err(e) => match self.compute.describe_instance(resource).await {
                Ok(None) => Outcome::failed("the instance is gone"),
                _ => Outcome::unknown(format!("read command: {}", e.message)),
            },
        }
    }

    async fn stop_command(&self, _c: &Computer, resource: &str, id: &str) -> Outcome<String> {
        match self
            .compute
            .run(resource, &stop_script(&self.paths.commands, id), None, 60)
            .await
        {
            Ok(o) if o.code == 0 => {
                Outcome::done(String::from_utf8_lossy(&o.stdout).trim().to_owned())
            }
            Ok(_) => Outcome::unknown("the stop command did not finish"),
            Err(e) => Outcome::unknown(format!("stop command: {}", e.message)),
        }
    }
}

/// Map a GCE image to an image record under its logical `name`.
pub fn image_record(name: &str, i: &ImageView) -> ImageRecord {
    let state = match i.status.as_str() {
        "READY" if !i.id.is_empty() => ImageState::Ready,
        "FAILED" => ImageState::Failed {
            reason: "GCE reports the image failed".into(),
        },
        "DELETING" => ImageState::Failed {
            reason: "GCE is deleting the image".into(),
        },
        _ => ImageState::Pending,
    };
    ImageRecord {
        name: name.into(),
        source: i.source_disk.clone().unwrap_or_default(),
        state,
        snapshot: (!i.id.is_empty()).then(|| i.id.clone()),
        size_bytes: i.archive_bytes,
    }
}

impl<T: Compute> Images for GceProvider<T> {
    async fn capture_image(
        &self,
        c: &Computer,
        resource: &str,
        name: &str,
    ) -> Outcome<ImageRecord> {
        if !matches!(c.purpose, Purpose::EnvironmentBuild { .. }) {
            return Outcome::failed("Only a dedicated builder's disk becomes an image.");
        }
        match self.read_image(name).await {
            Outcome::Done { value: Some(r) } if r.source == resource => return Outcome::done(r),
            Outcome::Done { value: Some(_) } => {
                return Outcome::failed("the image name belongs to another instance");
            }
            Outcome::Done { value: None } => {}
            Outcome::Failed { reason } | Outcome::Unknown { reason } => {
                return Outcome::unknown(format!("read before capture: {reason}"));
            }
        }
        match self.instance(resource).await {
            Ok(i) if i.stopped() => {}
            Ok(_) => return Outcome::failed("The builder must be stopped before capture."),
            Err(o) => return o.map_failed(),
        }
        let request = CreateImage {
            name: image_resource(name),
            source_disk: resource.into(),
            description: name.into(),
            labels: BTreeMap::from([
                (MANAGED_KEY.to_owned(), MANAGED_VALUE.to_owned()),
                (
                    "oa-env-environment".into(),
                    label_value(environment_of(&c.purpose)),
                ),
                ("oa-env-computer".into(), label_value(&c.id)),
            ]),
        };
        if let Err(e) = self.compute.create_image(&request).await {
            return e.outcome("create image");
        }
        match self.read_image(name).await {
            Outcome::Done { value: Some(r) } => Outcome::done(r),
            Outcome::Done { value: None } => Outcome::unknown("GCE does not list the image yet"),
            Outcome::Failed { reason } | Outcome::Unknown { reason } => Outcome::unknown(reason),
        }
    }

    async fn read_image(&self, name: &str) -> Outcome<Option<ImageRecord>> {
        match self
            .compute
            .describe_image(&self.config.project, &image_resource(name))
            .await
        {
            Ok(None) => Outcome::done(None),
            // Another image under the derived name: report a different
            // source so no owner treats it as this one.
            Ok(Some(i)) if !i.ours() || i.description != name => Outcome::done(Some(ImageRecord {
                name: name.into(),
                source: format!("foreign:{}", i.name),
                state: ImageState::Failed {
                    reason: "the name holds another image".into(),
                },
                snapshot: None,
                size_bytes: None,
            })),
            Ok(Some(i)) => Outcome::done(Some(image_record(name, &i))),
            Err(e) => Outcome::unknown(format!("read image: {}", e.message)),
        }
    }

    async fn hydration(&self, c: &Computer, resource: &str) -> Outcome<bool> {
        let i = match self.instance(resource).await {
            Ok(i) => i,
            Err(o) => return o.map_failed(),
        };
        if i.stopped() || i.status == "SUSPENDED" {
            return Outcome::failed(format!("the instance is {}", i.status));
        }
        if !i.running() {
            return Outcome::done(false);
        }
        // The disk reports the sealed image's ID, then the guest answers.
        let Some(name) = c.purpose.verify_image() else {
            return Outcome::failed("Only a verifier boots an output image.");
        };
        let expected = match self.read_image(name).await {
            Outcome::Done {
                value:
                    Some(ImageRecord {
                        state: ImageState::Ready,
                        snapshot: Some(id),
                        ..
                    }),
            } => id,
            Outcome::Done { .. } => return Outcome::failed("The sealed image is not ready."),
            Outcome::Failed { reason } | Outcome::Unknown { reason } => {
                return Outcome::unknown(reason);
            }
        };
        match self.booted_from(resource, &expected).await {
            Ok(true) => {}
            Ok(false) => return Outcome::failed("The boot disk is not the sealed image."),
            Err(e) => return e.outcome("read boot disk"),
        }
        match self.compute.run(resource, "true", None, 30).await {
            Ok(o) => Outcome::done(o.code == 0),
            Err(_) => Outcome::done(false),
        }
    }
}

impl<T> Outcome<T> {
    fn map_failed<U>(self) -> Outcome<U> {
        map_outcome(self)
    }
}

/// An in-memory GCE for tests. Instance disks are scratch directories and
/// remote scripts run under local `sh` with `/oa-fake` rooted in the
/// instance's directory, so the real command wrapper, reads, and
/// credential handling run end to end. `shm/` is the instance's tmpfs: a
/// stop clears it and no image copies it. Nothing here calls GCE.
pub mod fake {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    /// The path prefix the provider is configured with in tests.
    pub const ROOT: &str = "/oa-fake";

    pub fn paths() -> Paths {
        Paths {
            workdir: format!("{ROOT}/work"),
            commands: format!("{ROOT}/tmp/oa-commands"),
            runtime: format!("{ROOT}/shm/oa-env"),
            services: format!("{ROOT}/tmp/oa-services"),
        }
    }

    #[derive(Clone, Debug)]
    pub struct FakeInstance {
        pub view: InstanceView,
        pub disk: DiskView,
        pub dir: PathBuf,
    }

    #[derive(Default)]
    pub struct State {
        pub instances: BTreeMap<String, FakeInstance>,
        /// Images by (project, name), with their files.
        pub images: BTreeMap<(String, String), (ImageView, Option<PathBuf>)>,
        pub creates: Vec<CreateInstance>,
        pub calls: Vec<String>,
        /// Fail the next call of an operation (`definite`).
        pub fail: BTreeMap<&'static str, bool>,
        /// Perform the next call and then report an unknown outcome.
        pub lose_reply: BTreeSet<&'static str>,
        /// The guest does not answer while set.
        pub unreachable: bool,
        pub clock: u64,
        pub next_id: u64,
    }

    pub struct FakeCompute {
        pub root: PathBuf,
        pub project: String,
        pub state: Mutex<State>,
    }

    fn copy_dir(from: &Path, to: &Path, skip: &str) {
        std::fs::create_dir_all(to).unwrap();
        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let name = entry.file_name();
            if name == skip {
                continue;
            }
            let target = to.join(&name);
            if entry.file_type().unwrap().is_dir() {
                copy_dir(&entry.path(), &target, "");
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    impl FakeCompute {
        /// A fake project holding the base image `base` (`READY`).
        pub fn new(root: PathBuf, project: &str, base: &GceImage) -> Self {
            let mut state = State::default();
            state.images.insert(
                (base.project.clone(), base.name.clone()),
                (
                    ImageView {
                        name: base.name.clone(),
                        id: base.id.clone(),
                        status: "READY".into(),
                        ..Default::default()
                    },
                    None,
                ),
            );
            Self {
                root,
                project: project.into(),
                state: Mutex::new(state),
            }
        }
        fn begin(&self, op: &'static str) -> Call<()> {
            let mut s = self.state.lock().unwrap();
            s.calls.push(op.into());
            match s.fail.remove(op) {
                Some(true) => Err(CallError::definite(format!("injected refusal of {op}"))),
                Some(false) => Err(CallError::unknown(format!("injected loss of {op}"))),
                None => Ok(()),
            }
        }
        fn reply(&self, op: &'static str) -> Call<()> {
            if self.state.lock().unwrap().lose_reply.remove(op) {
                Err(CallError::unknown(format!("lost reply of {op}")))
            } else {
                Ok(())
            }
        }
        fn tick(s: &mut State) -> String {
            s.clock += 1;
            format!("2026-10-09T00:00:{:02}Z", s.clock)
        }
        pub fn instance(&self, name: &str) -> Option<FakeInstance> {
            self.state.lock().unwrap().instances.get(name).cloned()
        }
        pub fn calls(&self) -> Vec<String> {
            self.state.lock().unwrap().calls.clone()
        }
        pub fn fail_next(&self, op: &'static str, definite: bool) {
            self.state.lock().unwrap().fail.insert(op, definite);
        }
        pub fn lose_reply(&self, op: &'static str) {
            self.state.lock().unwrap().lose_reply.insert(op);
        }
        /// Something outside this adapter (the pool, a person) owns an
        /// instance or image.
        pub fn add_foreign_instance(&self, name: &str, managed: &str) {
            let dir = self.root.join("instances").join(name);
            std::fs::create_dir_all(&dir).unwrap();
            self.state.lock().unwrap().instances.insert(
                name.into(),
                FakeInstance {
                    view: InstanceView {
                        name: name.into(),
                        status: "RUNNING".into(),
                        labels: BTreeMap::from([(MANAGED_KEY.into(), managed.into())]),
                        ..Default::default()
                    },
                    disk: DiskView {
                        name: name.into(),
                        size_gb: 200,
                        ..Default::default()
                    },
                    dir,
                },
            );
        }
        /// Replace an image under the same name (a new ID).
        pub fn recreate_image(&self, project: &str, name: &str) {
            let mut s = self.state.lock().unwrap();
            s.next_id += 1;
            let id = format!("9{:018}", s.next_id);
            if let Some((view, _)) = s.images.get_mut(&(project.into(), name.into())) {
                view.id = id;
            }
        }
        pub fn set_image_status(&self, project: &str, name: &str, status: &str) {
            let mut s = self.state.lock().unwrap();
            if let Some((view, _)) = s.images.get_mut(&(project.into(), name.into())) {
                view.status = status.into();
            }
        }
        pub fn set_unreachable(&self, on: bool) {
            self.state.lock().unwrap().unreachable = on;
        }
    }

    impl Compute for FakeCompute {
        async fn describe_instance(&self, name: &str) -> Call<Option<InstanceView>> {
            self.begin("describe_instance")?;
            Ok(self.instance(name).map(|i| i.view))
        }
        async fn describe_disk(&self, name: &str) -> Call<Option<DiskView>> {
            self.begin("describe_disk")?;
            Ok(self.instance(name).map(|i| i.disk))
        }
        async fn create_instance(&self, r: &CreateInstance) -> Call<()> {
            self.begin("create_instance")?;
            {
                let mut s = self.state.lock().unwrap();
                if s.instances.contains_key(&r.name) {
                    return Err(CallError::unknown("already exists"));
                }
                let Some((image, files)) = s
                    .images
                    .get(&(r.image.project.clone(), r.image.name.clone()))
                    .cloned()
                else {
                    return Err(CallError::definite("The image was not found"));
                };
                let dir = self.root.join("instances").join(&r.name);
                std::fs::create_dir_all(dir.join("work")).unwrap();
                if let Some(files) = files {
                    copy_dir(&files, &dir, "");
                }
                let at = Self::tick(&mut s);
                s.creates.push(r.clone());
                s.instances.insert(
                    r.name.clone(),
                    FakeInstance {
                        view: InstanceView {
                            name: r.name.clone(),
                            status: "RUNNING".into(),
                            machine: "c3-standard-8".into(),
                            labels: r.labels.clone(),
                            created: at.clone(),
                            last_start: Some(at),
                            last_stop: None,
                        },
                        disk: DiskView {
                            name: r.name.clone(),
                            size_gb: 50,
                            source_image: Some(image.name.clone()),
                            source_image_id: Some(image.id.clone()),
                        },
                        dir,
                    },
                );
            }
            self.reply("create_instance")
        }
        async fn start_instance(&self, name: &str) -> Call<()> {
            self.begin("start_instance")?;
            let mut s = self.state.lock().unwrap();
            let at = Self::tick(&mut s);
            let i = s
                .instances
                .get_mut(name)
                .ok_or_else(|| CallError::definite("was not found"))?;
            i.view.status = "RUNNING".into();
            i.view.last_start = Some(at);
            Ok(())
        }
        async fn stop_instance(&self, name: &str) -> Call<()> {
            self.begin("stop_instance")?;
            {
                let mut s = self.state.lock().unwrap();
                let at = Self::tick(&mut s);
                let i = s
                    .instances
                    .get_mut(name)
                    .ok_or_else(|| CallError::definite("was not found"))?;
                i.view.status = "TERMINATED".into();
                i.view.last_stop = Some(at);
                let _ = std::fs::remove_dir_all(i.dir.join("shm"));
            }
            self.reply("stop_instance")
        }
        async fn delete_instance(&self, name: &str) -> Call<()> {
            self.begin("delete_instance")?;
            if let Some(i) = self.state.lock().unwrap().instances.remove(name) {
                let _ = std::fs::remove_dir_all(i.dir);
            }
            self.reply("delete_instance")
        }
        async fn describe_image(&self, project: &str, name: &str) -> Call<Option<ImageView>> {
            self.begin("describe_image")?;
            Ok(self
                .state
                .lock()
                .unwrap()
                .images
                .get(&(project.into(), name.into()))
                .map(|(v, _)| v.clone()))
        }
        async fn create_image(&self, r: &CreateImage) -> Call<()> {
            self.begin("create_image")?;
            {
                let mut s = self.state.lock().unwrap();
                let key = (self.project.clone(), r.name.clone());
                if s.images.contains_key(&key) {
                    return Err(CallError::unknown("already exists"));
                }
                let i = s
                    .instances
                    .get(&r.source_disk)
                    .cloned()
                    .ok_or_else(|| CallError::definite("The disk was not found"))?;
                if !i.view.stopped() {
                    return Err(CallError::definite("The disk resource is in use"));
                }
                s.next_id += 1;
                let id = format!("7{:018}", s.next_id);
                let files = self.root.join("images").join(&r.name);
                copy_dir(&i.dir, &files, "shm");
                s.images.insert(
                    key,
                    (
                        ImageView {
                            name: r.name.clone(),
                            id,
                            status: "READY".into(),
                            description: r.description.clone(),
                            source_disk: Some(r.source_disk.clone()),
                            archive_bytes: Some(1_000_000),
                            disk_gb: Some(i.disk.size_gb),
                            labels: r.labels.clone(),
                        },
                        Some(files),
                    ),
                );
            }
            self.reply("create_image")
        }
        async fn delete_image(&self, name: &str) -> Call<()> {
            self.begin("delete_image")?;
            self.state
                .lock()
                .unwrap()
                .images
                .remove(&(self.project.clone(), name.into()));
            Ok(())
        }
        async fn list_instances(&self) -> Call<Vec<InstanceView>> {
            self.begin("list_instances")?;
            Ok(self
                .state
                .lock()
                .unwrap()
                .instances
                .values()
                .map(|i| i.view.clone())
                .filter(InstanceView::ours)
                .collect())
        }
        async fn list_images(&self) -> Call<Vec<ImageView>> {
            self.begin("list_images")?;
            Ok(self
                .state
                .lock()
                .unwrap()
                .images
                .values()
                .map(|(v, _)| v.clone())
                .filter(ImageView::ours)
                .collect())
        }
        async fn run(
            &self,
            instance: &str,
            script: &str,
            input: Option<&[u8]>,
            timeout_seconds: u64,
        ) -> Call<RunOutput> {
            use tokio::io::AsyncWriteExt;
            self.begin("run")?;
            let dir = {
                let s = self.state.lock().unwrap();
                if s.unreachable {
                    return Err(CallError::unknown("connection refused"));
                }
                match s.instances.get(instance) {
                    Some(i) if i.view.running() => i.dir.clone(),
                    _ => return Err(CallError::unknown("connection refused")),
                }
            };
            let script = script.replace(ROOT, &dir.display().to_string());
            let mut child = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&script)
                .current_dir(&dir)
                .env("HOME", &dir)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()
                .map_err(|e| CallError::unknown(e.to_string()))?;
            let mut stdin = child.stdin.take().unwrap();
            if let Some(bytes) = input {
                stdin.write_all(bytes).await.unwrap();
            }
            drop(stdin);
            let out = tokio::time::timeout(
                Duration::from_secs(timeout_seconds),
                child.wait_with_output(),
            )
            .await
            .map_err(|_| CallError::unknown("timed out"))?
            .map_err(|e| CallError::unknown(e.to_string()))?;
            Ok(RunOutput {
                code: out.status.code().unwrap_or(-1),
                stdout: out.stdout,
                stderr: out.stderr,
            })
        }
    }
}

#[cfg(test)]
#[path = "gce_tests.rs"]
mod tests;
