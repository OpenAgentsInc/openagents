use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::Emitter;

const DEFAULT_COMMAND_TIMEOUT: Duration = Duration::from_secs(12);
const DEFAULT_PROOF_TIMEOUT_SECONDS: u64 = 180;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PylonBinaryStatus {
    pub installed: bool,
    pub binary_name: String,
    pub binary_path: Option<String>,
    pub source: String,
    pub detail: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PylonStatusProjection {
    pub installed: bool,
    pub configured: bool,
    pub process_state: String,
    pub provider_state: String,
    pub desired_mode: Option<String>,
    pub pid: Option<u32>,
    pub listen_addr: Option<String>,
    pub binary_path: Option<String>,
    pub config_path: Option<String>,
    pub pylon_home: Option<String>,
    pub execution_backend: Option<String>,
    pub ready_model: Option<String>,
    pub products_visible: Option<usize>,
    pub products_eligible: Option<usize>,
    pub queue_depth: Option<u64>,
    pub uptime_seconds: Option<u64>,
    pub blocker_codes: Vec<String>,
    pub last_action: Option<String>,
    pub last_error: Option<String>,
    pub last_exit_code: Option<i32>,
    pub last_updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PylonStartOptions {
    pub config_path: Option<String>,
    pub pylon_home: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProofRunOptions {
    pub lane: String,
    pub namespace: Option<String>,
    pub workers: Option<u32>,
    pub validators: Option<u32>,
    pub timeout_seconds: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProofNodeProjection {
    pub role: String,
    pub index: u64,
    pub label: String,
    pub running: bool,
    pub pid: Option<u32>,
    pub eligibility: Option<String>,
    pub hard_gate_reasons: Vec<String>,
    pub retained_state_fixture_id: Option<String>,
    pub training_status: Option<String>,
    pub training_error: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProofTransportProjection {
    pub authority: String,
    pub relay: String,
    pub artifact_store: String,
    pub node_surfaces: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProofFailedAuthorityWrite {
    pub source: String,
    pub method: Option<String>,
    pub url: Option<String>,
    pub status: Option<u16>,
    pub response_body: Option<String>,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProofArtifactsProjection {
    pub root: String,
    pub run_report_path: Option<String>,
    pub authority_trace_path: Option<String>,
    pub summary_path: Option<String>,
    pub artifact_trace_path: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProofRunProjection {
    pub namespace: String,
    pub lane: String,
    pub status: String,
    pub first_red_stage: Option<String>,
    pub first_red_subject: Option<String>,
    pub blocker_id: Option<String>,
    pub detail: Option<String>,
    pub run_id: Option<String>,
    pub window_id: Option<String>,
    pub assignment_id: Option<String>,
    pub lease_id: Option<String>,
    pub membership_revision: Option<String>,
    pub closeout_stage: Option<String>,
    pub closeout_next_action: Option<String>,
    pub closeout_last_error: Option<String>,
    pub workers: Vec<ProofNodeProjection>,
    pub validators: Vec<ProofNodeProjection>,
    pub transport: ProofTransportProjection,
    pub artifacts: ProofArtifactsProjection,
    pub first_failed_authority_write: Option<ProofFailedAuthorityWrite>,
    pub local_simulation: bool,
    pub simulated_treasury: bool,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ProofRuntimeProjection {
    pub namespace: String,
    pub status: String,
    pub detail: String,
    pub artifacts: ProofArtifactsProjection,
    pub updated_at: String,
}

#[derive(Default)]
pub struct PylonManager {
    child: Mutex<Option<ManagedPylonChild>>,
    last_proof: Arc<Mutex<Option<ProofRunProjection>>>,
}

impl PylonManager {
    pub fn store_proof(&self, proof: ProofRunProjection) {
        if let Ok(mut guard) = self.last_proof.lock() {
            *guard = Some(proof);
        }
    }

    pub fn proof_snapshot(&self) -> Option<ProofRunProjection> {
        self.last_proof.lock().ok().and_then(|guard| guard.clone())
    }

    fn proof_store(&self) -> Arc<Mutex<Option<ProofRunProjection>>> {
        Arc::clone(&self.last_proof)
    }
}

struct ManagedPylonChild {
    child: Child,
    binary_path: PathBuf,
    config_path: PathBuf,
    pylon_home: PathBuf,
    log_path: PathBuf,
    last_exit_code: Option<i32>,
}

#[tauri::command]
pub fn pylon_detect() -> PylonBinaryStatus {
    resolve_binary(BinaryKind::Pylon)
}

#[tauri::command]
pub fn pylon_get_status(state: tauri::State<'_, PylonManager>) -> PylonStatusProjection {
    pylon_status_projection(Some(&state), None, None)
}

#[tauri::command]
pub fn pylon_start(
    app: tauri::AppHandle,
    state: tauri::State<'_, PylonManager>,
    options: Option<PylonStartOptions>,
) -> Result<PylonStatusProjection, String> {
    let binary = resolve_binary_path(BinaryKind::Pylon)?;
    let pylon_home = options
        .as_ref()
        .and_then(|value| value.pylon_home.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(default_pylon_home);
    let config_path = options
        .as_ref()
        .and_then(|value| value.config_path.as_deref())
        .map(PathBuf::from)
        .unwrap_or_else(|| default_pylon_config_path_for_home(&pylon_home));
    let log_dir = default_log_dir();
    fs::create_dir_all(&log_dir).map_err(|error| format!("failed to create log dir: {error}"))?;
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "failed to create pylon config directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let log_path = log_dir.join(format!("pylon-serve-{}.log", timestamp_for_filename()));
    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
        .map_err(|error| format!("failed to open pylon log {}: {error}", log_path.display()))?;
    let stderr = stdout
        .try_clone()
        .map_err(|error| format!("failed to clone pylon log handle: {error}"))?;

    let mut guard = state
        .child
        .lock()
        .map_err(|_| "pylon process state lock poisoned".to_string())?;
    if let Some(managed) = guard.as_mut() {
        if managed_is_running(managed) {
            return Ok(pylon_status_projection_locked(
                Some(managed),
                Some("running"),
                None,
            ));
        }
    }

    let mut command = Command::new(&binary);
    command
        .arg("--config-path")
        .arg(&config_path)
        .arg("serve")
        .env("OPENAGENTS_PYLON_HOME", &pylon_home)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .stdin(Stdio::null());
    let child = command.spawn().map_err(|error| {
        format!(
            "failed to start pylon serve: {}",
            redact_sensitive(&error.to_string())
        )
    })?;
    let managed = ManagedPylonChild {
        child,
        binary_path: binary,
        config_path,
        pylon_home,
        log_path: log_path.clone(),
        last_exit_code: None,
    };
    *guard = Some(managed);
    let status = pylon_status_projection_locked(guard.as_mut(), Some("starting"), None);
    emit_status(&app, "pylon://status", &status);
    Ok(status)
}

#[tauri::command]
pub fn pylon_stop(
    app: tauri::AppHandle,
    state: tauri::State<'_, PylonManager>,
) -> Result<PylonStatusProjection, String> {
    let mut guard = state
        .child
        .lock()
        .map_err(|_| "pylon process state lock poisoned".to_string())?;
    if let Some(mut managed) = guard.take() {
        if managed_is_running(&mut managed) {
            managed
                .child
                .kill()
                .map_err(|error| format!("failed to stop pylon: {error}"))?;
            let _ = managed.child.wait();
        }
        let status = pylon_status_projection_locked(None, Some("stopped"), managed.last_exit_code);
        emit_status(&app, "pylon://status", &status);
        return Ok(status);
    }
    let status = pylon_status_projection_locked(None, Some("stopped"), None);
    emit_status(&app, "pylon://status", &status);
    Ok(status)
}

#[tauri::command]
pub fn pylon_restart(
    app: tauri::AppHandle,
    state: tauri::State<'_, PylonManager>,
    options: Option<PylonStartOptions>,
) -> Result<PylonStatusProjection, String> {
    let _ = pylon_stop(app.clone(), state.clone())?;
    pylon_start(app, state, options)
}

#[tauri::command]
pub fn pylon_set_mode(
    mode: String,
    state: tauri::State<'_, PylonManager>,
) -> Result<PylonStatusProjection, String> {
    let action = normalize_provider_mode(&mode)?;
    let binary = resolve_binary_path(BinaryKind::Pylon)?;
    let config_path = current_config_path(Some(&state));
    let args = [
        OsString::from("--config-path"),
        config_path.as_os_str().to_os_string(),
        OsString::from(action),
    ];
    let mut last_failure = None;
    for attempt in 0..10 {
        let output = run_command_with_timeout(&binary, &args, DEFAULT_COMMAND_TIMEOUT)?;
        if output.status.success() {
            return Ok(pylon_status_projection(Some(&state), Some("running"), None));
        }
        let failure = command_failure("pylon mode command", &output);
        if !failure.contains("database is locked") || attempt == 9 {
            return Err(failure);
        }
        last_failure = Some(failure);
        thread::sleep(Duration::from_millis(500));
    }
    Err(last_failure.unwrap_or_else(|| "pylon mode command failed".to_string()))
}

#[tauri::command]
pub fn pylon_open_logs() -> Result<String, String> {
    let path = default_log_dir();
    fs::create_dir_all(&path).map_err(|error| format!("failed to create log dir: {error}"))?;
    tauri_plugin_opener::open_path(&path, None::<&str>)
        .map_err(|error| format!("failed to open log dir {}: {error}", path.display()))?;
    Ok(path.display().to_string())
}

#[tauri::command]
pub fn proof_run(
    app: tauri::AppHandle,
    state: tauri::State<'_, PylonManager>,
    options: ProofRunOptions,
) -> Result<ProofRunProjection, String> {
    let lane = normalize_proof_lane(&options.lane)?;
    let namespace = options
        .namespace
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            format!(
                "proof.autopilot.{}.{}",
                lane.replace('-', "."),
                timestamp_for_filename()
            )
        });
    let workers = options.workers.unwrap_or_else(|| {
        if lane == "cs336-a1-replacement-attempt" {
            0
        } else {
            1
        }
    });
    let validators = options.validators.unwrap_or_else(|| {
        if lane == "cs336-a1-replacement-attempt" {
            0
        } else {
            1
        }
    });
    let timeout_seconds = options
        .timeout_seconds
        .unwrap_or(DEFAULT_PROOF_TIMEOUT_SECONDS);
    let binary = resolve_binary_path(BinaryKind::Oa)?;
    let args = vec![
        OsString::from("proof"),
        OsString::from("run"),
        OsString::from(&lane),
        OsString::from("--namespace"),
        OsString::from(&namespace),
        OsString::from("--workers"),
        OsString::from(workers.to_string()),
        OsString::from("--validators"),
        OsString::from(validators.to_string()),
        OsString::from("--timeout-seconds"),
        OsString::from(timeout_seconds.to_string()),
        OsString::from("--json"),
    ];
    let mut status = proof_get_projection(&namespace, &lane, Some("running"));
    status.detail = Some(format!(
        "started oa proof run {lane} in background namespace {namespace}"
    ));
    state.store_proof(status.clone());
    emit_status(&app, "proof://status", &status);
    let app_for_thread = app.clone();
    let background_namespace = namespace.clone();
    let background_lane = lane.clone();
    let proof_store = state.proof_store();

    thread::spawn(move || {
        let output = run_command_with_timeout(
            &binary,
            &args,
            Duration::from_secs(timeout_seconds.saturating_add(30)),
        );
        match output {
            Ok(output) if output.status.success() => {
                let projection =
                    proof_get_projection(&background_namespace, &background_lane, None);
                if let Ok(mut guard) = proof_store.lock() {
                    *guard = Some(projection.clone());
                }
                emit_status(&app_for_thread, "proof://summary", &projection);
            }
            Ok(output) => {
                let mut projection =
                    proof_get_projection(&background_namespace, &background_lane, Some("failed"));
                projection.detail = Some(command_failure("oa proof run", &output));
                if let Ok(mut guard) = proof_store.lock() {
                    *guard = Some(projection.clone());
                }
                emit_status(&app_for_thread, "proof://error", &projection);
            }
            Err(error) => {
                let mut projection =
                    proof_get_projection(&background_namespace, &background_lane, Some("failed"));
                projection.detail = Some(redact_sensitive(&error));
                if let Ok(mut guard) = proof_store.lock() {
                    *guard = Some(projection.clone());
                }
                emit_status(&app_for_thread, "proof://error", &projection);
            }
        }
    });

    Ok(status)
}

#[tauri::command]
pub fn proof_get(state: tauri::State<'_, PylonManager>, namespace: String) -> ProofRunProjection {
    let projection = proof_get_projection(&namespace, "cs336-a1", None);
    if projection.status == "idle" {
        if let Some(snapshot) = state
            .proof_snapshot()
            .filter(|snapshot| snapshot.namespace == namespace)
        {
            return snapshot;
        }
    }
    state.store_proof(projection.clone());
    projection
}

#[tauri::command]
pub fn proof_doctor(
    state: tauri::State<'_, PylonManager>,
    namespace: String,
) -> Result<ProofRunProjection, String> {
    let binary = resolve_binary_path(BinaryKind::Oa)?;
    let args = vec![
        OsString::from("proof"),
        OsString::from("doctor"),
        OsString::from("--namespace"),
        OsString::from(&namespace),
        OsString::from("--json"),
    ];
    let output = run_command_with_timeout(&binary, &args, DEFAULT_COMMAND_TIMEOUT)?;
    if !output.status.success() {
        return Err(command_failure("oa proof doctor", &output));
    }
    let mut projection = proof_get_projection(&namespace, "cs336-a1", None);
    if let Ok(value) = serde_json::from_slice::<Value>(&output.stdout) {
        projection.transport = project_transport(value.get("transport"));
        projection.detail = value
            .get("configured")
            .and_then(Value::as_bool)
            .map(|configured| format!("proof doctor configured={configured}"));
    }
    state.store_proof(projection.clone());
    Ok(projection)
}

#[tauri::command]
pub fn proof_stop(namespace: String) -> Result<ProofRuntimeProjection, String> {
    let binary = resolve_binary_path(BinaryKind::Oa)?;
    let fleet_args = vec![
        OsString::from("proof"),
        OsString::from("fleet"),
        OsString::from("down"),
        OsString::from("--namespace"),
        OsString::from(&namespace),
        OsString::from("--json"),
    ];
    let authority_args = vec![
        OsString::from("proof"),
        OsString::from("authority"),
        OsString::from("down"),
        OsString::from("--namespace"),
        OsString::from(&namespace),
        OsString::from("--json"),
    ];

    let mut details = Vec::new();
    let mut errors = Vec::new();

    match run_command_with_timeout(&binary, &fleet_args, DEFAULT_COMMAND_TIMEOUT) {
        Ok(output) if output.status.success() => {
            details.push("fleet down completed".to_string());
        }
        Ok(output) => {
            errors.push(command_failure("oa proof fleet down", &output));
        }
        Err(error) => {
            errors.push(redact_sensitive(&error));
        }
    }

    let authority_stopped =
        match run_command_with_timeout(&binary, &authority_args, DEFAULT_COMMAND_TIMEOUT) {
            Ok(output) if output.status.success() => {
                details.push("authority down completed".to_string());
                true
            }
            Ok(output) if proof_authority_output_stopped(&output) => {
                details.push("authority down reported stopped processes".to_string());
                true
            }
            Ok(output) => {
                errors.push(command_failure("oa proof authority down", &output));
                false
            }
            Err(error) => {
                errors.push(redact_sensitive(&error));
                false
            }
        };

    if !authority_stopped && !errors.is_empty() {
        return Err(errors.join(" | "));
    }

    Ok(ProofRuntimeProjection {
        namespace: namespace.clone(),
        status: "stopped".to_string(),
        detail: if errors.is_empty() {
            details.join("; ")
        } else {
            format!("{}; warnings: {}", details.join("; "), errors.join(" | "))
        },
        artifacts: proof_artifacts(&namespace),
        updated_at: now_rfc3339ish(),
    })
}

#[tauri::command]
pub fn proof_reset(namespace: String) -> Result<ProofRuntimeProjection, String> {
    let binary = resolve_binary_path(BinaryKind::Oa)?;
    let args = vec![
        OsString::from("proof"),
        OsString::from("fleet"),
        OsString::from("reset"),
        OsString::from("--namespace"),
        OsString::from(&namespace),
        OsString::from("--json"),
    ];
    let output = run_command_with_timeout(&binary, &args, DEFAULT_COMMAND_TIMEOUT)?;
    if !output.status.success() {
        return Err(command_failure("oa proof fleet reset", &output));
    }
    Ok(ProofRuntimeProjection {
        namespace: namespace.clone(),
        status: "reset".to_string(),
        detail: "proof namespace reset requested".to_string(),
        artifacts: proof_artifacts(&namespace),
        updated_at: now_rfc3339ish(),
    })
}

#[tauri::command]
pub fn proof_open_artifacts(namespace: String) -> Result<String, String> {
    let root = proof_namespace_root(&namespace);
    tauri_plugin_opener::open_path(&root, None::<&str>)
        .map_err(|error| format!("failed to open proof artifacts {}: {error}", root.display()))?;
    Ok(root.display().to_string())
}

fn pylon_status_projection(
    state: Option<&tauri::State<'_, PylonManager>>,
    forced_process_state: Option<&str>,
    last_exit_code: Option<i32>,
) -> PylonStatusProjection {
    let managed_snapshot = state.and_then(|manager| {
        manager
            .child
            .lock()
            .ok()
            .and_then(|mut guard| guard.as_mut().map(snapshot_managed_child))
    });
    let binary_status = resolve_binary(BinaryKind::Pylon);
    let config_path = managed_snapshot
        .as_ref()
        .map(|snapshot| snapshot.config_path.clone())
        .unwrap_or_else(default_pylon_config_path);
    let pylon_home = managed_snapshot
        .as_ref()
        .map(|snapshot| snapshot.pylon_home.clone())
        .unwrap_or_else(default_pylon_home);
    let process_state = forced_process_state
        .map(ToOwned::to_owned)
        .or_else(|| {
            managed_snapshot
                .as_ref()
                .map(|snapshot| snapshot.process_state.clone())
        })
        .unwrap_or_else(|| "stopped".to_string());

    if !binary_status.installed {
        return base_status_projection(
            false,
            config_path.exists(),
            "notInstalled",
            "unknown",
            binary_status.binary_path,
            Some(config_path),
            Some(pylon_home),
            last_exit_code,
            binary_status.detail,
        );
    }

    let Some(binary_path) = binary_status.binary_path.clone().map(PathBuf::from) else {
        return base_status_projection(
            false,
            config_path.exists(),
            "notInstalled",
            "unknown",
            None,
            Some(config_path),
            Some(pylon_home),
            last_exit_code,
            Some("pylon binary path missing".to_string()),
        );
    };

    let args = vec![
        OsString::from("--config-path"),
        config_path.as_os_str().to_os_string(),
        OsString::from("status"),
        OsString::from("--json"),
    ];
    match run_command_with_timeout(&binary_path, &args, DEFAULT_COMMAND_TIMEOUT) {
        Ok(output) if output.status.success() => {
            match serde_json::from_slice::<Value>(&output.stdout) {
                Ok(value) => project_pylon_status(
                    value,
                    process_state,
                    binary_status.binary_path,
                    config_path,
                    pylon_home,
                    managed_snapshot.as_ref().and_then(|snapshot| snapshot.pid),
                    last_exit_code.or_else(|| {
                        managed_snapshot
                            .as_ref()
                            .and_then(|snapshot| snapshot.last_exit_code)
                    }),
                    None,
                ),
                Err(error) => base_status_projection(
                    true,
                    config_path.exists(),
                    process_state.as_str(),
                    "error",
                    binary_status.binary_path,
                    Some(config_path),
                    Some(pylon_home),
                    last_exit_code,
                    Some(format!("failed to decode pylon status JSON: {error}")),
                ),
            }
        }
        Ok(output) => base_status_projection(
            true,
            config_path.exists(),
            process_state.as_str(),
            "error",
            binary_status.binary_path,
            Some(config_path),
            Some(pylon_home),
            last_exit_code,
            Some(command_failure("pylon status", &output)),
        ),
        Err(error) => base_status_projection(
            true,
            config_path.exists(),
            process_state.as_str(),
            "error",
            binary_status.binary_path,
            Some(config_path),
            Some(pylon_home),
            last_exit_code,
            Some(redact_sensitive(&error)),
        ),
    }
}

fn pylon_status_projection_locked(
    managed: Option<&mut ManagedPylonChild>,
    forced_process_state: Option<&str>,
    last_exit_code: Option<i32>,
) -> PylonStatusProjection {
    let snapshot = managed.map(snapshot_managed_child);
    let process_state = forced_process_state
        .map(ToOwned::to_owned)
        .or_else(|| snapshot.as_ref().map(|value| value.process_state.clone()))
        .unwrap_or_else(|| "stopped".to_string());
    let binary_path = snapshot
        .as_ref()
        .map(|value| value.binary_path.display().to_string())
        .or_else(|| resolve_binary(BinaryKind::Pylon).binary_path);
    let config_path = snapshot
        .as_ref()
        .map(|value| value.config_path.clone())
        .unwrap_or_else(default_pylon_config_path);
    let pylon_home = snapshot
        .as_ref()
        .map(|value| value.pylon_home.clone())
        .unwrap_or_else(default_pylon_home);
    base_status_projection(
        binary_path.is_some(),
        config_path.exists(),
        process_state.as_str(),
        "unknown",
        binary_path,
        Some(config_path),
        Some(pylon_home),
        last_exit_code.or_else(|| snapshot.and_then(|value| value.last_exit_code)),
        None,
    )
}

#[derive(Clone)]
struct ManagedSnapshot {
    binary_path: PathBuf,
    config_path: PathBuf,
    pylon_home: PathBuf,
    process_state: String,
    pid: Option<u32>,
    last_exit_code: Option<i32>,
}

fn snapshot_managed_child(managed: &mut ManagedPylonChild) -> ManagedSnapshot {
    let mut process_state = "running".to_string();
    let mut pid = Some(managed.child.id());
    match managed.child.try_wait() {
        Ok(Some(status)) => {
            process_state = "exited".to_string();
            pid = None;
            managed.last_exit_code = status.code();
        }
        Ok(None) => {}
        Err(error) => {
            process_state = "error".to_string();
            append_log(
                &managed.log_path,
                format!("failed to poll child: {error}\n").as_bytes(),
            );
        }
    }
    ManagedSnapshot {
        binary_path: managed.binary_path.clone(),
        config_path: managed.config_path.clone(),
        pylon_home: managed.pylon_home.clone(),
        process_state,
        pid,
        last_exit_code: managed.last_exit_code,
    }
}

fn managed_is_running(managed: &mut ManagedPylonChild) -> bool {
    matches!(managed.child.try_wait(), Ok(None))
}

fn project_pylon_status(
    value: Value,
    process_state: String,
    binary_path: Option<String>,
    config_path: PathBuf,
    pylon_home: PathBuf,
    pid: Option<u32>,
    last_exit_code: Option<i32>,
    fallback_error: Option<String>,
) -> PylonStatusProjection {
    let snapshot = value.get("snapshot");
    let runtime = snapshot.and_then(|snapshot| snapshot.get("runtime"));
    let availability = snapshot.and_then(|snapshot| snapshot.get("availability"));
    let local_gemma = availability.and_then(|value| value.get("local_gemma"));
    let inventory_rows = snapshot
        .and_then(|snapshot| snapshot.get("inventory_rows"))
        .and_then(Value::as_array);
    let provider_state = runtime
        .and_then(|runtime| runtime.get("authoritative_status"))
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let blocker_codes = runtime
        .and_then(|runtime| runtime.get("provider_blocker_codes"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let products_visible = inventory_rows.map(Vec::len);
    let products_eligible = inventory_rows.map(|rows| {
        rows.iter()
            .filter(|row| {
                row.get("eligible")
                    .and_then(Value::as_bool)
                    .unwrap_or(false)
            })
            .count()
    });
    PylonStatusProjection {
        installed: binary_path.is_some(),
        configured: config_path.exists() && provider_state != "unconfigured",
        process_state,
        provider_state,
        desired_mode: value
            .get("desired_mode")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        pid,
        listen_addr: value
            .get("listen_addr")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        binary_path,
        config_path: Some(config_path.display().to_string()),
        pylon_home: Some(pylon_home.display().to_string()),
        execution_backend: runtime
            .and_then(|runtime| runtime.get("execution_backend_label"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        ready_model: local_gemma
            .and_then(|value| value.get("ready_model"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        products_visible,
        products_eligible,
        queue_depth: runtime
            .and_then(|runtime| runtime.get("queue_depth"))
            .and_then(Value::as_u64),
        uptime_seconds: runtime
            .and_then(|runtime| runtime.get("online_uptime_seconds"))
            .and_then(Value::as_u64),
        blocker_codes,
        last_action: runtime
            .and_then(|runtime| runtime.get("last_action"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        last_error: runtime
            .and_then(|runtime| runtime.get("last_error"))
            .and_then(Value::as_str)
            .map(redact_sensitive)
            .or_else(|| fallback_error.map(|value| redact_sensitive(&value))),
        last_exit_code,
        last_updated_at: now_rfc3339ish(),
    }
}

fn base_status_projection(
    installed: bool,
    configured: bool,
    process_state: &str,
    provider_state: &str,
    binary_path: Option<String>,
    config_path: Option<PathBuf>,
    pylon_home: Option<PathBuf>,
    last_exit_code: Option<i32>,
    last_error: Option<String>,
) -> PylonStatusProjection {
    PylonStatusProjection {
        installed,
        configured,
        process_state: process_state.to_string(),
        provider_state: provider_state.to_string(),
        desired_mode: None,
        pid: None,
        listen_addr: None,
        binary_path,
        config_path: config_path.map(|path| path.display().to_string()),
        pylon_home: pylon_home.map(|path| path.display().to_string()),
        execution_backend: None,
        ready_model: None,
        products_visible: None,
        products_eligible: None,
        queue_depth: None,
        uptime_seconds: None,
        blocker_codes: Vec::new(),
        last_action: None,
        last_error: last_error.map(|value| redact_sensitive(&value)),
        last_exit_code,
        last_updated_at: now_rfc3339ish(),
    }
}

#[derive(Clone, Copy)]
enum BinaryKind {
    Pylon,
    Oa,
}

impl BinaryKind {
    const fn env_var(self) -> &'static str {
        match self {
            Self::Pylon => "OPENAGENTS_PYLON_BINARY",
            Self::Oa => "OPENAGENTS_OA_BINARY",
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Pylon => "pylon",
            Self::Oa => "oa",
        }
    }
}

fn resolve_binary(kind: BinaryKind) -> PylonBinaryStatus {
    match resolve_binary_candidate(kind) {
        Some((path, source)) => PylonBinaryStatus {
            installed: true,
            binary_name: kind.name().to_string(),
            binary_path: Some(path.display().to_string()),
            source,
            detail: None,
        },
        None => PylonBinaryStatus {
            installed: false,
            binary_name: kind.name().to_string(),
            binary_path: None,
            source: "missing".to_string(),
            detail: Some(format!(
                "{} not found. Set {} or install an approved binary.",
                kind.name(),
                kind.env_var()
            )),
        },
    }
}

fn resolve_binary_path(kind: BinaryKind) -> Result<PathBuf, String> {
    resolve_binary_candidate(kind)
        .map(|candidate| candidate.0)
        .ok_or_else(|| format!("{} binary not found; set {}", kind.name(), kind.env_var()))
}

fn resolve_binary_candidate(kind: BinaryKind) -> Option<(PathBuf, String)> {
    if let Ok(value) = env::var(kind.env_var()) {
        let path = PathBuf::from(value.trim());
        if is_executable_file(&path) {
            return Some((path, format!("env:{}", kind.env_var())));
        }
    }
    for path in bundled_binary_candidates(kind) {
        if is_executable_file(&path) {
            return Some((path, "bundle".to_string()));
        }
    }
    for path in workspace_binary_candidates(kind) {
        if is_executable_file(&path) {
            return Some((path, "workspace".to_string()));
        }
    }
    for path in app_cache_candidates(kind) {
        if is_executable_file(&path) {
            return Some((path, "app-cache".to_string()));
        }
    }
    for path in bootstrap_cache_candidates(kind) {
        if is_executable_file(&path) {
            return Some((path, "bootstrap-cache".to_string()));
        }
    }
    if let Some(path) = find_on_path(kind.name()) {
        return Some((path, "path".to_string()));
    }
    None
}

fn bundled_binary_candidates(kind: BinaryKind) -> Vec<PathBuf> {
    let Some(exe_path) = env::current_exe().ok() else {
        return Vec::new();
    };
    let Some(exe_dir) = exe_path.parent() else {
        return Vec::new();
    };
    let mut candidates = Vec::new();
    for name in executable_names(kind.name()) {
        candidates.push(exe_dir.join(&name));
        candidates.push(exe_dir.join("binaries").join(&name));
        candidates.push(exe_dir.join("../Resources").join(&name));
        candidates.push(exe_dir.join("../Resources/binaries").join(&name));
    }
    candidates
}

fn workspace_binary_candidates(kind: BinaryKind) -> Vec<PathBuf> {
    let Some(root) = workspace_root() else {
        return Vec::new();
    };
    let names = executable_names(kind.name());
    let mut candidates = Vec::new();
    for profile in ["debug", "fast-release", "release"] {
        for name in &names {
            candidates.push(root.join("target").join(profile).join(name));
        }
    }
    candidates
}

fn app_cache_candidates(kind: BinaryKind) -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    let root = home.join(".openagents").join("autopilot").join("bin");
    let mut candidates = Vec::new();
    for name in executable_names(kind.name()) {
        candidates.push(root.join("current").join(&name));
        candidates.push(root.join(&name));
    }
    candidates
}

fn bootstrap_cache_candidates(kind: BinaryKind) -> Vec<PathBuf> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };
    let root = home.join(".openagents").join("pylon").join("bootstrap");
    let mut candidates = Vec::new();
    for name in executable_names(kind.name()) {
        candidates.push(root.join("current").join(&name));
        candidates.push(root.join("bin").join(&name));
    }
    candidates
}

fn executable_names(name: &str) -> Vec<String> {
    #[cfg(windows)]
    {
        vec![format!("{name}.exe"), name.to_string()]
    }
    #[cfg(not(windows))]
    {
        vec![name.to_string()]
    }
}

fn workspace_root() -> Option<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    for ancestor in manifest_dir.ancestors() {
        if ancestor.join("apps").join("pylon").exists() && ancestor.join("Cargo.toml").exists() {
            return Some(ancestor.to_path_buf());
        }
    }
    env::current_dir().ok().and_then(|dir| {
        dir.ancestors()
            .find(|ancestor| {
                ancestor.join("apps").join("pylon").exists() && ancestor.join("Cargo.toml").exists()
            })
            .map(Path::to_path_buf)
    })
}

fn find_on_path(name: &str) -> Option<PathBuf> {
    let path_var = env::var_os("PATH")?;
    for dir in env::split_paths(&path_var) {
        for exe in executable_names(name) {
            let candidate = dir.join(exe);
            if is_executable_file(&candidate) {
                return Some(candidate);
            }
        }
    }
    None
}

fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

fn current_config_path(state: Option<&tauri::State<'_, PylonManager>>) -> PathBuf {
    if let Some(manager) = state {
        if let Ok(mut guard) = manager.child.lock() {
            if let Some(managed) = guard.as_mut() {
                return managed.config_path.clone();
            }
        }
    }
    default_pylon_config_path()
}

fn default_pylon_config_path() -> PathBuf {
    if let Ok(path) = env::var("OPENAGENTS_PYLON_CONFIG_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(path) = env::var("OPENAGENTS_AUTOPILOT_PYLON_CONFIG_PATH") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    default_pylon_config_path_for_home(&default_pylon_home())
}

fn default_pylon_config_path_for_home(home: &Path) -> PathBuf {
    home.join("config.json")
}

fn default_pylon_home() -> PathBuf {
    if let Ok(path) = env::var("OPENAGENTS_AUTOPILOT_PYLON_HOME") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    if let Ok(path) = env::var("OPENAGENTS_PYLON_HOME") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot")
        .join("pylon")
}

fn default_log_dir() -> PathBuf {
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("autopilot")
        .join("logs")
}

fn proof_namespace_root(namespace: &str) -> PathBuf {
    if let Ok(root) = env::var("OPENAGENTS_AUTOPILOT_PROOF_ROOT") {
        let trimmed = root.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed).join(namespace);
        }
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".openagents")
        .join("pylon")
        .join("proof")
        .join("namespaces")
        .join(namespace)
}

fn home_dir() -> Option<PathBuf> {
    env::var_os("HOME").map(PathBuf::from)
}

fn run_command_with_timeout(
    binary: &Path,
    args: &[OsString],
    timeout: Duration,
) -> Result<Output, String> {
    let mut child = Command::new(binary)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("failed to spawn {}: {error}", binary.display()))?;
    let start = SystemTime::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child
                    .wait_with_output()
                    .map_err(|error| format!("failed to collect command output: {error}"));
            }
            Ok(None) => {
                let elapsed = start.elapsed().unwrap_or(Duration::from_secs(0));
                if elapsed > timeout {
                    let _ = child.kill();
                    let output = child.wait_with_output().map_err(|error| {
                        format!("failed to collect timed-out command output: {error}")
                    })?;
                    return Err(format!(
                        "command timed out after {}s: {}",
                        timeout.as_secs(),
                        command_output_excerpt(&output)
                    ));
                }
                thread::sleep(Duration::from_millis(100));
            }
            Err(error) => return Err(format!("failed to poll command: {error}")),
        }
    }
}

fn command_failure(label: &str, output: &Output) -> String {
    let status = status_label(output.status);
    format!(
        "{label} failed with {status}: {}",
        command_output_excerpt(output)
    )
}

fn status_label(status: ExitStatus) -> String {
    status
        .code()
        .map(|code| format!("exit {code}"))
        .unwrap_or_else(|| "terminated by signal".to_string())
}

fn command_output_excerpt(output: &Output) -> String {
    let mut text = String::new();
    text.push_str(String::from_utf8_lossy(&output.stderr).as_ref());
    if text.trim().is_empty() {
        text.push_str(String::from_utf8_lossy(&output.stdout).as_ref());
    }
    let redacted = redact_sensitive(text.trim());
    redacted.chars().take(800).collect()
}

fn proof_authority_output_stopped(output: &Output) -> bool {
    let Ok(value) = serde_json::from_slice::<Value>(&output.stdout) else {
        return false;
    };
    let authority_running = value
        .get("authority_process")
        .and_then(|process| process.get("running"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let artifact_store_running = value
        .get("artifact_store_process")
        .and_then(|process| process.get("running"))
        .and_then(Value::as_bool)
        .unwrap_or(true);
    !authority_running && !artifact_store_running
}

fn normalize_provider_mode(mode: &str) -> Result<&'static str, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "online" => Ok("online"),
        "offline" => Ok("offline"),
        "pause" | "paused" => Ok("pause"),
        "resume" => Ok("resume"),
        other => Err(format!("unsupported pylon provider mode: {other}")),
    }
}

fn normalize_proof_lane(lane: &str) -> Result<String, String> {
    match lane.trim() {
        "cs336-a1" | "cs336_a1" | "cs336/a1" => Ok("cs336-a1".to_string()),
        "cs336-a1-stale-recovery" | "cs336_a1_stale_recovery" | "cs336/a1/stale-recovery" => {
            Ok("cs336-a1-stale-recovery".to_string())
        }
        "cs336-a1-replacement-attempt"
        | "cs336_a1_replacement_attempt"
        | "cs336/a1/replacement-attempt" => Ok("cs336-a1-replacement-attempt".to_string()),
        other => Err(format!("unsupported proof lane: {other}")),
    }
}

fn proof_get_projection(
    namespace: &str,
    fallback_lane: &str,
    forced_status: Option<&str>,
) -> ProofRunProjection {
    let artifacts = proof_artifacts(namespace);
    let summary = artifacts.summary_path.as_deref().and_then(read_json_path);
    let report = artifacts
        .run_report_path
        .as_deref()
        .and_then(read_json_path);
    let trace = artifacts
        .authority_trace_path
        .as_deref()
        .and_then(read_json_path);
    let source = summary.as_ref().or(report.as_ref()).or(trace.as_ref());
    let lane = source
        .and_then(|value| value.get("lane"))
        .and_then(Value::as_str)
        .unwrap_or(fallback_lane)
        .to_string();
    let status = forced_status
        .map(ToOwned::to_owned)
        .or_else(|| {
            source
                .and_then(|value| value.get("status"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .unwrap_or_else(|| "idle".to_string());
    let observed_run = report
        .as_ref()
        .and_then(|value| value.get("observed_run"))
        .or_else(|| trace.as_ref().and_then(|value| value.get("observed_run")));
    let first_write = summary
        .as_ref()
        .and_then(|value| value.get("first_failed_authority_write"))
        .or_else(|| {
            report
                .as_ref()
                .and_then(|value| value.get("first_failed_authority_write"))
        })
        .and_then(project_failed_write);
    let nodes = trace
        .as_ref()
        .and_then(|value| value.get("node_traces"))
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(project_node).collect::<Vec<_>>())
        .unwrap_or_default();
    let (workers, validators): (Vec<_>, Vec<_>) =
        nodes.into_iter().partition(|node| node.role == "worker");
    ProofRunProjection {
        namespace: namespace.to_string(),
        lane,
        status,
        first_red_stage: summary
            .as_ref()
            .and_then(|value| value.get("first_red_stage"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        first_red_subject: summary
            .as_ref()
            .and_then(|value| value.get("first_red_subject"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        blocker_id: source
            .and_then(|value| value.get("blocker_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        detail: source
            .and_then(|value| value.get("detail"))
            .and_then(Value::as_str)
            .map(redact_sensitive),
        run_id: observed_run
            .and_then(|value| value.get("run"))
            .and_then(|value| value.get("training_run_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        window_id: summary
            .as_ref()
            .and_then(|value| value.get("window_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        assignment_id: summary
            .as_ref()
            .and_then(|value| value.get("assignment_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        lease_id: summary
            .as_ref()
            .and_then(|value| value.get("lease_id"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        membership_revision: summary
            .as_ref()
            .and_then(|value| value.get("membership_revision"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        closeout_stage: summary
            .as_ref()
            .and_then(|value| value.get("closeout_stage"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        closeout_next_action: summary
            .as_ref()
            .and_then(|value| value.get("closeout_next_action"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        closeout_last_error: summary
            .as_ref()
            .and_then(|value| value.get("closeout_last_error"))
            .and_then(Value::as_str)
            .map(redact_sensitive),
        workers,
        validators,
        transport: project_transport(trace.as_ref().and_then(|value| value.get("transport"))),
        artifacts,
        first_failed_authority_write: first_write,
        local_simulation: true,
        simulated_treasury: true,
        updated_at: now_rfc3339ish(),
    }
}

fn proof_artifacts(namespace: &str) -> ProofArtifactsProjection {
    let root = proof_namespace_root(namespace);
    let fleet = root.join("fleet");
    let run_report = fleet.join("run-report.json");
    let authority_trace = fleet.join("authority-state-trace.json");
    let summary = fleet.join("proof-summary.json");
    let artifact_trace = root.join("artifacts").join("object-trace.jsonl");
    ProofArtifactsProjection {
        root: root.display().to_string(),
        run_report_path: run_report
            .exists()
            .then(|| run_report.display().to_string()),
        authority_trace_path: authority_trace
            .exists()
            .then(|| authority_trace.display().to_string()),
        summary_path: summary.exists().then(|| summary.display().to_string()),
        artifact_trace_path: artifact_trace
            .exists()
            .then(|| artifact_trace.display().to_string()),
    }
}

fn read_json_path(path: &str) -> Option<Value> {
    let payload = fs::read_to_string(path).ok()?;
    serde_json::from_str(&payload).ok()
}

fn project_failed_write(value: &Value) -> Option<ProofFailedAuthorityWrite> {
    Some(ProofFailedAuthorityWrite {
        source: value.get("source")?.as_str()?.to_string(),
        method: value
            .get("method")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        url: value
            .get("url")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        status: value
            .get("status")
            .and_then(Value::as_u64)
            .and_then(|status| u16::try_from(status).ok()),
        response_body: value
            .get("response_body")
            .and_then(Value::as_str)
            .map(redact_sensitive),
        detail: value
            .get("detail")
            .and_then(Value::as_str)
            .map(redact_sensitive)
            .unwrap_or_else(|| "authority write failed".to_string()),
    })
}

fn project_node(value: &Value) -> Option<ProofNodeProjection> {
    let role = value.get("role")?.as_str()?.to_string();
    let eligibility = value.get("eligibility");
    let training_status = value.get("training_status");
    Some(ProofNodeProjection {
        role,
        index: value.get("index").and_then(Value::as_u64).unwrap_or(0),
        label: value
            .get("node_label")
            .and_then(Value::as_str)
            .unwrap_or("node")
            .to_string(),
        running: training_status.is_some(),
        pid: None,
        eligibility: eligibility
            .and_then(|value| value.get("eligibility"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        hard_gate_reasons: eligibility
            .and_then(|value| value.get("hard_gate_reasons"))
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .map(ToOwned::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        retained_state_fixture_id: value
            .get("retained_state_fixture_id")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        training_status: training_status
            .and_then(|value| value.get("status"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        training_error: value
            .get("training_status_error")
            .and_then(Value::as_str)
            .map(redact_sensitive),
    })
}

fn project_transport(value: Option<&Value>) -> ProofTransportProjection {
    let Some(value) = value else {
        return ProofTransportProjection {
            authority: "unknown".to_string(),
            relay: "unknown".to_string(),
            artifact_store: "unknown".to_string(),
            node_surfaces: "unknown".to_string(),
        };
    };
    ProofTransportProjection {
        authority: route_group_state(value.get("authority_front_door")),
        relay: value
            .get("relay")
            .and_then(|relay| relay.get("authority_running"))
            .and_then(Value::as_bool)
            .map(|running| if running { "ok" } else { "down" }.to_string())
            .unwrap_or_else(|| "unknown".to_string()),
        artifact_store: route_group_state(value.get("artifact_store")),
        node_surfaces: route_group_state(value.get("node_surfaces")),
    }
}

fn route_group_state(value: Option<&Value>) -> String {
    let Some(array) = value.and_then(Value::as_array) else {
        return "unknown".to_string();
    };
    if array.is_empty() {
        return "unknown".to_string();
    }
    let successes = array
        .iter()
        .filter(|item| item.get("ok").and_then(Value::as_bool).unwrap_or(false))
        .count();
    if successes == array.len() {
        "ok".to_string()
    } else if successes == 0 {
        "down".to_string()
    } else {
        "degraded".to_string()
    }
}

fn emit_status<T: Serialize>(app: &tauri::AppHandle, event: &str, payload: &T) {
    let _ = app.emit(event, payload);
}

fn append_log(path: &Path, bytes: &[u8]) {
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = file.write_all(bytes);
    }
}

pub fn redact_sensitive(input: &str) -> String {
    let mut out = input.to_string();
    for key in [
        "TOKEN",
        "SECRET",
        "PRIVATE_KEY",
        "API_KEY",
        "BEARER",
        "MNEMONIC",
        "PASSWORD",
        "OPENAI_API_KEY",
        "SPARK_API_KEY",
    ] {
        out = redact_key_values(&out, key);
    }
    out
}

fn redact_key_values(input: &str, key: &str) -> String {
    let mut output = Vec::new();
    for part in input.split_whitespace() {
        let upper = part.to_ascii_uppercase();
        if upper.contains(key) && (part.contains('=') || part.contains(':')) {
            if let Some((prefix, _)) = part.split_once('=') {
                output.push(format!("{prefix}=<redacted>"));
            } else if let Some((prefix, _)) = part.split_once(':') {
                output.push(format!("{prefix}:<redacted>"));
            } else {
                output.push("<redacted>".to_string());
            }
        } else {
            output.push(part.to_string());
        }
    }
    output.join(" ")
}

fn now_rfc3339ish() -> String {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_millis();
    format!("{millis}")
}

fn timestamp_for_filename() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projects_unconfigured_status() {
        let value = serde_json::json!({
            "listen_addr": "127.0.0.1:9468",
            "desired_mode": "offline",
            "snapshot": {
                "runtime": {
                    "authoritative_status": "unconfigured",
                    "execution_backend_label": "not configured",
                    "queue_depth": 0,
                    "online_uptime_seconds": 0,
                    "provider_blocker_codes": ["CONFIG_MISSING"],
                    "last_action": "pylon is not initialized",
                    "last_error": "config missing"
                },
                "availability": {
                    "local_gemma": {}
                },
                "inventory_rows": []
            }
        });
        let status = project_pylon_status(
            value,
            "stopped".to_string(),
            Some("/tmp/pylon".to_string()),
            PathBuf::from("/tmp/missing-config.json"),
            PathBuf::from("/tmp/pylon-home"),
            None,
            None,
            None,
        );
        assert!(status.installed);
        assert!(!status.configured);
        assert_eq!(status.provider_state, "unconfigured");
        assert_eq!(status.blocker_codes, vec!["CONFIG_MISSING"]);
    }

    #[test]
    fn projects_ready_status() {
        let value = serde_json::json!({
            "listen_addr": "127.0.0.1:9468",
            "desired_mode": "online",
            "snapshot": {
                "runtime": {
                    "authoritative_status": "online",
                    "execution_backend_label": "local_gemma",
                    "queue_depth": 2,
                    "online_uptime_seconds": 10,
                    "provider_blocker_codes": []
                },
                "availability": {
                    "local_gemma": {
                        "ready_model": "gemma4:e4b"
                    }
                },
                "inventory_rows": [
                    {"eligible": true},
                    {"eligible": false}
                ]
            }
        });
        let status = project_pylon_status(
            value,
            "running".to_string(),
            Some("/tmp/pylon".to_string()),
            PathBuf::from("/tmp/config.json"),
            PathBuf::from("/tmp/pylon-home"),
            Some(42),
            None,
            None,
        );
        assert_eq!(status.provider_state, "online");
        assert_eq!(status.ready_model.as_deref(), Some("gemma4:e4b"));
        assert_eq!(status.products_visible, Some(2));
        assert_eq!(status.products_eligible, Some(1));
        assert_eq!(status.queue_depth, Some(2));
    }

    #[test]
    fn rejects_unknown_provider_mode() {
        assert!(normalize_provider_mode("destroy").is_err());
    }

    #[test]
    fn rejects_unknown_proof_lane() {
        assert!(normalize_proof_lane("prod").is_err());
    }

    #[test]
    fn redacts_sensitive_values() {
        let redacted = redact_sensitive("OPENAI_API_KEY=sk-test TOKEN:abc normal");
        assert!(redacted.contains("OPENAI_API_KEY=<redacted>"));
        assert!(redacted.contains("TOKEN:<redacted>"));
        assert!(!redacted.contains("sk-test"));
        assert!(!redacted.contains("abc"));
    }

    #[test]
    fn projects_failed_authority_write() {
        let value = serde_json::json!({
            "source": "replacement_window_seal",
            "method": "POST",
            "url": "http://127.0.0.1",
            "status": 400,
            "response_body": "{\"error\":\"kernel_error\"}",
            "detail": "failed"
        });
        let projected = project_failed_write(&value).unwrap_or_else(|| ProofFailedAuthorityWrite {
            source: String::new(),
            method: None,
            url: None,
            status: None,
            response_body: None,
            detail: String::new(),
        });
        assert_eq!(projected.source, "replacement_window_seal");
        assert_eq!(projected.status, Some(400));
    }

    #[test]
    fn fixture_pylon_status_contract_decodes() {
        let fixture =
            include_str!("../../../../fixtures/proof/autopilot/pylon-status-projection.json");
        let status: PylonStatusProjection = serde_json::from_str(fixture)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));

        assert!(status.installed);
        assert_eq!(status.provider_state, "online");
        assert_eq!(status.process_state, "running");
        assert_eq!(status.products_eligible, Some(1));
    }

    #[test]
    fn fixture_proof_run_contract_decodes() {
        let fixture =
            include_str!("../../../../fixtures/proof/autopilot/proof-run-projection.json");
        let proof: ProofRunProjection = serde_json::from_str(fixture)
            .unwrap_or_else(|error| panic!("fixture should decode: {error}"));

        assert_eq!(proof.lane, "cs336-a1");
        assert_eq!(proof.status, "accepted");
        assert_eq!(proof.transport.authority, "ok");
        assert_eq!(proof.workers.len(), 1);
        assert_eq!(proof.validators.len(), 1);
        assert!(proof.local_simulation);
        assert!(proof.simulated_treasury);
    }
}
