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
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

const DOCKER_BIN_TIMEOUT: Duration = Duration::from_secs(5);
const DOCKER_DAEMON_TIMEOUT: Duration = Duration::from_secs(10);
const CANARY_TIMEOUT: Duration = Duration::from_secs(25);
const HARBOR_TIMEOUT: Duration = Duration::from_secs(10);
const DISK_MIN_BYTES: u64 = 5u64 * 1024 * 1024 * 1024;
const HARBOR_RUNNER_IMAGE: &str = "openagents/harbor-runner:latest";
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
    let checks = probe_environment().await;
    match action {
        EnvAction::Probe => {
            if json {
                let report = env_report(&checks);
                match serde_json::to_string(&report) {
                    Ok(text) => println!("{text}"),
                    Err(error) => {
                        eprintln!("openagents gym env probe: could not encode JSON: {error}");
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
                        eprintln!("openagents gym env doctor: could not encode JSON: {error}");
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
            "B3 harbor-runner image has not landed",
            "Run `gym env pull` after issue #174 lands",
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

async fn check_harbor_runner_image() -> GymCheck {
    match timeout(
        HARBOR_TIMEOUT,
        Command::new("docker")
            .args(["image", "inspect", HARBOR_RUNNER_IMAGE])
            .output(),
    )
    .await
    {
        Ok(Ok(output)) if output.status.success() => ok(
            "harbor_runner_image",
            format!("{HARBOR_RUNNER_IMAGE} is present"),
        ),
        _ => not_yet_built(
            "harbor_runner_image",
            "B3 harbor-runner image has not landed",
            "Run `gym env pull` after issue #174 lands",
        ),
    }
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
                Some("Run `gym env pull` after issue #174 lands"),
            ),
        ];
        let (lines, exit) = diagnose(&checks);
        assert_eq!(exit, 1);
        let text = lines.join("\n");
        assert!(
            text.contains("Run `gym env pull` after issue #174 lands"),
            "{text}"
        );
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
}
