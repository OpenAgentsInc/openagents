//! Running the loop on a Terminal-Bench 4 task: find the task, start its
//! environment, run the loop with commands sent in by `docker exec`, then
//! run the task's own tests and read the reward.
//!
//! Microcoder runs a task the way Harbor does. Harbor graded the Fable 5.1
//! reference runs, and its Docker environment (Harbor 0.22,
//! `harbor/environments/docker/docker.py` and `harbor/trial/trial.py`) is
//! the spec this module follows:
//!
//! - Every task runs as a Docker Compose project. Its `main` service is the
//!   agent's container: the task's image, started with
//!   `sh -c "sleep infinity"` as the command so the image's own entrypoint
//!   still runs, with `[environment].env` set and `/logs/verifier`,
//!   `/logs/agent`, and `/logs/artifacts` bind-mounted from the host. A
//!   task's `environment/docker-compose.yaml` is layered on top, so its
//!   services start beside `main` and are reachable by service name.
//! - Without network access, a task without a Compose file runs `main` with
//!   no network. A task with one puts `main` and each service that doesn't
//!   choose its own network in one shared network namespace on an internal
//!   network with no route out, as Harbor's egress sidecar does; each
//!   service name is an alias there too. Images build with the network on.
//! - A separate verifier is its own Compose project built from `tests/`,
//!   with the network its own policy gives it. `[[verifier.collect]]`
//!   commands run in the agent's services first, and then the task's
//!   artifacts and `/logs/artifacts` are copied in.
//! - The tests run as `(/tests/test.sh) > /logs/verifier/test-stdout.txt`,
//!   as the verifier's user. The reward is `/logs/verifier/reward.json`
//!   when it exists (its `reward` key, or its only key), or else
//!   `reward.txt`. Anything else, and a verifier that runs past its time
//!   limit, leaves the reward unknown, with the reason.
//!
//! Not modeled: CPU and memory limits, artifact `exclude` patterns, a
//! shared verifier whose network policy differs from the agent's,
//! multi-step tasks, and an allowlist network policy, which runs with the
//! network on.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{Map, Value, json};
use tokio::process::Command;

use crate::env::{Docker, Env, UMASK};

/// The default Terminal-Bench 4 task folder.
#[must_use]
pub fn tasks_dir() -> PathBuf {
    if let Some(dir) = std::env::var_os("MICROCODER_TASKS") {
        return PathBuf::from(dir);
    }
    PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
        .join(".openagents/terminal-bench/upstream/terminal-bench-v4.0.0/tasks")
}

/// A network policy from `task.toml`: Harbor's `network_mode`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Network {
    #[default]
    Public,
    NoNetwork,
    /// Only the listed hosts. Microcoder can't enforce a list, so this
    /// runs with the network on.
    Allowlist,
}

impl Network {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "public" => Ok(Self::Public),
            "no-network" => Ok(Self::NoNetwork),
            "allowlist" => Ok(Self::Allowlist),
            other => Err(format!("unknown network_mode {other:?}")),
        }
    }

    /// The Docker network this policy runs on: `none` without access, and
    /// `bridge` otherwise.
    #[must_use]
    pub fn docker(self) -> &'static str {
        if self == Self::NoNetwork {
            "none"
        } else {
            "bridge"
        }
    }
}

/// An `[environment]` table, or a verifier's `[verifier.environment]`.
#[derive(Clone, Debug, Default)]
pub struct Environment {
    /// The baseline policy: `network_mode`, or the older `allow_internet`.
    pub network: Network,
    /// `env`, with `${VAR}` and `${VAR:-default}` taken from the host.
    pub env: Vec<(String, String)>,
    pub workdir: Option<String>,
    /// A prebuilt image to use instead of building one.
    pub docker_image: Option<String>,
    pub build_seconds: u64,
}

/// A path the verifier needs from one of the agent's services.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Artifact {
    pub source: String,
    pub service: String,
}

/// A `[[verifier.collect]]` command, run in a service after the agent
/// finishes and before its artifacts are copied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collect {
    pub command: String,
    pub service: String,
    pub seconds: u64,
    pub user: Option<String>,
}

/// What a task needs from `task.toml` and its folder.
#[derive(Clone, Debug)]
pub struct Task {
    pub name: String,
    pub dir: PathBuf,
    pub instruction: String,
    /// `[environment]`.
    pub environment: Environment,
    /// Whether `environment/docker-compose.yaml` adds services.
    pub compose: bool,
    /// `[agent].user`, which the loop's commands and the reference solution
    /// run as. `None` is the image's user.
    pub agent_user: Option<String>,
    pub agent_seconds: Option<u64>,
    /// The agent's network: `[agent].network_mode`, else the environment's.
    pub agent_network: Network,
    /// Whether the agent has network access.
    pub internet: bool,
    /// Whether the verifier runs in its own environment.
    pub separate: bool,
    /// The separate verifier's environment: `[verifier.environment]`, or a
    /// copy of `[environment]`.
    pub verifier_environment: Environment,
    /// The verifier's network: `[verifier].network_mode`, else its
    /// environment's.
    pub verifier_network: Network,
    pub verifier_user: Option<String>,
    pub verifier_env: Vec<(String, String)>,
    pub verifier_seconds: u64,
    pub artifacts: Vec<Artifact>,
    pub collect: Vec<Collect>,
    pub solution_env: Vec<(String, String)>,
}

/// Where Harbor mounts the verifier's logs, the agent's logs, and the
/// artifacts every task publishes.
const VERIFIER_LOGS: &str = "/logs/verifier";
const AGENT_LOGS: &str = "/logs/agent";
const ARTIFACTS: &str = "/logs/artifacts";
const MAIN: &str = "main";

/// Finds `name` under `tasks`.
///
/// # Errors
///
/// A message naming close matches when there's no such task, or saying
/// why the task can't run.
pub fn find(tasks: &Path, name: &str) -> Result<Task, String> {
    let dir = tasks.join(name);
    if !dir.join("instruction.md").is_file() {
        let mut names: Vec<String> = std::fs::read_dir(tasks)
            .map_err(|error| format!("can't read the task folder {}: {error}", tasks.display()))?
            .filter_map(Result::ok)
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.contains(name) || name.contains(n.as_str()) || shares_a_word(n, name))
            .collect();
        names.sort();
        return Err(if names.is_empty() {
            format!("no Terminal-Bench task named {name} in {}", tasks.display())
        } else {
            format!("no task named {name}; close names: {}", names.join(", "))
        });
    }
    let instruction = std::fs::read_to_string(dir.join("instruction.md"))
        .map_err(|error| format!("can't read {name}'s instruction: {error}"))?;
    let instruction = instruction
        .lines()
        .filter(|l| !l.contains("harbor-canary"))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();
    let toml = std::fs::read_to_string(dir.join("task.toml")).unwrap_or_default();
    let mut task = parse_task(name, &dir, &toml).map_err(|error| format!("{name}: {error}"))?;
    task.instruction = instruction;
    task.compose = dir.join("environment").join(COMPOSE_FILE).is_file();
    Ok(task)
}

/// The Compose file Harbor layers over a task's environment.
const COMPOSE_FILE: &str = "docker-compose.yaml";

fn shares_a_word(a: &str, b: &str) -> bool {
    b.split('-')
        .filter(|w| w.len() > 3)
        .any(|w| a.split('-').any(|x| x == w))
}

/// Reads `task.toml` the way Harbor's `TaskConfig` does, for the fields
/// Microcoder uses.
fn parse_task(name: &str, dir: &Path, text: &str) -> Result<Task, String> {
    let doc: toml::Table = text
        .parse()
        .map_err(|error| format!("task.toml doesn't parse: {error}"))?;
    if doc.get("steps").is_some() {
        return Err(
            "a multi-step task (`steps` in task.toml), which microcoder doesn't run".into(),
        );
    }
    let table = |t: &toml::Table, key: &str| t.get(key).and_then(toml::Value::as_table).cloned();
    let environment_table = table(&doc, "environment");
    let agent = table(&doc, "agent").unwrap_or_default();
    let verifier = table(&doc, "verifier").unwrap_or_default();
    let environment = parse_environment(environment_table.as_ref())?;
    let verifier_env_table = table(&verifier, "environment");
    let separate = match verifier
        .get("environment_mode")
        .and_then(toml::Value::as_str)
    {
        Some("separate") => true,
        Some("shared") => false,
        Some(other) => return Err(format!("unknown verifier environment_mode {other:?}")),
        None => verifier_env_table.is_some(),
    };
    let verifier_environment = match &verifier_env_table {
        Some(t) => parse_environment(Some(t))?,
        None => environment.clone(),
    };
    let phase = |t: &toml::Table| -> Result<Option<Network>, String> {
        t.get("network_mode")
            .and_then(toml::Value::as_str)
            .map(Network::parse)
            .transpose()
    };
    let agent_network = phase(&agent)?.unwrap_or(environment.network);
    // A shared verifier starts from the agent's environment; a separate
    // one from its own.
    let verifier_baseline = if separate {
        verifier_environment.network
    } else {
        environment.network
    };
    let verifier_network = phase(&verifier)?.unwrap_or(verifier_baseline);
    let artifacts = doc
        .get("artifacts")
        .and_then(toml::Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| match entry {
                    toml::Value::String(source) => Some(Artifact {
                        source: source.clone(),
                        service: MAIN.to_string(),
                    }),
                    toml::Value::Table(t) => Some(Artifact {
                        source: t.get("source")?.as_str()?.to_string(),
                        service: t
                            .get("service")
                            .and_then(toml::Value::as_str)
                            .unwrap_or(MAIN)
                            .to_string(),
                    }),
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default();
    let collect = verifier
        .get("collect")
        .and_then(toml::Value::as_array)
        .map(|hooks| {
            hooks
                .iter()
                .filter_map(toml::Value::as_table)
                .filter_map(|t| {
                    Some(Collect {
                        command: t.get("command")?.as_str()?.to_string(),
                        service: t
                            .get("service")
                            .and_then(toml::Value::as_str)
                            .unwrap_or(MAIN)
                            .to_string(),
                        seconds: seconds(t.get("timeout_sec")).unwrap_or(60),
                        user: t.get("user").map(plain),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Task {
        name: name.to_string(),
        dir: dir.to_path_buf(),
        instruction: String::new(),
        compose: false,
        agent_user: agent.get("user").map(plain),
        agent_seconds: seconds(agent.get("timeout_sec")),
        internet: agent_network != Network::NoNetwork,
        agent_network,
        separate,
        verifier_network,
        verifier_user: verifier.get("user").map(plain),
        verifier_env: env_table(verifier.get("env"))?,
        verifier_seconds: seconds(verifier.get("timeout_sec")).unwrap_or(600),
        artifacts,
        collect,
        solution_env: env_table(
            table(&doc, "solution")
                .and_then(|s| s.get("env").cloned())
                .as_ref(),
        )?,
        environment,
        verifier_environment,
    })
}

fn parse_environment(t: Option<&toml::Table>) -> Result<Environment, String> {
    let Some(t) = t else {
        return Ok(Environment {
            build_seconds: 600,
            ..Environment::default()
        });
    };
    let network = match (
        t.get("network_mode").and_then(toml::Value::as_str),
        t.get("allow_internet").and_then(toml::Value::as_bool),
    ) {
        // Harbor maps the older `allow_internet` onto `network_mode`.
        (_, Some(allow)) => {
            if allow {
                Network::Public
            } else {
                Network::NoNetwork
            }
        }
        (Some(mode), None) => Network::parse(mode)?,
        (None, None) => Network::Public,
    };
    Ok(Environment {
        network,
        env: env_table(t.get("env"))?,
        workdir: t
            .get("workdir")
            .and_then(toml::Value::as_str)
            .map(str::to_string),
        docker_image: t
            .get("docker_image")
            .and_then(toml::Value::as_str)
            .map(str::to_string),
        build_seconds: seconds(t.get("build_timeout_sec")).unwrap_or(600),
    })
}

/// A TOML value as text, without quotes around a string.
fn plain(value: &toml::Value) -> String {
    match value {
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Whole seconds from a number of seconds, rounded up.
fn seconds(value: Option<&toml::Value>) -> Option<u64> {
    let value = value?;
    let seconds = value
        .as_float()
        .or_else(|| value.as_integer().map(|i| i as f64))?;
    (seconds.is_finite() && seconds >= 0.0).then(|| seconds.ceil() as u64)
}

/// An `env` table with its templates resolved from the host, as Harbor's
/// `resolve_env_vars` does.
fn env_table(value: Option<&toml::Value>) -> Result<Vec<(String, String)>, String> {
    let Some(table) = value.and_then(toml::Value::as_table) else {
        return Ok(Vec::new());
    };
    table
        .iter()
        .map(|(key, value)| Ok((key.clone(), resolve(&plain(value))?)))
        .collect()
}

/// `${NAME}` or `${NAME:-default}` from the host's environment; any other
/// value as it is.
fn resolve(value: &str) -> Result<String, String> {
    let Some(inner) = value.strip_prefix("${").and_then(|v| v.strip_suffix('}')) else {
        return Ok(value.to_string());
    };
    let (name, default) = match inner.split_once(":-") {
        Some((name, default)) => (name, Some(default)),
        None => (inner, None),
    };
    if name.is_empty() || name.contains(['}', ':']) {
        return Ok(value.to_string());
    }
    match std::env::var(name) {
        Ok(found) => Ok(found),
        Err(_) => default
            .map(str::to_string)
            .ok_or_else(|| format!("task.toml needs {name} from the host's environment")),
    }
}

/// Runs `docker` with `args` and returns whether it succeeded and its
/// output.
async fn docker(args: &[&str]) -> (bool, String) {
    let args: Vec<String> = args.iter().map(|a| (*a).to_string()).collect();
    docker_with(&args, &[], None).await
}

/// Runs `docker` with `args`, `env` added to its environment, stopping it
/// at `deadline`.
async fn docker_with(
    args: &[String],
    env: &[(String, String)],
    deadline: Option<Duration>,
) -> (bool, String) {
    let mut command = Command::new("docker");
    command
        .args(args)
        .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let run = command.output();
    let finished = match deadline {
        Some(deadline) => match tokio::time::timeout(deadline, run).await {
            Ok(finished) => finished,
            Err(_) => {
                return (
                    false,
                    format!("timed out after {} seconds", deadline.as_secs()),
                );
            }
        },
        None => run.await,
    };
    match finished {
        Ok(out) => (
            out.status.success(),
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        ),
        Err(error) => (false, format!("couldn't run docker: {error}")),
    }
}

/// The agent's image: a kept `tbench-warm/<task>:environment-…` image when
/// there is one, the task's `docker_image`, or `microcoder-env/<task>`,
/// which Compose builds from `environment/` when it isn't there yet.
pub async fn image(task: &Task, say: &dyn Fn(&str)) -> String {
    let (_, listed) = docker(&[
        "images",
        "--format",
        "{{.Repository}}:{{.Tag}}",
        &format!("tbench-warm/{}", task.name),
    ])
    .await;
    if let Some(kept) = listed
        .lines()
        .find(|l| l.contains(":environment-"))
        .map(str::to_string)
    {
        say(&format!("using the kept image {kept}"));
        return kept;
    }
    if let Some(prebuilt) = &task.environment.docker_image {
        say(&format!("using the task's image {prebuilt}"));
        return prebuilt.clone();
    }
    format!("microcoder-env/{}:latest", task.name)
}

/// A Compose project to bring up: its `main` service and what it needs.
struct Project<'a> {
    /// The project's name, and `main`'s container name.
    name: &'a str,
    /// The folder with the Dockerfile and any `docker-compose.yaml`.
    dir: PathBuf,
    image: &'a str,
    /// Whether `main` is built from `dir` into `image`.
    build: bool,
    network: &'a str,
    env: &'a [(String, String)],
    /// Paths in `main` bind-mounted from the host.
    mounts: &'a [&'a str],
    build_seconds: u64,
}

/// The host folder for a project's generated Compose files and mounts.
fn scratch(name: &str) -> PathBuf {
    std::env::temp_dir().join("microcoder-compose").join(name)
}

/// Where a path mounted in `name`'s `main` lives on the host.
fn host_path(name: &str, target: &str) -> PathBuf {
    scratch(name)
        .join("mounts")
        .join(target.trim_start_matches('/'))
}

/// A Compose project name: lowercase letters, digits, `-`, and `_`.
fn project(name: &str) -> String {
    let name: String = name
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect();
    if name.starts_with(|c: char| c.is_ascii_alphanumeric()) {
        name
    } else {
        format!("0{name}")
    }
}

/// A task Compose file's services, and whether each chooses its own
/// networking (`network_mode` or `networks`).
fn task_services(config: &Value) -> Vec<(String, bool)> {
    config["services"]
        .as_object()
        .map(|services| {
            services
                .iter()
                .map(|(name, service)| {
                    (
                        name.clone(),
                        service.get("network_mode").is_some() || service.get("networks").is_some(),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The service that holds the shared network namespace when a Compose task
/// runs without network access.
const HOLDER: &str = "microcoder-net";
const SEALED: &str = "microcoder-sealed";

/// The Compose files Microcoder writes for a project, in the order they
/// follow the base file: the base itself, then (after any task file) the
/// environment, the mounts, and the network.
fn overlays(
    p: &Project,
    mounts: &[(String, String)],
    services: &[(String, bool)],
) -> (Value, Vec<Value>) {
    let mut main = json!({
        "image": p.image,
        "container_name": p.name,
        "command": ["sh", "-c", "sleep infinity"],
    });
    if p.build {
        main["build"] = json!({"context": p.dir.to_string_lossy()});
        main["pull_policy"] = json!("build");
    }
    let base = json!({"services": {MAIN: main}});
    let env: Map<String, Value> = p.env.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
    let volumes: Vec<Value> = mounts
        .iter()
        .map(|(source, target)| json!({"type": "bind", "source": source, "target": target}))
        .collect();
    let mut after = vec![
        json!({"services": {MAIN: {"environment": env}}}),
        json!({"services": {MAIN: {"volumes": volumes}}}),
    ];
    match p.network {
        "bridge" | "default" => {}
        "none" if services.iter().all(|(name, _)| name == MAIN) => {
            after.push(json!({"services": {MAIN: {"network_mode": "none"}}}));
        }
        "none" => {
            let mut shared: Vec<String> = vec![MAIN.to_string()];
            for (name, own) in services {
                if !own && !shared.contains(name) {
                    shared.push(name.clone());
                }
                if name == MAIN && *own {
                    shared.retain(|s| s != MAIN);
                }
            }
            let mut doc = json!({
                "services": {
                    HOLDER: {
                        "image": p.image,
                        "entrypoint": ["sleep", "infinity"],
                        "networks": {SEALED: {"aliases": shared}},
                    },
                },
                "networks": {SEALED: {"internal": true}},
            });
            for name in &shared {
                doc["services"][name] = json!({
                    "network_mode": format!("service:{HOLDER}"),
                    "depends_on": {HOLDER: {"condition": "service_started"}},
                });
            }
            after.push(doc);
        }
        "host" => after.push(json!({"services": {MAIN: {"network_mode": "host"}}})),
        other => after.push(json!({
            "services": {MAIN: {"networks": {"default": {}, other: {}}}},
            "networks": {other: {"external": true}},
        })),
    }
    (base, after)
}

/// The environment Compose interpolates files with: the task's `env`, and
/// Harbor's names for the image, the build folder, and the log mounts,
/// which win.
fn compose_env(p: &Project, mounts: &[(String, String)]) -> Vec<(String, String)> {
    let mut env: Vec<(String, String)> = p.env.to_vec();
    let mut infra = vec![
        ("MAIN_IMAGE_NAME".to_string(), p.image.to_string()),
        (
            "CONTEXT_DIR".to_string(),
            p.dir.to_string_lossy().to_string(),
        ),
    ];
    for (source, target) in mounts {
        let legacy = match target.rsplit('/').next() {
            Some("verifier") => "VERIFIER_LOGS",
            Some("agent") => "AGENT_LOGS",
            Some("artifacts") => "ARTIFACTS",
            _ => continue,
        };
        infra.push((format!("ENV_{legacy}_PATH"), target.clone()));
        infra.push((format!("HOST_{legacy}_PATH"), source.clone()));
    }
    env.retain(|(k, _)| !infra.iter().any(|(i, _)| i == k));
    env.extend(infra);
    env
}

/// Brings a project up and returns `main`'s container name and working
/// directory.
async fn bring_up(
    p: &Project<'_>,
    workdir: Option<&str>,
    say: &dyn Fn(&str),
) -> Result<(String, String), String> {
    let scratch = scratch(p.name);
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch)
        .map_err(|e| format!("can't make {}: {e}", scratch.display()))?;
    let mut mounts = Vec::new();
    for target in p.mounts {
        let source = host_path(p.name, target);
        std::fs::create_dir_all(&source)
            .map_err(|e| format!("can't make {}: {e}", source.display()))?;
        // The container's user may not be this one.
        let _ =
            std::fs::set_permissions(&source, std::os::unix::fs::PermissionsExt::from_mode(0o777));
        mounts.push((source.to_string_lossy().to_string(), (*target).to_string()));
    }
    let task_file = p.dir.join(COMPOSE_FILE);
    let task_file = task_file.is_file().then_some(task_file);
    let services = match &task_file {
        Some(file) => {
            let (ok, out) = docker(&[
                "compose",
                "--project-directory",
                &p.dir.to_string_lossy(),
                "-f",
                &file.to_string_lossy(),
                "config",
                "--no-normalize",
                "--no-interpolate",
                "--format",
                "json",
            ])
            .await;
            let config: Value = serde_json::from_str(&out).unwrap_or(Value::Null);
            if !ok || config.is_null() {
                return Err(format!(
                    "the task's Compose file doesn't load:\n{}",
                    crate::state::cut(&out, 500, 1500)
                ));
            }
            task_services(&config)
        }
        None => Vec::new(),
    };
    let (base, after) = overlays(p, &mounts, &services);
    let mut files = Vec::new();
    let write = |n: &str, doc: &Value| -> Result<String, String> {
        let path = scratch.join(n);
        std::fs::write(&path, serde_json::to_string_pretty(doc).unwrap_or_default())
            .map_err(|e| format!("can't write {}: {e}", path.display()))?;
        Ok(path.to_string_lossy().to_string())
    };
    files.push(write("base.json", &base)?);
    if let Some(file) = &task_file {
        files.push(file.to_string_lossy().to_string());
    }
    for (n, doc) in after.iter().enumerate() {
        files.push(write(&format!("overlay-{n}.json"), doc)?);
    }
    let env = compose_env(p, &mounts);
    let project = project(p.name);
    let compose = |rest: &[&str]| -> Vec<String> {
        let mut args: Vec<String> = vec![
            "compose".into(),
            "-p".into(),
            project.clone(),
            "--project-directory".into(),
            p.dir.to_string_lossy().to_string(),
        ];
        for file in &files {
            args.push("-f".into());
            args.push(file.clone());
        }
        args.extend(rest.iter().map(|s| (*s).to_string()));
        args
    };
    let _ = docker_with(
        &compose(&["down", "--remove-orphans"]),
        &env,
        Some(Duration::from_secs(120)),
    )
    .await;
    let build_deadline = Some(Duration::from_secs(p.build_seconds.max(60)));
    if p.build || task_file.is_some() {
        if p.build {
            say(&format!(
                "building {} from {} (this can take minutes)",
                p.image,
                p.dir.display()
            ));
        }
        let build = compose(&["--progress", "plain", "build"]);
        let (mut ok, mut output) = docker_with(&build, &env, build_deadline).await;
        if !ok {
            // A package index or registry can fail for a moment; Harbor's
            // runs retried a failed build as a new trial.
            say("the build failed; trying once more");
            tokio::time::sleep(Duration::from_secs(5)).await;
            (ok, output) = docker_with(&build, &env, build_deadline).await;
        }
        if !ok {
            return Err(format!(
                "the image build failed:\n{}",
                crate::state::cut(&output, 300, 2500)
            ));
        }
    }
    let (ok, output) = docker_with(
        &compose(&["up", "--detach", "--wait"]),
        &env,
        build_deadline,
    )
    .await;
    if !ok {
        let (_, logs) = docker_with(
            &compose(&["logs", "--tail", "30"]),
            &env,
            Some(Duration::from_secs(60)),
        )
        .await;
        remove(p.name).await;
        return Err(format!(
            "the environment didn't start: {}\n{}",
            crate::state::cut(output.trim(), 300, 1500),
            crate::state::cut(logs.trim(), 0, 1500)
        ));
    }
    let Some(container) = service_container(p.name, MAIN).await else {
        remove(p.name).await;
        return Err("the environment started without a running main service".to_string());
    };
    if !p.mounts.is_empty() {
        let dirs = p.mounts.join(" ");
        let _ = docker(&[
            "exec",
            "-u",
            "root",
            &container,
            "sh",
            "-c",
            &format!("mkdir -p {dirs} && chmod 777 {dirs}"),
        ])
        .await;
    }
    let workdir = match workdir {
        Some(w) => w.to_string(),
        None => {
            let (_, w) = docker(&["inspect", "-f", "{{.Config.WorkingDir}}", &container]).await;
            let w = w.trim();
            if w.is_empty() {
                "/".to_string()
            } else {
                w.to_string()
            }
        }
    };
    Ok((container, workdir))
}

/// The running container of `service` in the project `name`.
async fn service_container(name: &str, service: &str) -> Option<String> {
    let (ok, out) = docker(&[
        "ps",
        "--filter",
        &format!("label=com.docker.compose.project={}", project(name)),
        "--filter",
        &format!("label=com.docker.compose.service={service}"),
        "--format",
        "{{.Names}}",
    ])
    .await;
    if !ok {
        return None;
    }
    out.lines()
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// Starts the task's environment as the Compose project `name`, with the
/// agent in its `main` service, and returns that container.
///
/// # Errors
///
/// The build's or Compose's message when the environment doesn't start.
pub async fn start(
    task: &Task,
    image: &str,
    name: &str,
    network: &str,
    say: &dyn Fn(&str),
) -> Result<Docker, String> {
    let (present, _) = docker(&["image", "inspect", image]).await;
    let build = !present && image.starts_with("microcoder-env/");
    if present && image.starts_with("microcoder-env/") {
        say(&format!("using the image {image}"));
    }
    let project = Project {
        name,
        dir: task.dir.join("environment"),
        image,
        build,
        network,
        env: &task.environment.env,
        mounts: &[VERIFIER_LOGS, AGENT_LOGS, ARTIFACTS],
        build_seconds: task.environment.build_seconds,
    };
    let (container, workdir) = bring_up(&project, task.environment.workdir.as_deref(), say).await?;
    if task.compose {
        let (_, services) = docker(&[
            "ps",
            "--filter",
            &format!("label=com.docker.compose.project={}", self::project(name)),
            "--format",
            "{{.Label \"com.docker.compose.service\"}}",
        ])
        .await;
        let mut services: Vec<&str> = services
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != MAIN && *s != HOLDER)
            .collect();
        services.sort_unstable();
        say(&format!("services beside main: {}", services.join(", ")));
    }
    Ok(Docker {
        container,
        workdir,
        user: task.agent_user.clone(),
    })
}

/// Runs the task's `[[verifier.collect]]` commands in the agent's
/// services, as Harbor does before it copies a separate verifier's
/// artifacts. A failure is reported and doesn't stop grading.
pub async fn collect(task: &Task, name: &str, say: &dyn Fn(&str)) {
    for hook in &task.collect {
        let Some(container) = service_container(name, &hook.service).await else {
            say(&format!("collect: no running {} service", hook.service));
            continue;
        };
        let mut args: Vec<String> = vec!["exec".into()];
        if let Some(user) = &hook.user {
            args.extend(["-u".into(), user.clone()]);
        }
        if hook.service == MAIN
            && let Some(workdir) = &task.environment.workdir
        {
            args.extend(["-w".into(), workdir.clone()]);
        }
        let shell = if hook.service == MAIN { "bash" } else { "sh" };
        args.extend([
            container,
            shell.into(),
            "-c".into(),
            format!("{UMASK}; {}", hook.command),
        ]);
        let (ok, output) =
            docker_with(&args, &[], Some(Duration::from_secs(hook.seconds.max(1)))).await;
        if !ok {
            say(&format!(
                "collect in {} failed: {}",
                hook.service,
                crate::state::cut(output.trim(), 200, 300)
            ));
        }
    }
}

/// The artifacts a separate verifier gets: the task's, after Harbor's
/// convention entry, `/logs/artifacts` in `main`.
fn artifact_list(task: &Task) -> Vec<Artifact> {
    let mut list = task.artifacts.clone();
    if !list
        .iter()
        .any(|a| a.service == MAIN && a.source.trim_end_matches('/') == ARTIFACTS)
    {
        list.insert(
            0,
            Artifact {
                source: ARTIFACTS.to_string(),
                service: MAIN.to_string(),
            },
        );
    }
    list
}

/// Copies one artifact out of the agent's project `name` to `local`.
async fn fetch(name: &str, artifact: &Artifact, local: &Path) -> Result<(), String> {
    let container = service_container(name, &artifact.service)
        .await
        .ok_or_else(|| format!("no running {} service", artifact.service))?;
    if let Some(parent) = local.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let (ok, output) = docker(&[
        "cp",
        &format!("{container}:{}", artifact.source.trim_end_matches('/')),
        &local.to_string_lossy(),
    ])
    .await;
    if ok {
        Ok(())
    } else {
        Err(output.trim().to_string())
    }
}

/// Copies the task's artifacts out of the agent's environment into `dir`,
/// so a run can be graded again later. Returns the paths copied.
pub async fn save_artifacts(task: &Task, name: &str, dir: &Path) -> Vec<String> {
    let _ = std::fs::create_dir_all(dir);
    let mut saved = Vec::new();
    for artifact in &task.artifacts {
        let target = artifact.source.trim_end_matches('/');
        let mut local = dir.to_path_buf();
        if artifact.service != MAIN {
            local = local.join(&artifact.service);
        }
        let local = local.join(target.trim_start_matches('/'));
        if fetch(name, artifact, &local).await.is_ok() {
            saved.push(target.to_string());
        }
    }
    saved
}

/// Removes the Compose project `name` (containers, networks, and volumes)
/// and its scratch folder, ignoring one that's already gone.
pub async fn remove(name: &str) {
    let scratch = scratch(name);
    // Files the containers wrote in the mounts belong to their users; hand
    // them back so the scratch folder can go.
    if let Ok(meta) = std::fs::metadata(&scratch)
        && let Some(container) = service_container(name, MAIN).await
    {
        use std::os::unix::fs::MetadataExt;
        let owner = format!("{}:{}", meta.uid(), meta.gid());
        let _ = docker(&[
            "exec",
            "-u",
            "root",
            &container,
            "sh",
            "-c",
            &format!("chown -R {owner} {VERIFIER_LOGS} {AGENT_LOGS} {ARTIFACTS} 2>/dev/null; true"),
        ])
        .await;
    }
    let args: Vec<String> = [
        "compose",
        "-p",
        &project(name),
        "down",
        "--volumes",
        "--remove-orphans",
    ]
    .iter()
    .map(|s| (*s).to_string())
    .collect();
    let _ = docker_with(&args, &[], Some(Duration::from_secs(180))).await;
    let _ = docker(&["rm", "-f", name]).await;
    let _ = std::fs::remove_dir_all(&scratch);
}

/// A first look at the environment, for the state.
pub async fn describe(env: &Docker, network: &str) -> String {
    let probe = "echo \"Working directory: $(pwd)\"; echo \"User: $(id -un 2>/dev/null || id -u)\"; \
                 echo; echo 'Files at the top level:'; ls -la; echo; echo 'Tools:'; \
                 for t in python3 pip node npm cargo go gcc make git; do \
                 command -v $t >/dev/null 2>&1 && printf '%s: %s\\n' $t \"$($t --version 2>&1 | head -1)\"; done; true";
    let result = env.run(probe, Duration::from_secs(30)).await;
    format!(
        "The task runs in a Linux container {}. Commands run in {}.\n\n{}",
        if network == "none" {
            "with no network access"
        } else {
            "with network access"
        },
        env.workdir,
        result.output.trim()
    )
}

/// Runs the task's reference solution (`solution/solve.sh`) in the agent's
/// container, as Harbor's oracle agent does, to check that grading works
/// without a model call.
///
/// # Errors
///
/// When the task has no reference solution, or it fails.
pub async fn solve(task: &Task, agent: &Docker, name: &str) -> Result<(), String> {
    let solution = task.dir.join("solution");
    if !solution.join("solve.sh").is_file() {
        return Err(format!("{} has no reference solution", task.name));
    }
    let _ = docker(&[
        "exec",
        "-u",
        "root",
        &agent.container,
        "sh",
        "-c",
        &format!("{UMASK}; mkdir -p /solution"),
    ])
    .await;
    let (ok, output) = docker(&[
        "cp",
        &format!("{}/.", solution.to_string_lossy()),
        &format!("{}:/solution", agent.container),
    ])
    .await;
    if !ok {
        return Err(format!("couldn't copy the solution in: {}", output.trim()));
    }
    let _ = docker(&[
        "exec",
        "-u",
        "root",
        &agent.container,
        "chmod",
        "+x",
        "/solution/solve.sh",
    ])
    .await;
    let mut args: Vec<String> = vec!["exec".into()];
    if let Some(user) = &agent.user {
        args.extend(["-u".into(), user.clone()]);
    }
    args.extend(["-w".into(), agent.workdir.clone()]);
    args.extend(["-e".into(), "DEBIAN_FRONTEND=noninteractive".into()]);
    for (k, v) in &task.solution_env {
        args.extend(["-e".into(), format!("{k}={v}")]);
    }
    args.extend([
        agent.container.clone(),
        "bash".into(),
        "-c".into(),
        format!("{UMASK}; (/solution/solve.sh) > {AGENT_LOGS}/oracle.txt 2>&1"),
    ]);
    let deadline = Duration::from_secs(task.agent_seconds.unwrap_or(7_200).min(7_200));
    let (ok, output) = docker_with(&args, &[], Some(deadline)).await;
    if ok {
        return Ok(());
    }
    let log =
        std::fs::read_to_string(host_path(name, AGENT_LOGS).join("oracle.txt")).unwrap_or_default();
    Err(format!(
        "the reference solution failed: {}\n{}",
        output.trim(),
        crate::state::cut(&log, 500, 1500)
    ))
}

/// The verifier's result.
#[derive(Clone, Debug, serde::Serialize)]
pub struct Verdict {
    pub reward: Option<f64>,
    /// Everything the reward file held.
    pub rewards: Option<Value>,
    /// Why the reward is unknown, when it is.
    pub reason: Option<String>,
    /// The tests' output, cut.
    pub output: String,
}

impl Verdict {
    fn unknown(reason: &str, output: String) -> Self {
        Self {
            reward: None,
            rewards: None,
            reason: Some(reason.to_string()),
            output,
        }
    }
}

/// A reward read from a verifier's log folder.
#[derive(Clone, Debug, PartialEq)]
pub struct Reward {
    pub reward: Option<f64>,
    pub rewards: Option<Value>,
    pub reason: Option<String>,
}

/// Reads the reward the way Harbor's verifier does: `reward.json` when it
/// exists, or else `reward.txt`. From JSON, the reward is the `reward`
/// key, or the only key.
#[must_use]
pub fn read_reward(dir: &Path) -> Reward {
    let unknown = |reason: String, rewards: Option<Value>| Reward {
        reward: None,
        rewards,
        reason: Some(reason),
    };
    let json_path = dir.join("reward.json");
    let text_path = dir.join("reward.txt");
    if json_path.exists() {
        let text = match std::fs::read_to_string(&json_path) {
            Ok(text) => text,
            Err(error) => return unknown(format!("reward.json can't be read: {error}"), None),
        };
        if text.is_empty() {
            return unknown("reward.json is empty".into(), None);
        }
        let doc: Value = match serde_json::from_str(&text) {
            Ok(doc) => doc,
            Err(error) => return unknown(format!("reward.json isn't JSON: {error}"), None),
        };
        let Some(map) = doc.as_object() else {
            return unknown("reward.json isn't a JSON object".into(), Some(doc));
        };
        let value = match map.get("reward") {
            Some(value) => value,
            None if map.len() == 1 => map.values().next().unwrap_or(&Value::Null),
            None => {
                let keys: Vec<&str> = map.keys().map(String::as_str).collect();
                return unknown(
                    format!(
                        "reward.json has no \"reward\" key and more than one value ({})",
                        keys.join(", ")
                    ),
                    Some(doc.clone()),
                );
            }
        };
        let number = match value {
            Value::Number(n) => n.as_f64(),
            Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
            Value::String(s) => s.trim().parse().ok(),
            _ => None,
        };
        return match number {
            Some(reward) => Reward {
                reward: Some(reward),
                rewards: Some(doc.clone()),
                reason: None,
            },
            None => unknown(
                format!("reward.json's reward is {value}, not a number"),
                Some(doc.clone()),
            ),
        };
    }
    if text_path.exists() {
        let text = match std::fs::read_to_string(&text_path) {
            Ok(text) => text,
            Err(error) => return unknown(format!("reward.txt can't be read: {error}"), None),
        };
        if text.is_empty() {
            return unknown("reward.txt is empty".into(), None);
        }
        return match text.trim().parse::<f64>() {
            Ok(reward) => Reward {
                reward: Some(reward),
                rewards: Some(json!({"reward": reward})),
                reason: None,
            },
            Err(_) => unknown(
                format!(
                    "reward.txt holds {:?}, not a number",
                    crate::state::cut(text.trim(), 80, 0)
                ),
                None,
            ),
        };
    }
    unknown(
        format!("the tests wrote no {VERIFIER_LOGS}/reward.txt or reward.json"),
        None,
    )
}

/// Runs `/tests/test.sh` in `container` as Harbor's verifier does, its
/// output going to `/logs/verifier/test-stdout.txt`.
///
/// # Errors
///
/// When the tests run past `seconds`.
async fn run_tests(
    container: &str,
    user: Option<&str>,
    workdir: Option<&str>,
    env: &[(String, String)],
    seconds: u64,
) -> Result<(), String> {
    let _ = docker(&[
        "exec",
        "-u",
        "root",
        container,
        "chmod",
        "+x",
        "/tests/test.sh",
    ])
    .await;
    let mut args: Vec<String> = vec!["exec".into()];
    if let Some(user) = user {
        args.extend(["-u".into(), user.to_string()]);
    }
    if let Some(workdir) = workdir {
        args.extend(["-w".into(), workdir.to_string()]);
    }
    for (k, v) in env {
        args.extend(["-e".into(), format!("{k}={v}")]);
    }
    args.extend([
        container.to_string(),
        "bash".into(),
        "-c".into(),
        format!("{UMASK}; (/tests/test.sh) > {VERIFIER_LOGS}/test-stdout.txt 2>&1"),
    ]);
    let (_, output) = docker_with(&args, &[], Some(Duration::from_secs(seconds.max(1)))).await;
    if output.starts_with("timed out after") {
        Err(format!(
            "the tests ran past the verifier's {seconds}-second limit"
        ))
    } else {
        Ok(())
    }
}

/// Reads the tests' output and reward from the host side of the project
/// `name`'s `/logs/verifier`.
fn verdict(name: &str, ran: Result<(), String>) -> Verdict {
    let dir = host_path(name, VERIFIER_LOGS);
    let stdout = std::fs::read_to_string(dir.join("test-stdout.txt")).unwrap_or_default();
    let output = crate::state::cut(&stdout, 1_500, 3_000);
    if let Err(reason) = ran {
        return Verdict::unknown(&reason, output);
    }
    let reward = read_reward(&dir);
    Verdict {
        reward: reward.reward,
        rewards: reward.rewards,
        reason: reward.reason,
        output,
    }
}

/// Runs the task's tests and reads the reward: in the agent's container,
/// or in a verifier environment of its own when the task asks for one.
pub async fn verify(task: &Task, agent: &Docker, name: &str, say: &dyn Fn(&str)) -> Verdict {
    if task.separate {
        return verify_separately(task, name, say).await;
    }
    if task.verifier_network != task.agent_network {
        say(&format!(
            "the verifier's network policy is {:?}; it runs on the agent's network",
            task.verifier_network
        ));
    }
    let tests = task.dir.join("tests");
    let _ = docker(&[
        "exec",
        "-u",
        "root",
        &agent.container,
        "sh",
        "-c",
        &format!("{UMASK}; mkdir -p /tests"),
    ])
    .await;
    let (ok, output) = docker(&[
        "cp",
        &format!("{}/.", tests.to_string_lossy()),
        &format!("{}:/tests", agent.container),
    ])
    .await;
    if !ok {
        return Verdict::unknown(
            "the tests couldn't be copied into the container",
            output.trim().to_string(),
        );
    }
    say("running the task's tests");
    let ran = run_tests(
        &agent.container,
        task.verifier_user.as_deref(),
        task.environment.workdir.as_deref(),
        &task.verifier_env,
        task.verifier_seconds,
    )
    .await;
    verdict(name, ran)
}

async fn verify_separately(task: &Task, name: &str, say: &dyn Fn(&str)) -> Verdict {
    let verifier = format!("{name}-verify");
    remove(&verifier).await;
    let environment = &task.verifier_environment;
    let (_, listed) = docker(&[
        "images",
        "--format",
        "{{.Repository}}:{{.Tag}}",
        &format!("tbench-warm/{}", task.name),
    ])
    .await;
    // A kept image Harbor built from `tests/`, the environment's
    // `docker_image`, or one Compose builds.
    let image = listed
        .lines()
        .find(|l| l.contains(":tests-"))
        .map(str::to_string)
        .or_else(|| environment.docker_image.clone())
        .unwrap_or_else(|| format!("microcoder-verify/{}:latest", task.name));
    let (present, _) = docker(&["image", "inspect", &image]).await;
    let project = Project {
        name: &verifier,
        dir: task.dir.join("tests"),
        image: &image,
        build: !present && image.starts_with("microcoder-verify/"),
        network: task.verifier_network.docker(),
        env: &environment.env,
        mounts: &[VERIFIER_LOGS],
        build_seconds: environment.build_seconds,
    };
    let container = match bring_up(&project, environment.workdir.as_deref(), say).await {
        Ok((container, _)) => container,
        Err(error) => {
            remove(&verifier).await;
            return Verdict::unknown("the verifier's environment didn't start", error);
        }
    };
    let _ = docker(&[
        "exec",
        "-u",
        "root",
        &container,
        "sh",
        "-c",
        &format!("find {VERIFIER_LOGS} -mindepth 1 -delete; chmod 777 {VERIFIER_LOGS}"),
    ])
    .await;
    let scratch = scratch(&verifier).join("artifacts");
    for (n, artifact) in artifact_list(task).iter().enumerate() {
        let local = scratch.join(n.to_string());
        if let Err(error) = fetch(name, artifact, &local).await {
            if artifact.source != ARTIFACTS {
                say(&format!(
                    "artifact {} wasn't copied: {error}",
                    artifact.source
                ));
            }
            continue;
        }
        let target = artifact.source.trim_end_matches('/');
        if local.is_dir() {
            // Harbor empties a directory artifact's target, then uploads
            // its contents.
            let _ = docker(&[
                "exec",
                "-u",
                "root",
                &container,
                "sh",
                "-c",
                &format!("rm -rf '{target}' && mkdir -p '{target}' && chmod 777 '{target}'"),
            ])
            .await;
            let _ = docker(&[
                "cp",
                &format!("{}/.", local.to_string_lossy()),
                &format!("{container}:{target}"),
            ])
            .await;
        } else {
            let parent = Path::new(target)
                .parent()
                .map_or("/".to_string(), |p| p.to_string_lossy().to_string());
            let _ = docker(&[
                "exec",
                "-u",
                "root",
                &container,
                "sh",
                "-c",
                &format!("{UMASK}; mkdir -p '{parent}'"),
            ])
            .await;
            let _ = docker(&[
                "cp",
                &local.to_string_lossy(),
                &format!("{container}:{target}"),
            ])
            .await;
        }
    }
    say("running the task's tests");
    let ran = run_tests(
        &container,
        task.verifier_user.as_deref(),
        environment.workdir.as_deref(),
        &task.verifier_env,
        task.verifier_seconds,
    )
    .await;
    let verdict = verdict(&verifier, ran);
    remove(&verifier).await;
    verdict
}
/// Fable 5.1 low's passes, median time, and median cost on `task`, from
/// the public replays reference.
#[must_use]
pub fn fable(task: &str) -> Option<(usize, usize, f64, f64)> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../bench/terminal-bench/reference/fable-5.1-replays.json");
    let doc: Value = serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()?;
    let trials: Vec<&Value> = doc["trials"]
        .as_array()?
        .iter()
        .filter(|t| t["task"] == task && t["effort"] == "low")
        .collect();
    if trials.is_empty() {
        return None;
    }
    let passed: Vec<&&Value> = trials
        .iter()
        .filter(|t| t["reward"].as_f64().unwrap_or(0.0) >= 1.0)
        .collect();
    let median = |mut xs: Vec<f64>| -> f64 {
        if xs.is_empty() {
            return 0.0;
        }
        xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let n = xs.len();
        if n % 2 == 1 {
            xs[n / 2]
        } else {
            f64::midpoint(xs[n / 2 - 1], xs[n / 2])
        }
    };
    let seconds = |t: &Value| -> Option<f64> {
        let parse = |s: &str| -> Option<f64> {
            // RFC 3339 to seconds, enough for a difference: hours, minutes,
            // and seconds of the day plus the day of the month.
            let (date, time) = s.split_once('T')?;
            let day: f64 = date.rsplit('-').next()?.parse().ok()?;
            let time = time.trim_end_matches('Z');
            let time = time.split(['+', '-']).next()?;
            let mut parts = time.split(':');
            let h: f64 = parts.next()?.parse().ok()?;
            let m: f64 = parts.next()?.parse().ok()?;
            let s: f64 = parts.next()?.parse().ok()?;
            Some(day * 86_400.0 + h * 3_600.0 + m * 60.0 + s)
        };
        Some(parse(t["finished_at"].as_str()?)? - parse(t["started_at"].as_str()?)?)
    };
    Some((
        passed.len(),
        trials.len(),
        median(passed.iter().filter_map(|t| seconds(t)).collect()),
        median(
            passed
                .iter()
                .filter_map(|t| t["cost_usd"].as_f64())
                .collect(),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TOML: &str = r#"
schema_version = "1.0"
# artifacts in a comment don't count
artifacts = [
  "/app/rules.json",
  "/app/out/",
  { source = "/var/lib/db/dump.sql", service = "db" },
]

[verifier]
environment_mode = "separate"
timeout_sec = 300.0
network_mode = "public"
user = "tester"

[verifier.env]
MODE = "strict"
FROM_HOST = "${MICROCODER_TEST_UNSET_VAR:-fallback}"

[[verifier.collect]]
command = "pg_dump > /var/lib/db/dump.sql"
service = "db"
timeout_sec = 30

[[verifier.collect]]
command = "git diff > /tmp/agent.patch"

[agent]
timeout_sec = 28800.0
user = 1000

[environment]
allow_internet = false
build_timeout_sec = 900.0
workdir = "/app"

[environment.env]
GREETING = "hi"
"#;

    #[test]
    fn task_toml_reads_like_harbors_task_config() {
        let task = parse_task("t", Path::new("/tasks/t"), TOML).unwrap();
        assert_eq!(
            task.artifacts,
            [
                Artifact {
                    source: "/app/rules.json".into(),
                    service: "main".into()
                },
                Artifact {
                    source: "/app/out/".into(),
                    service: "main".into()
                },
                Artifact {
                    source: "/var/lib/db/dump.sql".into(),
                    service: "db".into()
                },
            ]
        );
        assert!(task.separate);
        assert_eq!(task.verifier_seconds, 300);
        assert_eq!(task.verifier_user.as_deref(), Some("tester"));
        assert_eq!(
            task.verifier_env,
            [
                ("FROM_HOST".to_string(), "fallback".to_string()),
                ("MODE".to_string(), "strict".to_string()),
            ]
        );
        assert_eq!(task.agent_user.as_deref(), Some("1000"));
        assert_eq!(task.agent_network, Network::NoNetwork);
        assert!(!task.internet);
        // The verifier's own network_mode wins over its environment's.
        assert_eq!(task.verifier_network, Network::Public);
        // With no [verifier.environment], the verifier's is a copy of the task's.
        assert_eq!(task.verifier_environment.network, Network::NoNetwork);
        assert_eq!(task.verifier_environment.workdir.as_deref(), Some("/app"));
        assert_eq!(task.environment.build_seconds, 900);
        assert_eq!(
            task.environment.env,
            [("GREETING".to_string(), "hi".to_string())]
        );
        assert_eq!(
            task.collect,
            [
                Collect {
                    command: "pg_dump > /var/lib/db/dump.sql".into(),
                    service: "db".into(),
                    seconds: 30,
                    user: None,
                },
                Collect {
                    command: "git diff > /tmp/agent.patch".into(),
                    service: "main".into(),
                    seconds: 60,
                    user: None,
                },
            ]
        );
    }

    #[test]
    fn a_verifier_environment_implies_separate_and_has_its_own_network() {
        let toml = r#"
[environment]
network_mode = "no-network"

[verifier.environment]
cpus = 1
"#;
        let task = parse_task("t", Path::new("/t"), toml).unwrap();
        assert!(task.separate);
        assert_eq!(task.agent_network, Network::NoNetwork);
        // A [verifier.environment] starts from Harbor's default, public.
        assert_eq!(task.verifier_network, Network::Public);
        assert_eq!(task.verifier_seconds, 600);

        let shared = parse_task("t", Path::new("/t"), "[verifier]\ntimeout_sec = 10\n").unwrap();
        assert!(!shared.separate);
        assert_eq!(shared.agent_network, Network::Public);
        assert!(parse_task("t", Path::new("/t"), "[[steps]]\nname = \"a\"\n").is_err());
        assert!(
            parse_task(
                "t",
                Path::new("/t"),
                "[environment.env]\nK = \"${MICROCODER_TEST_UNSET_VAR}\"\n"
            )
            .unwrap_err()
            .contains("MICROCODER_TEST_UNSET_VAR")
        );
    }

    fn reward_dir(files: &[(&str, &str)]) -> PathBuf {
        static N: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "microcoder-reward-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, text) in files {
            std::fs::write(dir.join(name), text).unwrap();
        }
        dir
    }

    #[test]
    fn rewards_read_like_harbors_verifier() {
        let read = |files: &[(&str, &str)]| {
            let dir = reward_dir(files);
            let reward = read_reward(&dir);
            let _ = std::fs::remove_dir_all(&dir);
            reward
        };
        assert_eq!(read(&[("reward.txt", "1\n")]).reward, Some(1.0));
        assert_eq!(read(&[("reward.txt", " 0.25 ")]).reward, Some(0.25));
        assert_eq!(
            read(&[("reward.json", r#"{"reward": 1, "tests": 7}"#)]).reward,
            Some(1.0)
        );
        assert_eq!(
            read(&[("reward.json", r#"{"score": 0.5}"#)]).reward,
            Some(0.5)
        );
        // reward.json wins over reward.txt, as in Harbor.
        assert_eq!(
            read(&[("reward.json", r#"{"reward": 0.0}"#), ("reward.txt", "1")]).reward,
            Some(0.0)
        );
        for (files, why) in [
            (vec![], "wrote no /logs/verifier/reward.txt or reward.json"),
            (vec![("reward.txt", "")], "reward.txt is empty"),
            (vec![("reward.txt", "PASSED")], "not a number"),
            (vec![("reward.json", "")], "reward.json is empty"),
            (vec![("reward.json", "{oops")], "isn't JSON"),
            (vec![("reward.json", "[1]")], "isn't a JSON object"),
            (
                vec![("reward.json", r#"{"a": 1, "b": 0}"#)],
                "no \"reward\" key",
            ),
            (vec![("reward.json", r#"{"reward": null}"#)], "not a number"),
        ] {
            let reward = read(&files);
            assert_eq!(reward.reward, None, "{files:?}");
            let reason = reward.reason.unwrap();
            assert!(reason.contains(why), "{files:?}: {reason}");
        }
    }

    fn project<'a>(network: &'a str, build: bool) -> Project<'a> {
        Project {
            name: "microcoder-t-1",
            dir: PathBuf::from("/tasks/t/environment"),
            image: "microcoder-env/t:latest",
            build,
            network,
            env: &[],
            mounts: &[VERIFIER_LOGS],
            build_seconds: 600,
        }
    }

    #[test]
    fn overlays_start_main_as_harbor_does() {
        let mounts = [("/host/logs/verifier".to_string(), VERIFIER_LOGS.to_string())];
        let (base, after) = overlays(&project("bridge", true), &mounts, &[]);
        let main = &base["services"]["main"];
        assert_eq!(main["command"], json!(["sh", "-c", "sleep infinity"]));
        assert_eq!(
            main["entrypoint"],
            Value::Null,
            "the image's entrypoint still runs"
        );
        assert_eq!(main["build"]["context"], "/tasks/t/environment");
        assert_eq!(main["container_name"], "microcoder-t-1");
        assert_eq!(after.len(), 2, "no network overlay on bridge");
        assert_eq!(
            after[1]["services"]["main"]["volumes"][0]["target"],
            VERIFIER_LOGS
        );

        let (base, after) = overlays(&project("none", false), &mounts, &[]);
        assert_eq!(base["services"]["main"]["build"], Value::Null);
        assert_eq!(after[2]["services"]["main"]["network_mode"], "none");
    }

    #[test]
    fn without_network_services_share_one_sealed_namespace() {
        let services = [
            ("main".to_string(), false),
            ("db".to_string(), false),
            ("proxy".to_string(), true),
        ];
        let (_, after) = overlays(&project("none", true), &[], &services);
        let doc = &after[2];
        assert_eq!(doc["networks"][SEALED]["internal"], true);
        assert_eq!(
            doc["services"][HOLDER]["networks"][SEALED]["aliases"],
            json!(["main", "db"])
        );
        for name in ["main", "db"] {
            assert_eq!(
                doc["services"][name]["network_mode"],
                "service:microcoder-net"
            );
        }
        assert_eq!(
            doc["services"]["proxy"],
            Value::Null,
            "a service's own networking is kept"
        );
    }

    #[test]
    fn a_separate_verifier_gets_the_artifacts_convention_folder() {
        let mut task = parse_task("t", Path::new("/t"), "artifacts = [\"/app/x\"]\n").unwrap();
        assert_eq!(artifact_list(&task)[0].source, ARTIFACTS);
        assert_eq!(artifact_list(&task).len(), 2);
        task.artifacts.push(Artifact {
            source: "/logs/artifacts/".into(),
            service: "main".into(),
        });
        assert_eq!(artifact_list(&task).len(), 2);
    }

    #[test]
    fn an_unknown_task_names_close_matches() {
        let dir = std::env::temp_dir().join(format!("microcoder-tasks-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("sound-change-cascade")).unwrap();
        std::fs::write(dir.join("sound-change-cascade/instruction.md"), "x").unwrap();
        let error = find(&dir, "sound-change").unwrap_err();
        assert!(error.contains("sound-change-cascade"), "{error}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The fixture tasks under `fixtures/tasks/`.
    fn fixture(name: &str) -> Task {
        find(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/tasks"),
            name,
        )
        .unwrap()
    }

    /// Containers and networks Compose still has for the project `name`.
    async fn leftovers(name: &str) -> String {
        let label = format!("label=com.docker.compose.project={}", super::project(name));
        let (_, containers) = docker(&["ps", "-a", "-q", "--filter", &label]).await;
        let (_, networks) = docker(&["network", "ls", "-q", "--filter", &label]).await;
        format!("{containers}{networks}").trim().to_string()
    }

    async fn grade_fixture(task: &Task, network: &str) {
        let say = |text: &str| println!("{text}");
        let name = format!(
            "microcoder-fixture-{}-{network}-{}",
            task.name,
            std::process::id()
        );
        let image = image(task, &say).await;
        let env = start(task, &image, &name, network, &say).await.unwrap();
        let unsolved = verify(task, &env, &name, &say).await;
        assert_eq!(unsolved.reward, Some(0.0), "{unsolved:?}");
        solve(task, &env, &name).await.unwrap();
        collect(task, &name, &say).await;
        let solved = verify(task, &env, &name, &say).await;
        assert_eq!(solved.reward, Some(1.0), "{solved:?}");
        remove(&name).await;
        assert_eq!(leftovers(&name).await, "");
        assert_eq!(leftovers(&format!("{name}-verify")).await, "");
        assert!(!scratch(&name).exists());
    }

    /// Needs Docker and the `python:3.13-slim` image; run with
    /// `cargo test -p microcoder -- --ignored fixture`.
    #[tokio::test]
    #[ignore = "needs Docker"]
    async fn fixture_compose_sidecar_reachable_by_name() {
        let task = fixture("compose-sidecar");
        assert!(task.compose);
        grade_fixture(&task, "bridge").await;
        grade_fixture(&task, "none").await;
    }

    /// Files the loop's commands, the reference solution, a collect
    /// command, and the tests create are 644 and directories 755, even where
    /// `docker exec` would hand them the daemon's umask of 0000.
    #[tokio::test]
    #[ignore = "needs Docker"]
    async fn fixture_file_modes_follow_umask_022() {
        let task = fixture("file-modes");
        let say = |text: &str| println!("{text}");
        let name = format!("microcoder-fixture-file-modes-{}", std::process::id());
        let image = image(&task, &say).await;
        let env = start(&task, &image, &name, "none", &say).await.unwrap();
        // The loop's commands and acceptance tests both run through `Env::run`.
        let made = env
            .run(
                "touch /app/loop.txt && mkdir /app/loop-dir && stat -c %a /app/loop.txt /app/loop-dir",
                Duration::from_secs(30),
            )
            .await;
        assert_eq!(
            made.output.split_whitespace().collect::<Vec<_>>(),
            ["644", "755"],
            "{made:?}"
        );
        solve(&task, &env, &name).await.unwrap();
        collect(&task, &name, &say).await;
        let verdict = verify(&task, &env, &name, &say).await;
        remove(&name).await;
        assert_eq!(verdict.reward, Some(1.0), "{verdict:?}");
        assert_eq!(leftovers(&name).await, "");
    }

    #[tokio::test]
    #[ignore = "needs Docker"]
    async fn fixture_separate_verifier_installs_packages_and_writes_reward_json() {
        let task = fixture("separate-verifier");
        assert!(task.separate);
        assert!(!task.internet);
        grade_fixture(&task, "none").await;
    }
}
