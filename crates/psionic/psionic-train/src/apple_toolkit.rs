//! Legacy developer-only Apple toolkit oracle.
//!
//! This module is intentionally excluded from the default `psionic-train`
//! build and is only compiled when the non-default
//! `legacy-apple-toolkit-oracle` feature is enabled.

use std::{
    collections::BTreeMap,
    env,
    ffi::OsString,
    fs,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc::{self, RecvTimeoutError},
    thread,
    time::{Duration, Instant},
};

use psionic_adapters::{
    APPLE_FM_ADAPTER_METADATA_FILE, APPLE_FM_ADAPTER_WEIGHTS_FILE, AppleFmAdapterPackageMetadata,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Explicit toolkit-backed Apple adapter training/export installation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterToolkitInstallation {
    /// Root directory of the Apple adapter training toolkit checkout.
    pub toolkit_root: PathBuf,
    /// Python interpreter used to run the toolkit modules.
    pub python_path: PathBuf,
}

impl AppleAdapterToolkitInstallation {
    /// Discovers one usable Apple adapter training toolkit installation.
    pub fn discover() -> Result<Self, AppleAdapterToolkitError> {
        if let Some(root) = env::var_os("OPENAGENTS_APPLE_TOOLKIT_ROOT") {
            let toolkit_root = validate_toolkit_root(PathBuf::from(root))?;
            return Ok(Self {
                python_path: discover_python_path(&toolkit_root)?,
                toolkit_root,
            });
        }

        let mut candidates = Vec::new();
        if let Some(parent) = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(4) {
            candidates.push(parent.join("adapter_training_toolkit_v26_0_0"));
        }
        if let Some(home) = env::var_os("HOME") {
            candidates.push(
                PathBuf::from(home)
                    .join("code")
                    .join("adapter_training_toolkit_v26_0_0"),
            );
        }

        for candidate in &candidates {
            if candidate.exists() {
                let toolkit_root = validate_toolkit_root(candidate.clone())?;
                return Ok(Self {
                    python_path: discover_python_path(&toolkit_root)?,
                    toolkit_root,
                });
            }
        }

        Err(AppleAdapterToolkitError::ToolkitNotFound {
            candidates: candidates
                .into_iter()
                .map(|path| path.display().to_string())
                .collect(),
        })
    }
}

/// Precision configuration forwarded to the Apple toolkit CLI.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterToolkitPrecision {
    F32,
    Bf16,
    Bf16Mixed,
    F16Mixed,
}

impl AppleAdapterToolkitPrecision {
    fn cli_label(self) -> &'static str {
        match self {
            Self::F32 => "f32",
            Self::Bf16 => "bf16",
            Self::Bf16Mixed => "bf16-mixed",
            Self::F16Mixed => "f16-mixed",
        }
    }
}

/// Training request for the toolkit-backed live Apple adapter lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterToolkitTrainingRequest {
    /// Train split path in Apple toolkit JSONL format.
    pub train_data_path: PathBuf,
    /// Optional held-out evaluation split path.
    pub eval_data_path: Option<PathBuf>,
    /// Directory where toolkit checkpoints are written.
    pub checkpoint_dir: PathBuf,
    /// Number of epochs to train.
    pub epochs: u32,
    /// Learning rate.
    pub learning_rate: String,
    /// Mini-batch size.
    pub batch_size: u32,
    /// Gradient accumulation count.
    pub gradient_accumulation_steps: u32,
    /// Whether activation checkpointing should be enabled.
    pub activation_checkpointing: bool,
    /// Precision policy.
    pub precision: AppleAdapterToolkitPrecision,
    /// Optional maximum sequence length for packing.
    pub max_sequence_length: Option<u32>,
    /// Whether to enable packed-sequence training.
    pub pack_sequences: bool,
    /// Whether to force fixed-sized sequences.
    pub fixed_sized_sequences: bool,
    /// Loss update frequency forwarded to tqdm.
    pub loss_update_frequency: u32,
    /// Checkpoint save frequency in epochs.
    pub checkpoint_frequency: u32,
    /// Extra environment variables for the toolkit command.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, String>,
}

impl AppleAdapterToolkitTrainingRequest {
    fn validate(&self) -> Result<(), AppleAdapterToolkitError> {
        if !self.train_data_path.exists() {
            return Err(AppleAdapterToolkitError::MissingInputPath {
                path: self.train_data_path.display().to_string(),
            });
        }
        if let Some(eval) = &self.eval_data_path {
            if !eval.exists() {
                return Err(AppleAdapterToolkitError::MissingInputPath {
                    path: eval.display().to_string(),
                });
            }
        }
        if self.epochs == 0 {
            return Err(AppleAdapterToolkitError::InvalidTrainingArgument {
                argument: "epochs",
                value: self.epochs.to_string(),
            });
        }
        if self.batch_size == 0 {
            return Err(AppleAdapterToolkitError::InvalidTrainingArgument {
                argument: "batch_size",
                value: self.batch_size.to_string(),
            });
        }
        if self.gradient_accumulation_steps == 0 {
            return Err(AppleAdapterToolkitError::InvalidTrainingArgument {
                argument: "gradient_accumulation_steps",
                value: self.gradient_accumulation_steps.to_string(),
            });
        }
        if self.pack_sequences && self.max_sequence_length.is_none() {
            return Err(AppleAdapterToolkitError::MissingMaxSequenceLength);
        }
        Ok(())
    }
}

/// Export request for the toolkit-backed live Apple adapter lane.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterToolkitExportRequest {
    /// Directory where the toolkit writes the `.fmadapter` package.
    pub output_dir: PathBuf,
    /// Adapter name passed to the toolkit exporter.
    pub adapter_name: String,
    /// Toolkit checkpoint to export.
    pub checkpoint_path: PathBuf,
    /// Optional author field forwarded to the toolkit metadata.
    pub author: Option<String>,
    /// Optional description forwarded to the toolkit metadata.
    pub description: Option<String>,
    /// Extra environment variables for the toolkit command.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub environment: BTreeMap<String, String>,
}

impl AppleAdapterToolkitExportRequest {
    fn validate(&self) -> Result<(), AppleAdapterToolkitError> {
        if self.adapter_name.trim().is_empty() {
            return Err(AppleAdapterToolkitError::InvalidTrainingArgument {
                argument: "adapter_name",
                value: self.adapter_name.clone(),
            });
        }
        if !self.checkpoint_path.exists() {
            return Err(AppleAdapterToolkitError::MissingInputPath {
                path: self.checkpoint_path.display().to_string(),
            });
        }
        Ok(())
    }
}

/// Resource usage parsed from `/usr/bin/time -l`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterToolkitResourceUsage {
    /// Measured wall-clock runtime in milliseconds when reported by `time`.
    pub real_ms: Option<u64>,
    /// Measured user CPU time in milliseconds when reported by `time`.
    pub user_ms: Option<u64>,
    /// Measured system CPU time in milliseconds when reported by `time`.
    pub sys_ms: Option<u64>,
    /// Maximum resident set size reported by `time`.
    pub max_resident_set_size_bytes: Option<u64>,
    /// Peak memory footprint reported by `time`.
    pub peak_memory_footprint_bytes: Option<u64>,
}

/// One toolkit command receipt kept for operator reports and audits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterToolkitCommandReceipt {
    /// Executable invoked by the repo-owned wrapper.
    pub executable: String,
    /// Argument vector forwarded to the executable.
    pub args: Vec<String>,
    /// Working directory used for the command.
    pub working_directory: String,
    /// Wall-clock duration measured by the wrapper.
    pub duration_ms: u64,
    /// Parsed resource usage from `/usr/bin/time -l` when available.
    pub resource_usage: AppleAdapterToolkitResourceUsage,
    /// Captured stdout.
    pub stdout: String,
    /// Captured stderr, including `/usr/bin/time` output.
    pub stderr: String,
}

/// One live output line observed from the toolkit wrapper.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppleAdapterToolkitOutputStream {
    Stdout,
    Stderr,
}

impl AppleAdapterToolkitOutputStream {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Stdout => "stdout",
            Self::Stderr => "stderr",
        }
    }
}

/// One live toolkit output event emitted before the wrapped command exits.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterToolkitOutputEvent {
    /// Which stream produced the line.
    pub stream: AppleAdapterToolkitOutputStream,
    /// UTF-8 lossy decoded line content without trailing newlines.
    pub line: String,
    /// Elapsed wall-clock milliseconds since command start.
    pub elapsed_ms: u64,
}

/// Successful toolkit training result.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppleAdapterToolkitTrainingOutcome {
    /// Discovered toolkit installation.
    pub installation: AppleAdapterToolkitInstallation,
    /// Final checkpoint path emitted by the toolkit.
    pub checkpoint_path: PathBuf,
    /// Final checkpoint size in bytes.
    pub checkpoint_size_bytes: u64,
    /// Captured training command receipt.
    pub receipt: AppleAdapterToolkitCommandReceipt,
}

/// Successful toolkit export result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleAdapterToolkitExportOutcome {
    /// Discovered toolkit installation.
    pub installation: AppleAdapterToolkitInstallation,
    /// Raw toolkit `.fmadapter` directory path.
    pub toolkit_package_path: PathBuf,
    /// Raw runtime asset bytes emitted by Apple's exporter.
    pub adapter_weights_bytes: Vec<u8>,
    /// Stable sha256 digest over the runtime asset bytes.
    pub adapter_weights_sha256: String,
    /// Toolkit-generated metadata.
    pub toolkit_metadata: AppleFmAdapterPackageMetadata,
    /// Captured export command receipt.
    pub receipt: AppleAdapterToolkitCommandReceipt,
}

/// Failure surfaced by the toolkit-backed Apple adapter lane.
#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AppleAdapterToolkitError {
    /// No toolkit checkout could be discovered.
    #[error("Apple adapter toolkit was not found; checked {candidates:?}")]
    ToolkitNotFound { candidates: Vec<String> },
    /// No usable Python interpreter was found.
    #[error("Apple adapter toolkit python was not found; checked {candidates:?}")]
    PythonNotFound { candidates: Vec<String> },
    /// One required toolkit input path is missing.
    #[error("Apple adapter toolkit input path does not exist: {path}")]
    MissingInputPath { path: String },
    /// One CLI argument is invalid.
    #[error("Apple adapter toolkit argument `{argument}` is invalid: {value}")]
    InvalidTrainingArgument {
        argument: &'static str,
        value: String,
    },
    /// Packed-sequence training requires an explicit max sequence length.
    #[error("Apple adapter toolkit packed-sequence training requires `max_sequence_length`")]
    MissingMaxSequenceLength,
    /// The discovered toolkit root does not look like the expected toolkit checkout.
    #[error("Apple adapter toolkit root is invalid: {path}")]
    InvalidToolkitRoot { path: String },
    /// Spawning one toolkit command failed.
    #[error("failed to launch Apple adapter toolkit command `{command}`: {message}")]
    SpawnFailed { command: String, message: String },
    /// Streaming one toolkit output pipe failed.
    #[error("failed to stream Apple adapter toolkit {stream} for `{command}`: {message}")]
    StreamReadFailed {
        command: String,
        stream: &'static str,
        message: String,
    },
    /// One toolkit command exited with failure.
    #[error("Apple adapter toolkit command `{command}` failed with status {status:?}")]
    CommandFailed {
        command: String,
        status: Option<i32>,
        stdout: String,
        stderr: String,
    },
    /// The expected toolkit output was missing after a successful command.
    #[error("Apple adapter toolkit output is missing: {path}")]
    MissingOutputPath { path: String },
    /// Reading one toolkit output failed.
    #[error("failed to read Apple adapter toolkit output {path}: {message}")]
    ReadOutputFailed { path: String, message: String },
    /// Decoding toolkit metadata failed.
    #[error("failed to decode Apple adapter toolkit metadata {path}: {message}")]
    DecodeMetadataFailed { path: String, message: String },
}

/// Runs Apple toolkit training and returns the final checkpoint.
pub fn run_apple_adapter_toolkit_training(
    request: &AppleAdapterToolkitTrainingRequest,
) -> Result<AppleAdapterToolkitTrainingOutcome, AppleAdapterToolkitError> {
    run_apple_adapter_toolkit_training_with_progress(request, |_| {})
}

/// Runs Apple toolkit training and streams live stdout/stderr lines before the
/// final command receipt is returned.
pub fn run_apple_adapter_toolkit_training_with_progress<F>(
    request: &AppleAdapterToolkitTrainingRequest,
    on_output: F,
) -> Result<AppleAdapterToolkitTrainingOutcome, AppleAdapterToolkitError>
where
    F: FnMut(&AppleAdapterToolkitOutputEvent),
{
    request.validate()?;
    let installation = AppleAdapterToolkitInstallation::discover()?;
    fs::create_dir_all(request.checkpoint_dir.as_path()).map_err(|error| {
        AppleAdapterToolkitError::ReadOutputFailed {
            path: request.checkpoint_dir.display().to_string(),
            message: error.to_string(),
        }
    })?;

    let mut args = vec![
        OsString::from("-m"),
        OsString::from("examples.train_adapter"),
        OsString::from("--train-data"),
        request.train_data_path.as_os_str().to_os_string(),
    ];
    if let Some(eval_path) = &request.eval_data_path {
        args.push(OsString::from("--eval-data"));
        args.push(eval_path.as_os_str().to_os_string());
    }
    args.extend([
        OsString::from("--epochs"),
        OsString::from(request.epochs.to_string()),
        OsString::from("--learning-rate"),
        OsString::from(request.learning_rate.clone()),
        OsString::from("--batch-size"),
        OsString::from(request.batch_size.to_string()),
        OsString::from("--gradient-accumulation-steps"),
        OsString::from(request.gradient_accumulation_steps.to_string()),
        OsString::from("--precision"),
        OsString::from(request.precision.cli_label()),
        OsString::from("--loss-update-frequency"),
        OsString::from(request.loss_update_frequency.to_string()),
        OsString::from("--checkpoint-dir"),
        request.checkpoint_dir.as_os_str().to_os_string(),
        OsString::from("--checkpoint-frequency"),
        OsString::from(request.checkpoint_frequency.to_string()),
    ]);
    if request.activation_checkpointing {
        args.push(OsString::from("--activation-checkpointing"));
    }
    if let Some(max_sequence_length) = request.max_sequence_length {
        args.push(OsString::from("--max-sequence-length"));
        args.push(OsString::from(max_sequence_length.to_string()));
    }
    if request.pack_sequences {
        args.push(OsString::from("--pack-sequences"));
    }
    if request.fixed_sized_sequences {
        args.push(OsString::from("--fixed_sized_sequences"));
    }

    let mut environment = default_toolkit_environment();
    for (key, value) in &request.environment {
        environment.insert(key.clone(), value.clone());
    }
    let receipt = run_toolkit_command(
        &installation,
        installation.python_path.as_path(),
        args.as_slice(),
        environment,
        on_output,
    )?;

    let checkpoint_path = request.checkpoint_dir.join("adapter-final.pt");
    let checkpoint_size_bytes = fs::metadata(checkpoint_path.as_path())
        .map_err(|_| AppleAdapterToolkitError::MissingOutputPath {
            path: checkpoint_path.display().to_string(),
        })?
        .len();
    Ok(AppleAdapterToolkitTrainingOutcome {
        installation,
        checkpoint_path,
        checkpoint_size_bytes,
        receipt,
    })
}

/// Runs Apple toolkit export and returns the raw runtime asset bytes plus toolkit metadata.
pub fn run_apple_adapter_toolkit_export(
    request: &AppleAdapterToolkitExportRequest,
) -> Result<AppleAdapterToolkitExportOutcome, AppleAdapterToolkitError> {
    run_apple_adapter_toolkit_export_with_progress(request, |_| {})
}

/// Runs Apple toolkit export and streams live stdout/stderr lines before the
/// final command receipt is returned.
pub fn run_apple_adapter_toolkit_export_with_progress<F>(
    request: &AppleAdapterToolkitExportRequest,
    on_output: F,
) -> Result<AppleAdapterToolkitExportOutcome, AppleAdapterToolkitError>
where
    F: FnMut(&AppleAdapterToolkitOutputEvent),
{
    request.validate()?;
    let installation = AppleAdapterToolkitInstallation::discover()?;
    fs::create_dir_all(request.output_dir.as_path()).map_err(|error| {
        AppleAdapterToolkitError::ReadOutputFailed {
            path: request.output_dir.display().to_string(),
            message: error.to_string(),
        }
    })?;

    let mut args = vec![
        OsString::from("-m"),
        OsString::from("export.export_fmadapter"),
        OsString::from("-o"),
        request.output_dir.as_os_str().to_os_string(),
        OsString::from("-n"),
        OsString::from(request.adapter_name.clone()),
        OsString::from("-c"),
        request.checkpoint_path.as_os_str().to_os_string(),
    ];
    if let Some(author) = request
        .author
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.push(OsString::from("--author"));
        args.push(OsString::from(author));
    }
    if let Some(description) = request
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        args.push(OsString::from("--description"));
        args.push(OsString::from(description));
    }
    let receipt = run_toolkit_command(
        &installation,
        installation.python_path.as_path(),
        args.as_slice(),
        request.environment.clone(),
        on_output,
    )?;

    let package_path = request
        .output_dir
        .join(format!("{}.fmadapter", request.adapter_name));
    let metadata_path = package_path.join(APPLE_FM_ADAPTER_METADATA_FILE);
    let weights_path = package_path.join(APPLE_FM_ADAPTER_WEIGHTS_FILE);
    let metadata_raw = fs::read(metadata_path.as_path()).map_err(|error| {
        AppleAdapterToolkitError::ReadOutputFailed {
            path: metadata_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let adapter_weights_bytes = fs::read(weights_path.as_path()).map_err(|error| {
        AppleAdapterToolkitError::ReadOutputFailed {
            path: weights_path.display().to_string(),
            message: error.to_string(),
        }
    })?;
    let toolkit_metadata = serde_json::from_slice::<AppleFmAdapterPackageMetadata>(&metadata_raw)
        .map_err(|error| AppleAdapterToolkitError::DecodeMetadataFailed {
        path: metadata_path.display().to_string(),
        message: error.to_string(),
    })?;
    let adapter_weights_sha256 = hex::encode(Sha256::digest(adapter_weights_bytes.as_slice()));
    Ok(AppleAdapterToolkitExportOutcome {
        installation,
        toolkit_package_path: package_path,
        adapter_weights_sha256,
        adapter_weights_bytes,
        toolkit_metadata,
        receipt,
    })
}

fn validate_toolkit_root(toolkit_root: PathBuf) -> Result<PathBuf, AppleAdapterToolkitError> {
    let expected_paths = [
        toolkit_root.join("examples").join("train_adapter.py"),
        toolkit_root.join("export").join("export_fmadapter.py"),
    ];
    if expected_paths.iter().all(|path| path.exists()) {
        Ok(toolkit_root)
    } else {
        Err(AppleAdapterToolkitError::InvalidToolkitRoot {
            path: toolkit_root.display().to_string(),
        })
    }
}

fn discover_python_path(toolkit_root: &Path) -> Result<PathBuf, AppleAdapterToolkitError> {
    if let Some(explicit) = env::var_os("OPENAGENTS_APPLE_TOOLKIT_PYTHON") {
        return Ok(PathBuf::from(explicit));
    }
    let candidates = [
        toolkit_root.join(".venv").join("bin").join("python"),
        toolkit_root.join(".venv").join("bin").join("python3"),
    ];
    for candidate in &candidates {
        if candidate.exists() {
            return Ok(candidate.clone());
        }
    }
    Err(AppleAdapterToolkitError::PythonNotFound {
        candidates: candidates
            .iter()
            .map(|path| path.display().to_string())
            .collect(),
    })
}

fn default_toolkit_environment() -> BTreeMap<String, String> {
    let mut env_map = BTreeMap::new();
    env_map.insert(
        String::from("PYTORCH_MPS_HIGH_WATERMARK_RATIO"),
        String::from("0.0"),
    );
    env_map
}

fn run_toolkit_command(
    installation: &AppleAdapterToolkitInstallation,
    executable: &Path,
    args: &[OsString],
    environment: BTreeMap<String, String>,
    mut on_output: impl FnMut(&AppleAdapterToolkitOutputEvent),
) -> Result<AppleAdapterToolkitCommandReceipt, AppleAdapterToolkitError> {
    let mut command = if Path::new("/usr/bin/time").exists() {
        let mut command = Command::new("/usr/bin/time");
        command.arg("-l");
        command.arg(executable);
        command.args(args);
        command
    } else {
        let mut command = Command::new(executable);
        command.args(args);
        command
    };
    command.current_dir(installation.toolkit_root.as_path());
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    for (key, value) in environment {
        command.env(key, value);
    }
    let started = Instant::now();
    let rendered_command = render_command(executable, args);
    let mut child = command
        .spawn()
        .map_err(|error| AppleAdapterToolkitError::SpawnFailed {
            command: rendered_command.clone(),
            message: error.to_string(),
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AppleAdapterToolkitError::SpawnFailed {
            command: rendered_command.clone(),
            message: "stdout pipe was not available".to_string(),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AppleAdapterToolkitError::SpawnFailed {
            command: rendered_command.clone(),
            message: "stderr pipe was not available".to_string(),
        })?;
    let (event_tx, event_rx) = mpsc::channel::<AppleAdapterToolkitOutputEvent>();
    let stdout_handle = spawn_toolkit_stream_reader(
        stdout,
        AppleAdapterToolkitOutputStream::Stdout,
        event_tx.clone(),
        started,
    );
    let stderr_handle = spawn_toolkit_stream_reader(
        stderr,
        AppleAdapterToolkitOutputStream::Stderr,
        event_tx,
        started,
    );

    let mut status = None;
    loop {
        match event_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(event) => on_output(&event),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => {
                status =
                    Some(
                        child
                            .wait()
                            .map_err(|error| AppleAdapterToolkitError::SpawnFailed {
                                command: rendered_command.clone(),
                                message: error.to_string(),
                            })?,
                    );
                break;
            }
        }
        if status.is_none() {
            status = child
                .try_wait()
                .map_err(|error| AppleAdapterToolkitError::SpawnFailed {
                    command: rendered_command.clone(),
                    message: error.to_string(),
                })?;
        }
    }

    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let stdout = join_toolkit_stream(stdout_handle, rendered_command.as_str(), "stdout")?;
    let stderr = join_toolkit_stream(stderr_handle, rendered_command.as_str(), "stderr")?;
    let status = if let Some(status) = status {
        status
    } else {
        child
            .wait()
            .map_err(|error| AppleAdapterToolkitError::SpawnFailed {
                command: rendered_command.clone(),
                message: error.to_string(),
            })?
    };
    if !status.success() {
        return Err(AppleAdapterToolkitError::CommandFailed {
            command: rendered_command,
            status: status.code(),
            stdout,
            stderr,
        });
    }
    Ok(AppleAdapterToolkitCommandReceipt {
        executable: executable.display().to_string(),
        args: args
            .iter()
            .map(|value| value.to_string_lossy().into_owned())
            .collect(),
        working_directory: installation.toolkit_root.display().to_string(),
        duration_ms,
        resource_usage: parse_time_output(stderr.as_str()),
        stdout,
        stderr,
    })
}

fn render_command(executable: &Path, args: &[OsString]) -> String {
    let mut rendered = executable.display().to_string();
    for arg in args {
        rendered.push(' ');
        rendered.push_str(arg.to_string_lossy().as_ref());
    }
    rendered
}

fn spawn_toolkit_stream_reader(
    stream: impl std::io::Read + Send + 'static,
    kind: AppleAdapterToolkitOutputStream,
    event_tx: mpsc::Sender<AppleAdapterToolkitOutputEvent>,
    started: Instant,
) -> thread::JoinHandle<Result<String, String>> {
    thread::spawn(move || {
        let mut reader = BufReader::new(stream);
        let mut captured = String::new();
        let mut buffer = Vec::new();
        loop {
            buffer.clear();
            let read = reader
                .read_until(b'\n', &mut buffer)
                .map_err(|error| error.to_string())?;
            if read == 0 {
                break;
            }
            let chunk = String::from_utf8_lossy(buffer.as_slice()).into_owned();
            captured.push_str(chunk.as_str());
            let event = AppleAdapterToolkitOutputEvent {
                stream: kind,
                line: chunk.trim_end_matches(['\r', '\n']).to_string(),
                elapsed_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            };
            let _ = event_tx.send(event);
        }
        Ok(captured)
    })
}

fn join_toolkit_stream(
    handle: thread::JoinHandle<Result<String, String>>,
    command: &str,
    stream: &'static str,
) -> Result<String, AppleAdapterToolkitError> {
    handle
        .join()
        .map_err(|_| AppleAdapterToolkitError::StreamReadFailed {
            command: command.to_string(),
            stream,
            message: "reader thread panicked".to_string(),
        })?
        .map_err(|message| AppleAdapterToolkitError::StreamReadFailed {
            command: command.to_string(),
            stream,
            message,
        })
}

fn parse_time_output(stderr: &str) -> AppleAdapterToolkitResourceUsage {
    let mut usage = AppleAdapterToolkitResourceUsage::default();
    for line in stderr.lines() {
        let trimmed = line.trim();
        if trimmed.ends_with("real") || trimmed.contains(" real ") {
            let fields = trimmed.split_whitespace().collect::<Vec<_>>();
            if fields.len() >= 6 {
                usage.real_ms = parse_seconds_ms(fields[0]);
                usage.user_ms = parse_seconds_ms(fields[2]);
                usage.sys_ms = parse_seconds_ms(fields[4]);
            }
            continue;
        }
        if let Some((value, label)) = trimmed.split_once("  ") {
            let value = value.trim();
            let label = label.trim();
            if label == "maximum resident set size" {
                usage.max_resident_set_size_bytes = value.parse::<u64>().ok();
            } else if label == "peak memory footprint" {
                usage.peak_memory_footprint_bytes = value.parse::<u64>().ok();
            }
        }
    }
    usage
}

fn parse_seconds_ms(value: &str) -> Option<u64> {
    let seconds = value.parse::<f64>().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Some((seconds * 1000.0).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn parse_time_output_extracts_wall_clock_and_memory() {
        let usage = parse_time_output(
            "        1.25 real         0.50 user         0.10 sys\n\
             1179648  maximum resident set size\n\
             901312  peak memory footprint\n",
        );
        assert_eq!(usage.real_ms, Some(1_250));
        assert_eq!(usage.user_ms, Some(500));
        assert_eq!(usage.sys_ms, Some(100));
        assert_eq!(usage.max_resident_set_size_bytes, Some(1_179_648));
        assert_eq!(usage.peak_memory_footprint_bytes, Some(901_312));
    }

    #[test]
    fn run_toolkit_command_streams_lines_and_keeps_receipt_capture() {
        let toolkit_root = tempdir().expect("temp toolkit root");
        let installation = AppleAdapterToolkitInstallation {
            toolkit_root: toolkit_root.path().to_path_buf(),
            python_path: PathBuf::from("/bin/sh"),
        };
        let args = vec![
            OsString::from("-c"),
            OsString::from("printf 'stdout-line\\n'; sleep 0.1; printf 'stderr-line\\n' >&2"),
        ];
        let mut events = Vec::new();
        let receipt = run_toolkit_command(
            &installation,
            Path::new("/bin/sh"),
            args.as_slice(),
            BTreeMap::new(),
            |event| events.push(event.clone()),
        )
        .expect("streamed toolkit command should succeed");

        assert!(receipt.stdout.contains("stdout-line"));
        assert!(receipt.stderr.contains("stderr-line"));
        assert!(events.iter().any(|event| {
            event.stream == AppleAdapterToolkitOutputStream::Stdout && event.line == "stdout-line"
        }));
        assert!(events.iter().any(|event| {
            event.stream == AppleAdapterToolkitOutputStream::Stderr && event.line == "stderr-line"
        }));
        let stdout_elapsed = events
            .iter()
            .find(|event| event.stream == AppleAdapterToolkitOutputStream::Stdout)
            .map(|event| event.elapsed_ms)
            .expect("stdout event");
        let stderr_elapsed = events
            .iter()
            .find(|event| event.stream == AppleAdapterToolkitOutputStream::Stderr)
            .map(|event| event.elapsed_ms)
            .expect("stderr event");
        assert!(stderr_elapsed >= stdout_elapsed);
    }
}
