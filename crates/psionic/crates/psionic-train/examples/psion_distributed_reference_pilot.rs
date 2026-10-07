use std::{
    collections::BTreeMap,
    env,
    error::Error,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use psionic_train::{
    run_psion_dual_host_reference_pilot, PsionReferencePilotConfig,
    PsionReferencePilotContributionBackend, PsionReferencePilotDualHostConfig,
    PsionReferencePilotJointContributionReceipt, PsionReferencePilotJointContributionRequest,
    TrainingLoopBudget,
};

const DEFAULT_CONTROL_PLANE_BATCH_ROWS: usize = 2;
const DEFAULT_CONTROL_PLANE_METAL_BATCH_ROWS: usize = 8;
const DEFAULT_PRIMARY_CUDA_BATCH_ROWS: usize = 12;
const DEFAULT_PRIMARY_METAL_BATCH_ROWS: usize = 8;
const DEFAULT_PRIMARY_CPU_BATCH_ROWS: usize = 4;
const DEFAULT_SECONDARY_CUDA_BATCH_ROWS: usize = 8;
const DEFAULT_SECONDARY_METAL_BATCH_ROWS: usize = 8;
const DEFAULT_SECONDARY_CPU_BATCH_ROWS: usize = 2;

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/psion_distributed_reference_pilot"));
    fs::create_dir_all(&output_dir)?;

    let remote = RemoteContributionInvoker::from_env(&output_dir)?;
    let mut config = PsionReferencePilotConfig::distributed_dual_host()?;
    apply_env_overrides(&mut config)?;
    let dual_host = dual_host_config_from_env()?;
    let run =
        run_psion_dual_host_reference_pilot(root.as_path(), &config, &dual_host, |request| {
            remote.invoke(request)
        })?;
    run.write_to_dir(output_dir.as_path())?;

    println!(
        "psion distributed reference pilot completed: stage={} checkpoint={} output={}",
        run.run.stage_receipt.stage_id,
        run.run.checkpoint_artifact.manifest.checkpoint_ref,
        output_dir.display()
    );
    println!(
        "distributed topology={}",
        run.topology_receipt.execution_topology_classification
    );
    Ok(())
}

struct RemoteContributionInvoker {
    exchange_root: PathBuf,
    targets: BTreeMap<String, RemoteContributionTarget>,
}

struct RemoteContributionTarget {
    ssh_target: String,
    remote_worktree_dir: String,
    remote_output_dir: String,
    remote_target_dir: String,
    remote_tmp_dir: String,
}

impl RemoteContributionInvoker {
    fn from_env(output_dir: &Path) -> Result<Self, Box<dyn Error>> {
        let primary_host = required_env("PSION_REFERENCE_PILOT_REMOTE_HOST")?;
        let mut targets = BTreeMap::new();
        targets.insert(
            primary_host,
            RemoteContributionTarget {
                ssh_target: required_env("PSION_REFERENCE_PILOT_REMOTE_SSH_TARGET")?,
                remote_worktree_dir: required_env("PSION_REFERENCE_PILOT_REMOTE_WORKTREE_DIR")?,
                remote_output_dir: required_env("PSION_REFERENCE_PILOT_REMOTE_OUTPUT_DIR")?,
                remote_target_dir: required_env("PSION_REFERENCE_PILOT_REMOTE_TARGET_DIR")?,
                remote_tmp_dir: required_env("PSION_REFERENCE_PILOT_REMOTE_TMP_DIR")?,
            },
        );
        if let Some(secondary_host) =
            optional_nonempty_env("PSION_REFERENCE_PILOT_SECONDARY_REMOTE_HOST")?
        {
            targets.insert(
                secondary_host,
                RemoteContributionTarget {
                    ssh_target: required_env("PSION_REFERENCE_PILOT_SECONDARY_REMOTE_SSH_TARGET")?,
                    remote_worktree_dir: required_env(
                        "PSION_REFERENCE_PILOT_SECONDARY_REMOTE_WORKTREE_DIR",
                    )?,
                    remote_output_dir: required_env(
                        "PSION_REFERENCE_PILOT_SECONDARY_REMOTE_OUTPUT_DIR",
                    )?,
                    remote_target_dir: required_env(
                        "PSION_REFERENCE_PILOT_SECONDARY_REMOTE_TARGET_DIR",
                    )?,
                    remote_tmp_dir: required_env("PSION_REFERENCE_PILOT_SECONDARY_REMOTE_TMP_DIR")?,
                },
            );
        }
        Ok(Self {
            exchange_root: output_dir.join("psion_reference_pilot_dual_host_exchange"),
            targets,
        })
    }

    fn invoke(
        &self,
        request: &PsionReferencePilotJointContributionRequest,
    ) -> Result<PsionReferencePilotJointContributionReceipt, psionic_train::PsionReferencePilotError>
    {
        let Some(target) = self.targets.get(&request.contributor_host) else {
            return Err(
                psionic_train::PsionReferencePilotError::RemoteContribution {
                    message: format!(
                        "no remote contribution target configured for host `{}`",
                        request.contributor_host
                    ),
                },
            );
        };
        let step_root = self
            .exchange_root
            .join(format!("step-{:04}", request.global_step));
        fs::create_dir_all(&step_root).map_err(io_as_remote_error)?;
        let request_label = sanitize_component(request.contributor_host.as_str());
        let local_request_path = step_root.join(format!("{request_label}-request.json"));
        let local_response_path = step_root.join(format!("{request_label}-response.json"));
        fs::write(
            &local_request_path,
            serde_json::to_vec_pretty(request).map_err(|error| {
                psionic_train::PsionReferencePilotError::RemoteContribution {
                    message: error.to_string(),
                }
            })?,
        )
        .map_err(io_as_remote_error)?;

        let remote_step_dir = format!(
            "{}/step-{:04}",
            target.remote_output_dir, request.global_step
        );
        let remote_request_path = format!("{remote_step_dir}/joint_request.json");
        let remote_response_path = format!("{remote_step_dir}/joint_response.json");

        ssh_status(
            target.ssh_target.as_str(),
            [
                String::from("mkdir"),
                String::from("-p"),
                remote_step_dir.clone(),
            ]
            .as_slice(),
        )?;
        scp_upload(
            &local_request_path,
            format!("{}:{}", target.ssh_target, remote_request_path).as_str(),
        )?;
        let remote_command = format!(
            "export PATH=\"$HOME/.cargo/bin:$PATH\"; export CARGO_TARGET_DIR={}; export TMPDIR={}; export RUST_MIN_STACK=16777216; mkdir -p {} {}; cd {} && cargo run -q -p psionic-train --example psion_reference_pilot_joint_contribution -- {} {}",
            remote_shell_path(target.remote_target_dir.as_str()),
            remote_shell_path(target.remote_tmp_dir.as_str()),
            remote_shell_path(target.remote_target_dir.as_str()),
            remote_shell_path(target.remote_tmp_dir.as_str()),
            remote_shell_path(target.remote_worktree_dir.as_str()),
            remote_shell_path(remote_request_path.as_str()),
            remote_shell_path(remote_response_path.as_str()),
        );
        ssh_bash(target.ssh_target.as_str(), remote_command.as_str())?;
        scp_download(
            format!("{}:{}", target.ssh_target, remote_response_path).as_str(),
            &local_response_path,
        )?;
        serde_json::from_slice(&fs::read(&local_response_path).map_err(io_as_remote_error)?)
            .map_err(
                |error| psionic_train::PsionReferencePilotError::RemoteContribution {
                    message: error.to_string(),
                },
            )
    }
}

fn dual_host_config_from_env() -> Result<PsionReferencePilotDualHostConfig, Box<dyn Error>> {
    let control_plane_host = required_env("PSION_REFERENCE_PILOT_CONTROL_PLANE_HOST")?;
    let remote_worker_host = required_env("PSION_REFERENCE_PILOT_REMOTE_HOST")?;
    let primary_backend = env_backend(
        "PSION_REFERENCE_PILOT_DUAL_HOST_REMOTE_BACKEND",
        PsionReferencePilotContributionBackend::Cuda,
    )?;
    let control_plane_backend = env_backend(
        "PSION_REFERENCE_PILOT_CONTROL_PLANE_BACKEND",
        default_control_plane_backend(),
    )?;
    let secondary_backend =
        optional_nonempty_env("PSION_REFERENCE_PILOT_DUAL_HOST_SECONDARY_REMOTE_BACKEND")?
            .map(|value| {
                parse_backend(
                    "PSION_REFERENCE_PILOT_DUAL_HOST_SECONDARY_REMOTE_BACKEND",
                    &value,
                )
            })
            .transpose()?
            .unwrap_or(PsionReferencePilotContributionBackend::Cpu);
    let mut config = PsionReferencePilotDualHostConfig::new(control_plane_host, remote_worker_host)
        .with_control_plane_backend(control_plane_backend)
        .with_control_plane_batch_rows(default_control_plane_batch_rows(control_plane_backend))
        .with_remote_worker_batch_rows(default_primary_batch_rows(primary_backend))
        .with_secondary_remote_worker_batch_rows(default_secondary_batch_rows(secondary_backend))
        .with_remote_worker_backend(primary_backend)
        .with_secondary_remote_worker_backend(secondary_backend);
    if let Ok(value) = env::var("PSION_REFERENCE_PILOT_CONTROL_PLANE_TAILNET_IP") {
        if !value.trim().is_empty() {
            config = config.with_control_plane_tailnet_ip(value);
        }
    }
    if let Ok(value) = env::var("PSION_REFERENCE_PILOT_REMOTE_TAILNET_IP") {
        if !value.trim().is_empty() {
            config = config.with_remote_worker_tailnet_ip(value);
        }
    }
    if let Some(batch_rows) =
        optional_env_usize("PSION_REFERENCE_PILOT_DUAL_HOST_CONTROL_BATCH_ROWS")?
    {
        config = config.with_control_plane_batch_rows(batch_rows.max(1));
    }
    if let Some(batch_rows) =
        optional_env_usize("PSION_REFERENCE_PILOT_DUAL_HOST_REMOTE_BATCH_ROWS")?
    {
        config = config.with_remote_worker_batch_rows(batch_rows.max(1));
    }
    if let Some(value) = optional_nonempty_env("PSION_REFERENCE_PILOT_SECONDARY_REMOTE_HOST")? {
        config = config.with_secondary_remote_worker_host(value);
    }
    if let Some(value) = optional_nonempty_env("PSION_REFERENCE_PILOT_SECONDARY_REMOTE_TAILNET_IP")?
    {
        config = config.with_secondary_remote_worker_tailnet_ip(value);
    }
    if let Some(batch_rows) =
        optional_env_usize("PSION_REFERENCE_PILOT_DUAL_HOST_SECONDARY_REMOTE_BATCH_ROWS")?
    {
        config = config.with_secondary_remote_worker_batch_rows(batch_rows.max(1));
    }
    Ok(config)
}

fn default_primary_batch_rows(backend: PsionReferencePilotContributionBackend) -> usize {
    match backend {
        PsionReferencePilotContributionBackend::Cpu => DEFAULT_PRIMARY_CPU_BATCH_ROWS,
        PsionReferencePilotContributionBackend::Cuda => DEFAULT_PRIMARY_CUDA_BATCH_ROWS,
        PsionReferencePilotContributionBackend::Metal => DEFAULT_PRIMARY_METAL_BATCH_ROWS,
    }
}

fn default_secondary_batch_rows(backend: PsionReferencePilotContributionBackend) -> usize {
    match backend {
        PsionReferencePilotContributionBackend::Cpu => DEFAULT_SECONDARY_CPU_BATCH_ROWS,
        PsionReferencePilotContributionBackend::Cuda => DEFAULT_SECONDARY_CUDA_BATCH_ROWS,
        PsionReferencePilotContributionBackend::Metal => DEFAULT_SECONDARY_METAL_BATCH_ROWS,
    }
}

fn default_control_plane_backend() -> PsionReferencePilotContributionBackend {
    if cfg!(target_os = "macos") {
        PsionReferencePilotContributionBackend::Metal
    } else {
        PsionReferencePilotContributionBackend::Cpu
    }
}

fn default_control_plane_batch_rows(backend: PsionReferencePilotContributionBackend) -> usize {
    match backend {
        PsionReferencePilotContributionBackend::Metal => DEFAULT_CONTROL_PLANE_METAL_BATCH_ROWS,
        PsionReferencePilotContributionBackend::Cpu
        | PsionReferencePilotContributionBackend::Cuda => DEFAULT_CONTROL_PLANE_BATCH_ROWS,
    }
}

fn env_backend(
    name: &str,
    default: PsionReferencePilotContributionBackend,
) -> Result<PsionReferencePilotContributionBackend, Box<dyn Error>> {
    optional_nonempty_env(name)?
        .map(|value| parse_backend(name, &value))
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn parse_backend(
    name: &str,
    value: &str,
) -> Result<PsionReferencePilotContributionBackend, Box<dyn Error>> {
    match value {
        "cpu" => Ok(PsionReferencePilotContributionBackend::Cpu),
        "cuda" => Ok(PsionReferencePilotContributionBackend::Cuda),
        "metal" | "mlx" => Ok(PsionReferencePilotContributionBackend::Metal),
        other => Err(format!("unsupported {name} `{other}`").into()),
    }
}

fn apply_env_overrides(config: &mut PsionReferencePilotConfig) -> Result<(), Box<dyn Error>> {
    let max_steps = optional_env_u64("PSION_REFERENCE_PILOT_MAX_STEPS")?;
    let steps_per_window = optional_env_u64("PSION_REFERENCE_PILOT_STEPS_PER_WINDOW")?;
    let windows_per_cadence = optional_env_u64("PSION_REFERENCE_PILOT_WINDOWS_PER_CADENCE")?;
    if max_steps.is_some() || steps_per_window.is_some() || windows_per_cadence.is_some() {
        config.budget = TrainingLoopBudget::new(
            max_steps.unwrap_or(config.budget.max_steps),
            steps_per_window.unwrap_or(config.budget.steps_per_window),
            windows_per_cadence.unwrap_or(config.budget.windows_per_cadence),
        )?;
    }
    if let Some(step_duration_ms) = optional_env_u64("PSION_REFERENCE_PILOT_STEP_DURATION_MS")? {
        if step_duration_ms == 0 {
            return Err("PSION_REFERENCE_PILOT_STEP_DURATION_MS must be greater than zero".into());
        }
        config.step_duration_ms = step_duration_ms;
    }
    Ok(())
}

fn ssh_status(
    target: &str,
    argv: &[String],
) -> Result<(), psionic_train::PsionReferencePilotError> {
    let status = Command::new("ssh")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg(target)
        .args(argv)
        .status()
        .map_err(io_as_remote_error)?;
    if !status.success() {
        return Err(
            psionic_train::PsionReferencePilotError::RemoteContribution {
                message: format!("ssh command failed with status {status}"),
            },
        );
    }
    Ok(())
}

fn ssh_bash(target: &str, command: &str) -> Result<(), psionic_train::PsionReferencePilotError> {
    let wrapped = format!("bash -lc {}", shell_quote(command));
    let status = Command::new("ssh")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg(target)
        .arg(wrapped)
        .status()
        .map_err(io_as_remote_error)?;
    if !status.success() {
        return Err(
            psionic_train::PsionReferencePilotError::RemoteContribution {
                message: format!("remote contributor command failed with status {status}"),
            },
        );
    }
    Ok(())
}

fn scp_upload(
    local_path: &Path,
    remote_target: &str,
) -> Result<(), psionic_train::PsionReferencePilotError> {
    let status = Command::new("scp")
        .arg("-O")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg(local_path)
        .arg(remote_target)
        .status()
        .map_err(io_as_remote_error)?;
    if !status.success() {
        return Err(
            psionic_train::PsionReferencePilotError::RemoteContribution {
                message: format!("scp upload failed with status {status}"),
            },
        );
    }
    Ok(())
}

fn scp_download(
    remote_source: &str,
    local_path: &Path,
) -> Result<(), psionic_train::PsionReferencePilotError> {
    let status = Command::new("scp")
        .arg("-O")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg("-o")
        .arg("ConnectTimeout=5")
        .arg(remote_source)
        .arg(local_path)
        .status()
        .map_err(io_as_remote_error)?;
    if !status.success() {
        return Err(
            psionic_train::PsionReferencePilotError::RemoteContribution {
                message: format!("scp download failed with status {status}"),
            },
        );
    }
    Ok(())
}

fn shell_quote(value: &str) -> String {
    let escaped = value.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

fn remote_shell_path(value: &str) -> String {
    if let Some(suffix) = value.strip_prefix("$HOME/") {
        return format!("\"$HOME/{}\"", suffix.replace('"', "\\\""));
    }
    shell_quote(value)
}

fn io_as_remote_error(error: std::io::Error) -> psionic_train::PsionReferencePilotError {
    psionic_train::PsionReferencePilotError::RemoteContribution {
        message: error.to_string(),
    }
}

fn required_env(name: &str) -> Result<String, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(value),
        Ok(_) => Err(format!("environment variable {name} must not be empty").into()),
        Err(error) => Err(Box::new(error)),
    }
}

fn optional_env_u64(name: &str) -> Result<Option<u64>, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value.parse::<u64>()?)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(Box::new(error)),
    }
}

fn optional_env_usize(name: &str) -> Result<Option<usize>, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value.parse::<usize>()?)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(Box::new(error)),
    }
}

fn optional_nonempty_env(name: &str) -> Result<Option<String>, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) if !value.trim().is_empty() => Ok(Some(value)),
        Ok(_) => Ok(None),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(Box::new(error)),
    }
}

fn sanitize_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' => ch,
            _ => '-',
        })
        .collect()
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}
