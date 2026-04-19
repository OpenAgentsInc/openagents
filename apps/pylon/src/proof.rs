use std::collections::BTreeMap;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use axum::body::Bytes;
use axum::extract::{Path as AxumPath, State};
use axum::http::StatusCode;
use axum::routing::{get, put};
use axum::{Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;
use tokio::time::Instant;

use crate::render_byte_size;

const PROOF_RUNTIME_SCHEMA_VERSION: u32 = 1;
const DEFAULT_PROOF_NAMESPACE: &str = "authority";
const PROOF_PORT_BASE: u16 = 43_000;
const PROOF_PORT_SLOTS: u16 = 2_000;
const PROOF_PORT_STRIDE: u16 = 10;
const PROOF_ROUTE_TIMEOUT: Duration = Duration::from_secs(10);
const PROOF_POLL_INTERVAL: Duration = Duration::from_millis(200);
const PROOF_ARTIFACT_BUCKET: &str = "gs://proof-local-artifacts";
const PROOF_ARTIFACT_UPLOAD_PREFIX: &str = "/upload";
const TEST_GCS_SERVICE_ACCOUNT_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQC9YHg+P4UZig1h\nzoW/m8IbzylR9O6/9xrqmIzlSfA2S1Cz7w0P+viRoyzLBmYhTmI0p3RmNAMKWwph\nly6a0UkdsGbWsoKoWt8r+gB1zUyP+1tG4A7HDTcTnxG+T2dtJcwE/A0Y8rF4PKEt\nV0qTdHYjRZrEorBYKJdgUbdv1Pgkw0U9SuCJciRLs3SI3PPrKNhNyWERS5Ta0Hnr\nXtwzZ7e44KNJ8F8iMOgh70p0nLN/KtKl+2Gb/CuJh3Mfodkoc+sADKoofBXZct2+\nsGSw66S08q7WfuPkseaqxDlOgSfaHEjzTIMyoxvjyjRWjulVbUIz8i+JWSZUglfP\nIBsQcN1pAgMBAAECggEAAR3yRH5byNkVX4mXVscdkaBZQ35/6qLkz5cZ/3+VeXrA\nUP8uPYGoXQMOEfuoyfFhTZ0OTxRz0lVpmNX63oZ72kWS+jIPUqqeDt/YNwVeQIrp\nCAYGEwV8I+K+Si69sIm9kf2dYEJndw4Zd/QtYGrC+8R+vBaXRagvV2k0wggXVdzx\n7Wq5zqOz9QkeoG11hTkYAgTmVl5PBnAoRE/sNMtYUOf6JnQWmFpEwOTdTf+F8NL1\nFg+ecNH7tjoqsTBjD/lMSaA/kr10fUw4KoITkn2IvtuF2ZFZp2R/Viy9KnfsLyF7\nyb1NJSP2cn3gYp4+BEe5wOdQNO2+lZN7EQKmRzS7uwKBgQD7dtqD6pAw9VFSiLWN\nW8EcDevKOP48lOP++2esUCsXfip3Omn0lmyb+8i11GRz0QwiMywQ21p7sEUwn9HE\nTk2ZjPnaNdPN+i/vZ+RgcmHVeEzeTPNAXeAQ5zAlrJ8Ibh3239BeWHLxCa/p2nsD\nPL3dPXg/CQm68Ph/UjG9XiXSbwKBgQDAyuzEzrqgdc51x2Z40lcQ56zUZVVtW+A8\n485dS5VQMdwFglXzC5QTQ4T3zI1qT+Dd5ATtCkyMNpL07nC/9rQhI0+HTsRZE8P+\nKeSGIFOSvkA2ZwHWKKcctO8n1vOlAwJnjqYEJAZMIg01MtpOFRN0qrDd/9BDUbHi\nHO2smCRZpwKBgAn28r/Jer9F6VwQ6MjaOvPGpXJVAdYavFItWjVc0+hRapNg8DPu\nBg3EU3bJHNXuEcIFLxjX6GUAXi2IF8Lkq3SLPpdkDKmb4WxmPImJ3tCbvMgOWpFR\nZwCkeKb1iTPHUU6oHdSvQpbEoIDu1HMTZB6xQeOVkxoiVGaPNkNfyLXnAoGAeEXg\nQcNKUFJOM9HqzpNCN8ygWHzDN48qrDHeCvvdMYN5ZIJ0BkUB4qarrD+TNXCRszvO\nCuby7EIbmeuqsUdCBq5Vre7otT2MduJBq589I/3GZ2oJjkYcQt9pl2wU4aun81zd\nmxWyTAquPLL11+J0GcNmxYgSr/ymQY6Ug6kCfF8CgYEA2fSIcskydJ94TpX8Dpqm\nBwDXhRIZo6hkLjAqt6hHa7Fs/2qZXAeeX7/oxxfHBWqtPcTnp3N91xgfkPjarPeM\nth0qg1Cu4Y4ZyQfpaVaZB3aWIJB0PdWdMBZa/EUZDu9kFoaExF3BdzA2j7pmMDj4\nOZi9gzTa10z894ZuBJJkMPA=\n-----END PRIVATE KEY-----\n";

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofCommand {
    Authority { command: ProofAuthorityCommand },
    Internal { command: ProofInternalCommand },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofAuthorityCommand {
    Up {
        mode: ProofAuthorityMode,
        json: bool,
    },
    Status {
        json: bool,
    },
    Down {
        json: bool,
    },
    Reset {
        json: bool,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProofInternalCommand {
    ArtifactStoreServe {
        listen_addr: SocketAddr,
        store_root: PathBuf,
        trace_path: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProofAuthorityMode {
    ProdShaped,
    DebugAuthority,
}

impl ProofAuthorityMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::ProdShaped => "prod_shaped",
            Self::DebugAuthority => "debug_authority",
        }
    }

    fn authority_binary(self) -> &'static str {
        match self {
            Self::ProdShaped => "nexus-relay",
            Self::DebugAuthority => "nexus-control",
        }
    }

    fn authority_package(self) -> &'static str {
        self.authority_binary()
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofProcessRecord {
    binary: String,
    pid: Option<u32>,
    log_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofPersistedPaths {
    namespace_root: String,
    authority_env_path: String,
    relay_data_dir: String,
    receipt_log_path: String,
    kernel_state_path: String,
    treasury_state_path: String,
    treasury_wallet_dir: String,
    treasury_wallet_mnemonic_path: String,
    training_trn_identity_path: String,
    signer_credentials_path: String,
    artifact_store_root: String,
    artifact_trace_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofAuthoritySurfaceUrls {
    authority_base_url: String,
    artifact_store_base_url: String,
    relay_ws_url: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofNamespacePorts {
    relay_http: u16,
    relay_upstream: u16,
    control_http: u16,
    artifact_store: u16,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofArtifactSmokeReport {
    artifact_id: String,
    relative_object_path: String,
    expected_digest: String,
    payload_size_bytes: u64,
    trace_entry_count: usize,
    verified_at_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
struct ProofAuthorityRuntimeState {
    schema_version: u32,
    namespace: String,
    mode: ProofAuthorityMode,
    started_at_ms: i64,
    admin_bearer_token: String,
    treasury_enabled: bool,
    ports: ProofNamespacePorts,
    paths: ProofPersistedPaths,
    urls: ProofAuthoritySurfaceUrls,
    authority_process: ProofProcessRecord,
    artifact_store_process: ProofProcessRecord,
    last_artifact_smoke: Option<ProofArtifactSmokeReport>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofProcessStatus {
    binary: String,
    pid: Option<u32>,
    running: bool,
    log_path: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofRouteProbe {
    route_id: String,
    url: String,
    ok: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    status: Option<u16>,
    detail: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
struct ProofAuthorityStatusReport {
    configured: bool,
    namespace: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    mode: Option<ProofAuthorityMode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    started_at_ms: Option<i64>,
    admin_auth_configured: bool,
    treasury_enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ports: Option<ProofNamespacePorts>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    paths: Option<ProofPersistedPaths>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    urls: Option<ProofAuthoritySurfaceUrls>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    authority_process: Option<ProofProcessStatus>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    artifact_store_process: Option<ProofProcessStatus>,
    probes: Vec<ProofRouteProbe>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    artifact_smoke: Option<ProofArtifactSmokeReport>,
}

#[derive(Clone)]
struct ArtifactStoreState {
    store_root: PathBuf,
    trace_path: PathBuf,
}

#[derive(Clone, Debug)]
struct ProofLayout {
    namespace_root: PathBuf,
    authority_env_path: PathBuf,
    relay_data_dir: PathBuf,
    receipt_log_path: PathBuf,
    kernel_state_path: PathBuf,
    treasury_state_path: PathBuf,
    treasury_wallet_dir: PathBuf,
    treasury_wallet_mnemonic_path: PathBuf,
    training_trn_identity_path: PathBuf,
    signer_credentials_path: PathBuf,
    artifact_store_root: PathBuf,
    artifact_trace_path: PathBuf,
    runtime_state_path: PathBuf,
    authority_log_path: PathBuf,
    artifact_store_log_path: PathBuf,
}

pub fn parse_proof_command(args: &[String], start_index: usize) -> Result<ProofCommand> {
    match args.get(start_index).map(String::as_str) {
        Some("authority") => Ok(ProofCommand::Authority {
            command: parse_proof_authority_command(args, start_index + 1)?,
        }),
        Some("internal") => Ok(ProofCommand::Internal {
            command: parse_proof_internal_command(args, start_index + 1)?,
        }),
        Some(other) => bail!("unknown proof command: {other}"),
        None => bail!("missing proof subcommand"),
    }
}

pub async fn run_proof_command(config_path: &Path, command: ProofCommand) -> Result<Option<String>> {
    match command {
        ProofCommand::Authority { command } => {
            let report = match command {
                ProofAuthorityCommand::Up { mode, .. } => {
                    ensure_proof_authority_up(config_path, mode).await?
                }
                ProofAuthorityCommand::Status { .. } => collect_proof_status(config_path).await?,
                ProofAuthorityCommand::Down { .. } => {
                    let _ = stop_proof_authority(config_path, false).await?;
                    collect_proof_status(config_path).await?
                }
                ProofAuthorityCommand::Reset { .. } => {
                    let _ = stop_proof_authority(config_path, true).await?;
                    collect_proof_status(config_path).await?
                }
            };
            let json = matches!(
                command,
                ProofAuthorityCommand::Up { json: true, .. }
                    | ProofAuthorityCommand::Status { json: true }
                    | ProofAuthorityCommand::Down { json: true }
                    | ProofAuthorityCommand::Reset { json: true }
            );
            if json {
                return Ok(Some(serde_json::to_string_pretty(&report)?));
            }
            Ok(Some(render_proof_status_report(&report)))
        }
        ProofCommand::Internal { command } => match command {
            ProofInternalCommand::ArtifactStoreServe {
                listen_addr,
                store_root,
                trace_path,
            } => {
                run_artifact_store_server(listen_addr, store_root, trace_path).await?;
                Ok(None)
            }
        },
    }
}

fn parse_proof_authority_command(
    args: &[String],
    start_index: usize,
) -> Result<ProofAuthorityCommand> {
    match args.get(start_index).map(String::as_str) {
        Some("up") => {
            let mut json = false;
            let mut mode = ProofAuthorityMode::ProdShaped;
            let mut index = start_index + 1;
            while index < args.len() {
                match args[index].as_str() {
                    "--json" => {
                        json = true;
                        index += 1;
                    }
                    "--mode" => {
                        let value = args
                            .get(index + 1)
                            .ok_or_else(|| anyhow!("missing value for proof authority up --mode"))?;
                        mode = parse_proof_authority_mode(value.as_str())?;
                        index += 2;
                    }
                    other => bail!("unexpected argument for proof authority up: {other}"),
                }
            }
            Ok(ProofAuthorityCommand::Up { mode, json })
        }
        Some("status") => Ok(ProofAuthorityCommand::Status {
            json: parse_json_only(args, start_index + 1, "proof authority status")?,
        }),
        Some("down") => Ok(ProofAuthorityCommand::Down {
            json: parse_json_only(args, start_index + 1, "proof authority down")?,
        }),
        Some("reset") => Ok(ProofAuthorityCommand::Reset {
            json: parse_json_only(args, start_index + 1, "proof authority reset")?,
        }),
        Some(other) => bail!("unknown proof authority command: {other}"),
        None => bail!("missing proof authority command"),
    }
}

fn parse_proof_internal_command(args: &[String], start_index: usize) -> Result<ProofInternalCommand> {
    match (
        args.get(start_index).map(String::as_str),
        args.get(start_index + 1).map(String::as_str),
    ) {
        (Some("artifact-store"), Some("serve")) => {
            let mut listen_addr = None;
            let mut store_root = None;
            let mut trace_path = None;
            let mut index = start_index + 2;
            while index < args.len() {
                match args[index].as_str() {
                    "--listen-addr" => {
                        let value = args
                            .get(index + 1)
                            .ok_or_else(|| anyhow!("missing value for --listen-addr"))?;
                        listen_addr = Some(
                            value
                                .parse::<SocketAddr>()
                                .with_context(|| format!("invalid proof listen addr `{value}`"))?,
                        );
                        index += 2;
                    }
                    "--store-root" => {
                        let value = args
                            .get(index + 1)
                            .ok_or_else(|| anyhow!("missing value for --store-root"))?;
                        store_root = Some(PathBuf::from(value));
                        index += 2;
                    }
                    "--trace-path" => {
                        let value = args
                            .get(index + 1)
                            .ok_or_else(|| anyhow!("missing value for --trace-path"))?;
                        trace_path = Some(PathBuf::from(value));
                        index += 2;
                    }
                    other => bail!("unexpected argument for proof internal artifact-store serve: {other}"),
                }
            }
            Ok(ProofInternalCommand::ArtifactStoreServe {
                listen_addr: listen_addr
                    .ok_or_else(|| anyhow!("proof internal artifact-store serve requires --listen-addr"))?,
                store_root: store_root
                    .ok_or_else(|| anyhow!("proof internal artifact-store serve requires --store-root"))?,
                trace_path: trace_path
                    .ok_or_else(|| anyhow!("proof internal artifact-store serve requires --trace-path"))?,
            })
        }
        (Some(other), _) => bail!("unknown proof internal command: {other}"),
        _ => bail!("missing proof internal command"),
    }
}

fn parse_json_only(args: &[String], start_index: usize, context: &str) -> Result<bool> {
    match args.get(start_index) {
        None => Ok(false),
        Some(value) if value == "--json" && start_index + 1 == args.len() => Ok(true),
        Some(value) if value == "--json" => bail!("{context} --json does not accept additional arguments"),
        Some(other) => bail!("unexpected argument for {context}: {other}"),
    }
}

fn parse_proof_authority_mode(value: &str) -> Result<ProofAuthorityMode> {
    match value {
        "prod-shaped" | "prod_shaped" => Ok(ProofAuthorityMode::ProdShaped),
        "debug-authority" | "debug_authority" => Ok(ProofAuthorityMode::DebugAuthority),
        other => bail!("unknown proof authority mode: {other}"),
    }
}

async fn ensure_proof_authority_up(
    config_path: &Path,
    mode: ProofAuthorityMode,
) -> Result<ProofAuthorityStatusReport> {
    let namespace = DEFAULT_PROOF_NAMESPACE.to_string();
    let layout = proof_layout(config_path, namespace.as_str());
    if let Some(state) = load_runtime_state(layout.runtime_state_path.as_path())? {
        let authority_running = process_is_running(&state.authority_process);
        let artifact_running = process_is_running(&state.artifact_store_process);
        if authority_running && artifact_running && state.mode == mode {
            return collect_proof_status(config_path).await;
        }
        let _ = stop_runtime_processes(&state).await;
    }

    ensure_layout_dirs(&layout)?;
    write_signer_credentials(layout.signer_credentials_path.as_path())?;

    let ports = proof_namespace_ports(namespace.as_str());
    let admin_bearer_token = format!("proof_admin_{}", super::random_token());
    let artifact_store_base_url =
        format!("http://127.0.0.1:{}{}", ports.artifact_store, PROOF_ARTIFACT_UPLOAD_PREFIX);
    let authority_base_url = match mode {
        ProofAuthorityMode::ProdShaped => format!("http://127.0.0.1:{}", ports.relay_http),
        ProofAuthorityMode::DebugAuthority => format!("http://127.0.0.1:{}", ports.control_http),
    };
    let urls = ProofAuthoritySurfaceUrls {
        authority_base_url: authority_base_url.clone(),
        artifact_store_base_url: artifact_store_base_url.clone(),
        relay_ws_url: (mode == ProofAuthorityMode::ProdShaped)
            .then(|| format!("ws://127.0.0.1:{}/", ports.relay_http)),
    };
    let paths = ProofPersistedPaths {
        namespace_root: layout.namespace_root.display().to_string(),
        authority_env_path: layout.authority_env_path.display().to_string(),
        relay_data_dir: layout.relay_data_dir.display().to_string(),
        receipt_log_path: layout.receipt_log_path.display().to_string(),
        kernel_state_path: layout.kernel_state_path.display().to_string(),
        treasury_state_path: layout.treasury_state_path.display().to_string(),
        treasury_wallet_dir: layout.treasury_wallet_dir.display().to_string(),
        treasury_wallet_mnemonic_path: layout.treasury_wallet_mnemonic_path.display().to_string(),
        training_trn_identity_path: layout.training_trn_identity_path.display().to_string(),
        signer_credentials_path: layout.signer_credentials_path.display().to_string(),
        artifact_store_root: layout.artifact_store_root.display().to_string(),
        artifact_trace_path: layout.artifact_trace_path.display().to_string(),
    };

    let current_exe = current_executable_path()?;
    let artifact_args = vec![
        "proof".to_string(),
        "internal".to_string(),
        "artifact-store".to_string(),
        "serve".to_string(),
        "--listen-addr".to_string(),
        format!("127.0.0.1:{}", ports.artifact_store),
        "--store-root".to_string(),
        layout.artifact_store_root.display().to_string(),
        "--trace-path".to_string(),
        layout.artifact_trace_path.display().to_string(),
    ];
    let artifact_pid = spawn_logged_process(
        current_exe.as_path(),
        artifact_args.as_slice(),
        &[],
        layout.artifact_store_log_path.as_path(),
    )?;
    wait_for_route(
        format!("http://127.0.0.1:{}/healthz", ports.artifact_store).as_str(),
        &[StatusCode::OK],
    )
    .await?;

    let authority_binary =
        resolve_workspace_binary(mode.authority_binary(), mode.authority_package())?;
    let authority_env = authority_environment(
        mode,
        &ports,
        &layout,
        admin_bearer_token.as_str(),
        urls.relay_ws_url.as_deref(),
        artifact_store_base_url.as_str(),
    );
    write_env_manifest(layout.authority_env_path.as_path(), &authority_env)?;
    let authority_pid = spawn_logged_process(
        authority_binary.as_path(),
        &[],
        authority_env
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>()
            .as_slice(),
        layout.authority_log_path.as_path(),
    )?;
    wait_for_route(
        format!("{authority_base_url}/healthz").as_str(),
        &[StatusCode::OK],
    )
    .await?;

    let mut state = ProofAuthorityRuntimeState {
        schema_version: PROOF_RUNTIME_SCHEMA_VERSION,
        namespace,
        mode,
        started_at_ms: super::now_epoch_ms(),
        admin_bearer_token,
        treasury_enabled: true,
        ports,
        paths,
        urls,
        authority_process: ProofProcessRecord {
            binary: authority_binary.display().to_string(),
            pid: Some(authority_pid),
            log_path: layout.authority_log_path.display().to_string(),
        },
        artifact_store_process: ProofProcessRecord {
            binary: current_exe.display().to_string(),
            pid: Some(artifact_pid),
            log_path: layout.artifact_store_log_path.display().to_string(),
        },
        last_artifact_smoke: None,
    };
    let artifact_smoke = run_artifact_smoke(&state).await?;
    state.last_artifact_smoke = Some(artifact_smoke);
    save_runtime_state(layout.runtime_state_path.as_path(), &state)?;
    collect_proof_status(config_path).await
}

async fn collect_proof_status(config_path: &Path) -> Result<ProofAuthorityStatusReport> {
    let layout = proof_layout(config_path, DEFAULT_PROOF_NAMESPACE);
    let Some(state) = load_runtime_state(layout.runtime_state_path.as_path())? else {
        return Ok(ProofAuthorityStatusReport {
            configured: false,
            namespace: DEFAULT_PROOF_NAMESPACE.to_string(),
            mode: None,
            started_at_ms: None,
            admin_auth_configured: false,
            treasury_enabled: false,
            ports: None,
            paths: None,
            urls: None,
            authority_process: None,
            artifact_store_process: None,
            probes: Vec::new(),
            artifact_smoke: None,
        });
    };

    let authority_process = ProofProcessStatus {
        binary: state.authority_process.binary.clone(),
        pid: state.authority_process.pid,
        running: process_is_running(&state.authority_process),
        log_path: state.authority_process.log_path.clone(),
    };
    let artifact_store_process = ProofProcessStatus {
        binary: state.artifact_store_process.binary.clone(),
        pid: state.artifact_store_process.pid,
        running: process_is_running(&state.artifact_store_process),
        log_path: state.artifact_store_process.log_path.clone(),
    };
    let probes = collect_route_probes(&state).await;

    Ok(ProofAuthorityStatusReport {
        configured: true,
        namespace: state.namespace,
        mode: Some(state.mode),
        started_at_ms: Some(state.started_at_ms),
        admin_auth_configured: !state.admin_bearer_token.is_empty(),
        treasury_enabled: state.treasury_enabled,
        ports: Some(state.ports),
        paths: Some(state.paths),
        urls: Some(state.urls),
        authority_process: Some(authority_process),
        artifact_store_process: Some(artifact_store_process),
        probes,
        artifact_smoke: state.last_artifact_smoke,
    })
}

async fn stop_proof_authority(config_path: &Path, reset: bool) -> Result<bool> {
    let layout = proof_layout(config_path, DEFAULT_PROOF_NAMESPACE);
    let Some(state) = load_runtime_state(layout.runtime_state_path.as_path())? else {
        return Ok(false);
    };
    stop_runtime_processes(&state).await?;
    if reset {
        if layout.namespace_root.exists() {
            fs::remove_dir_all(layout.namespace_root.as_path()).with_context(|| {
                format!(
                    "failed to remove proof namespace {}",
                    layout.namespace_root.display()
                )
            })?;
        }
    } else {
        let mut stopped = state;
        stopped.authority_process.pid = None;
        stopped.artifact_store_process.pid = None;
        save_runtime_state(layout.runtime_state_path.as_path(), &stopped)?;
    }
    Ok(true)
}

async fn stop_runtime_processes(state: &ProofAuthorityRuntimeState) -> Result<()> {
    if let Some(pid) = state.authority_process.pid {
        stop_pid(pid).await?;
    }
    if let Some(pid) = state.artifact_store_process.pid {
        stop_pid(pid).await?;
    }
    Ok(())
}

async fn stop_pid(pid: u32) -> Result<()> {
    terminate_pid(pid, false)?;
    if let Err(error) = wait_for_pid_exit(pid).await {
        if super::training_supervisor_pid_is_running(pid) {
            terminate_pid(pid, true)?;
            wait_for_pid_exit(pid).await?;
        } else {
            return Err(error);
        }
    }
    Ok(())
}

async fn wait_for_pid_exit(pid: u32) -> Result<()> {
    let deadline = Instant::now() + PROOF_ROUTE_TIMEOUT;
    while Instant::now() < deadline {
        if !super::training_supervisor_pid_is_running(pid) {
            return Ok(());
        }
        tokio::time::sleep(PROOF_POLL_INTERVAL).await;
    }
    bail!("timed out waiting for pid {pid} to exit")
}

fn process_is_running(process: &ProofProcessRecord) -> bool {
    process
        .pid
        .is_some_and(super::training_supervisor_pid_is_running)
}

fn current_executable_path() -> Result<PathBuf> {
    std::env::current_exe().context("failed to resolve current executable")
}

fn resolve_workspace_binary(binary: &str, package: &str) -> Result<PathBuf> {
    let executable = platform_binary_name(binary);
    let current_exe = current_executable_path()?;
    let mut candidates = Vec::new();
    if let Some(parent) = current_exe.parent() {
        candidates.push(parent.join(executable.as_str()));
        if let Some(grandparent) = parent.parent() {
            candidates.push(grandparent.join(executable.as_str()));
        }
    }
    candidates.push(
        workspace_root()
            .join("target")
            .join("debug")
            .join(executable.as_str()),
    );
    if let Some(existing) = candidates.iter().find(|candidate| candidate.is_file()) {
        return Ok(existing.clone());
    }

    let status = StdCommand::new("cargo")
        .current_dir(workspace_root())
        .args(["build", "-p", package, "--bin", binary])
        .status()
        .with_context(|| format!("failed to build supporting binary `{binary}`"))?;
    if !status.success() {
        bail!("cargo build failed for supporting binary `{binary}`");
    }
    candidates
        .into_iter()
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| anyhow!("supporting binary `{binary}` was not produced in target/debug"))
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn platform_binary_name(binary: &str) -> String {
    if cfg!(windows) {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    }
}

fn spawn_logged_process(
    binary: &Path,
    args: &[String],
    envs: &[(&str, &str)],
    log_path: &Path,
) -> Result<u32> {
    if let Some(parent) = log_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create log dir {}", parent.display()))?;
    }
    let log_file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .with_context(|| format!("failed to open log file {}", log_path.display()))?;
    let stderr_file = log_file
        .try_clone()
        .with_context(|| format!("failed to clone log file {}", log_path.display()))?;
    let mut command = StdCommand::new(binary);
    command.args(args);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        // Proof services must survive the short-lived CLI process that launched
        // them, so move them into a fresh session before spawn.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    for (key, value) in envs {
        command.env(key, value);
    }
    let child = command
        .stdin(Stdio::null())
        .stdout(Stdio::from(log_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .with_context(|| format!("failed to spawn {}", binary.display()))?;
    Ok(child.id())
}

fn terminate_pid(pid: u32, force: bool) -> Result<()> {
    #[cfg(unix)]
    {
        let pid_text = pid.to_string();
        let signal = if force { "-KILL" } else { "-TERM" };
        let status = StdCommand::new("kill")
            .args([signal, pid_text.as_str()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .with_context(|| format!("failed to invoke kill {signal}"))?;
        if !status.success() && super::training_supervisor_pid_is_running(pid) {
            bail!("kill {signal} exited unsuccessfully for pid {pid}");
        }
        Ok(())
    }
    #[cfg(windows)]
    {
        let mut args = vec!["/PID", &pid.to_string(), "/T"];
        if force {
            args.push("/F");
        }
        let status = StdCommand::new("taskkill")
            .args(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .context("failed to invoke taskkill")?;
        if !status.success() {
            bail!("taskkill exited unsuccessfully for pid {pid}");
        }
        Ok(())
    }
}

fn ensure_layout_dirs(layout: &ProofLayout) -> Result<()> {
    for path in [
        layout.namespace_root.as_path(),
        layout.relay_data_dir.as_path(),
        layout.artifact_store_root.as_path(),
        layout
            .authority_log_path
            .parent()
            .ok_or_else(|| anyhow!("authority log path missing parent"))?,
        layout
            .kernel_state_path
            .parent()
            .ok_or_else(|| anyhow!("kernel state path missing parent"))?,
    ] {
        fs::create_dir_all(path)
            .with_context(|| format!("failed to create {}", path.display()))?;
    }
    Ok(())
}

fn proof_layout(config_path: &Path, namespace: &str) -> ProofLayout {
    let base_root = config_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(super::default_home_dir);
    let namespace_root = base_root.join("proof").join("namespaces").join(namespace);
    ProofLayout {
        authority_env_path: namespace_root.join("authority").join("authority.env"),
        relay_data_dir: namespace_root.join("authority").join("relay-data"),
        receipt_log_path: namespace_root.join("state").join("receipts.jsonl"),
        kernel_state_path: namespace_root.join("state").join("kernel-state.json"),
        treasury_state_path: namespace_root.join("state").join("treasury-state.json"),
        treasury_wallet_dir: namespace_root.join("state").join("treasury-wallet"),
        treasury_wallet_mnemonic_path: namespace_root.join("state").join("treasury.mnemonic"),
        training_trn_identity_path: namespace_root.join("state").join("training-trn-identity.mnemonic"),
        signer_credentials_path: namespace_root.join("artifacts").join("gcs-signer.json"),
        artifact_store_root: namespace_root.join("artifacts").join("store"),
        artifact_trace_path: namespace_root.join("artifacts").join("object-trace.jsonl"),
        runtime_state_path: namespace_root.join("runtime-state.json"),
        authority_log_path: namespace_root.join("logs").join("authority.log"),
        artifact_store_log_path: namespace_root.join("logs").join("artifact-store.log"),
        namespace_root,
    }
}

fn proof_namespace_ports(namespace: &str) -> ProofNamespacePorts {
    let digest = Sha256::digest(namespace.as_bytes());
    let slot = u16::from_be_bytes([digest[0], digest[1]]) % PROOF_PORT_SLOTS;
    let base = PROOF_PORT_BASE + slot * PROOF_PORT_STRIDE;
    ProofNamespacePorts {
        relay_http: base,
        relay_upstream: base + 1,
        control_http: base + 2,
        artifact_store: base + 3,
    }
}

fn write_signer_credentials(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create signer dir {}", parent.display()))?;
    }
    let payload = serde_json::to_string_pretty(&json!({
        "client_email": "proof-local-signer@test.openagents.invalid",
        "private_key": TEST_GCS_SERVICE_ACCOUNT_PRIVATE_KEY,
    }))?;
    fs::write(path, format!("{payload}\n"))
        .with_context(|| format!("failed to write signer credentials {}", path.display()))
}

fn authority_environment(
    mode: ProofAuthorityMode,
    ports: &ProofNamespacePorts,
    layout: &ProofLayout,
    admin_bearer_token: &str,
    relay_ws_url: Option<&str>,
    artifact_store_base_url: &str,
) -> BTreeMap<String, String> {
    let authority_base_port = match mode {
        ProofAuthorityMode::ProdShaped => ports.relay_http,
        ProofAuthorityMode::DebugAuthority => ports.control_http,
    };
    let mut env = BTreeMap::from([
        (
            "NEXUS_CONTROL_ADMIN_BEARER_TOKEN".to_string(),
            admin_bearer_token.to_string(),
        ),
        (
            "NEXUS_CONTROL_HOSTED_NEXUS_RELAY_URL".to_string(),
            relay_ws_url
                .unwrap_or_else(|| "ws://127.0.0.1:0/")
                .to_string(),
        ),
        (
            "NEXUS_CONTROL_KERNEL_STATE_PATH".to_string(),
            layout.kernel_state_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_LISTEN_ADDR".to_string(),
            format!("127.0.0.1:{authority_base_port}"),
        ),
        (
            "NEXUS_CONTROL_RECEIPT_LOG_PATH".to_string(),
            layout.receipt_log_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_TRAINING_GCS_BUCKET_URI".to_string(),
            PROOF_ARTIFACT_BUCKET.to_string(),
        ),
        (
            "NEXUS_CONTROL_TRAINING_GCS_ENDPOINT".to_string(),
            artifact_store_base_url.to_string(),
        ),
        (
            "NEXUS_CONTROL_TRAINING_GCS_SIGNING_CREDENTIALS_PATH".to_string(),
            layout.signer_credentials_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_TRAINING_TRN_IDENTITY_PATH".to_string(),
            layout.training_trn_identity_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_DAILY_BUDGET_CAP_SATS".to_string(),
            "10000".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_ENABLED".to_string(),
            "true".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_PAYOUT_INTERVAL_SECONDS".to_string(),
            "60".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_PAYOUT_SATS_PER_WINDOW".to_string(),
            "120".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_PLACEHOLDER_PAYOUT_MODE".to_string(),
            "presence_only".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_STATE_PATH".to_string(),
            layout.treasury_state_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_WALLET_MNEMONIC_PATH".to_string(),
            layout.treasury_wallet_mnemonic_path.display().to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_WALLET_NETWORK".to_string(),
            "regtest".to_string(),
        ),
        (
            "NEXUS_CONTROL_TREASURY_WALLET_STORAGE_DIR".to_string(),
            layout.treasury_wallet_dir.display().to_string(),
        ),
    ]);
    if mode == ProofAuthorityMode::ProdShaped {
        env.insert(
            "NEXUS_RELAY_DATA_DIR".to_string(),
            layout.relay_data_dir.display().to_string(),
        );
        env.insert(
            "NEXUS_RELAY_LISTEN_ADDR".to_string(),
            format!("127.0.0.1:{}", ports.relay_http),
        );
        env.insert(
            "NEXUS_RELAY_PUBLIC_WS_URL".to_string(),
            relay_ws_url.unwrap_or("ws://127.0.0.1:0/").to_string(),
        );
        env.insert(
            "NEXUS_RELAY_UPSTREAM_LISTEN_ADDR".to_string(),
            format!("127.0.0.1:{}", ports.relay_upstream),
        );
    }
    env
}

fn write_env_manifest(path: &Path, env: &BTreeMap<String, String>) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create env dir {}", parent.display()))?;
    }
    let mut lines = Vec::new();
    for (key, value) in env {
        lines.push(format!("{key}={value}"));
    }
    fs::write(path, format!("{}\n", lines.join("\n")))
        .with_context(|| format!("failed to write env manifest {}", path.display()))
}

fn load_runtime_state(path: &Path) -> Result<Option<ProofAuthorityRuntimeState>> {
    if !path.is_file() {
        return Ok(None);
    }
    let payload = fs::read_to_string(path)
        .with_context(|| format!("failed to read proof runtime state {}", path.display()))?;
    let state = serde_json::from_str(payload.as_str())
        .with_context(|| format!("failed to parse proof runtime state {}", path.display()))?;
    Ok(Some(state))
}

fn save_runtime_state(path: &Path, state: &ProofAuthorityRuntimeState) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create runtime state dir {}", parent.display()))?;
    }
    let payload = serde_json::to_vec_pretty(state)?;
    let temp_path = path.with_extension("json.partial");
    fs::write(temp_path.as_path(), payload)
        .with_context(|| format!("failed to write proof runtime state {}", temp_path.display()))?;
    fs::rename(temp_path.as_path(), path).with_context(|| {
        format!(
            "failed to finalize proof runtime state {}",
            path.display()
        )
    })
}

async fn collect_route_probes(state: &ProofAuthorityRuntimeState) -> Vec<ProofRouteProbe> {
    let authority_running = process_is_running(&state.authority_process);
    let artifact_running = process_is_running(&state.artifact_store_process);
    if !authority_running || !artifact_running {
        return vec![
            ProofRouteProbe {
                route_id: "healthz".to_string(),
                url: format!("{}/healthz", state.urls.authority_base_url),
                ok: false,
                status: None,
                detail: "authority process is not running".to_string(),
            },
            ProofRouteProbe {
                route_id: "artifact_store_healthz".to_string(),
                url: format!(
                    "http://127.0.0.1:{}/healthz",
                    state.ports.artifact_store
                ),
                ok: false,
                status: None,
                detail: "artifact store process is not running".to_string(),
            },
        ];
    }

    let client = reqwest::Client::new();
    vec![
        probe_route(
            &client,
            "healthz",
            format!("{}/healthz", state.urls.authority_base_url),
            reqwest::Method::GET,
            &[StatusCode::OK],
        )
        .await,
        probe_route(
            &client,
            "training_artifact_layout",
            format!(
                "{}/v1/kernel/compute/training/artifact-storage-layout",
                state.urls.authority_base_url
            ),
            reqwest::Method::GET,
            &[StatusCode::OK],
        )
        .await,
        probe_route(
            &client,
            "treasury_status",
            format!("{}/v1/treasury/status", state.urls.authority_base_url),
            reqwest::Method::GET,
            &[StatusCode::OK],
        )
        .await,
        probe_route(
            &client,
            "admin_demo_launch_route",
            format!(
                "{}/v1/admin/training/demo-runs/cs336-a1/launch",
                state.urls.authority_base_url
            ),
            reqwest::Method::GET,
            &[StatusCode::METHOD_NOT_ALLOWED, StatusCode::UNAUTHORIZED],
        )
        .await,
        probe_route(
            &client,
            "artifact_store_healthz",
            format!("http://127.0.0.1:{}/healthz", state.ports.artifact_store),
            reqwest::Method::GET,
            &[StatusCode::OK],
        )
        .await,
    ]
}

async fn probe_route(
    client: &reqwest::Client,
    route_id: &str,
    url: String,
    method: reqwest::Method,
    expected: &[StatusCode],
) -> ProofRouteProbe {
    let response = client.request(method, url.as_str()).send().await;
    match response {
        Ok(response) => {
            let status = response.status();
            let ok = expected.contains(&status);
            let detail = if ok {
                format!("route reachable with {}", status.as_u16())
            } else {
                format!("unexpected status {}", status.as_u16())
            };
            ProofRouteProbe {
                route_id: route_id.to_string(),
                url,
                ok,
                status: Some(status.as_u16()),
                detail,
            }
        }
        Err(error) => ProofRouteProbe {
            route_id: route_id.to_string(),
            url,
            ok: false,
            status: None,
            detail: error.to_string(),
        },
    }
}

async fn wait_for_route(url: &str, expected: &[StatusCode]) -> Result<()> {
    let client = reqwest::Client::new();
    let deadline = Instant::now() + PROOF_ROUTE_TIMEOUT;
    while Instant::now() < deadline {
        if let Ok(response) = client.get(url).send().await {
            if expected.contains(&response.status()) {
                return Ok(());
            }
        }
        tokio::time::sleep(PROOF_POLL_INTERVAL).await;
    }
    bail!("timed out waiting for route {}", url);
}

async fn run_artifact_smoke(
    state: &ProofAuthorityRuntimeState,
) -> Result<ProofArtifactSmokeReport> {
    let artifact_id = synthetic_artifact_id(state.namespace.as_str());
    let resolver_url = format!(
        "{}/v1/kernel/compute/training/artifacts/{}",
        state.urls.authority_base_url, artifact_id
    );
    let client = reqwest::Client::new();
    let resolver = client
        .get(resolver_url.as_str())
        .send()
        .await
        .with_context(|| format!("failed to fetch resolver {resolver_url}"))?
        .error_for_status()
        .with_context(|| format!("resolver probe failed for {artifact_id}"))?
        .json::<Value>()
        .await
        .context("failed to decode artifact resolver response")?;
    let relative_object_path = resolver
        .get("relative_object_path")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("artifact resolver response missing relative_object_path"))?
        .to_string();

    let payload = serde_json::to_vec_pretty(&json!({
        "schema_version": "openagents.proof.runtime.smoke.v1",
        "namespace": state.namespace,
        "mode": state.mode.label(),
        "artifact_id": artifact_id,
    }))?;
    let expected_digest = sha256_prefixed_bytes(payload.as_slice());
    let size_bytes = u64::try_from(payload.len()).unwrap_or(u64::MAX);

    let signed_access_url = format!(
        "{}/v1/kernel/compute/training/artifacts/{}/signed-access",
        state.urls.authority_base_url, artifact_id
    );
    let write_access = client
        .post(signed_access_url.as_str())
        .json(&json!({
            "mode": "write",
            "ttl_seconds": 300,
            "digest": expected_digest,
            "size_bytes": size_bytes,
        }))
        .send()
        .await
        .with_context(|| format!("failed to request write access for {artifact_id}"))?
        .error_for_status()
        .with_context(|| format!("write access failed for {artifact_id}"))?
        .json::<Value>()
        .await
        .context("failed to decode write access response")?;
    let write_url = write_access
        .get("signed_url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("write access response missing signed_url"))?;
    client
        .put(write_url)
        .body(payload.clone())
        .send()
        .await
        .context("failed to upload artifact smoke payload")?
        .error_for_status()
        .context("artifact smoke upload returned non-success")?;

    let read_access = client
        .post(signed_access_url.as_str())
        .json(&json!({
            "mode": "read",
            "ttl_seconds": 300,
        }))
        .send()
        .await
        .with_context(|| format!("failed to request read access for {artifact_id}"))?
        .error_for_status()
        .with_context(|| format!("read access failed for {artifact_id}"))?
        .json::<Value>()
        .await
        .context("failed to decode read access response")?;
    let read_url = read_access
        .get("signed_url")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow!("read access response missing signed_url"))?;
    let downloaded = client
        .get(read_url)
        .send()
        .await
        .context("failed to download artifact smoke payload")?
        .error_for_status()
        .context("artifact smoke read returned non-success")?
        .bytes()
        .await
        .context("failed to read artifact smoke payload bytes")?;
    if downloaded.as_ref() != payload.as_slice() {
        bail!("artifact smoke readback payload mismatched uploaded bytes");
    }

    let trace_entry_count = fs::read_to_string(state.paths.artifact_trace_path.as_str())
        .ok()
        .map(|contents| contents.lines().filter(|line| !line.trim().is_empty()).count())
        .unwrap_or(0);
    Ok(ProofArtifactSmokeReport {
        artifact_id,
        relative_object_path,
        expected_digest,
        payload_size_bytes: size_bytes,
        trace_entry_count,
        verified_at_ms: super::now_epoch_ms(),
    })
}

fn synthetic_artifact_id(namespace: &str) -> String {
    let slug = namespace_slug(namespace);
    format!(
        "oa.train_artifact.v1~kind~local_update~network~trainnet.{slug}~run~run.{slug}~window~window.{slug}.0001~assignment~assign.{slug}.worker.1.attempt1"
    )
}

fn namespace_slug(namespace: &str) -> String {
    namespace
        .chars()
        .map(|ch| match ch {
            'a'..='z' | '0'..='9' => ch,
            'A'..='Z' => ch.to_ascii_lowercase(),
            _ => '.',
        })
        .collect::<String>()
}

fn render_proof_status_report(report: &ProofAuthorityStatusReport) -> String {
    if !report.configured {
        return format!(
            "proof authority: configured=false namespace={} detail=run `oa proof authority up`",
            report.namespace
        );
    }

    let mut lines = Vec::new();
    lines.push(format!(
        "proof authority: configured=true namespace={} mode={} treasury_enabled={} admin_auth_configured={}",
        report.namespace,
        report.mode.map(ProofAuthorityMode::label).unwrap_or("unknown"),
        report.treasury_enabled,
        report.admin_auth_configured
    ));
    if let Some(urls) = report.urls.as_ref() {
        lines.push(format!(
            "authority_url: {}",
            urls.authority_base_url
        ));
        lines.push(format!(
            "artifact_store_url: {}",
            urls.artifact_store_base_url
        ));
        if let Some(relay_ws_url) = urls.relay_ws_url.as_deref() {
            lines.push(format!("relay_ws_url: {relay_ws_url}"));
        }
    }
    if let Some(paths) = report.paths.as_ref() {
        lines.push(format!("kernel_state_path: {}", paths.kernel_state_path));
        lines.push(format!("receipt_log_path: {}", paths.receipt_log_path));
        lines.push(format!("treasury_state_path: {}", paths.treasury_state_path));
        lines.push(format!("artifact_trace_path: {}", paths.artifact_trace_path));
    }
    if let Some(process) = report.authority_process.as_ref() {
        lines.push(format!(
            "authority_process: running={} pid={} log={}",
            process.running,
            process.pid.map(|pid| pid.to_string()).unwrap_or_else(|| "-".to_string()),
            process.log_path
        ));
    }
    if let Some(process) = report.artifact_store_process.as_ref() {
        lines.push(format!(
            "artifact_store_process: running={} pid={} log={}",
            process.running,
            process.pid.map(|pid| pid.to_string()).unwrap_or_else(|| "-".to_string()),
            process.log_path
        ));
    }
    for probe in &report.probes {
        lines.push(format!(
            "probe {}: ok={} status={} detail={}",
            probe.route_id,
            probe.ok,
            probe
                .status
                .map(|status| status.to_string())
                .unwrap_or_else(|| "-".to_string()),
            probe.detail
        ));
    }
    if let Some(smoke) = report.artifact_smoke.as_ref() {
        lines.push(format!(
            "artifact_smoke: artifact_id={} bytes={} digest={} trace_entries={}",
            smoke.artifact_id,
            render_byte_size(smoke.payload_size_bytes),
            smoke.expected_digest,
            smoke.trace_entry_count
        ));
    }
    lines.join("\n")
}

async fn run_artifact_store_server(
    listen_addr: SocketAddr,
    store_root: PathBuf,
    trace_path: PathBuf,
) -> Result<()> {
    fs::create_dir_all(store_root.as_path())
        .with_context(|| format!("failed to create artifact store {}", store_root.display()))?;
    if let Some(parent) = trace_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create trace dir {}", parent.display()))?;
    }
    let app = Router::new()
        .route("/healthz", get(artifact_store_healthz))
        .route("/upload/{bucket}/{*object_path}", put(artifact_store_put).get(artifact_store_get))
        .with_state(Arc::new(ArtifactStoreState {
            store_root,
            trace_path,
        }));
    let listener = TcpListener::bind(listen_addr)
        .await
        .with_context(|| format!("failed to bind artifact store {}", listen_addr))?;
    axum::serve(listener, app)
        .await
        .context("artifact store server exited unexpectedly")
}

async fn artifact_store_healthz() -> Json<Value> {
    Json(json!({
        "ok": true,
        "service": "proof_artifact_store",
    }))
}

async fn artifact_store_put(
    State(state): State<Arc<ArtifactStoreState>>,
    AxumPath((bucket, object_path)): AxumPath<(String, String)>,
    body: Bytes,
) -> Result<StatusCode, (StatusCode, String)> {
    let destination = state
        .store_root
        .join(bucket.as_str())
        .join(object_path.as_str());
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(internal_error)?;
    }
    fs::write(destination.as_path(), body.as_ref()).map_err(internal_error)?;
    append_trace(
        state.trace_path.as_path(),
        "write",
        bucket.as_str(),
        object_path.as_str(),
        Some(body.as_ref()),
    )
    .map_err(internal_error)?;
    Ok(StatusCode::OK)
}

async fn artifact_store_get(
    State(state): State<Arc<ArtifactStoreState>>,
    AxumPath((bucket, object_path)): AxumPath<(String, String)>,
) -> Result<(StatusCode, Vec<u8>), (StatusCode, String)> {
    let source = state
        .store_root
        .join(bucket.as_str())
        .join(object_path.as_str());
    match fs::read(source.as_path()) {
        Ok(payload) => {
            append_trace(
                state.trace_path.as_path(),
                "read",
                bucket.as_str(),
                object_path.as_str(),
                None,
            )
            .map_err(internal_error)?;
            Ok((StatusCode::OK, payload))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((StatusCode::NOT_FOUND, Vec::new()))
        }
        Err(error) => Err(internal_error(error)),
    }
}

fn append_trace(
    trace_path: &Path,
    operation: &str,
    bucket: &str,
    object_path: &str,
    payload: Option<&[u8]>,
) -> Result<()> {
    let payload_digest = payload.map(sha256_prefixed_bytes);
    let payload_size_bytes = payload.and_then(|bytes| u64::try_from(bytes.len()).ok());
    let line = serde_json::to_string(&json!({
        "recorded_at_ms": super::now_epoch_ms(),
        "operation": operation,
        "bucket": bucket,
        "object_path": object_path,
        "canonical_object_uri": format!("gs://{bucket}/{object_path}"),
        "payload_digest": payload_digest,
        "payload_size_bytes": payload_size_bytes,
    }))?;
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(trace_path)
        .with_context(|| format!("failed to open artifact trace {}", trace_path.display()))?;
    use std::io::Write as _;
    writeln!(file, "{line}").with_context(|| {
        format!(
            "failed to append artifact trace {}",
            trace_path.display()
        )
    })
}

fn sha256_prefixed_bytes(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn internal_error(error: impl std::fmt::Display) -> (StatusCode, String) {
    (StatusCode::INTERNAL_SERVER_ERROR, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        PROOF_ARTIFACT_UPLOAD_PREFIX, proof_namespace_ports, render_proof_status_report,
        run_artifact_store_server,
    };

    use anyhow::{Result, anyhow};
    use axum::http::StatusCode;
    use serde_json::Value;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn namespace_ports_are_stable() {
        let left = proof_namespace_ports("authority");
        let right = proof_namespace_ports("authority");
        assert_eq!(left, right);
        assert_ne!(left.relay_http, left.artifact_store);
    }

    #[tokio::test]
    async fn artifact_store_round_trips_and_traces_paths() -> Result<()> {
        let dir = tempdir()?;
        let store_root = dir.path().join("store");
        let trace_path = dir.path().join("trace.jsonl");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let local_addr = listener.local_addr()?;
        drop(listener);
        let server = tokio::spawn(run_artifact_store_server(
            local_addr,
            store_root.clone(),
            trace_path.clone(),
        ));
        super::wait_for_route(format!("http://{local_addr}/healthz").as_str(), &[StatusCode::OK])
            .await?;

        let client = reqwest::Client::new();
        let upload_url = format!(
            "http://{local_addr}{PROOF_ARTIFACT_UPLOAD_PREFIX}/bucket/networks/trainnet.alpha/runs/run.alpha/manifests/run_manifest.json"
        );
        let payload = br#"{"ok":true}"#.to_vec();
        client
            .put(upload_url.as_str())
            .body(payload.clone())
            .send()
            .await?
            .error_for_status()?;
        let downloaded = client
            .get(upload_url.as_str())
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        assert_eq!(downloaded.as_ref(), payload.as_slice());

        let trace: Value = serde_json::from_str(
            std::fs::read_to_string(trace_path.as_path())?
                .lines()
                .next()
                .ok_or_else(|| anyhow!("missing trace line"))?,
        )?;
        assert_eq!(
            trace["canonical_object_uri"],
            json!("gs://bucket/networks/trainnet.alpha/runs/run.alpha/manifests/run_manifest.json")
        );

        server.abort();
        Ok(())
    }

    #[test]
    fn status_renderer_surfaces_unconfigured_detail() {
        let rendered = render_proof_status_report(&super::ProofAuthorityStatusReport {
            configured: false,
            namespace: "authority".to_string(),
            mode: None,
            started_at_ms: None,
            admin_auth_configured: false,
            treasury_enabled: false,
            ports: None,
            paths: None,
            urls: None,
            authority_process: None,
            artifact_store_process: None,
            probes: Vec::new(),
            artifact_smoke: None,
        });
        assert!(rendered.contains("configured=false"));
    }
}
