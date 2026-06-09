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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HomeworkStageProjection {
    pub id: String,
    pub label: String,
    pub state: String,
    pub detail: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HomeworkAssignmentProjection {
    pub kind: String,
    pub state: String,
    pub training_run_id: Option<String>,
    pub window_id: Option<String>,
    pub assignment_id: Option<String>,
    pub lease_id: Option<String>,
    pub membership_revision: Option<String>,
    pub role: Option<String>,
    pub network_id: Option<String>,
    pub runtime_lane_id: Option<String>,
    pub runtime_operation: Option<String>,
    pub runtime_work_class: Option<String>,
    pub runtime_manifest_path: Option<String>,
    pub updated_at_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HomeworkRuntimeProjection {
    pub training_run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub lease_id: String,
    pub role: String,
    pub desired_state: String,
    pub process_state: String,
    pub pid: Option<u32>,
    pub last_heartbeat_at_ms: Option<i64>,
    pub last_failure_reason: Option<String>,
    pub manifest_path: String,
    pub run_root: String,
    pub launch_count: u64,
    pub restart_count: u64,
    pub updated_at_ms: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HomeworkCloseoutProjection {
    pub training_run_id: String,
    pub window_id: String,
    pub assignment_id: String,
    pub role: String,
    pub stage: String,
    pub next_action: Option<String>,
    pub challenge_id: Option<String>,
    pub acceptance_state: Option<String>,
    pub accepted_outcome_id: Option<String>,
    pub payout_state: Option<String>,
    pub payout_id: Option<String>,
    pub payout_receipt_id: Option<String>,
    pub payout_reconciliation_status: Option<String>,
    pub last_error: Option<String>,
    pub blocking_class: Option<String>,
    pub updated_at_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HomeworkIssueProjection {
    pub kind: String,
    pub subject_id: String,
    pub reason: String,
    pub blocking_class: Option<String>,
    pub owner: Option<String>,
    pub retryable: Option<bool>,
    pub observed_at_ms: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HomeworkTrainingProjection {
    pub node_label: String,
    pub provider_pubkey: Option<String>,
    pub checkpoint_serve_url: String,
    pub runtime_surface_detected: bool,
    pub runtime_surface_error: Option<String>,
    pub contributor_supported: bool,
    pub current_run_id: Option<String>,
    pub active_window_id: Option<String>,
    pub manifest_count: u64,
    pub work_offer_count: u64,
    pub pending_publication_count: u64,
    pub closeout_count: u64,
    pub validator_queue_count: u64,
    pub recent_trn_event_count: u64,
    pub blocked_label_keys: Vec<String>,
    pub active_runtime: Option<HomeworkRuntimeProjection>,
    pub leased_assignment: Option<HomeworkAssignmentProjection>,
    pub recent_work_offers: Vec<HomeworkAssignmentProjection>,
    pub recent_closeout_progress: Vec<HomeworkCloseoutProjection>,
    pub recent_issues: Vec<HomeworkIssueProjection>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct HomeworkSnapshotProjection {
    pub assignment_label: String,
    pub status: String,
    pub detail: String,
    pub payout_policy: String,
    pub updated_at: String,
    pub pylon: PylonStatusProjection,
    pub training: Option<HomeworkTrainingProjection>,
    pub training_error: Option<String>,
    pub proof: Option<ProofRunProjection>,
    pub stages: Vec<HomeworkStageProjection>,
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
pub fn pylon_homework_get(state: tauri::State<'_, PylonManager>) -> HomeworkSnapshotProjection {
    let pylon = pylon_status_projection(Some(&state), None, None);
    let config_path = current_config_path(Some(&state));
    let (training, training_error) = load_homework_training_projection(config_path.as_path());
    let proof = state.proof_snapshot();

    build_homework_snapshot(pylon, training, training_error, proof)
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

fn load_homework_training_projection(
    config_path: &Path,
) -> (Option<HomeworkTrainingProjection>, Option<String>) {
    let binary = match resolve_binary_path(BinaryKind::Pylon) {
        Ok(binary) => binary,
        Err(error) => return (None, Some(redact_sensitive(&error))),
    };
    let args = vec![
        OsString::from("--config-path"),
        config_path.as_os_str().to_os_string(),
        OsString::from("training"),
        OsString::from("status"),
        OsString::from("--json"),
    ];

    match run_command_with_timeout(&binary, &args, DEFAULT_COMMAND_TIMEOUT) {
        Ok(output) if output.status.success() => {
            match serde_json::from_slice::<Value>(&output.stdout) {
                Ok(value) => (Some(project_homework_training_status(&value)), None),
                Err(error) => (
                    None,
                    Some(format!("failed to decode homework status JSON: {error}")),
                ),
            }
        }
        Ok(output) => (
            None,
            Some(command_failure("pylon training status", &output)),
        ),
        Err(error) => (None, Some(redact_sensitive(&error))),
    }
}

fn build_homework_snapshot(
    pylon: PylonStatusProjection,
    training: Option<HomeworkTrainingProjection>,
    training_error: Option<String>,
    proof: Option<ProofRunProjection>,
) -> HomeworkSnapshotProjection {
    let (status, detail) = homework_status_and_detail(
        &pylon,
        training.as_ref(),
        training_error.as_deref(),
        proof.as_ref(),
    );
    let stages = homework_stages(&pylon, training.as_ref(), proof.as_ref());

    HomeworkSnapshotProjection {
        assignment_label: "CS336 Assignment 1 homework".to_string(),
        status,
        detail,
        payout_policy: "pay only accepted homework work; no recurring liveness payouts".to_string(),
        updated_at: now_rfc3339ish(),
        pylon,
        training,
        training_error,
        proof,
        stages,
    }
}

fn homework_status_and_detail(
    pylon: &PylonStatusProjection,
    training: Option<&HomeworkTrainingProjection>,
    training_error: Option<&str>,
    proof: Option<&ProofRunProjection>,
) -> (String, String) {
    if !pylon.installed {
        return (
            "Pylon missing".to_string(),
            "Install or select a Pylon binary before homework work can run.".to_string(),
        );
    }

    if let Some(proof) = proof {
        if proof_reached_paid_closeout(proof) {
            return ("Homework paid".to_string(), proof_paid_detail(proof));
        }
    }

    if pylon.provider_state != "online" {
        if pylon.process_state == "running" || pylon.process_state == "starting" {
            return (
                "Starting Pylon".to_string(),
                "Pylon is running; set it online to receive homework work.".to_string(),
            );
        }
        return (
            "Offline".to_string(),
            "Start Pylon and set provider mode online to wait for homework work.".to_string(),
        );
    }

    if let Some(training) = training {
        let relevant_issue = first_relevant_homework_issue(training);
        if !training.blocked_label_keys.is_empty() || relevant_issue.is_some() {
            return (
                "Needs attention".to_string(),
                relevant_issue
                    .map(summarize_homework_issue)
                    .unwrap_or_else(|| {
                        format!("blocked labels: {}", training.blocked_label_keys.join(", "))
                    }),
            );
        }
        if let Some(runtime) = training.active_runtime.as_ref() {
            return (
                "Training homework".to_string(),
                format!(
                    "{} {} {}",
                    runtime.role, runtime.process_state, runtime.assignment_id
                ),
            );
        }
        if let Some(closeout) = first_relevant_homework_closeout(training) {
            if homework_closeout_paid(closeout) {
                return (
                    "Homework paid".to_string(),
                    closeout.payout_id.clone().unwrap_or_else(|| {
                        closeout
                            .accepted_outcome_id
                            .clone()
                            .unwrap_or_else(|| closeout.stage.clone())
                    }),
                );
            }
            if !homework_closeout_terminal(closeout.stage.as_str()) {
                return (
                    "Closing out homework".to_string(),
                    closeout
                        .next_action
                        .clone()
                        .unwrap_or_else(|| closeout.stage.clone()),
                );
            }
        }
        if let Some(assignment) = training.leased_assignment.as_ref() {
            return (
                "Homework assigned".to_string(),
                assignment
                    .assignment_id
                    .clone()
                    .unwrap_or_else(|| assignment.state.clone()),
            );
        }
        if !training.recent_work_offers.is_empty() || training.contributor_supported {
            return (
                "Ready for homework".to_string(),
                "Online node can receive homework work when an admin launches a run.".to_string(),
            );
        }
    }

    if let Some(proof) = proof {
        if proof.status == "running" || proof.status == "starting" {
            return (
                "Testing homework".to_string(),
                "Local homework proof lane is running.".to_string(),
            );
        }
        if proof.status == "accepted" || proof.status == "completed" || proof.status == "paid" {
            return (
                "Homework proof complete".to_string(),
                proof.detail.clone().unwrap_or_else(|| {
                    "Local homework proof completed; waiting for payout evidence.".to_string()
                }),
            );
        }
    }

    if let Some(error) = training_error {
        if pylon.process_state == "running" || pylon.provider_state == "online" {
            return (
                "Preparing homework".to_string(),
                format!("Homework status not available yet: {error}"),
            );
        }
    }

    if pylon.provider_state == "online" {
        return (
            "Ready for homework".to_string(),
            "Pylon is online; waiting for homework work.".to_string(),
        );
    }
    (
        "Offline".to_string(),
        "Start Pylon and set provider mode online to wait for homework work.".to_string(),
    )
}

fn proof_reached_paid_closeout(proof: &ProofRunProjection) -> bool {
    proof.status == "paid"
        || proof
            .closeout_stage
            .as_deref()
            .map(homework_closeout_terminal)
            .unwrap_or(false)
}

fn proof_paid_detail(proof: &ProofRunProjection) -> String {
    proof
        .detail
        .clone()
        .unwrap_or_else(|| "Latest local homework proof reached accepted work.".to_string())
}

fn homework_closeout_terminal(stage: &str) -> bool {
    matches!(
        stage,
        "rewarded" | "accepted" | "paid" | "confirmed" | "delivered" | "settled"
    )
}

fn homework_closeout_paid(closeout: &HomeworkCloseoutProjection) -> bool {
    homework_closeout_terminal(closeout.stage.as_str())
        || closeout
            .acceptance_state
            .as_deref()
            .is_some_and(|state| matches!(state, "accepted" | "paid" | "confirmed"))
        || closeout.payout_state.as_deref().is_some_and(|state| {
            matches!(
                state,
                "confirmed" | "accepted" | "paid" | "settled" | "rewarded"
            )
        })
        || closeout.payout_receipt_id.is_some()
        || closeout.payout_id.is_some()
}

fn current_homework_run_id(training: &HomeworkTrainingProjection) -> Option<&str> {
    training
        .current_run_id
        .as_deref()
        .or_else(|| {
            training
                .active_runtime
                .as_ref()
                .map(|runtime| runtime.training_run_id.as_str())
        })
        .or_else(|| {
            training
                .leased_assignment
                .as_ref()
                .and_then(|assignment| assignment.training_run_id.as_deref())
        })
}

fn current_homework_identifiers(training: &HomeworkTrainingProjection) -> Vec<&str> {
    let mut identifiers = Vec::new();
    if let Some(run_id) = current_homework_run_id(training) {
        identifiers.push(run_id);
    }
    if let Some(window_id) = training.active_window_id.as_deref() {
        identifiers.push(window_id);
    }
    if let Some(runtime) = training.active_runtime.as_ref() {
        identifiers.push(runtime.window_id.as_str());
        identifiers.push(runtime.assignment_id.as_str());
        identifiers.push(runtime.lease_id.as_str());
    }
    if let Some(assignment) = training.leased_assignment.as_ref() {
        if let Some(window_id) = assignment.window_id.as_deref() {
            identifiers.push(window_id);
        }
        if let Some(assignment_id) = assignment.assignment_id.as_deref() {
            identifiers.push(assignment_id);
        }
        if let Some(lease_id) = assignment.lease_id.as_deref() {
            identifiers.push(lease_id);
        }
    }
    identifiers
}

fn homework_issue_matches_current_run(
    training: &HomeworkTrainingProjection,
    issue: &HomeworkIssueProjection,
) -> bool {
    let identifiers = current_homework_identifiers(training);
    identifiers.is_empty()
        || identifiers
            .iter()
            .any(|identifier| issue.subject_id.contains(*identifier))
}

fn first_relevant_homework_issue(
    training: &HomeworkTrainingProjection,
) -> Option<&HomeworkIssueProjection> {
    training
        .recent_issues
        .iter()
        .find(|issue| homework_issue_matches_current_run(training, issue))
}

fn first_relevant_homework_closeout(
    training: &HomeworkTrainingProjection,
) -> Option<&HomeworkCloseoutProjection> {
    let current_run_id = current_homework_run_id(training);
    training.recent_closeout_progress.iter().find(|closeout| {
        current_run_id
            .map(|run_id| closeout.training_run_id == run_id)
            .unwrap_or(true)
    })
}

fn relevant_homework_closeouts(
    training: &HomeworkTrainingProjection,
) -> impl Iterator<Item = &HomeworkCloseoutProjection> {
    let current_run_id = current_homework_run_id(training);
    training
        .recent_closeout_progress
        .iter()
        .filter(move |closeout| {
            current_run_id
                .map(|run_id| closeout.training_run_id == run_id)
                .unwrap_or(true)
        })
}

fn summarize_homework_issue(issue: &HomeworkIssueProjection) -> String {
    if issue.reason.contains("File name too long") {
        return "Homework replay artifact path exceeded the local filesystem limit; wait for a fresh assignment.".to_string();
    }
    if issue.reason.contains("artifact_incomplete") {
        return "Homework replay artifact is not complete yet; wait for the next assignment or retry."
            .to_string();
    }
    if issue
        .reason
        .contains("training_scheduler_assignment_not_found")
    {
        return "A stale homework assignment retry was rejected; wait for a fresh assignment."
            .to_string();
    }

    let reason = redact_sensitive(issue.reason.as_str());
    let reason = reason
        .split('`')
        .enumerate()
        .filter_map(|(index, part)| (index % 2 == 0).then_some(part))
        .collect::<String>();
    let reason = reason.split_whitespace().collect::<Vec<_>>().join(" ");
    if reason.is_empty() {
        issue.kind.clone()
    } else {
        format!(
            "{}: {}",
            issue.kind,
            truncate_homework_detail(reason.as_str(), 160)
        )
    }
}

fn truncate_homework_detail(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn homework_stages(
    pylon: &PylonStatusProjection,
    training: Option<&HomeworkTrainingProjection>,
    proof: Option<&ProofRunProjection>,
) -> Vec<HomeworkStageProjection> {
    let online_state = if pylon.provider_state == "online" {
        "ready"
    } else if pylon.process_state == "running" || pylon.process_state == "starting" {
        "starting"
    } else {
        "offline"
    };
    let intake_state = training
        .map(|training| {
            if !training.blocked_label_keys.is_empty()
                || first_relevant_homework_issue(training).is_some()
            {
                "attention"
            } else if training.contributor_supported {
                "ready"
            } else {
                "blocked"
            }
        })
        .unwrap_or("unknown");
    let assignment_state = training
        .map(|training| {
            if training.active_runtime.is_some() || training.leased_assignment.is_some() {
                "active"
            } else if !training.recent_work_offers.is_empty() || training.current_run_id.is_some() {
                "ready"
            } else {
                "waiting"
            }
        })
        .unwrap_or("unknown");
    let runtime_state = training
        .and_then(|training| training.active_runtime.as_ref())
        .map(|runtime| runtime.process_state.as_str())
        .or_else(|| proof.map(|proof| proof.status.as_str()))
        .unwrap_or("idle");
    let closeout_state = training
        .and_then(first_relevant_homework_closeout)
        .map(|closeout| closeout.stage.as_str())
        .or_else(|| proof.and_then(|proof| proof.closeout_stage.as_deref()))
        .unwrap_or("waiting");
    let payout_state = training
        .and_then(|training| {
            relevant_homework_closeouts(training).find_map(|entry| entry.payout_state.as_deref())
        })
        .or_else(|| {
            proof.and_then(|proof| {
                if proof_reached_paid_closeout(proof) {
                    Some("accepted")
                } else if proof.status == "accepted" {
                    Some("accepted")
                } else {
                    None
                }
            })
        })
        .unwrap_or("waiting");

    vec![
        HomeworkStageProjection {
            id: "online".to_string(),
            label: "Online".to_string(),
            state: online_state.to_string(),
            detail: format!(
                "process={} provider={}",
                pylon.process_state, pylon.provider_state
            ),
        },
        HomeworkStageProjection {
            id: "intake".to_string(),
            label: "Homework intake".to_string(),
            state: intake_state.to_string(),
            detail: training
                .map(|training| format!("{} work offer(s)", training.work_offer_count))
                .unwrap_or_else(|| "training status not loaded".to_string()),
        },
        HomeworkStageProjection {
            id: "assignment".to_string(),
            label: "Assignment".to_string(),
            state: assignment_state.to_string(),
            detail: training
                .and_then(|training| {
                    training
                        .active_runtime
                        .as_ref()
                        .map(|runtime| runtime.assignment_id.clone())
                        .or_else(|| {
                            training
                                .leased_assignment
                                .as_ref()
                                .and_then(|assignment| assignment.assignment_id.clone())
                        })
                })
                .or_else(|| proof.and_then(|proof| proof.assignment_id.clone()))
                .unwrap_or_else(|| "waiting for admin-launched homework".to_string()),
        },
        HomeworkStageProjection {
            id: "runtime".to_string(),
            label: "Runtime".to_string(),
            state: runtime_state.to_string(),
            detail: training
                .and_then(|training| training.active_runtime.as_ref())
                .map(|runtime| format!("{} pid={:?}", runtime.role, runtime.pid))
                .unwrap_or_else(|| "no active homework runtime".to_string()),
        },
        HomeworkStageProjection {
            id: "closeout".to_string(),
            label: "Closeout".to_string(),
            state: closeout_state.to_string(),
            detail: training
                .and_then(first_relevant_homework_closeout)
                .and_then(|closeout| closeout.next_action.clone())
                .unwrap_or_else(|| "no closeout pending".to_string()),
        },
        HomeworkStageProjection {
            id: "payout".to_string(),
            label: "Payout".to_string(),
            state: payout_state.to_string(),
            detail: training
                .and_then(|training| {
                    relevant_homework_closeouts(training).find_map(|entry| entry.payout_id.clone())
                })
                .or_else(|| {
                    proof
                        .and_then(|proof| proof.closeout_stage.clone())
                        .map(|stage| format!("local proof closeout {stage}"))
                })
                .unwrap_or_else(|| "paid only after accepted homework".to_string()),
        },
    ]
}

fn project_homework_training_status(value: &Value) -> HomeworkTrainingProjection {
    let recent_work_offers = value_array(value, "recent_work_offers")
        .iter()
        .take(6)
        .map(|entry| project_homework_assignment("offer", entry))
        .collect::<Vec<_>>();
    let recent_closeout_progress = value_array(value, "recent_closeout_progress")
        .iter()
        .take(8)
        .map(project_homework_closeout)
        .collect::<Vec<_>>();
    let recent_issues = value_array(value, "recent_issues")
        .iter()
        .take(8)
        .map(project_homework_issue)
        .collect::<Vec<_>>();

    HomeworkTrainingProjection {
        node_label: value_string(value, "node_label").unwrap_or_else(|| "local pylon".to_string()),
        provider_pubkey: value_string(value, "provider_pubkey"),
        checkpoint_serve_url: value_string(value, "checkpoint_serve_url").unwrap_or_default(),
        runtime_surface_detected: value_bool(value, "runtime_surface_detected").unwrap_or(false),
        runtime_surface_error: value_string(value, "runtime_surface_error")
            .map(redact_sensitive_owned),
        contributor_supported: value_bool(value, "contributor_supported").unwrap_or(false),
        current_run_id: value_string(value, "current_run_id"),
        active_window_id: value_string(value, "active_window_id"),
        manifest_count: value_u64(value, "manifest_count").unwrap_or(0),
        work_offer_count: recent_work_offers.len() as u64,
        pending_publication_count: value_u64(value, "pending_publication_count").unwrap_or(0),
        closeout_count: value_u64(value, "closeout_count").unwrap_or(0),
        validator_queue_count: value_array(value, "validator_queue").len() as u64,
        recent_trn_event_count: value_array(value, "recent_trn_events").len() as u64,
        blocked_label_keys: value_array(value, "blocked_label_keys")
            .iter()
            .filter_map(Value::as_str)
            .map(ToOwned::to_owned)
            .collect(),
        active_runtime: value
            .get("active_runtime")
            .and_then(project_homework_runtime),
        leased_assignment: value
            .get("leased_assignment")
            .filter(|entry| !entry.is_null())
            .map(|entry| project_homework_assignment("leased", entry)),
        recent_work_offers,
        recent_closeout_progress,
        recent_issues,
    }
}

fn project_homework_runtime(value: &Value) -> Option<HomeworkRuntimeProjection> {
    Some(HomeworkRuntimeProjection {
        training_run_id: value_string(value, "training_run_id")?,
        window_id: value_string(value, "window_id")?,
        assignment_id: value_string(value, "assignment_id")?,
        lease_id: value_string(value, "lease_id")?,
        role: value_string(value, "role").unwrap_or_else(|| "worker".to_string()),
        desired_state: value_string(value, "desired_state")
            .unwrap_or_else(|| "unknown".to_string()),
        process_state: value_string(value, "process_state")
            .unwrap_or_else(|| "unknown".to_string()),
        pid: value_u64(value, "pid").and_then(|value| u32::try_from(value).ok()),
        last_heartbeat_at_ms: value_i64(value, "last_heartbeat_at_ms"),
        last_failure_reason: value_string(value, "last_failure_reason").map(redact_sensitive_owned),
        manifest_path: value_string(value, "manifest_path").unwrap_or_default(),
        run_root: value_string(value, "run_root").unwrap_or_default(),
        launch_count: value_u64(value, "launch_count").unwrap_or(0),
        restart_count: value_u64(value, "restart_count").unwrap_or(0),
        updated_at_ms: value_i64(value, "updated_at_ms").unwrap_or(0),
    })
}

fn project_homework_assignment(kind: &str, value: &Value) -> HomeworkAssignmentProjection {
    HomeworkAssignmentProjection {
        kind: kind.to_string(),
        state: value_string(value, "state").unwrap_or_else(|| "unknown".to_string()),
        training_run_id: value_string(value, "training_run_id"),
        window_id: value_string(value, "window_id"),
        assignment_id: value_string(value, "assignment_id"),
        lease_id: value_string(value, "lease_id"),
        membership_revision: value_string(value, "membership_revision"),
        role: value_string(value, "role"),
        network_id: value_string(value, "network_id"),
        runtime_lane_id: value_string(value, "runtime_lane_id"),
        runtime_operation: value_string(value, "runtime_operation"),
        runtime_work_class: value_string(value, "runtime_work_class"),
        runtime_manifest_path: value_string(value, "runtime_manifest_path"),
        updated_at_ms: value_i64(value, "updated_at_ms"),
    }
}

fn project_homework_closeout(value: &Value) -> HomeworkCloseoutProjection {
    HomeworkCloseoutProjection {
        training_run_id: value_string(value, "training_run_id").unwrap_or_default(),
        window_id: value_string(value, "window_id").unwrap_or_default(),
        assignment_id: value_string(value, "assignment_id").unwrap_or_default(),
        role: value_string(value, "role").unwrap_or_default(),
        stage: value_string(value, "stage").unwrap_or_else(|| "unknown".to_string()),
        next_action: value_string(value, "next_action"),
        challenge_id: value_string(value, "challenge_id"),
        acceptance_state: value_string(value, "acceptance_state"),
        accepted_outcome_id: value_string(value, "accepted_outcome_id"),
        payout_state: value_string(value, "payout_state"),
        payout_id: value_string(value, "payout_id"),
        payout_receipt_id: value_string(value, "payout_receipt_id"),
        payout_reconciliation_status: value_string(value, "payout_reconciliation_status"),
        last_error: value_string(value, "last_error").map(redact_sensitive_owned),
        blocking_class: value_string(value, "blocking_class"),
        updated_at_ms: value_i64(value, "updated_at_ms"),
    }
}

fn project_homework_issue(value: &Value) -> HomeworkIssueProjection {
    HomeworkIssueProjection {
        kind: value_string(value, "kind").unwrap_or_else(|| "issue".to_string()),
        subject_id: value_string(value, "subject_id").unwrap_or_else(|| "unknown".to_string()),
        reason: value_string(value, "reason")
            .map(|value| redact_sensitive(&value))
            .unwrap_or_else(|| "unknown".to_string()),
        blocking_class: value_string(value, "blocking_class"),
        owner: value_string(value, "owner"),
        retryable: value_bool(value, "retryable"),
        observed_at_ms: value_i64(value, "observed_at_ms"),
    }
}

fn value_array<'a>(value: &'a Value, key: &str) -> &'a [Value] {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .filter(|value| !value.trim().is_empty())
}

fn extract_closeout_stage_from_detail(detail: &str) -> Option<String> {
    let marker = "closeout=";
    let start = detail.find(marker)? + marker.len();
    let stage = detail[start..]
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'))
        .next()
        .unwrap_or_default();
    if homework_closeout_terminal(stage) {
        Some(stage.to_string())
    } else {
        None
    }
}

fn value_bool(value: &Value, key: &str) -> Option<bool> {
    value.get(key).and_then(Value::as_bool)
}

fn value_u64(value: &Value, key: &str) -> Option<u64> {
    value.get(key).and_then(Value::as_u64)
}

fn value_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(Value::as_i64)
}

fn redact_sensitive_owned(value: String) -> String {
    redact_sensitive(&value)
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
    let closeout_stage = summary
        .as_ref()
        .and_then(|value| value.get("closeout_stage"))
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .or_else(|| {
            observed_run
                .and_then(|value| value.get("run"))
                .and_then(|value| value.get("latest_closeout_status"))
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            observed_run
                .and_then(|value| value.get("windows"))
                .and_then(Value::as_array)
                .and_then(|windows| {
                    windows
                        .iter()
                        .filter_map(|window| window.get("closeout_status").and_then(Value::as_str))
                        .find(|stage| homework_closeout_terminal(stage))
                })
                .map(ToOwned::to_owned)
        })
        .or_else(|| {
            source
                .and_then(|value| value.get("detail"))
                .and_then(Value::as_str)
                .and_then(extract_closeout_stage_from_detail)
        });
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
        closeout_stage,
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
    fn projects_homework_training_status() {
        let value = serde_json::json!({
            "node_label": "pylon.local",
            "checkpoint_serve_url": "http://127.0.0.1:43000",
            "runtime_surface_detected": true,
            "contributor_supported": true,
            "current_run_id": "run.cs336.a1.test",
            "active_window_id": "window.cs336.a1.test.0001",
            "manifest_count": 1,
            "pending_publication_count": 0,
            "closeout_count": 1,
            "blocked_label_keys": [],
            "active_runtime": {
                "training_run_id": "run.cs336.a1.test",
                "window_id": "window.cs336.a1.test.0001",
                "assignment_id": "assign.worker.1",
                "lease_id": "lease.worker.1",
                "role": "worker",
                "desired_state": "running",
                "process_state": "running",
                "pid": 1234,
                "manifest_path": "/tmp/manifest.json",
                "run_root": "/tmp/run",
                "launch_count": 1,
                "restart_count": 0,
                "updated_at_ms": 42
            },
            "recent_closeout_progress": [{
                "training_run_id": "run.cs336.a1.test",
                "window_id": "window.cs336.a1.test.0001",
                "assignment_id": "assign.worker.1",
                "role": "worker",
                "stage": "paid",
                "payout_state": "confirmed",
                "payout_id": "payout.accepted.1",
                "payout_receipt_id": "payment.1",
                "payout_reconciliation_status": "settled",
                "updated_at_ms": 43
            }]
        });

        let training = project_homework_training_status(&value);

        assert!(training.contributor_supported);
        assert_eq!(
            training.current_run_id.as_deref(),
            Some("run.cs336.a1.test")
        );
        assert_eq!(
            training
                .active_runtime
                .as_ref()
                .map(|runtime| runtime.assignment_id.as_str()),
            Some("assign.worker.1")
        );
        assert_eq!(
            training
                .recent_closeout_progress
                .first()
                .and_then(|closeout| closeout.payout_state.as_deref()),
            Some("confirmed")
        );
    }

    #[test]
    fn homework_projection_treats_null_assignment_as_absent_and_requires_online() {
        let value = serde_json::json!({
            "node_label": "pylon.local",
            "checkpoint_serve_url": "http://127.0.0.1:43000",
            "runtime_surface_detected": true,
            "contributor_supported": true,
            "leased_assignment": null,
            "recent_work_offers": [{
                "state": "available",
                "training_run_id": "run.cs336.a1.waiting",
                "window_id": "window.cs336.a1.waiting",
                "assignment_id": "assign.waiting",
                "runtime_work_class": "homework"
            }]
        });

        let training = project_homework_training_status(&value);

        assert!(training.leased_assignment.is_none());
        assert_eq!(training.work_offer_count, 1);

        let offline = test_pylon_status("stopped", "offline");
        let (offline_status, _) = homework_status_and_detail(&offline, Some(&training), None, None);
        assert_eq!(offline_status, "Offline");

        let online = test_pylon_status("running", "online");
        let (online_status, _) = homework_status_and_detail(&online, Some(&training), None, None);
        assert_eq!(online_status, "Ready for homework");
    }

    #[test]
    fn homework_projection_ignores_stale_issue_when_current_proof_is_rewarded() {
        let value = serde_json::json!({
            "node_label": "pylon.local",
            "checkpoint_serve_url": "http://127.0.0.1:43000",
            "runtime_surface_detected": true,
            "contributor_supported": true,
            "current_run_id": "run.cs336.a1.current",
            "active_window_id": "window.cs336.a1.current.0001",
            "leased_assignment": {
                "state": "active",
                "training_run_id": "run.cs336.a1.current",
                "window_id": "window.cs336.a1.current.0001",
                "assignment_id": "assign.current",
                "lease_id": "lease.current",
                "role": "worker",
                "runtime_work_class": "homework"
            },
            "recent_closeout_progress": [{
                "training_run_id": "run.cs336.a1.old",
                "window_id": "window.cs336.a1.old.0001",
                "assignment_id": "assign.old",
                "role": "validator",
                "stage": "terminal_failed",
                "last_error": "artifact_incomplete",
                "updated_at_ms": 43
            }],
            "recent_issues": [{
                "kind": "closeout_progress_error",
                "subject_id": "assign.old",
                "reason": "artifact_incomplete: failed to write `/tmp/very/internal/path`: File name too long",
                "blocking_class": "local_queue_replay"
            }]
        });

        let training = project_homework_training_status(&value);
        let proof = test_proof_run("completed", Some("rewarded"));
        let online = test_pylon_status("running", "online");
        let (status, detail) =
            homework_status_and_detail(&online, Some(&training), None, Some(&proof));
        let stages = homework_stages(&online, Some(&training), Some(&proof));

        assert_eq!(status, "Homework paid");
        assert!(!detail.contains("File name too long"));
        assert_eq!(
            stages
                .iter()
                .find(|stage| stage.id == "closeout")
                .map(|stage| stage.state.as_str()),
            Some("rewarded")
        );
        assert_eq!(
            stages
                .iter()
                .find(|stage| stage.id == "payout")
                .map(|stage| stage.state.as_str()),
            Some("accepted")
        );
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

    fn test_proof_run(status: &str, closeout_stage: Option<&str>) -> ProofRunProjection {
        ProofRunProjection {
            namespace: "proof.test".to_string(),
            lane: "cs336-a1".to_string(),
            status: status.to_string(),
            first_red_stage: None,
            first_red_subject: None,
            blocker_id: None,
            detail: Some("proof closeout=rewarded".to_string()),
            run_id: Some("run.cs336.a1.proof".to_string()),
            window_id: Some("window.cs336.a1.proof.0001".to_string()),
            assignment_id: Some("assign.proof".to_string()),
            lease_id: Some("lease.proof".to_string()),
            membership_revision: Some("members.rev1".to_string()),
            closeout_stage: closeout_stage.map(ToOwned::to_owned),
            closeout_next_action: None,
            closeout_last_error: None,
            workers: vec![ProofNodeProjection {
                role: "worker".to_string(),
                index: 1,
                label: "worker-1".to_string(),
                running: true,
                pid: None,
                eligibility: Some("eligible".to_string()),
                hard_gate_reasons: Vec::new(),
                retained_state_fixture_id: None,
                training_status: None,
                training_error: None,
            }],
            validators: vec![ProofNodeProjection {
                role: "validator".to_string(),
                index: 1,
                label: "validator-1".to_string(),
                running: true,
                pid: None,
                eligibility: Some("eligible".to_string()),
                hard_gate_reasons: Vec::new(),
                retained_state_fixture_id: None,
                training_status: None,
                training_error: None,
            }],
            transport: ProofTransportProjection {
                authority: "ok".to_string(),
                relay: "ok".to_string(),
                artifact_store: "ok".to_string(),
                node_surfaces: "down".to_string(),
            },
            artifacts: ProofArtifactsProjection {
                root: "/tmp/proof".to_string(),
                run_report_path: Some("/tmp/proof/run-report.json".to_string()),
                authority_trace_path: Some("/tmp/proof/authority-state-trace.json".to_string()),
                summary_path: Some("/tmp/proof/proof-summary.json".to_string()),
                artifact_trace_path: Some("/tmp/proof/object-trace.jsonl".to_string()),
            },
            first_failed_authority_write: None,
            local_simulation: true,
            simulated_treasury: true,
            updated_at: "0".to_string(),
        }
    }

    fn test_pylon_status(process_state: &str, provider_state: &str) -> PylonStatusProjection {
        PylonStatusProjection {
            installed: true,
            configured: true,
            process_state: process_state.to_string(),
            provider_state: provider_state.to_string(),
            desired_mode: Some(provider_state.to_string()),
            pid: None,
            listen_addr: Some("127.0.0.1:9468".to_string()),
            binary_path: Some("/tmp/pylon".to_string()),
            config_path: Some("/tmp/config.json".to_string()),
            pylon_home: Some("/tmp/pylon-home".to_string()),
            execution_backend: Some("test".to_string()),
            ready_model: Some("fake:gemma".to_string()),
            products_visible: Some(1),
            products_eligible: Some(1),
            queue_depth: Some(0),
            uptime_seconds: Some(0),
            blocker_codes: Vec::new(),
            last_action: None,
            last_error: None,
            last_exit_code: None,
            last_updated_at: "0".to_string(),
        }
    }
}
