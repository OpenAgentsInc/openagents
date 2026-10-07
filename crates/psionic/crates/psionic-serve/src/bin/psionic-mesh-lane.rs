#![cfg_attr(test, allow(clippy::expect_used))]

use std::{
    env, fs,
    io::{self, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
    process::ExitCode,
    time::{SystemTime, UNIX_EPOCH},
};

use psionic_net::{
    AdmissionToken, ClusterAdmissionConfig, ClusterJoinAdmissionMaterial, ClusterJoinBundle,
    ClusterJoinBundleImportOutcome, ClusterJoinBundleTrustMetadata, ClusterNamespace,
    ClusterTrustPolicy, LocalClusterConfig, LocalClusterNode, NodeRole,
};
use psionic_observe::{TokioRuntimeTelemetryConfig, build_main_runtime};
use psionic_serve::{OpenAiCompatBackend, OpenAiCompatConfig, OpenAiCompatServer};
use rand::random;
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;

const DEFAULT_SERVICE_NAME: &str = "psionic-mesh-lane";
const DEFAULT_HOST: &str = "127.0.0.1";
const DEFAULT_SERVER_PORT: u16 = 8080;
const DEFAULT_MESH_BIND_ADDR: &str = "0.0.0.0:47470";
const CONFIG_SCHEMA_VERSION: u32 = 1;

fn main() -> ExitCode {
    match run_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "{error}");
            ExitCode::FAILURE
        }
    }
}

fn run_main() -> Result<(), String> {
    let command = parse_command()?;
    match command {
        MeshLaneCommand::Install(request) => {
            let binary_path = env::current_exe()
                .map_err(|error| format!("failed to resolve current executable: {error}"))?;
            let report = install_mesh_lane(&request, &binary_path)?;
            let mut stdout = io::stdout();
            let _ = writeln!(
                stdout,
                "installed psionic mesh lane root={} config={} identity={} network_state={} wrapper={} systemd={} launchd={}",
                report.root.display(),
                report.config_path.display(),
                report.identity_path.display(),
                report.network_state_path.display(),
                report.run_script_path.display(),
                report.systemd_unit_path.display(),
                report.launchd_plist_path.display(),
            );
            if let Some(generated_admission_token) = report.generated_admission_token.as_deref() {
                let _ = writeln!(
                    stdout,
                    "generated shared admission token={generated_admission_token}"
                );
            }
            let _ = writeln!(
                stdout,
                "next: macOS => launchctl bootstrap gui/$(id -u) {} ; Linux => systemctl --user enable --now {}",
                shell_escape(report.launchd_plist_path.to_string_lossy().as_ref()),
                shell_escape(report.systemd_unit_path.to_string_lossy().as_ref()),
            );
            Ok(())
        }
        MeshLaneCommand::Run { root } => {
            let telemetry = TokioRuntimeTelemetryConfig::from_env()
                .map_err(|error| format!("failed to load Tokio telemetry config: {error}"))?;
            let (runtime, _telemetry_guard) = build_main_runtime(&telemetry)
                .map_err(|error| format!("failed to build Tokio runtime: {error}"))?;
            runtime.block_on(run_mesh_lane(root))
        }
        MeshLaneCommand::ExportJoinBundle(request) => export_join_bundle(request),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum MeshLaneCommand {
    Install(MeshLaneInstallRequest),
    Run { root: PathBuf },
    ExportJoinBundle(MeshLaneExportJoinBundleRequest),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct MeshLaneInstallRequest {
    root: PathBuf,
    model_paths: Vec<PathBuf>,
    host: Option<String>,
    port: Option<u16>,
    backend: Option<MeshLaneBackend>,
    reasoning_budget: Option<u8>,
    mesh_bind_addr: Option<SocketAddr>,
    seed_peers: Vec<SocketAddr>,
    service_name: Option<String>,
    namespace: Option<String>,
    admission_token: Option<String>,
    node_role: Option<MeshLaneNodeRole>,
    join_bundle_path: Option<PathBuf>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MeshLaneExportJoinBundleRequest {
    root: PathBuf,
    out: PathBuf,
    mesh_label: Option<String>,
    advertised_control_plane_addrs: Vec<SocketAddr>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MeshLaneBackend {
    Cpu,
    Cuda,
}

impl MeshLaneBackend {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "cpu" => Ok(Self::Cpu),
            "cuda" => Ok(Self::Cuda),
            other => Err(format!(
                "invalid --backend value `{other}` (expected cpu or cuda)"
            )),
        }
    }

    const fn into_openai_backend(self) -> OpenAiCompatBackend {
        match self {
            Self::Cpu => OpenAiCompatBackend::Cpu,
            Self::Cuda => OpenAiCompatBackend::Cuda,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MeshLaneNodeRole {
    CoordinatorOnly,
    ExecutorOnly,
    Mixed,
}

impl MeshLaneNodeRole {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "coordinator_only" => Ok(Self::CoordinatorOnly),
            "executor_only" => Ok(Self::ExecutorOnly),
            "mixed" => Ok(Self::Mixed),
            other => Err(format!(
                "invalid --node-role value `{other}` (expected coordinator_only, executor_only, or mixed)"
            )),
        }
    }

    const fn into_node_role(self) -> NodeRole {
        match self {
            Self::CoordinatorOnly => NodeRole::CoordinatorOnly,
            Self::ExecutorOnly => NodeRole::ExecutorOnly,
            Self::Mixed => NodeRole::Mixed,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct MeshLaneConfig {
    schema_version: u32,
    service_name: String,
    model_paths: Vec<PathBuf>,
    host: String,
    port: u16,
    backend: MeshLaneBackend,
    reasoning_budget: u8,
    mesh_bind_addr: SocketAddr,
    seed_peers: Vec<SocketAddr>,
    namespace: String,
    admission_token: String,
    node_role: MeshLaneNodeRole,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    join_bundle: Option<ClusterJoinBundle>,
}

impl MeshLaneConfig {
    fn load_json(path: &Path) -> Result<Self, String> {
        let bytes = fs::read(path).map_err(|error| {
            format!(
                "failed to read mesh lane config `{}`: {error}",
                path.display()
            )
        })?;
        let config: Self = serde_json::from_slice(&bytes).map_err(|error| {
            format!(
                "failed to parse mesh lane config `{}`: {error}",
                path.display()
            )
        })?;
        if config.schema_version != CONFIG_SCHEMA_VERSION {
            return Err(format!(
                "unsupported mesh lane config schema version {} in `{}` (expected {})",
                config.schema_version,
                path.display(),
                CONFIG_SCHEMA_VERSION,
            ));
        }
        Ok(config)
    }

    fn load_optional(path: &Path) -> Result<Option<Self>, String> {
        if !path.exists() {
            return Ok(None);
        }
        Self::load_json(path).map(Some)
    }

    fn store_json(&self, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                format!(
                    "failed to create mesh lane config directory `{}`: {error}",
                    parent.display()
                )
            })?;
        }
        let encoded = serde_json::to_vec_pretty(self)
            .map_err(|error| format!("failed to serialize mesh lane config: {error}"))?;
        fs::write(path, encoded).map_err(|error| {
            format!(
                "failed to write mesh lane config `{}`: {error}",
                path.display()
            )
        })
    }

    fn openai_config(&self) -> OpenAiCompatConfig {
        let mut config = OpenAiCompatConfig::new(
            self.model_paths
                .first()
                .cloned()
                .unwrap_or_else(|| PathBuf::from("missing-model")),
        );
        config.model_paths = self.model_paths.clone();
        config.host = self.host.clone();
        config.port = self.port;
        config.backend = self.backend.into_openai_backend();
        config.reasoning_budget = self.reasoning_budget;
        config
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MeshLanePaths {
    root: PathBuf,
    bin_dir: PathBuf,
    config_dir: PathBuf,
    state_dir: PathBuf,
    logs_dir: PathBuf,
    models_dir: PathBuf,
    run_dir: PathBuf,
    service_dir: PathBuf,
    config_path: PathBuf,
    identity_path: PathBuf,
    network_state_path: PathBuf,
}

impl MeshLanePaths {
    fn new(root: PathBuf) -> Self {
        let bin_dir = root.join("bin");
        let config_dir = root.join("config");
        let state_dir = root.join("state");
        let logs_dir = root.join("logs");
        let models_dir = root.join("models");
        let run_dir = root.join("run");
        let service_dir = root.join("service");
        Self {
            config_path: config_dir.join("mesh-lane.json"),
            identity_path: state_dir.join("node.identity.json"),
            network_state_path: state_dir.join("network-state.json"),
            root,
            bin_dir,
            config_dir,
            state_dir,
            logs_dir,
            models_dir,
            run_dir,
            service_dir,
        }
    }

    fn ensure_layout(&self) -> Result<(), String> {
        for path in [
            &self.root,
            &self.bin_dir,
            &self.config_dir,
            &self.state_dir,
            &self.logs_dir,
            &self.models_dir,
            &self.run_dir,
            &self.service_dir,
        ] {
            fs::create_dir_all(path)
                .map_err(|error| format!("failed to create `{}`: {error}", path.display()))?;
        }
        Ok(())
    }

    fn run_script_path(&self, service_name: &str) -> PathBuf {
        self.bin_dir.join(format!("{service_name}.sh"))
    }

    fn systemd_unit_path(&self, service_name: &str) -> PathBuf {
        self.service_dir.join(format!("{service_name}.service"))
    }

    fn launchd_plist_path(&self, service_name: &str) -> PathBuf {
        self.service_dir
            .join(format!("com.openagents.psionic.{service_name}.plist"))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct MeshLaneInstallReport {
    root: PathBuf,
    config_path: PathBuf,
    identity_path: PathBuf,
    network_state_path: PathBuf,
    run_script_path: PathBuf,
    systemd_unit_path: PathBuf,
    launchd_plist_path: PathBuf,
    generated_admission_token: Option<String>,
}

fn parse_command() -> Result<MeshLaneCommand, String> {
    parse_command_from(env::args().skip(1))
}

fn parse_command_from<I, S>(args: I) -> Result<MeshLaneCommand, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args = args.into_iter().map(Into::into);
    match args.next().as_deref() {
        Some("install") => parse_install_command(args),
        Some("run") => parse_run_command(args),
        Some("export-join-bundle") => parse_export_join_bundle_command(args),
        Some("-h") | Some("--help") | None => Err(usage()),
        Some(other) => Err(format!("unrecognized subcommand `{other}`\n\n{}", usage())),
    }
}

fn parse_install_command(args: impl Iterator<Item = String>) -> Result<MeshLaneCommand, String> {
    let mut request = MeshLaneInstallRequest::default();
    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => {
                request.root = PathBuf::from(next_value(&mut args, "--root")?);
            }
            "-m" | "--model" => {
                request
                    .model_paths
                    .push(PathBuf::from(next_value(&mut args, argument.as_str())?));
            }
            "--host" => {
                request.host = Some(next_value(&mut args, "--host")?);
            }
            "--port" => {
                request.port = Some(parse_u16(next_value(&mut args, "--port")?, "--port")?);
            }
            "--backend" => {
                request.backend = Some(MeshLaneBackend::parse(
                    next_value(&mut args, "--backend")?.as_str(),
                )?);
            }
            "--reasoning-budget" => {
                request.reasoning_budget = Some(parse_u8(
                    next_value(&mut args, "--reasoning-budget")?,
                    "--reasoning-budget",
                )?);
            }
            "--mesh-bind" => {
                request.mesh_bind_addr = Some(parse_socket_addr(
                    next_value(&mut args, "--mesh-bind")?,
                    "--mesh-bind",
                )?);
            }
            "--seed-peer" => {
                request.seed_peers.push(parse_socket_addr(
                    next_value(&mut args, "--seed-peer")?,
                    "--seed-peer",
                )?);
            }
            "--service-name" => {
                request.service_name = Some(next_value(&mut args, "--service-name")?);
            }
            "--namespace" => {
                request.namespace = Some(next_value(&mut args, "--namespace")?);
            }
            "--admission-token" => {
                request.admission_token = Some(next_value(&mut args, "--admission-token")?);
            }
            "--node-role" => {
                request.node_role = Some(MeshLaneNodeRole::parse(
                    next_value(&mut args, "--node-role")?.as_str(),
                )?);
            }
            "--join-bundle" => {
                request.join_bundle_path =
                    Some(PathBuf::from(next_value(&mut args, "--join-bundle")?));
            }
            "-h" | "--help" => return Err(install_usage()),
            other => {
                return Err(format!(
                    "unrecognized install argument `{other}`\n\n{}",
                    install_usage()
                ));
            }
        }
    }
    if request.root.as_os_str().is_empty() {
        return Err(format!("missing required `--root`\n\n{}", install_usage()));
    }
    Ok(MeshLaneCommand::Install(request))
}

fn parse_run_command(args: impl Iterator<Item = String>) -> Result<MeshLaneCommand, String> {
    let mut root = None;
    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => {
                root = Some(PathBuf::from(next_value(&mut args, "--root")?));
            }
            "-h" | "--help" => return Err(run_usage()),
            other => {
                return Err(format!(
                    "unrecognized run argument `{other}`\n\n{}",
                    run_usage()
                ));
            }
        }
    }
    let Some(root) = root else {
        return Err(format!("missing required `--root`\n\n{}", run_usage()));
    };
    Ok(MeshLaneCommand::Run { root })
}

fn parse_export_join_bundle_command(
    args: impl Iterator<Item = String>,
) -> Result<MeshLaneCommand, String> {
    let mut root = None;
    let mut out = None;
    let mut mesh_label = None;
    let mut advertised_control_plane_addrs = Vec::new();
    let mut args = args.peekable();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--root" => {
                root = Some(PathBuf::from(next_value(&mut args, "--root")?));
            }
            "--out" => {
                out = Some(PathBuf::from(next_value(&mut args, "--out")?));
            }
            "--mesh-label" => {
                mesh_label = Some(next_value(&mut args, "--mesh-label")?);
            }
            "--advertise" => {
                advertised_control_plane_addrs.push(parse_socket_addr(
                    next_value(&mut args, "--advertise")?,
                    "--advertise",
                )?);
            }
            "-h" | "--help" => return Err(export_join_bundle_usage()),
            other => {
                return Err(format!(
                    "unrecognized export argument `{other}`\n\n{}",
                    export_join_bundle_usage()
                ));
            }
        }
    }
    let Some(root) = root else {
        return Err(format!(
            "missing required `--root`\n\n{}",
            export_join_bundle_usage()
        ));
    };
    let Some(out) = out else {
        return Err(format!(
            "missing required `--out`\n\n{}",
            export_join_bundle_usage()
        ));
    };
    Ok(MeshLaneCommand::ExportJoinBundle(
        MeshLaneExportJoinBundleRequest {
            root,
            out,
            mesh_label,
            advertised_control_plane_addrs,
        },
    ))
}

fn install_mesh_lane(
    request: &MeshLaneInstallRequest,
    binary_path: &Path,
) -> Result<MeshLaneInstallReport, String> {
    let root = absolute_path(&request.root)?;
    let paths = MeshLanePaths::new(root.clone());
    paths.ensure_layout()?;
    let existing = MeshLaneConfig::load_optional(&paths.config_path)?;
    let (config, generated_admission_token) =
        materialize_install_config(request, existing.as_ref(), &paths)?;
    config.store_json(&paths.config_path)?;
    write_install_artifacts(&paths, &config, binary_path)?;
    Ok(MeshLaneInstallReport {
        root,
        config_path: paths.config_path.clone(),
        identity_path: paths.identity_path.clone(),
        network_state_path: paths.network_state_path.clone(),
        run_script_path: paths.run_script_path(config.service_name.as_str()),
        systemd_unit_path: paths.systemd_unit_path(config.service_name.as_str()),
        launchd_plist_path: paths.launchd_plist_path(config.service_name.as_str()),
        generated_admission_token,
    })
}

fn materialize_install_config(
    request: &MeshLaneInstallRequest,
    existing: Option<&MeshLaneConfig>,
    paths: &MeshLanePaths,
) -> Result<(MeshLaneConfig, Option<String>), String> {
    let requested_join_bundle = match request.join_bundle_path.as_ref() {
        Some(path) => Some(load_join_bundle(path)?),
        None => None,
    };
    if let Some(bundle) = requested_join_bundle.as_ref()
        && !matches!(
            bundle.admission,
            ClusterJoinAdmissionMaterial::SharedAdmission { .. }
        )
    {
        return Err(String::from(
            "psionic mesh lane service mode currently supports only shared-admission join bundles; signed-introduction bootstrap is not published here yet",
        ));
    }
    let join_bundle =
        requested_join_bundle.or_else(|| existing.and_then(|config| config.join_bundle.clone()));
    let service_name = request
        .service_name
        .clone()
        .or_else(|| existing.map(|config| config.service_name.clone()))
        .unwrap_or_else(|| String::from(DEFAULT_SERVICE_NAME));
    validate_service_name(service_name.as_str())?;
    let namespace = request
        .namespace
        .clone()
        .or_else(|| existing.map(|config| config.namespace.clone()))
        .or_else(|| {
            join_bundle
                .as_ref()
                .map(|bundle| bundle.namespace.as_str().to_string())
        })
        .unwrap_or_else(|| service_name.clone());

    let explicit_admission_token = request
        .admission_token
        .clone()
        .or_else(|| existing.map(|config| config.admission_token.clone()))
        .or_else(|| admission_token_from_join_bundle(join_bundle.as_ref()))
        .filter(|value| !value.is_empty());
    let (admission_token, generated_admission_token) = match explicit_admission_token {
        Some(token) => (token, None),
        None => {
            let token = hex::encode(random::<[u8; 16]>());
            (token.clone(), Some(token))
        }
    };
    if let Some(bundle) = join_bundle.as_ref() {
        if bundle.namespace.as_str() != namespace {
            return Err(format!(
                "join bundle namespace `{}` does not match configured namespace `{namespace}`",
                bundle.namespace.as_str()
            ));
        }
        if let Some(bundle_token) = admission_token_from_join_bundle(Some(bundle))
            && bundle_token != admission_token
        {
            return Err(String::from(
                "join bundle admission token does not match configured shared admission token",
            ));
        }
    }

    if let Some(existing) = existing {
        let previous_cluster_id = cluster_id_for_config(existing);
        let next_cluster_id = cluster_id_from_values(namespace.as_str(), admission_token.as_str());
        if previous_cluster_id != next_cluster_id
            && (paths.identity_path.exists() || paths.network_state_path.exists())
        {
            return Err(String::from(
                "refusing to change namespace or admission token for an existing mesh lane root because that would invalidate persisted node identity and network state; use a new root if you need a different lane identity",
            ));
        }
    }

    let model_paths = if request.model_paths.is_empty() {
        existing
            .map(|config| config.model_paths.clone())
            .filter(|paths| !paths.is_empty())
            .ok_or_else(|| format!("missing required `-m` / `--model`\n\n{}", install_usage()))?
    } else {
        request
            .model_paths
            .iter()
            .map(|path| normalize_model_path(paths, path))
            .collect()
    };
    let host = request
        .host
        .clone()
        .or_else(|| existing.map(|config| config.host.clone()))
        .unwrap_or_else(|| String::from(DEFAULT_HOST));
    let port = request
        .port
        .or_else(|| existing.map(|config| config.port))
        .unwrap_or(DEFAULT_SERVER_PORT);
    let backend = request
        .backend
        .or_else(|| existing.map(|config| config.backend))
        .unwrap_or(MeshLaneBackend::Cpu);
    let reasoning_budget = request
        .reasoning_budget
        .or_else(|| existing.map(|config| config.reasoning_budget))
        .unwrap_or(0);
    let mesh_bind_addr = request
        .mesh_bind_addr
        .or_else(|| existing.map(|config| config.mesh_bind_addr))
        .unwrap_or(parse_socket_addr(
            String::from(DEFAULT_MESH_BIND_ADDR),
            "--mesh-bind",
        )?);
    let seed_peers = if request.seed_peers.is_empty() {
        dedup_socket_addrs(
            existing
                .map(|config| config.seed_peers.clone())
                .unwrap_or_default(),
        )
    } else {
        dedup_socket_addrs(request.seed_peers.clone())
    };
    let node_role = request
        .node_role
        .or_else(|| existing.map(|config| config.node_role))
        .unwrap_or(MeshLaneNodeRole::Mixed);

    Ok((
        MeshLaneConfig {
            schema_version: CONFIG_SCHEMA_VERSION,
            service_name,
            model_paths,
            host,
            port,
            backend,
            reasoning_budget,
            mesh_bind_addr,
            seed_peers,
            namespace,
            admission_token,
            node_role,
            join_bundle,
        },
        generated_admission_token,
    ))
}

async fn run_mesh_lane(root: PathBuf) -> Result<(), String> {
    let root = absolute_path(&root)?;
    let paths = MeshLanePaths::new(root.clone());
    paths.ensure_layout()?;
    let config = MeshLaneConfig::load_json(&paths.config_path)?;
    let transport = LocalClusterNode::spawn(local_cluster_config(&config, &paths))
        .await
        .map_err(|error| format!("failed to start mesh transport: {error}"))?;
    let join_import = maybe_import_configured_join_bundle(&transport, &config).await?;
    let durable_network_state = transport.durable_network_state().await;

    let server_config = config.openai_config();
    let address = server_config
        .socket_addr()
        .map_err(|error| error.to_string())?;
    let listener = TcpListener::bind(address)
        .await
        .map_err(|error| format!("failed to bind {address}: {error}"))?;
    let server = OpenAiCompatServer::from_config(&server_config)
        .map_err(|error| format!("failed to load models: {error}"))?;
    server.apply_persisted_mesh_network_state(&durable_network_state);

    let join_posture = if durable_network_state.last_joined_mesh_preference.is_some() {
        "joined"
    } else if durable_network_state.last_imported_join_bundle.is_some() {
        "pending_import"
    } else {
        "standalone"
    };
    let mut stdout = io::stdout();
    let _ = writeln!(
        stdout,
        "psionic mesh lane listening on http://{} transport={} node_id={} node_epoch={} root={} identity={} network_state={} join_posture={} models={} backend={:?}",
        listener
            .local_addr()
            .map_err(|error| format!("failed to query listener address: {error}"))?,
        transport.local_addr(),
        transport.local_identity().node_id.as_str(),
        transport.local_identity().node_epoch.as_u64(),
        root.display(),
        paths.identity_path.display(),
        paths.network_state_path.display(),
        join_posture,
        config
            .model_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(","),
        config.backend,
    );
    if let Some(outcome) = join_import {
        let _ = writeln!(
            stdout,
            "mesh join bundle import outcome={}",
            serde_json::to_string(&outcome).unwrap_or_else(|_| String::from("\"unavailable\""))
        );
    }

    server
        .serve(listener)
        .await
        .map_err(|error| format!("server failed: {error}"))
}

fn export_join_bundle(request: MeshLaneExportJoinBundleRequest) -> Result<(), String> {
    let root = absolute_path(&request.root)?;
    let paths = MeshLanePaths::new(root);
    let config = MeshLaneConfig::load_json(&paths.config_path)?;
    let mesh_label = request
        .mesh_label
        .or_else(|| {
            config
                .join_bundle
                .as_ref()
                .map(|bundle| bundle.mesh_label.clone())
        })
        .unwrap_or_else(|| config.service_name.clone());
    let advertised_control_plane_addrs = if request.advertised_control_plane_addrs.is_empty() {
        vec![config.mesh_bind_addr]
    } else {
        dedup_socket_addrs(request.advertised_control_plane_addrs)
    };
    let bundle = ClusterJoinBundle::shared_admission(
        mesh_label,
        &ClusterAdmissionConfig::new(config.namespace.clone(), config.admission_token.clone()),
        advertised_control_plane_addrs,
        ClusterJoinBundleTrustMetadata::from_policies(&ClusterTrustPolicy::trusted_lan(), None),
        unix_timestamp_ms(),
    );
    let out = absolute_path(&request.out)?;
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create `{}`: {error}", parent.display()))?;
    }
    let encoded = serde_json::to_vec_pretty(&bundle)
        .map_err(|error| format!("failed to serialize join bundle: {error}"))?;
    fs::write(&out, encoded)
        .map_err(|error| format!("failed to write join bundle `{}`: {error}", out.display()))?;
    let mut stdout = io::stdout();
    let _ = writeln!(stdout, "wrote mesh join bundle {}", out.display());
    Ok(())
}

fn local_cluster_config(config: &MeshLaneConfig, paths: &MeshLanePaths) -> LocalClusterConfig {
    let mut seed_peers = config.seed_peers.clone();
    if let Some(bundle) = config.join_bundle.as_ref() {
        seed_peers.extend(bundle.advertised_control_plane_addrs.iter().copied());
    }
    LocalClusterConfig::new(
        config.namespace.clone(),
        config.admission_token.clone(),
        config.mesh_bind_addr,
        config.node_role.into_node_role(),
    )
    .with_seed_peers(dedup_socket_addrs(seed_peers))
    .with_file_backed_identity(paths.identity_path.clone())
    .with_file_backed_network_state(paths.network_state_path.clone())
}

async fn maybe_import_configured_join_bundle(
    node: &LocalClusterNode,
    config: &MeshLaneConfig,
) -> Result<Option<ClusterJoinBundleImportOutcome>, String> {
    let Some(bundle) = config.join_bundle.clone() else {
        return Ok(None);
    };
    if !matches!(
        bundle.admission,
        ClusterJoinAdmissionMaterial::SharedAdmission { .. }
    ) {
        return Err(String::from(
            "psionic mesh lane run refused a signed-introduction join bundle because the published service mode still requires shared admission material for local cluster identity",
        ));
    }
    let current_state = node.durable_network_state().await;
    if current_state
        .last_imported_join_bundle
        .as_ref()
        .is_some_and(|record| record.bundle == bundle)
    {
        return Ok(None);
    }
    let outcome = node
        .import_join_bundle(bundle, unix_timestamp_ms())
        .await
        .map_err(|error| format!("failed to import configured mesh join bundle: {error}"))?;
    match &outcome {
        ClusterJoinBundleImportOutcome::Refused { reason } => Err(format!(
            "configured mesh join bundle was refused: {}",
            serde_json::to_string(reason).unwrap_or_else(|_| format!("{reason:?}"))
        )),
        _ => Ok(Some(outcome)),
    }
}

fn write_install_artifacts(
    paths: &MeshLanePaths,
    config: &MeshLaneConfig,
    binary_path: &Path,
) -> Result<(), String> {
    let run_script_path = paths.run_script_path(config.service_name.as_str());
    let script = mesh_lane_run_script(paths, binary_path);
    write_text_file(&run_script_path, script.as_str())?;
    set_executable(&run_script_path)?;

    let systemd_unit_path = paths.systemd_unit_path(config.service_name.as_str());
    let systemd_unit = mesh_lane_systemd_unit(paths, config);
    write_text_file(&systemd_unit_path, systemd_unit.as_str())?;

    let launchd_plist_path = paths.launchd_plist_path(config.service_name.as_str());
    let launchd_plist = mesh_lane_launchd_plist(paths, config);
    write_text_file(&launchd_plist_path, launchd_plist.as_str())
}

fn mesh_lane_run_script(paths: &MeshLanePaths, binary_path: &Path) -> String {
    format!(
        "#!/bin/sh\nset -eu\nmkdir -p {logs}\ncd {root}\nexec >>{stdout_log} 2>>{stderr_log}\nexec {binary} run --root {root}\n",
        logs = shell_escape(paths.logs_dir.to_string_lossy().as_ref()),
        root = shell_escape(paths.root.to_string_lossy().as_ref()),
        stdout_log = shell_escape(paths.logs_dir.join("stdout.log").to_string_lossy().as_ref()),
        stderr_log = shell_escape(paths.logs_dir.join("stderr.log").to_string_lossy().as_ref()),
        binary = shell_escape(binary_path.to_string_lossy().as_ref()),
    )
}

fn mesh_lane_systemd_unit(paths: &MeshLanePaths, config: &MeshLaneConfig) -> String {
    let script = shell_escape(
        paths
            .run_script_path(config.service_name.as_str())
            .to_string_lossy()
            .as_ref(),
    );
    format!(
        "[Unit]\nDescription=Psionic mesh lane {service_name}\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nWorkingDirectory={root}\nExecStart=/bin/sh -lc {script}\nRestart=always\nRestartSec=2\n\n[Install]\nWantedBy=default.target\n",
        service_name = config.service_name,
        root = paths.root.to_string_lossy(),
        script = script,
    )
}

fn mesh_lane_launchd_plist(paths: &MeshLanePaths, config: &MeshLaneConfig) -> String {
    let label = format!("com.openagents.psionic.{}", config.service_name);
    let script = shell_escape(
        paths
            .run_script_path(config.service_name.as_str())
            .to_string_lossy()
            .as_ref(),
    );
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n  <key>Label</key>\n  <string>{label}</string>\n  <key>ProgramArguments</key>\n  <array>\n    <string>/bin/sh</string>\n    <string>-lc</string>\n    <string>{script}</string>\n  </array>\n  <key>WorkingDirectory</key>\n  <string>{working_directory}</string>\n  <key>RunAtLoad</key>\n  <true/>\n  <key>KeepAlive</key>\n  <true/>\n</dict>\n</plist>\n",
        label = xml_escape(label.as_str()),
        script = xml_escape(script.as_str()),
        working_directory = xml_escape(paths.root.to_string_lossy().as_ref()),
    )
}

fn load_join_bundle(path: &Path) -> Result<ClusterJoinBundle, String> {
    let bytes = fs::read(path)
        .map_err(|error| format!("failed to read join bundle `{}`: {error}", path.display()))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| format!("failed to parse join bundle `{}`: {error}", path.display()))
}

fn admission_token_from_join_bundle(bundle: Option<&ClusterJoinBundle>) -> Option<String> {
    match bundle?.admission.clone() {
        ClusterJoinAdmissionMaterial::SharedAdmission { admission_token } => {
            Some(admission_token.as_str().to_string())
        }
        ClusterJoinAdmissionMaterial::SignedIntroduction { .. } => None,
    }
}

fn cluster_id_for_config(config: &MeshLaneConfig) -> String {
    cluster_id_from_values(config.namespace.as_str(), config.admission_token.as_str())
}

fn cluster_id_from_values(namespace: &str, admission_token: &str) -> String {
    let namespace = ClusterNamespace::new(namespace);
    let admission_token = AdmissionToken::new(admission_token);
    psionic_net::ClusterId::new(&namespace, &admission_token)
        .as_str()
        .to_string()
}

fn normalize_model_path(paths: &MeshLanePaths, model_path: &Path) -> PathBuf {
    if model_path.is_absolute() {
        model_path.to_path_buf()
    } else {
        paths.models_dir.join(model_path)
    }
}

fn dedup_socket_addrs(mut values: Vec<SocketAddr>) -> Vec<SocketAddr> {
    values.sort_unstable();
    values.dedup();
    values
}

fn validate_service_name(value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.'))
    {
        return Err(String::from(
            "invalid --service-name value (expected only ASCII letters, digits, '.', '-', or '_')",
        ));
    }
    Ok(())
}

fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(path))
            .map_err(|error| format!("failed to resolve current directory: {error}"))
    }
}

fn write_text_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("failed to create `{}`: {error}", parent.display()))?;
    }
    fs::write(path, content)
        .map_err(|error| format!("failed to write `{}`: {error}", path.display()))
}

#[cfg(unix)]
fn set_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)
        .map_err(|error| format!("failed to stat `{}`: {error}", path.display()))?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("failed to mark `{}` executable: {error}", path.display()))
}

#[cfg(not(unix))]
fn set_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for `{flag}`"))
}

fn parse_socket_addr(value: String, flag: &str) -> Result<SocketAddr, String> {
    value
        .parse()
        .map_err(|error| format!("invalid {flag} value `{value}`: {error}"))
}

fn parse_u16(value: String, flag: &str) -> Result<u16, String> {
    value
        .parse()
        .map_err(|error| format!("invalid {flag} value `{value}`: {error}"))
}

fn parse_u8(value: String, flag: &str) -> Result<u8, String> {
    value
        .parse()
        .map_err(|error| format!("invalid {flag} value `{value}`: {error}"))
}

fn shell_escape(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
        .replace('\'', "&apos;")
}

fn unix_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn usage() -> String {
    format!(
        "usage: psionic-mesh-lane <install|run|export-join-bundle> ...\n\n{}\n{}\n{}",
        install_usage(),
        run_usage(),
        export_join_bundle_usage()
    )
}

fn install_usage() -> String {
    String::from(
        "install: psionic-mesh-lane install --root <dir> -m <model> [-m <model> ...] [--backend cpu|cuda] [--host <ip>] [--port <port>] [--reasoning-budget <n>] [--mesh-bind <ip:port>] [--seed-peer <ip:port> ...] [--service-name <name>] [--namespace <value>] [--admission-token <value>] [--node-role coordinator_only|executor_only|mixed] [--join-bundle <path>]",
    )
}

fn run_usage() -> String {
    String::from("run: psionic-mesh-lane run --root <dir>")
}

fn export_join_bundle_usage() -> String {
    String::from(
        "export-join-bundle: psionic-mesh-lane export-join-bundle --root <dir> --out <path> [--mesh-label <label>] [--advertise <ip:port> ...]",
    )
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::SocketAddr,
        path::{Path, PathBuf},
    };

    use super::{
        MeshLaneBackend, MeshLaneCommand, MeshLaneConfig, MeshLaneInstallRequest, MeshLaneNodeRole,
        MeshLanePaths, admission_token_from_join_bundle, install_mesh_lane, local_cluster_config,
        maybe_import_configured_join_bundle, parse_command_from,
    };
    use psionic_net::{
        ClusterJoinBundleImportOutcome, LocalClusterConfig, LocalClusterNode, NodeRole,
    };
    use tempfile::tempdir;

    #[test]
    fn parse_install_command_accepts_shared_mesh_flags() {
        let command = parse_command_from([
            "install",
            "--root",
            "/tmp/mesh-root",
            "-m",
            "gemma4-e4b.gguf",
            "--backend",
            "cuda",
            "--mesh-bind",
            "0.0.0.0:47470",
            "--node-role",
            "mixed",
        ])
        .expect("install command");

        match command {
            MeshLaneCommand::Install(request) => {
                assert_eq!(request.root, PathBuf::from("/tmp/mesh-root"));
                assert_eq!(request.model_paths, vec![PathBuf::from("gemma4-e4b.gguf")]);
                assert_eq!(request.backend, Some(MeshLaneBackend::Cuda));
                assert_eq!(request.node_role, Some(MeshLaneNodeRole::Mixed));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn install_writes_layout_and_service_artifacts() -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempdir()?;
        let root = temp.path().join("mesh-lane");
        let report = install_mesh_lane(
            &MeshLaneInstallRequest {
                root: root.clone(),
                model_paths: vec![PathBuf::from("tiny-model.gguf")],
                service_name: Some(String::from("mesh-lane-a")),
                ..Default::default()
            },
            Path::new("/opt/psionic/bin/psionic-mesh-lane"),
        )?;
        let paths = MeshLanePaths::new(root);
        let config = MeshLaneConfig::load_json(&paths.config_path)?;

        assert_eq!(config.service_name, "mesh-lane-a");
        assert_eq!(
            config.model_paths,
            vec![paths.models_dir.join("tiny-model.gguf")]
        );
        assert!(report.generated_admission_token.is_some());
        assert!(paths.identity_path.parent().is_some_and(Path::exists));
        assert!(paths.network_state_path.parent().is_some_and(Path::exists));
        let script = fs::read_to_string(paths.run_script_path("mesh-lane-a"))?;
        assert!(script.contains("run --root"));
        assert!(script.contains("stdout.log"));
        let systemd = fs::read_to_string(paths.systemd_unit_path("mesh-lane-a"))?;
        assert!(systemd.contains("Restart=always"));
        let plist = fs::read_to_string(paths.launchd_plist_path("mesh-lane-a"))?;
        assert!(plist.contains("com.openagents.psionic.mesh-lane-a"));
        Ok(())
    }

    #[test]
    fn mesh_lane_transport_paths_preserve_identity_and_join_state_across_restart()
    -> Result<(), Box<dyn std::error::Error>> {
        let runtime = tokio::runtime::Runtime::new()?;
        runtime.block_on(async {
            let temp = tempdir()?;
            let root = temp.path().join("mesh-lane");
            install_mesh_lane(
                &MeshLaneInstallRequest {
                    root: root.clone(),
                    model_paths: vec![PathBuf::from("tiny-model.gguf")],
                    namespace: Some(String::from("mesh-home")),
                    admission_token: Some(String::from("shared-secret")),
                    mesh_bind_addr: Some(SocketAddr::from(([127, 0, 0, 1], 0))),
                    ..Default::default()
                },
                Path::new("/opt/psionic/bin/psionic-mesh-lane"),
            )?;
            let paths = MeshLanePaths::new(root);
            let mut config = MeshLaneConfig::load_json(&paths.config_path)?;
            let exporter = LocalClusterNode::spawn(LocalClusterConfig::new(
                "mesh-home",
                "shared-secret",
                SocketAddr::from(([127, 0, 0, 1], 0)),
                NodeRole::Mixed,
            ))
            .await?;
            let bundle =
                exporter.export_join_bundle("mesh-home", vec![exporter.local_addr()], 41_000);
            assert_eq!(
                admission_token_from_join_bundle(Some(&bundle)).as_deref(),
                Some("shared-secret")
            );
            config.join_bundle = Some(bundle);
            config.store_json(&paths.config_path)?;

            let first = LocalClusterNode::spawn(local_cluster_config(&config, &paths)).await?;
            let first_outcome = maybe_import_configured_join_bundle(&first, &config).await?;
            assert!(matches!(
                first_outcome,
                Some(ClusterJoinBundleImportOutcome::SharedAdmissionImported { .. })
            ));
            let first_state = first.durable_network_state().await;
            let first_node_id = first.local_identity().node_id.clone();
            let first_epoch = first.local_identity().node_epoch;
            drop(first);

            let restarted = LocalClusterNode::spawn(local_cluster_config(&config, &paths)).await?;
            let second_outcome = maybe_import_configured_join_bundle(&restarted, &config).await?;
            assert!(second_outcome.is_none());
            let restarted_state = restarted.durable_network_state().await;
            assert_eq!(restarted.local_identity().node_id, first_node_id);
            assert!(restarted.local_identity().node_epoch > first_epoch);
            assert_eq!(
                restarted_state.last_joined_mesh_preference,
                first_state.last_joined_mesh_preference
            );
            Ok::<_, Box<dyn std::error::Error>>(())
        })?;
        Ok(())
    }
}
