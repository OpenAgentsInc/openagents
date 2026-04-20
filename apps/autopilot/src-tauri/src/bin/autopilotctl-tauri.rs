#![allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "CLI intentionally prints operator-facing control results."
)]

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use autopilot_lib::control::{ControlManifest, control_manifest_path};
use serde_json::{Value, json};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cli = Cli::parse(env::args().skip(1).collect())?;
    let command = cli.command.iter().map(String::as_str).collect::<Vec<_>>();
    if matches!(command.as_slice(), [] | ["help"] | ["--help"] | ["-h"]) {
        print_usage();
        return Ok(());
    }
    let target = ControlTarget::resolve(&cli)?;
    let value = match command.as_slice() {
        ["status"] => request_json(&target, "GET", "/v1/status", None)?,
        ["pylon", "status"] => request_json(&target, "GET", "/v1/pylon/status", None)?,
        ["pylon", "start"] => request_json(&target, "POST", "/v1/pylon/start", Some(json!({})))?,
        ["pylon", "stop"] => request_json(&target, "POST", "/v1/pylon/stop", Some(json!({})))?,
        ["pylon", "restart"] => {
            request_json(&target, "POST", "/v1/pylon/restart", Some(json!({})))?
        }
        ["pylon", "logs"] => request_json(&target, "POST", "/v1/pylon/logs", Some(json!({})))?,
        ["pylon", "mode", mode] => request_json(
            &target,
            "POST",
            "/v1/pylon/mode",
            Some(json!({ "mode": mode })),
        )?,
        ["proof", "run", lane, rest @ ..] => {
            let options = ProofRunArgs::parse(lane, rest)?;
            request_json(&target, "POST", "/v1/proof/run", Some(options.into_json()))?
        }
        ["proof", "status"] => request_json(&target, "GET", "/v1/status", None)?
            .get("activeProof")
            .cloned()
            .unwrap_or(Value::Null),
        ["proof", "status", namespace] | ["proof", "get", namespace] => request_json(
            &target,
            "POST",
            "/v1/proof/get",
            Some(json!({ "namespace": namespace })),
        )?,
        ["proof", "doctor", namespace] => request_json(
            &target,
            "POST",
            "/v1/proof/doctor",
            Some(json!({ "namespace": namespace })),
        )?,
        ["proof", "stop", namespace] => request_json(
            &target,
            "POST",
            "/v1/proof/stop",
            Some(json!({ "namespace": namespace })),
        )?,
        ["proof", "reset", namespace] => request_json(
            &target,
            "POST",
            "/v1/proof/reset",
            Some(json!({ "namespace": namespace })),
        )?,
        ["proof", "artifacts", namespace] => request_json(
            &target,
            "POST",
            "/v1/proof/artifacts",
            Some(json!({ "namespace": namespace })),
        )?,
        ["wait", condition, rest @ ..] => wait_condition(&target, condition, rest)?,
        ["smoke", rest @ ..] => run_smoke(&target, rest)?,
        ["homework", "matrix", rest @ ..] | ["proof", "matrix", rest @ ..] => {
            run_homework_matrix(&target, rest)?
        }
        other => return Err(format!("unsupported command: {}", other.join(" "))),
    };

    print_value(&value, cli.json);
    Ok(())
}

#[derive(Debug)]
struct Cli {
    manifest: Option<PathBuf>,
    base_url: Option<String>,
    auth_token: Option<String>,
    json: bool,
    command: Vec<String>,
}

impl Cli {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut manifest = None;
        let mut base_url = None;
        let mut auth_token = None;
        let mut json = false;
        let mut command = Vec::new();
        let mut index = 0;
        while index < args.len() {
            match args[index].as_str() {
                "--manifest" => {
                    index += 1;
                    let value = args
                        .get(index)
                        .ok_or_else(|| "--manifest requires a path".to_string())?;
                    manifest = Some(PathBuf::from(value));
                }
                "--base-url" => {
                    index += 1;
                    base_url = Some(
                        args.get(index)
                            .ok_or_else(|| "--base-url requires a URL".to_string())?
                            .to_string(),
                    );
                }
                "--auth-token" => {
                    index += 1;
                    auth_token = Some(
                        args.get(index)
                            .ok_or_else(|| "--auth-token requires a token".to_string())?
                            .to_string(),
                    );
                }
                "--json" => {
                    json = true;
                }
                value => {
                    command.push(value.to_string());
                    command.extend(args[index + 1..].iter().cloned());
                    break;
                }
            }
            index += 1;
        }
        Ok(Self {
            manifest,
            base_url,
            auth_token,
            json,
            command,
        })
    }
}

#[derive(Debug)]
struct ControlTarget {
    host: String,
    port: u16,
    base_url: String,
    auth_token: String,
}

impl ControlTarget {
    fn resolve(cli: &Cli) -> Result<Self, String> {
        match (&cli.base_url, &cli.auth_token) {
            (Some(base_url), Some(auth_token)) => Self::from_parts(base_url, auth_token),
            (Some(_), None) | (None, Some(_)) => {
                Err("--base-url and --auth-token must be supplied together".to_string())
            }
            (None, None) => {
                let manifest_path = cli.manifest.clone().unwrap_or_else(control_manifest_path);
                let payload = fs::read_to_string(&manifest_path).map_err(|error| {
                    format!(
                        "failed to read Autopilot Tauri control manifest {}: {error}",
                        manifest_path.display()
                    )
                })?;
                let manifest: ControlManifest =
                    serde_json::from_str(&payload).map_err(|error| {
                        format!(
                            "failed to decode Autopilot Tauri control manifest {}: {error}",
                            manifest_path.display()
                        )
                    })?;
                Self::from_parts(&manifest.base_url, &manifest.auth_token)
            }
        }
    }

    fn from_parts(base_url: &str, auth_token: &str) -> Result<Self, String> {
        let trimmed = base_url.trim().trim_end_matches('/');
        let rest = trimmed
            .strip_prefix("http://")
            .ok_or_else(|| format!("only http:// control URLs are supported: {trimmed}"))?;
        let (host, port) = rest
            .rsplit_once(':')
            .ok_or_else(|| format!("control URL must include host:port: {trimmed}"))?;
        let port = port
            .parse::<u16>()
            .map_err(|error| format!("invalid control port in {trimmed}: {error}"))?;
        Ok(Self {
            host: host.to_string(),
            port,
            base_url: trimmed.to_string(),
            auth_token: auth_token.to_string(),
        })
    }
}

#[derive(Debug)]
struct ProofRunArgs {
    lane: String,
    namespace: Option<String>,
    workers: Option<u32>,
    validators: Option<u32>,
    timeout_seconds: Option<u64>,
}

#[derive(Debug, Clone, Copy)]
struct HomeworkLaneSpec {
    lane: &'static str,
    namespace_suffix: &'static str,
    workers: u32,
    validators: u32,
    timeout_seconds: u64,
    min_workers: usize,
    min_validators: usize,
    expected_closeout_stage: Option<&'static str>,
}

const HOMEWORK_LANES: &[HomeworkLaneSpec] = &[
    HomeworkLaneSpec {
        lane: "cs336-a1",
        namespace_suffix: "clean",
        workers: 1,
        validators: 1,
        timeout_seconds: 360,
        min_workers: 1,
        min_validators: 1,
        expected_closeout_stage: Some("rewarded"),
    },
    HomeworkLaneSpec {
        lane: "cs336-a1-replacement-attempt",
        namespace_suffix: "replacement",
        workers: 0,
        validators: 0,
        timeout_seconds: 180,
        min_workers: 0,
        min_validators: 0,
        expected_closeout_stage: None,
    },
    HomeworkLaneSpec {
        lane: "cs336-a1-stale-recovery",
        namespace_suffix: "stale",
        workers: 1,
        validators: 1,
        timeout_seconds: 360,
        min_workers: 1,
        min_validators: 1,
        expected_closeout_stage: Some("rewarded"),
    },
];

impl ProofRunArgs {
    fn parse(lane: &str, args: &[&str]) -> Result<Self, String> {
        let mut parsed = Self {
            lane: lane.to_string(),
            namespace: None,
            workers: None,
            validators: None,
            timeout_seconds: None,
        };
        let mut index = 0;
        while index < args.len() {
            match args[index] {
                "--namespace" => {
                    index += 1;
                    parsed.namespace = Some(required(args, index, "--namespace")?.to_string());
                }
                "--workers" => {
                    index += 1;
                    parsed.workers = Some(parse_u32(required(args, index, "--workers")?)?);
                }
                "--validators" => {
                    index += 1;
                    parsed.validators = Some(parse_u32(required(args, index, "--validators")?)?);
                }
                "--timeout-seconds" => {
                    index += 1;
                    parsed.timeout_seconds =
                        Some(parse_u64(required(args, index, "--timeout-seconds")?)?);
                }
                other => return Err(format!("unsupported proof run flag: {other}")),
            }
            index += 1;
        }
        Ok(parsed)
    }

    fn into_json(self) -> Value {
        json!({
            "lane": self.lane,
            "namespace": self.namespace,
            "workers": self.workers,
            "validators": self.validators,
            "timeoutSeconds": self.timeout_seconds,
        })
    }
}

fn wait_condition(target: &ControlTarget, condition: &str, args: &[&str]) -> Result<Value, String> {
    let namespace = option_value(args, "--namespace");
    let timeout_ms = option_value(args, "--timeout-ms")
        .map(parse_u64)
        .transpose()?
        .unwrap_or(120_000);
    let poll_ms = option_value(args, "--poll-ms")
        .map(parse_u64)
        .transpose()?
        .unwrap_or(1_000);
    let started = Instant::now();
    loop {
        let value = match condition {
            "proof-completed" => {
                let namespace = namespace
                    .ok_or_else(|| "wait proof-completed requires --namespace".to_string())?;
                request_json(
                    target,
                    "POST",
                    "/v1/proof/get",
                    Some(json!({ "namespace": namespace })),
                )?
            }
            "pylon-running" => request_json(target, "GET", "/v1/pylon/status", None)?,
            other => return Err(format!("unsupported wait condition: {other}")),
        };
        if wait_satisfied(condition, &value) {
            return Ok(value);
        }
        if started.elapsed() > Duration::from_millis(timeout_ms) {
            return Err(format!(
                "timed out after {timeout_ms}ms waiting for {condition}"
            ));
        }
        thread::sleep(Duration::from_millis(poll_ms));
    }
}

fn wait_satisfied(condition: &str, value: &Value) -> bool {
    match condition {
        "proof-completed" => value
            .get("status")
            .and_then(Value::as_str)
            .map(|status| !matches!(status, "idle" | "running" | "starting"))
            .unwrap_or(false),
        "pylon-running" => value
            .get("processState")
            .and_then(Value::as_str)
            .map(|state| state == "running")
            .unwrap_or(false),
        _ => false,
    }
}

fn run_smoke(target: &ControlTarget, args: &[&str]) -> Result<Value, String> {
    let namespace = option_value(args, "--namespace")
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("proof.autopilot.ctl.smoke.{}", epoch_seconds()));
    let timeout_ms = option_value(args, "--timeout-ms")
        .map(parse_u64)
        .transpose()?
        .unwrap_or(180_000);
    let mut steps = Vec::new();

    push_step(
        &mut steps,
        "control_status",
        request_json(target, "GET", "/v1/status", None)?,
    );
    push_step(
        &mut steps,
        "pylon_status",
        request_json(target, "GET", "/v1/pylon/status", None)?,
    );
    push_step(
        &mut steps,
        "pylon_start",
        request_json(target, "POST", "/v1/pylon/start", Some(json!({})))?,
    );
    let pylon_mode = request_json(
        target,
        "POST",
        "/v1/pylon/mode",
        Some(json!({ "mode": "offline" })),
    )?;
    let pylon_validation = validate_pylon_configured(&pylon_mode);
    let pylon_ok = pylon_validation
        .get("ok")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    push_step(&mut steps, "pylon_mode_offline", pylon_mode);
    push_step(&mut steps, "pylon_configured_validation", pylon_validation);
    if !pylon_ok {
        return Err(format!(
            "pylon is not configured for smoke: {}",
            serde_json::to_string_pretty(steps.last().unwrap_or(&Value::Null))
                .unwrap_or_else(|_| "validation unavailable".to_string())
        ));
    }
    push_step(
        &mut steps,
        "pylon_stop",
        request_json(target, "POST", "/v1/pylon/stop", Some(json!({})))?,
    );
    push_step(
        &mut steps,
        "proof_run",
        request_json(
            target,
            "POST",
            "/v1/proof/run",
            Some(json!({
                "lane": "cs336-a1-replacement-attempt",
                "namespace": namespace.clone(),
                "workers": 0,
                "validators": 0,
                "timeoutSeconds": 60
            })),
        )?,
    );
    push_step(
        &mut steps,
        "proof_wait",
        wait_condition_for_smoke(target, namespace.as_str(), timeout_ms)?,
    );
    push_step(
        &mut steps,
        "proof_doctor",
        request_json(
            target,
            "POST",
            "/v1/proof/doctor",
            Some(json!({ "namespace": namespace.clone() })),
        )?,
    );
    push_step(
        &mut steps,
        "proof_stop",
        request_json(
            target,
            "POST",
            "/v1/proof/stop",
            Some(json!({ "namespace": namespace.clone() })),
        )?,
    );

    Ok(json!({
        "ok": true,
        "namespace": namespace,
        "target": target.base_url,
        "steps": steps,
    }))
}

fn run_homework_matrix(target: &ControlTarget, args: &[&str]) -> Result<Value, String> {
    let namespace_prefix = option_value(args, "--namespace-prefix")
        .or_else(|| option_value(args, "--namespace"))
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| format!("proof.autopilot.homework.{}", epoch_seconds()));
    let timeout_ms = option_value(args, "--timeout-ms")
        .map(parse_u64)
        .transpose()?
        .unwrap_or(240_000);
    let poll_ms = option_value(args, "--poll-ms")
        .map(parse_u64)
        .transpose()?
        .unwrap_or(1_000);
    let keep_pylon_running = flag_present(args, "--keep-pylon-running");
    let mut steps = Vec::new();
    let mut lanes = Vec::new();
    let mut matrix_ok = true;

    push_step(
        &mut steps,
        "control_status",
        request_json(target, "GET", "/v1/status", None)?,
    );
    push_step(
        &mut steps,
        "pylon_status",
        request_json(target, "GET", "/v1/pylon/status", None)?,
    );
    push_step(
        &mut steps,
        "pylon_start",
        request_json(target, "POST", "/v1/pylon/start", Some(json!({})))?,
    );
    let pylon_mode = request_json(
        target,
        "POST",
        "/v1/pylon/mode",
        Some(json!({ "mode": "offline" })),
    )?;
    let pylon_validation = validate_pylon_configured(&pylon_mode);
    matrix_ok &= pylon_validation
        .get("ok")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    push_step(&mut steps, "pylon_mode_offline", pylon_mode);
    push_step(&mut steps, "pylon_configured_validation", pylon_validation);

    for spec in HOMEWORK_LANES {
        let namespace = format!("{}.{}", namespace_prefix, spec.namespace_suffix);
        let run = request_json(
            target,
            "POST",
            "/v1/proof/run",
            Some(json!({
                "lane": spec.lane,
                "namespace": namespace.clone(),
                "workers": spec.workers,
                "validators": spec.validators,
                "timeoutSeconds": spec.timeout_seconds,
            })),
        )?;
        let timeout = timeout_ms.to_string();
        let poll = poll_ms.to_string();
        let wait_args = [
            "--namespace",
            namespace.as_str(),
            "--timeout-ms",
            timeout.as_str(),
            "--poll-ms",
            poll.as_str(),
        ];
        let completed = wait_condition(target, "proof-completed", &wait_args)?;
        let doctor = request_json(
            target,
            "POST",
            "/v1/proof/doctor",
            Some(json!({ "namespace": namespace.clone() })),
        )?;
        let validation = validate_homework_lane(*spec, &completed, &doctor);
        let lane_ok = validation
            .get("ok")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        matrix_ok &= lane_ok;
        let stop = request_json(
            target,
            "POST",
            "/v1/proof/stop",
            Some(json!({ "namespace": namespace.clone() })),
        )?;
        lanes.push(json!({
            "lane": spec.lane,
            "namespace": namespace,
            "run": run,
            "completed": completed,
            "doctor": doctor,
            "stop": stop,
            "validation": validation,
        }));
    }

    if !keep_pylon_running {
        push_step(
            &mut steps,
            "pylon_stop",
            request_json(target, "POST", "/v1/pylon/stop", Some(json!({})))?,
        );
    }

    let result = json!({
        "ok": matrix_ok,
        "kind": "homework-proof-matrix",
        "namespacePrefix": namespace_prefix,
        "target": target.base_url,
        "lanes": lanes,
        "steps": steps,
    });
    if !matrix_ok {
        return Err(format!(
            "homework proof matrix failed validation: {}",
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string())
        ));
    }
    Ok(result)
}

fn validate_homework_lane(spec: HomeworkLaneSpec, completed: &Value, doctor: &Value) -> Value {
    let mut checks = Vec::new();
    push_check(
        &mut checks,
        "completed",
        completed
            .get("status")
            .and_then(Value::as_str)
            .map(|status| status == "completed")
            .unwrap_or(false),
        str_field(completed, "status"),
    );
    push_check(
        &mut checks,
        "lane",
        completed
            .get("lane")
            .and_then(Value::as_str)
            .map(|lane| lane == spec.lane)
            .unwrap_or(false),
        str_field(completed, "lane"),
    );
    push_check(
        &mut checks,
        "workers",
        max_array_len(completed, doctor, "workers") >= spec.min_workers,
        &format!(
            "{} >= {}",
            max_array_len(completed, doctor, "workers"),
            spec.min_workers
        ),
    );
    push_check(
        &mut checks,
        "validators",
        max_array_len(completed, doctor, "validators") >= spec.min_validators,
        &format!(
            "{} >= {}",
            max_array_len(completed, doctor, "validators"),
            spec.min_validators
        ),
    );
    push_check(
        &mut checks,
        "run_report_artifact",
        artifact_exists_any(completed, doctor, "runReportPath"),
        "runReportPath exists",
    );
    push_check(
        &mut checks,
        "authority_trace_artifact",
        artifact_exists_any(completed, doctor, "authorityTracePath"),
        "authorityTracePath exists",
    );
    push_check(
        &mut checks,
        "proof_summary_artifact",
        artifact_exists_any(completed, doctor, "summaryPath"),
        "summaryPath exists",
    );
    push_check(
        &mut checks,
        "object_trace_artifact",
        artifact_exists_any(completed, doctor, "artifactTracePath"),
        "artifactTracePath exists",
    );
    let (transport_ok, transport_detail) = transport_acceptable(spec, completed, doctor);
    push_check(
        &mut checks,
        "transport_split",
        transport_ok,
        transport_detail.as_str(),
    );
    if let Some(expected) = spec.expected_closeout_stage {
        let stage = completed
            .get("closeoutStage")
            .and_then(Value::as_str)
            .unwrap_or("none");
        let detail = completed
            .get("detail")
            .and_then(Value::as_str)
            .unwrap_or("");
        push_check(
            &mut checks,
            "closeout_stage",
            stage == expected || detail.contains(&format!("closeout={expected}")),
            &format!("stage={stage} detail={detail}"),
        );
    }
    let ok = checks
        .iter()
        .all(|check| check.get("ok").and_then(Value::as_bool).unwrap_or(false));
    json!({
        "ok": ok,
        "checks": checks,
    })
}

fn validate_pylon_configured(status: &Value) -> Value {
    let blockers = status
        .get("blockerCodes")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let configured = status
        .get("configured")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let missing_setup = blockers
        .iter()
        .any(|code| matches!(code.as_str(), "CONFIG_MISSING" | "IDENTITY_MISSING"));
    let ok = configured && !missing_setup;
    json!({
        "ok": ok,
        "configured": configured,
        "providerState": str_field(status, "providerState"),
        "configPath": str_field(status, "configPath"),
        "pylonHome": str_field(status, "pylonHome"),
        "blockerCodes": blockers,
    })
}

fn push_check(checks: &mut Vec<Value>, name: &str, ok: bool, detail: &str) {
    checks.push(json!({
        "name": name,
        "ok": ok,
        "detail": detail,
    }));
}

fn push_step(steps: &mut Vec<Value>, name: &str, value: Value) {
    steps.push(json!({
        "name": name,
        "result": value,
    }));
}

fn request_json(
    target: &ControlTarget,
    method: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value, String> {
    let payload = body
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| format!("failed to encode request JSON: {error}"))?
        .unwrap_or_default();
    let mut stream = TcpStream::connect((target.host.as_str(), target.port)).map_err(|error| {
        format!(
            "failed to connect to Autopilot Tauri control plane at {}: {error}",
            target.base_url
        )
    })?;
    let request = format!(
        "{method} {path} HTTP/1.1\r\nhost: {}\r\nauthorization: Bearer {}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
        target.host,
        target.auth_token,
        payload.len(),
        payload
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| format!("failed to write control request: {error}"))?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("failed to read control response: {error}"))?;
    parse_response(response.as_slice())
}

fn parse_response(response: &[u8]) -> Result<Value, String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "control response missing HTTP header terminator".to_string())?;
    let header = String::from_utf8_lossy(&response[..header_end]);
    let status_line = header
        .lines()
        .next()
        .ok_or_else(|| "control response missing status line".to_string())?;
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(500);
    let body = &response[header_end + 4..];
    let value: Value = serde_json::from_slice(body)
        .map_err(|error| format!("failed to decode control response JSON: {error}"))?;
    if status >= 400 {
        return Err(value
            .get("error")
            .and_then(Value::as_str)
            .unwrap_or("control request failed")
            .to_string());
    }
    Ok(value)
}

fn print_value(value: &Value, json_output: bool) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        );
        return;
    }
    if let Some(status) = value
        .get("pylonStatus")
        .or_else(|| value.get("pylon_status"))
    {
        print_pylon_status(status);
    } else if value.get("processState").is_some() || value.get("providerState").is_some() {
        print_pylon_status(value);
    } else if value.get("namespace").is_some() && value.get("status").is_some() {
        print_proof_status(value);
    } else {
        println!(
            "{}",
            serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string())
        );
    }
}

fn print_pylon_status(value: &Value) {
    println!(
        "pylon process={} provider={} binary={} config={}",
        str_field(value, "processState"),
        str_field(value, "providerState"),
        str_field(value, "binaryPath"),
        str_field(value, "configPath")
    );
}

fn print_proof_status(value: &Value) {
    println!(
        "proof namespace={} lane={} status={} first_red={} detail={}",
        str_field(value, "namespace"),
        str_field(value, "lane"),
        str_field(value, "status"),
        str_field(value, "firstRedStage"),
        str_field(value, "detail")
    );
}

fn str_field<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or("none")
}

fn array_len(value: &Value, key: &str) -> usize {
    value
        .get(key)
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0)
}

fn max_array_len(left: &Value, right: &Value, key: &str) -> usize {
    array_len(left, key).max(array_len(right, key))
}

fn artifact_exists(value: &Value, key: &str) -> bool {
    let Some(path) = value
        .get("artifacts")
        .and_then(|artifacts| artifacts.get(key))
        .and_then(Value::as_str)
    else {
        return false;
    };
    fs::metadata(path).is_ok()
}

fn artifact_exists_any(left: &Value, right: &Value, key: &str) -> bool {
    artifact_exists(left, key) || artifact_exists(right, key)
}

fn transport_component_ok(left: &Value, right: &Value, key: &str) -> bool {
    [left, right].iter().any(|value| {
        value
            .get("transport")
            .and_then(|transport| transport.get(key))
            .and_then(Value::as_str)
            .map(|status| status == "ok")
            .unwrap_or(false)
    })
}

fn transport_status(left: &Value, right: &Value, key: &str) -> String {
    for value in [right, left] {
        if let Some(status) = value
            .get("transport")
            .and_then(|transport| transport.get(key))
            .and_then(Value::as_str)
        {
            if status != "unknown" {
                return status.to_string();
            }
        }
    }
    "unknown".to_string()
}

fn transport_acceptable(
    spec: HomeworkLaneSpec,
    completed: &Value,
    doctor: &Value,
) -> (bool, String) {
    let authority = transport_component_ok(completed, doctor, "authority");
    let relay = transport_component_ok(completed, doctor, "relay");
    let artifact_store = transport_component_ok(completed, doctor, "artifactStore");
    let node_status = transport_status(completed, doctor, "nodeSurfaces");
    let detail = completed
        .get("detail")
        .and_then(Value::as_str)
        .or_else(|| doctor.get("detail").and_then(Value::as_str))
        .unwrap_or("");
    let has_expected_nodes = spec.min_workers + spec.min_validators > 0;
    let node_surfaces = node_status == "ok"
        || (!has_expected_nodes && matches!(node_status.as_str(), "unknown" | "down"))
        || (node_status == "down"
            && detail.contains("workers_quiesced")
            && detail.contains("validators_quiesced"));
    (
        authority && relay && artifact_store && node_surfaces,
        format!(
            "authority={} relay={} artifactStore={} nodeSurfaces={} detail={}",
            transport_status(completed, doctor, "authority"),
            transport_status(completed, doctor, "relay"),
            transport_status(completed, doctor, "artifactStore"),
            node_status,
            detail
        ),
    )
}

fn wait_condition_for_smoke(
    target: &ControlTarget,
    namespace: &str,
    timeout_ms: u64,
) -> Result<Value, String> {
    let timeout = timeout_ms.to_string();
    let args = ["--namespace", namespace, "--timeout-ms", timeout.as_str()];
    wait_condition(target, "proof-completed", &args)
}

fn required<'a>(args: &'a [&str], index: usize, flag: &str) -> Result<&'a str, String> {
    args.get(index)
        .copied()
        .ok_or_else(|| format!("{flag} requires a value"))
}

fn option_value<'a>(args: &'a [&str], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|pair| pair.first().copied() == Some(flag))
        .and_then(|pair| pair.get(1))
        .copied()
}

fn flag_present(args: &[&str], flag: &str) -> bool {
    args.iter().any(|value| *value == flag)
}

fn parse_u32(value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|error| format!("invalid integer {value}: {error}"))
}

fn parse_u64(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid integer {value}: {error}"))
}

fn epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn print_usage() {
    println!(
        "Usage: autopilotctl-tauri [--manifest <path>|--base-url <url> --auth-token <token>] [--json] <command>\n\nCommands:\n  status\n  pylon status|start|stop|restart|logs|mode <online|offline|pause|resume>\n  proof status [namespace]\n  proof run <lane> [--namespace <ns>] [--workers <n>] [--validators <n>] [--timeout-seconds <n>]\n  proof matrix [--namespace-prefix <ns>] [--timeout-ms <n>] [--poll-ms <n>]\n  proof doctor|stop|reset|artifacts <namespace>\n  homework matrix [--namespace-prefix <ns>] [--timeout-ms <n>] [--poll-ms <n>]\n  wait proof-completed --namespace <ns> [--timeout-ms <n>] [--poll-ms <n>]\n  wait pylon-running [--timeout-ms <n>] [--poll-ms <n>]\n  smoke [--namespace <ns>] [--timeout-ms <n>]\n\nDefault manifest: {}",
        control_manifest_path().display()
    );
}
