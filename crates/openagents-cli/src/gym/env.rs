use crate::auth::home_directory;
use crate::gym::schemas::{EnvCheck, EnvReport};
use clap::{Args, Subcommand};

/// Arguments for `openagents gym env`.
#[derive(Args, Debug)]
pub struct EnvArgs {
    #[command(subcommand)]
    pub action: EnvAction,
}

/// Subcommands for `openagents gym env`.
#[derive(Subcommand, Debug)]
pub enum EnvAction {
    /// Probe this machine's Gym environment.
    Probe,
    /// Probe and print the exact fix for every failure.
    Doctor,
    /// Build or pull the pinned harbor-runner image.
    Pull,
}

/// The tri-state verdict of one check, before the frozen contract's pass/fail.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckState {
    Ok,
    Failed,
    NotYetBuilt,
    Warning,
}

impl std::fmt::Display for CheckState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            CheckState::Ok => "ok",
            CheckState::Failed => "failed",
            CheckState::NotYetBuilt => "not_yet_built",
            CheckState::Warning => "warning",
        };
        f.write_str(label)
    }
}
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

const DOCKER_BIN_TIMEOUT: Duration = Duration::from_secs(5);
const DOCKER_DAEMON_TIMEOUT: Duration = Duration::from_secs(10);
const CANARY_TIMEOUT: Duration = Duration::from_secs(25);
const HARBOR_TIMEOUT: Duration = Duration::from_secs(10);
const PULL_TIMEOUT: Duration = Duration::from_secs(600);
const DISK_MIN_BYTES: u64 = 5u64 * 1024 * 1024 * 1024;
const HARBOR_RUNNER_VERSION: &str = "0.1.0";
pub const HARBOR_RUNNER_IMAGE: &str = "harbor-runner:0.1.0";
pub const HARBOR_PULL_REMEDY: &str = "Run `openagents gym env pull`";
const CANARY_IMAGE: &str = "hello-world";

/// One check with the internal remedy used by `doctor`.
#[derive(Debug, Clone)]
pub struct GymCheck {
    pub name: String,
    pub state: CheckState,
    pub detail: String,
    pub remedy: Option<String>,
}

impl GymCheck {
    pub fn to_env_check(&self) -> EnvCheck {
        EnvCheck {
            name: self.name.clone(),
            passed: self.state == CheckState::Ok || self.state == CheckState::Warning,
            required: true,
            remedy: self.remedy.clone(),
            observed: Some(self.detail.clone()),
        }
    }
}

fn ok(name: &str, detail: impl Into<String>) -> GymCheck {
    GymCheck {
        name: name.to_string(),
        state: CheckState::Ok,
        detail: detail.into(),
        remedy: None,
    }
}

fn failed(name: &str, detail: impl Into<String>, remedy: impl Into<String>) -> GymCheck {
    GymCheck {
        name: name.to_string(),
        state: CheckState::Failed,
        detail: detail.into(),
        remedy: Some(remedy.into()),
    }
}

fn not_yet_built(name: &str, detail: impl Into<String>, remedy: impl Into<String>) -> GymCheck {
    GymCheck {
        name: name.to_string(),
        state: CheckState::NotYetBuilt,
        detail: detail.into(),
        remedy: Some(remedy.into()),
    }
}

fn warning(name: &str, detail: impl Into<String>, remedy: impl Into<String>) -> GymCheck {
    GymCheck {
        name: name.to_string(),
        state: CheckState::Warning,
        detail: detail.into(),
        remedy: Some(remedy.into()),
    }
}

pub fn env_report(checks: &[GymCheck]) -> EnvReport {
    let sufficient = checks
        .iter()
        .all(|check| check.state == CheckState::Ok || check.state == CheckState::Warning);
    EnvReport {
        schema: crate::gym::schemas::ENV_REPORT_SCHEMA.to_string(),
        target: "local".to_string(),
        sufficient_for_scored_run: sufficient,
        generated_at: crate::computer::now_iso8601(),
        checks: checks.iter().map(GymCheck::to_env_check).collect(),
    }
}

/// Print the report and exit as the command requires.
pub async fn run(action: EnvAction, json: bool) {
    match action {
        EnvAction::Pull => run_pull(json).await,
        _ => {
            let checks = probe_environment().await;
            match action {
                EnvAction::Probe => {
                    if json {
                        let report = env_report(&checks);
                        match serde_json::to_string(&report) {
                            Ok(text) => println!("{text}"),
                            Err(error) => {
                                eprintln!(
                                    "openagents gym env probe: could not encode JSON: {error}"
                                );
                                std::process::exit(1);
                            }
                        }
                    } else {
                        for check in &checks {
                            println!("{}: {} — {}", check.name, check.state, check.detail);
                        }
                    }
                }
                EnvAction::Doctor => {
                    let (lines, exit) = diagnose(&checks);
                    if json {
                        let report = env_report(&checks);
                        match serde_json::to_string(&report) {
                            Ok(text) => println!("{text}"),
                            Err(error) => {
                                eprintln!(
                                    "openagents gym env doctor: could not encode JSON: {error}"
                                );
                                std::process::exit(1);
                            }
                        }
                    } else {
                        for line in &lines {
                            println!("{line}");
                        }
                    }
                    let _ = std::io::Write::flush(&mut std::io::stdout());
                    if exit != 0 {
                        std::process::exit(exit);
                    }
                }
                EnvAction::Pull => unreachable!(),
            }
        }
    }
}

/// Probe this machine for everything a scored Gym run needs.
pub async fn probe_environment() -> Vec<GymCheck> {
    let mut checks = Vec::new();
    checks.push(host_check());

    let docker_bin = check_docker_binary().await;
    let docker_available = docker_bin.state == CheckState::Ok;
    checks.push(docker_bin);

    checks.push(if docker_available {
        check_docker_daemon().await
    } else {
        failed(
            "docker_daemon",
            "Docker client is not installed",
            "Install Docker for your platform and start the Docker daemon",
        )
    });

    let daemon_reachable = docker_available
        && checks
            .iter()
            .any(|c| c.name == "docker_daemon" && c.state == CheckState::Ok);

    if docker_available && daemon_reachable {
        checks.push(run_amd64_canary().await);
        checks.push(check_harbor_runner_image().await);
    } else {
        checks.push(failed(
            "amd64_canary",
            "Docker is not available to run the amd64 canary",
            "Enable Rosetta in Docker Desktop Settings → General, then start Docker",
        ));
        checks.push(not_yet_built(
            "harbor_runner_image",
            "Docker is not available to inspect the harbor-runner image",
            HARBOR_PULL_REMEDY,
        ));
    }

    checks.push(check_disk_headroom().await);
    checks
}

fn host_check() -> GymCheck {
    let host = crate::computer::probe_host();
    ok(
        "host",
        format!(
            "{} {} ({} CPUs, {} MB)",
            host.os, host.arch, host.num_cpus, host.total_memory_mb
        ),
    )
}

/// Return the human lines and the exit code. Exit 0 means a scored run could
/// happen now; exit 1 means a prereq is missing and the fix is printed.
pub fn diagnose(checks: &[GymCheck]) -> (Vec<String>, i32) {
    let mut lines = Vec::new();
    let mut all_ok = true;
    for check in checks {
        lines.push(format!(
            "{}: {} — {}",
            check.name, check.state, check.detail
        ));
        match check.state {
            CheckState::Ok => {}
            CheckState::Failed | CheckState::Warning | CheckState::NotYetBuilt => {
                all_ok = false;
                if let Some(remedy) = &check.remedy {
                    lines.push(format!("  Fix: {remedy}"));
                }
            }
        }
    }
    let exit = if all_ok { 0 } else { 1 };
    (lines, exit)
}

async fn check_docker_binary() -> GymCheck {
    match timeout(
        DOCKER_BIN_TIMEOUT,
        Command::new("docker").arg("--version").output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => {
            let text = String::from_utf8_lossy(&output.stdout);
            ok(
                "docker_binary",
                format!("Docker client found: {}", text.trim()),
            )
        }
        Ok(Ok(output)) => failed(
            "docker_binary",
            format!("Docker client returned exit {}", output.status),
            "Install Docker for your platform",
        ),
        Ok(Err(error)) => failed(
            "docker_binary",
            format!("Docker client is not on PATH: {error}"),
            "Install Docker for your platform",
        ),
        Err(_) => failed(
            "docker_binary",
            "Docker client check timed out",
            "Install Docker for your platform",
        ),
    }
}

async fn check_docker_daemon() -> GymCheck {
    match timeout(
        DOCKER_DAEMON_TIMEOUT,
        Command::new("docker").arg("version").output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => {
            ok("docker_daemon", "Docker daemon is reachable")
        }
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            failed(
                "docker_daemon",
                format!("Docker daemon is not reachable: {stderr}")
                    .trim()
                    .to_string(),
                "Start the Docker daemon",
            )
        }
        Ok(Err(error)) => failed(
            "docker_daemon",
            format!("Could not run `docker version`: {error}"),
            "Start the Docker daemon",
        ),
        Err(_) => failed(
            "docker_daemon",
            "Docker daemon check timed out",
            "Start the Docker daemon",
        ),
    }
}

async fn run_amd64_canary() -> GymCheck {
    let output = timeout(
        CANARY_TIMEOUT,
        Command::new("docker")
            .args(["run", "--rm", "--platform", "linux/amd64", CANARY_IMAGE])
            .output(),
    )
    .await;

    match output {
        Ok(Ok(output)) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if stdout.contains("Hello from Docker!") {
                ok(
                    "amd64_canary",
                    "linux/amd64 hello-world ran cleanly (Rosetta/qemu working)",
                )
            } else {
                failed(
                    "amd64_canary",
                    "linux/amd64 hello-world ran but did not emit the expected greeting",
                    "Enable Rosetta in Docker Desktop Settings → General",
                )
            }
        }
        Ok(Ok(output)) => {
            let stderr = String::from_utf8_lossy(&output.stderr);
            failed(
                "amd64_canary",
                format!(
                    "linux/amd64 canary failed (exit {}): {stderr}",
                    output.status.code().unwrap_or(-1)
                )
                .trim()
                .to_string(),
                "Enable Rosetta in Docker Desktop Settings → General",
            )
        }
        Ok(Err(error)) => failed(
            "amd64_canary",
            format!("Could not start the amd64 canary: {error}"),
            "Enable Rosetta in Docker Desktop Settings → General",
        ),
        Err(_) => failed(
            "amd64_canary",
            "amd64 canary timed out (qemu/Rosetta may be misconfigured)",
            "Enable Rosetta in Docker Desktop Settings → General",
        ),
    }
}

fn harbor_runner_image_check(present: bool, detail: &str) -> GymCheck {
    if present {
        ok("harbor_runner_image", detail)
    } else {
        not_yet_built("harbor_runner_image", detail, HARBOR_PULL_REMEDY)
    }
}

async fn check_harbor_runner_image() -> GymCheck {
    match timeout(
        HARBOR_TIMEOUT,
        Command::new("docker")
            .args(["image", "inspect", HARBOR_RUNNER_IMAGE])
            .output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => {
            let id = docker_image_id_from_inspect(&output.stdout);
            harbor_runner_image_check(true, &format!("{HARBOR_RUNNER_IMAGE} is present ({id})"))
        }
        _ => harbor_runner_image_check(false, &format!("{HARBOR_RUNNER_IMAGE} is not present")),
    }
}

fn docker_image_id_from_inspect(output: &[u8]) -> String {
    // `docker image inspect` emits a JSON array; trim to a short id for the
    // observed detail without pulling in a JSON parser dependency.
    let text = String::from_utf8_lossy(output);
    text.lines()
        .find(|l| l.contains("\"Id\":"))
        .map(|l| {
            l.split("\"")
                .nth(3)
                .map(|s| s.chars().take(19).collect::<String>())
                .unwrap_or_default()
        })
        .unwrap_or_default()
}

async fn check_disk_headroom() -> GymCheck {
    let home = home_directory();
    let jobs_dir = home.join(".openagents/gym/jobs");
    let probe_path = if jobs_dir.is_dir() { jobs_dir } else { home };

    match disk_available_bytes(&probe_path) {
        None => warning(
            "disk_headroom",
            "Could not measure free disk space on this platform",
            "Ensure the volume for job directories has at least 5 GB free",
        ),
        Some(free) if free >= DISK_MIN_BYTES => ok(
            "disk_headroom",
            format!("{} GB free", free / 1024 / 1024 / 1024),
        ),
        Some(free) => {
            let needed = (DISK_MIN_BYTES - free).div_ceil(1024 * 1024 * 1024);
            failed(
                "disk_headroom",
                format!(
                    "{} GB free; {} GB more needed",
                    free / 1024 / 1024 / 1024,
                    needed
                ),
                format!("Free at least {needed} GB on the job-directory volume"),
            )
        }
    }
}

#[cfg(unix)]
fn disk_available_bytes(path: &Path) -> Option<u64> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut buf: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut buf) } != 0 {
        return None;
    }
    let free = buf.f_bavail as u64 * buf.f_frsize as u64;
    Some(free)
}

#[cfg(not(unix))]
fn disk_available_bytes(_path: &Path) -> Option<u64> {
    None
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct HarborRunnerState {
    image_id: String,
    context_digest: String,
    tag: String,
    built_at: String,
}

fn harbor_runner_state_dir() -> PathBuf {
    home_directory().join(".openagents/gym/harbor-runner")
}

fn harbor_runner_state_path() -> PathBuf {
    harbor_runner_state_dir().join("digest.json")
}

fn read_harbor_runner_state() -> Option<HarborRunnerState> {
    let path = harbor_runner_state_path();
    let text = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_harbor_runner_state(state: &HarborRunnerState) -> Result<(), String> {
    let dir = harbor_runner_state_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(harbor_runner_state_path(), text).map_err(|e| e.to_string())
}

fn find_repo_root() -> Result<PathBuf, String> {
    let mut dir = std::env::current_dir().map_err(|e| e.to_string())?;
    loop {
        let marker = dir.join("docker/harbor-runner/Dockerfile");
        if marker.is_file() {
            return Ok(dir);
        }
        match dir.parent() {
            Some(parent) => dir = parent.to_path_buf(),
            None => return Err("could not find docker/harbor-runner/Dockerfile".to_string()),
        }
    }
}

fn hash_bytes_to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut out = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(out, "{b:02x}");
    }
    out
}

fn compute_context_digest(repo_root: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    let mut hasher = Sha256::new();
    let inputs = [
        "docker/harbor-runner/Dockerfile",
        "docker/harbor-runner/requirements.txt",
        "docker/harbor-runner/build.sh",
        "docker/harbor-runner/VERSION",
        "bench/adapters/openagents_coder.py",
        "bench/post_gym_run.py",
        "bench/run-suite.sh",
    ];
    for rel in inputs {
        let path = repo_root.join(rel);
        let bytes = std::fs::read(&path).map_err(|e| format!("{rel}: {e}"))?;
        hasher.update(rel.as_bytes());
        hasher.update(&bytes);
    }

    let suites = repo_root.join("bench/suites");
    if suites.is_dir() {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(&suites)
            .map_err(|e| format!("bench/suites: {e}"))?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .collect();
        entries.sort();
        for entry in entries {
            let rel = entry
                .strip_prefix(repo_root)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .into_owned();
            if entry.is_file() {
                let bytes = std::fs::read(&entry).map_err(|e| format!("{rel}: {e}"))?;
                hasher.update(rel.as_bytes());
                hasher.update(&bytes);
            }
        }
    }

    let digest = hasher.finalize();
    Ok(format!("sha256:{}", hash_bytes_to_hex(&digest)))
}

/// True when `docker image inspect` finds the pinned harbor-runner tag.
pub async fn harbor_runner_image_present() -> bool {
    current_image_id().await.is_some()
}

async fn current_image_id() -> Option<String> {
    let output = timeout(
        DOCKER_BIN_TIMEOUT,
        Command::new("docker")
            .args([
                "image",
                "inspect",
                HARBOR_RUNNER_IMAGE,
                "--format",
                "{{.Id}}",
            ])
            .output(),
    )
    .await
    .ok()?;
    let output = output.ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

async fn build_harbor_runner_image(repo_root: &Path) -> Result<String, String> {
    let dockerfile = repo_root.join("docker/harbor-runner/Dockerfile");
    let mut cmd = Command::new("docker");
    cmd.arg("build")
        .arg("-f")
        .arg(dockerfile.as_os_str())
        .arg("-t")
        .arg(HARBOR_RUNNER_IMAGE)
        .arg("--build-arg")
        .arg(format!("HARBOR_RUNNER_VERSION={HARBOR_RUNNER_VERSION}"))
        .arg(repo_root.as_os_str());
    let output = timeout(PULL_TIMEOUT, cmd.output())
        .await
        .map_err(|_| "docker build timed out".to_string())?
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("docker build failed: {stderr}"));
    }
    current_image_id()
        .await
        .ok_or_else(|| "could not inspect image after build".to_string())
}

async fn run_pull(json: bool) {
    let repo_root = match find_repo_root() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("openagents gym env pull: {error}");
            std::process::exit(1);
        }
    };

    let context_digest = match compute_context_digest(&repo_root) {
        Ok(d) => d,
        Err(error) => {
            eprintln!("openagents gym env pull: {error}");
            std::process::exit(1);
        }
    };

    let current_id = current_image_id().await;
    let saved = read_harbor_runner_state();

    if let (Some(state), Some(id)) = (saved, current_id.as_ref()) {
        if state.context_digest == context_digest && state.image_id == *id {
            if json {
                let up_to_date = serde_json::json!({
                    "schema": "openagents.gym.harbor_runner_pull.v1",
                    "tag": state.tag,
                    "image_id": state.image_id,
                    "context_digest": state.context_digest,
                    "built_at": state.built_at,
                    "up_to_date": true,
                });
                println!("{}", serde_json::to_string(&up_to_date).unwrap());
            } else {
                println!("{} is up to date", state.tag);
            }
            return;
        }
    }

    let image_id = match build_harbor_runner_image(&repo_root).await {
        Ok(id) => id,
        Err(error) => {
            eprintln!("openagents gym env pull: {error}");
            std::process::exit(1);
        }
    };

    let state = HarborRunnerState {
        image_id,
        context_digest,
        tag: HARBOR_RUNNER_IMAGE.to_string(),
        built_at: crate::computer::now_iso8601(),
    };

    if let Err(error) = write_harbor_runner_state(&state) {
        eprintln!("openagents gym env pull: could not record state: {error}");
        std::process::exit(1);
    }

    if json {
        let value = serde_json::json!({
            "schema": "openagents.gym.harbor_runner_pull.v1",
            "tag": state.tag,
            "image_id": state.image_id,
            "context_digest": state.context_digest,
            "built_at": state.built_at,
            "up_to_date": false,
        });
        println!("{}", serde_json::to_string(&value).unwrap());
    } else {
        println!(
            "Built {} ({}) and recorded digest",
            state.tag, state.image_id
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn check(name: &str, state: CheckState, detail: &str, remedy: Option<&str>) -> GymCheck {
        GymCheck {
            name: name.to_string(),
            state,
            detail: detail.to_string(),
            remedy: remedy.map(|s| s.to_string()),
        }
    }

    #[test]
    fn doctor_exits_zero_when_all_checks_pass() {
        let checks = vec![
            check("host", CheckState::Ok, "macos arm64", None),
            check("docker_binary", CheckState::Ok, "found", None),
            check("amd64_canary", CheckState::Ok, "ok", None),
            check("disk_headroom", CheckState::Ok, "10 GB free", None),
        ];
        let (lines, exit) = diagnose(&checks);
        assert_eq!(exit, 0);
        assert!(lines.iter().all(|l| !l.contains("Fix:")));
    }

    #[test]
    fn doctor_exits_one_and_names_remedy_for_failed_check() {
        let checks = vec![
            check("host", CheckState::Ok, "macos arm64", None),
            check(
                "amd64_canary",
                CheckState::Failed,
                "segfault under qemu",
                Some("Enable Rosetta in Docker Desktop Settings → General"),
            ),
            check("disk_headroom", CheckState::Ok, "10 GB free", None),
        ];
        let (lines, exit) = diagnose(&checks);
        assert_eq!(exit, 1);
        let text = lines.join("\n");
        assert!(
            text.contains("Enable Rosetta in Docker Desktop Settings → General"),
            "{text}"
        );
    }

    #[test]
    fn doctor_exits_one_and_names_pull_for_not_yet_built() {
        let checks = vec![
            check("host", CheckState::Ok, "macos arm64", None),
            check(
                "harbor_runner_image",
                CheckState::NotYetBuilt,
                "B3 not landed",
                Some(HARBOR_PULL_REMEDY),
            ),
        ];
        let (lines, exit) = diagnose(&checks);
        assert_eq!(exit, 1);
        let text = lines.join("\n");
        assert!(text.contains("Run `openagents gym env pull`"), "{text}");
        assert!(!text.contains("after issue #174 lands"), "{text}");
    }

    #[test]
    fn env_report_json_shape_is_pinned() {
        let checks = vec![
            GymCheck {
                name: "docker_binary".to_string(),
                state: CheckState::Ok,
                detail: "Docker client found".to_string(),
                remedy: None,
            },
            GymCheck {
                name: "amd64_canary".to_string(),
                state: CheckState::Failed,
                detail: "segfault".to_string(),
                remedy: Some("Enable Rosetta".to_string()),
            },
        ];
        let report = env_report(&checks);
        assert_eq!(report.schema, "openagents.gym.env_report.v1");
        let json = serde_json::to_string(&report).expect("serialize");
        assert!(
            json.contains(r#""schema":"openagents.gym.env_report.v1""#),
            "{json}"
        );
        assert!(json.contains(r#""name":"docker_binary""#), "{json}");
        // The frozen v1 contract: pass/fail plus observed detail, not a state enum.
        assert!(json.contains(r#""passed":true"#), "{json}");
        assert!(json.contains(r#""passed":false"#), "{json}");
        assert!(
            json.contains(r#""observed":"Docker client found""#),
            "{json}"
        );
        assert!(json.contains(r#""remedy":"Enable Rosetta""#), "{json}");
        assert!(
            json.contains(r#""sufficient_for_scored_run":false"#),
            "{json}"
        );
    }

    #[test]
    fn harbor_runner_image_check_passes_and_fails() {
        let ok = harbor_runner_image_check(true, "harbor-runner:0.1.0 is present");
        assert_eq!(ok.state, CheckState::Ok);
        assert_eq!(ok.name, "harbor_runner_image");
        assert!(ok.remedy.is_none());

        let not = harbor_runner_image_check(false, "harbor-runner:0.1.0 is not present");
        assert_eq!(not.state, CheckState::NotYetBuilt);
        assert_eq!(not.remedy.as_deref(), Some(HARBOR_PULL_REMEDY));
    }

    #[test]
    fn docker_image_id_from_inspect_parses_id() {
        let sample = br#"[{"Id":"sha256:10c58d78690f6871567e73b8d907ea08a36bc0efa91c0c91fce3f690692a647a"}]"#;
        let id = docker_image_id_from_inspect(sample);
        assert!(id.starts_with("sha256:"));
        assert!(id.len() < 40);
    }

    #[test]
    fn context_digest_is_stable_for_fixtures() {
        // The command is expected to run from the repo root. This test only
        // runs when that root is the current directory; otherwise it is skipped.
        let repo_root = std::env::current_dir()
            .ok()
            .filter(|d| d.join("docker/harbor-runner/Dockerfile").is_file());
        if repo_root.is_none() {
            return;
        }
        let repo_root = repo_root.unwrap();
        let d1 = compute_context_digest(&repo_root).expect("digest");
        let d2 = compute_context_digest(&repo_root).expect("digest");
        assert_eq!(d1, d2, "context digest is deterministic");
        assert!(d1.starts_with("sha256:"));
    }
}
