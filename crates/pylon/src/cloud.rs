//! `pylon cloud up|down|status` (#12402): launch a DeepSeek-V4.1 group on
//! rented machines through the spend guard, head it from this computer,
//! and stop it on `down`, at the machines' expiry, when the budget passes
//! its cap, or when the group is lost.
//!
//! - **Plan.** A named placement ([`crate::placement`]) says which stages
//!   run on which machine. Each machine is one `scripts/dev/cloud-budget.py
//!   create` with an expiry, labels, and the budget; the guard prices it,
//!   refuses it over the cap or without a price, and refuses a second
//!   allocating host or ledger. `--dry-run` prints the guard and gcloud
//!   commands and creates nothing.
//! - **Machines.** A machine boots `scripts/pylon/v41-stage-startup.sh`,
//!   which copies the shards its stages need from the weights bucket and
//!   installs the stage worker.
//! - **Group.** A pylon heads the group (group mode,
//!   [`crate::group_serve`]): this computer by default, or, with `--head
//!   stage0|vm` (#12399), the stage-0 machine or a small machine in the
//!   same zone ([`crate::cloud_head`]), which this computer starts and
//!   stops over one ssh session. Each stage is a managed worker that
//!   `scripts/pylon/v41-stage-ssh.sh` starts over ssh, with the stage's
//!   control connection forwarded back to the head. Killing the worker
//!   ends the ssh session and the remote stage; a lost machine ends the
//!   session, so spot loss shows as a stage failing, the group going
//!   unready, and its beacons dropping, never as a hang. With `--transport
//!   direct` the stages send stage state straight to each other over
//!   `stage_link` on [`DATA_PORT`] and up, inside the zone.
//! - **End.** The head drains, publishes its beacons offline, and every
//!   machine is deleted through the guard, which records its cost.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::oneshot;

use crate::cloud_head::{self, HeadPlacement, RemoteHead};
use crate::engine::Sampling;
use crate::identity::Identity;
use crate::lease::Dedicated;
use crate::online::{Options, load_live};
use crate::placement::{self, Placement};
use crate::share::{self, GroupLane, GroupStage, Settings};
use crate::stage_group::Transport;
use crate::{DEFAULT_RELAY, now};

/// `cloud.json`'s schema.
pub const SCHEMA: &str = "openagents.pylon.cloud.v1";
/// The launch record in the pylon home.
pub const FILE: &str = "cloud.json";
/// The runtime a V4.1 stage worker announces on the CPU backend.
pub const RUNTIME: &str = "psionic-v41-stage/1";
/// The runtime it announces on the CUDA backend, which a cloud group runs.
pub const RUNTIME_CUDA: &str = "psionic-v41-stage/1+cuda";
/// The image a stage machine boots: CUDA 12.9 with the NVIDIA 580 driver.
pub const IMAGE_FAMILY: &str = "common-cu129-ubuntu-2404-nvidia-580";
pub const IMAGE_PROJECT: &str = "deeplearning-platform-release";
/// The first loopback port a stage's control connection is forwarded to on
/// its machine; stage `i` uses this plus `i`.
pub const REMOTE_PORT: u16 = 47_100;
/// The first port a stage's link listener uses on its machine under the
/// direct transport; stage `i` uses this plus `i`.
pub const DATA_PORT: u16 = 47_200;
/// A stage machine's boot-disk throughput in MiB/s: Hyperdisk Balanced's
/// maximum for one disk.
pub const DISK_MIBPS: u32 = 2_400;
/// The IOPS that throughput needs: Hyperdisk Balanced moves at most
/// 256 KiB per operation, and provisioning one needs both.
pub const DISK_IOPS: u32 = 12_000;
/// How long the head waits for a killed stage to be reaped, in ms.
pub const STOP_MS: u64 = 30_000;
/// The image a head-only machine boots.
pub const HEAD_IMAGE_FAMILY: &str = "debian-12";
pub const HEAD_IMAGE_PROJECT: &str = "debian-cloud";
/// The file `pylon cloud down` leaves for a running `up`.
const DOWN: &str = "cloud.down";

/// The spend guard and how to run it.
#[derive(Debug, Clone)]
pub struct Guard {
    pub python: PathBuf,
    /// `scripts/dev/cloud-budget.py`.
    pub script: PathBuf,
    pub budget: String,
    /// The cap to open a new budget with; an existing ledger keeps its own.
    pub cap: Option<f64>,
    /// Extra environment for the guard, such as `OA_CLOUD_BUDGET_DIR`.
    pub env: Vec<(String, String)>,
}

impl Guard {
    /// The guard in the checkout at `repo`, on the `deepseek-v41` budget.
    #[must_use]
    pub fn new(repo: &Path) -> Self {
        Self {
            python: "python3".into(),
            script: repo.join("scripts/dev/cloud-budget.py"),
            budget: "deepseek-v41".into(),
            cap: None,
            env: Vec::new(),
        }
    }

    /// The full command line for `words` (a guard subcommand and its own
    /// options; gcloud flags go after `--` at the end).
    #[must_use]
    pub fn argv(&self, words: &[String]) -> Vec<String> {
        let split = words.iter().position(|w| w == "--").unwrap_or(words.len());
        let mut argv = vec![
            self.python.display().to_string(),
            self.script.display().to_string(),
        ];
        argv.extend(words[..split].iter().cloned());
        argv.extend(["--budget".into(), self.budget.clone()]);
        if let Some(cap) = self.cap {
            argv.extend(["--cap".into(), cap.to_string()]);
        }
        argv.extend(words[split..].iter().cloned());
        argv
    }

    fn env_var(&self, name: &str) -> Option<String> {
        self.env
            .iter()
            .rev()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.clone())
            .or_else(|| std::env::var(name).ok())
    }

    /// Run the guard; its standard output, or why it refused.
    fn run(&self, words: &[String]) -> Result<String, String> {
        let argv = self.argv(words);
        let output = std::process::Command::new(&argv[0])
            .args(&argv[1..])
            .envs(self.env.iter().map(|(k, v)| (k, v)))
            .stdin(std::process::Stdio::null())
            .output()
            .map_err(|e| format!("the spend guard did not run: {e}"))?;
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        if output.status.success() {
            return Ok(stdout);
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let why = stderr
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("it gave no reason")
            .trim()
            .trim_start_matches("cloud-budget: ")
            .trim_start_matches("refused: ")
            .to_string();
        // Keep gcloud's own error, such as a zone out of capacity, which
        // the guard prints before its last line.
        let gcloud: Vec<&str> = stderr
            .lines()
            .map(str::trim)
            .skip_while(|l| !l.starts_with("ERROR:"))
            .filter(|l| !l.is_empty() && !l.starts_with("cloud-budget:"))
            .take(12)
            .collect();
        if gcloud.is_empty() {
            Err(format!("the spend guard refused: {why}"))
        } else {
            Err(format!(
                "the spend guard refused: {why} ({})",
                gcloud.join(" ")
            ))
        }
    }

    /// The budget's ledger file.
    #[must_use]
    pub fn ledger(&self) -> PathBuf {
        let dir = self.env_var("OA_CLOUD_BUDGET_DIR").map_or_else(
            || {
                PathBuf::from(self.env_var("HOME").unwrap_or_default())
                    .join(".openagents")
                    .join("cloud-budget")
            },
            PathBuf::from,
        );
        dir.join(format!("{}.jsonl", self.budget))
    }

    /// The ledger's records, skipping a torn line.
    #[must_use]
    pub fn records(&self) -> Vec<Value> {
        std::fs::read_to_string(self.ledger())
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str(l).ok())
            .collect()
    }

    /// `report --offline --json`.
    ///
    /// # Errors
    ///
    /// When the guard fails or prints no report.
    pub fn report(&self) -> Result<Value, String> {
        let out = self.run(&["report".into(), "--offline".into(), "--json".into()])?;
        serde_json::from_str(&out).map_err(|e| format!("the spend guard's report: {e}"))
    }

    /// Delete `vm` and record its cost.
    ///
    /// # Errors
    ///
    /// When the guard can't delete it.
    pub fn stop(&self, vm: &str) -> Result<(), String> {
        self.run(&["stop".into(), vm.into()]).map(|_| ())
    }
}

/// What `pylon cloud up` launches.
#[derive(Debug, Clone)]
pub struct Up {
    /// `four-single`, `two-dual`, or `one-quad`.
    pub placement: String,
    /// Another machine type for the placement.
    pub machine_type: Option<String>,
    pub zone: String,
    pub project: String,
    /// Hours until the guard's expiry deletes each machine.
    pub hours: f64,
    pub spot: bool,
    /// The group's slug; machine names start with `oa-<group>-`.
    pub group: String,
    /// The checkpoint bucket prefix.
    pub weights: String,
    /// The stage worker binary in a bucket (W11 builds it); a dry run
    /// leaves it unset.
    pub worker: Option<String>,
    /// The SHA-256 of the worker build's `BUILD.json`, which pins the
    /// worker, its libraries, its target, and its profile; each stage
    /// machine refuses another build (#12407).
    pub worker_build: Option<String>,
    /// The SHA-256 of the pylon build's `BUILD.json`, for a head in the zone.
    pub pylon_build: Option<String>,
    /// A launch without the build pins (`--development`). A qualified
    /// launch needs `worker_build`, and `pylon_build` for a head in the
    /// zone; a development launch is recorded and reported as one (#12410).
    pub development: bool,
    pub deadline_secs: u64,
    pub max_output_tokens: u32,
    pub max_context: usize,
    pub max_step_tokens: Option<usize>,
    pub prefill_chunk: Option<usize>,
    pub max_input_tokens: Option<usize>,
    pub sampling: Sampling,
    /// How the stages render prompts and parse answers (`--mode`).
    pub mode: crate::stage_group::PromptMode,
    pub relay: String,
    /// Keys that may send jobs; `None` admits any key.
    pub allow: Option<BTreeSet<String>>,
    pub guard: Guard,
    /// The checkout holding `scripts/pylon/`.
    pub repo: PathBuf,
    /// The pylon home this computer heads the group from.
    pub home: PathBuf,
    /// The weights digest and runtime every stage must announce.
    pub stage_weights: Option<String>,
    pub stage_runtime: String,
    /// The `--backend` each stage runs, if any.
    pub stage_backend: Option<String>,
    /// Run each stage on this computer with this program and arguments
    /// instead of over ssh: tests' synthetic stages.
    pub local_stages: Option<(PathBuf, Vec<String>)>,
    /// How long the stages may take to load before the launch gives up.
    pub ready_timeout: Duration,
    /// End the group this long before the first machine's expiry.
    pub expiry_margin: Duration,
    /// How often the running group is checked.
    pub every: Duration,
    /// How often the budget is checked against its cap.
    pub budget_every: Duration,
    pub step_timeout: Duration,
    pub startup_timeout: Duration,
    pub max_restarts: u32,
    /// Where stage state travels between stages.
    pub transport: Transport,
    /// Where the group head runs.
    pub head: HeadPlacement,
    /// The machine type of a head-only machine (`--head vm`).
    pub head_machine_type: String,
    /// The pylon binary in a bucket, for a head in the zone.
    pub pylon: Option<String>,
    /// Run the head's session on this computer with this program and
    /// arguments instead of over ssh: tests' in-zone head.
    pub local_head: Option<(PathBuf, Vec<String>)>,
}

impl Up {
    /// The defaults for `placement`, from the checkout at `repo`.
    #[must_use]
    pub fn new(placement: &str, repo: &Path, home: &Path) -> Self {
        Self {
            placement: placement.into(),
            machine_type: None,
            zone: "us-east1-b".into(),
            project: "openagentsgemini".into(),
            hours: 4.0,
            spot: true,
            group: format!("v41-{placement}"),
            weights: placement::WEIGHTS.into(),
            worker: None,
            worker_build: None,
            pylon_build: None,
            development: false,
            deadline_secs: 90,
            max_output_tokens: 1_024,
            max_context: 8192,
            max_step_tokens: None,
            prefill_chunk: None,
            max_input_tokens: None,
            sampling: Sampling::new(0.6, 0.95, 0).unwrap_or_else(|_| unreachable!()),
            mode: crate::stage_group::PromptMode::default(),
            relay: DEFAULT_RELAY.into(),
            allow: None,
            guard: Guard::new(repo),
            repo: repo.into(),
            home: home.into(),
            stage_weights: None,
            stage_runtime: RUNTIME_CUDA.into(),
            stage_backend: Some("cuda".into()),
            local_stages: None,
            ready_timeout: Duration::from_secs(1_800),
            expiry_margin: Duration::from_secs(300),
            every: Duration::from_secs(1),
            budget_every: Duration::from_secs(60),
            step_timeout: Duration::from_secs(30),
            startup_timeout: Duration::from_secs(900),
            max_restarts: 3,
            transport: Transport::Front,
            head: HeadPlacement::Local,
            head_machine_type: "e2-small".into(),
            pylon: None,
            local_head: None,
        }
    }
}

/// One stage on one machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagePlan {
    pub name: String,
    pub stage: u16,
    pub from: u32,
    pub to: u32,
    pub gpu: u32,
    pub embeddings: bool,
    pub head: bool,
}

/// One machine and the guard command that creates it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachinePlan {
    pub name: String,
    pub host: String,
    pub machine_type: String,
    pub zone: String,
    pub stages: Vec<StagePlan>,
    /// The group head runs here.
    #[serde(default)]
    pub head: bool,
    /// The guard command, without `--dry-run`.
    pub guard: Vec<String>,
}

/// The machines a launch creates.
#[derive(Debug, Clone)]
pub struct Plan {
    pub placement: Placement,
    pub machines: Vec<MachinePlan>,
}

/// Where a launch keeps the group's ssh key for a head in the zone.
#[must_use]
pub fn key_dir(up: &Up) -> PathBuf {
    up.home.join(format!("cloud-{}", up.group))
}

/// A machine's name inside the zone's network.
#[must_use]
pub fn internal(up: &Up, machine: &str) -> String {
    format!("{machine}.{}.c.{}.internal", up.zone, up.project)
}

fn layer_list(stages: &[StagePlan]) -> String {
    stages
        .iter()
        .map(|s| format!("{}-{}", s.from, s.to))
        .collect::<Vec<_>>()
        .join("+")
}

/// Plan `up`'s machines.
///
/// # Errors
///
/// When the placement is unknown or doesn't fit its machines, the group
/// slug is invalid, or the hours aren't positive.
pub fn plan(up: &Up) -> Result<Plan, String> {
    let step = up
        .max_step_tokens
        .or(up.prefill_chunk)
        .unwrap_or(up.max_context);
    if up.max_context < 2
        || step == 0
        || step > up.max_context
        || up.prefill_chunk.is_some_and(|n| n == 0 || n > step)
    {
        return Err("the chunk and step capacities must be positive and fit the context".into());
    }
    let placement = placement::named(&up.placement, up.machine_type.as_deref())?;
    placement::check_capacities(&placement, up.max_context, step)?;
    if up.group.is_empty()
        || up.group.len() > 40
        || !up
            .group
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("a group is named by 1 to 40 of a-z, 0-9, and -".into());
    }
    if !(up.hours > 0.0) {
        return Err("--hours must be more than zero".into());
    }
    let startup = up.repo.join("scripts/pylon/v41-stage-startup.sh");
    let stage_ssh = up.repo.join("scripts/pylon/v41-stage-ssh.sh");
    // The build checker the startup script installs (#12410).
    let check = up.repo.join("scripts/pylon/v41-build-check.py");
    let keys = key_dir(up).join("ssh-keys");
    let disk_gb = |stages: usize| 100 + 250 * stages;
    let pylon = up.pylon.clone().unwrap_or_else(|| "unset".into());
    let mut machines: Vec<MachinePlan> = placement
        .hosts
        .iter()
        .map(|host| {
            let name = format!("oa-{}-{}", up.group, host.host);
            let stages: Vec<StagePlan> = host
                .stages
                .iter()
                .map(|s| StagePlan {
                    name: format!("s{}", s.stage),
                    stage: s.stage,
                    from: s.first_layer,
                    to: s.end_layer,
                    gpu: s.gpu,
                    embeddings: s.embeddings,
                    head: s.head,
                })
                .collect();
            let head = up.head == HeadPlacement::Stage0 && stages.iter().any(|s| s.stage == 0);
            let mut metadata = vec![
                ("oa-group", up.group.clone()),
                ("oa-manifest", placement.digest.clone()),
                ("oa-weights", up.weights.clone()),
                ("oa-layers", layer_list(&stages)),
                (
                    "oa-embeddings",
                    u8::from(stages.iter().any(|s| s.embeddings)).to_string(),
                ),
                (
                    "oa-head",
                    u8::from(stages.iter().any(|s| s.head)).to_string(),
                ),
                (
                    "oa-worker",
                    up.worker.clone().unwrap_or_else(|| "unset".into()),
                ),
            ];
            metadata.extend(build_pins(up));
            let mut files = vec![
                format!("startup-script={}", startup.display()),
                format!("oa-verify={}", check.display()),
            ];
            if up.head.remote() {
                // The head starts this machine's stages with the group's
                // own key.
                files.push(format!("ssh-keys={}", keys.display()));
            }
            if head {
                metadata.push(("oa-group-head", "1".into()));
                metadata.push(("oa-pylon", pylon.clone()));
                files.push(format!("oa-stage-ssh={}", stage_ssh.display()));
            }
            let metadata = metadata
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join(",");
            let mut guard = vec![
                "create".to_string(),
                name.clone(),
                "--machine-type".into(),
                placement.machine_type.clone(),
                "--zone".into(),
                up.zone.clone(),
                "--hours".into(),
                format!("{}", up.hours),
                "--project".into(),
                up.project.clone(),
                "--label".into(),
                "issue=12402".into(),
                "--label".into(),
                format!("pylon-group={}", up.group),
                "--json".into(),
            ];
            if up.spot {
                guard.push("--spot".into());
            }
            guard.extend([
                "--".into(),
                format!("--metadata-from-file={}", files.join(",")),
                format!("--metadata={metadata}"),
                format!("--boot-disk-size={}GB", disk_gb(stages.len())),
                "--boot-disk-type=hyperdisk-balanced".into(),
                // The weights load from this disk; its default throughput
                // would take most of an hour for a whole checkpoint.
                format!("--boot-disk-provisioned-throughput={DISK_MIBPS}"),
                format!("--boot-disk-provisioned-iops={DISK_IOPS}"),
                format!("--image-family={IMAGE_FAMILY}"),
                format!("--image-project={IMAGE_PROJECT}"),
                "--scopes=storage-ro".into(),
            ]);
            MachinePlan {
                name,
                host: host.host.clone(),
                machine_type: placement.machine_type.clone(),
                zone: up.zone.clone(),
                stages,
                head,
                guard,
            }
        })
        .collect();
    if up.head == HeadPlacement::Vm {
        // A small on-demand machine that only heads the group: losing a
        // spot head would lose the group.
        let name = format!("oa-{}-head", up.group);
        let mut metadata = vec![
            ("oa-group", up.group.clone()),
            ("oa-role", "head".into()),
            ("oa-group-head", "1".into()),
            ("oa-pylon", pylon),
        ];
        metadata.extend(build_pins(up));
        let metadata = metadata
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect::<Vec<_>>()
            .join(",");
        machines.push(MachinePlan {
            guard: vec![
                "create".into(),
                name.clone(),
                "--machine-type".into(),
                up.head_machine_type.clone(),
                "--zone".into(),
                up.zone.clone(),
                "--hours".into(),
                format!("{}", up.hours),
                "--project".into(),
                up.project.clone(),
                "--label".into(),
                "issue=12399".into(),
                "--label".into(),
                format!("pylon-group={}", up.group),
                "--json".into(),
                "--".into(),
                format!(
                    "--metadata-from-file=startup-script={},oa-verify={},oa-stage-ssh={}",
                    startup.display(),
                    check.display(),
                    stage_ssh.display()
                ),
                format!("--metadata={metadata}"),
                "--boot-disk-size=20GB".into(),
                format!("--image-family={HEAD_IMAGE_FAMILY}"),
                format!("--image-project={HEAD_IMAGE_PROJECT}"),
                "--scopes=storage-ro".into(),
            ],
            name,
            host: "head".into(),
            machine_type: up.head_machine_type.clone(),
            zone: up.zone.clone(),
            stages: Vec::new(),
            head: true,
        });
    }
    Ok(Plan {
        placement,
        machines,
    })
}

fn shell(argv: &[String]) -> String {
    argv.iter()
        .map(|a| {
            if !a.is_empty()
                && a.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_./=:,@+%".contains(&b))
            {
                a.clone()
            } else {
                format!("'{}'", a.replace('\'', r"'\''"))
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Ask the guard about every machine without creating any (`create
/// --dry-run`), and refuse when the machines together would pass the cap.
/// Returns the plan: each machine's guard command, the gcloud command the
/// guard would run, and its cost to expiry.
///
/// # Errors
///
/// When planning fails or the guard refuses a machine or the set.
pub fn dry_run(up: &Up, plan: &Plan) -> Result<Value, String> {
    let mut machines = Vec::new();
    let mut total = None;
    let mut cap = 0.0;
    for machine in &plan.machines {
        let mut words = machine.guard.clone();
        let split = words.iter().position(|w| w == "--").unwrap_or(words.len());
        words.insert(split, "--dry-run".into());
        let out = up.guard.run(&words)?;
        let summary: Value =
            serde_json::from_str(&out).map_err(|e| format!("the spend guard's plan: {e}"))?;
        let cost = summary["cost_if_run_to_expiry"].as_f64().unwrap_or(0.0);
        cap = summary["cap"].as_f64().unwrap_or(0.0);
        total = Some(match total {
            None => summary["total_after"].as_f64().unwrap_or(0.0),
            Some(sum) => sum + cost,
        });
        let gcloud: Vec<String> = summary["command"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        machines.push(json!({
            "name": machine.name,
            "host": machine.host,
            "machine_type": machine.machine_type,
            "zone": machine.zone,
            "stages": machine.stages,
            "head": machine.head,
            "guard": shell(&up.guard.argv(&machine.guard)),
            "gcloud": shell(&gcloud),
            "price_per_hour": summary["price_per_hour"],
            "hours": summary["hours"],
            "cost_to_expiry": cost,
            "expires_at": summary["expires_at"],
        }));
    }
    let total = total.unwrap_or(0.0);
    if total > cap + 1e-9 {
        return Err(format!(
            "the spend guard refused: these {} machines would bring budget {} to ${total:.2}, over its ${cap:.2} cap",
            plan.machines.len(),
            up.guard.budget
        ));
    }
    Ok(json!({
        "placement": plan.placement.name,
        "manifest": format!("sha256:{}", plan.placement.digest),
        "artifact": plan.placement.manifest.artifact(),
        "group": up.group,
        "budget": up.guard.budget,
        "cap": cap,
        "total_after": (total * 100.0).round() / 100.0,
        "machines": machines,
        "head": head_plan(up, plan),
        "build": build_summary(up),
    }))
}

/// Where the head runs, how this computer reaches it, and how the head
/// starts each stage.
fn head_plan(up: &Up, plan: &Plan) -> Value {
    let lane = group_lane(up, plan);
    let session = up
        .head
        .remote()
        .then(|| head_session(up, plan))
        .flatten()
        .map(|(program, args)| {
            shell(
                &std::iter::once(program.display().to_string())
                    .chain(args)
                    .collect::<Vec<_>>(),
            )
        });
    json!({
        "placement": up.head.word(),
        "machine": plan.machines.iter().find(|m| m.head).map(|m| m.name.clone()),
        "transport": up.transport,
        "session": session,
        "stages": lane.stages.iter().map(|s| json!({
            "name": s.name,
            "launch": shell(
                &std::iter::once(s.program.display().to_string())
                    .chain(s.args.iter().cloned())
                    .collect::<Vec<_>>(),
            ),
        })).collect::<Vec<_>>(),
    })
}

/// The program and arguments of a remote head's session: ssh to its
/// machine, or the tests' local stand-in.
fn head_session(up: &Up, plan: &Plan) -> Option<(PathBuf, Vec<String>)> {
    if let Some(local) = &up.local_head {
        return Some(local.clone());
    }
    let machine = plan.machines.iter().find(|m| m.head)?;
    Some((
        up.repo.join("scripts/pylon/v41-head-ssh.sh"),
        vec![
            machine.name.clone(),
            machine.zone.clone(),
            up.project.clone(),
        ],
    ))
}

/// The head's version 6 settings for `lane`, on top of `base`.
fn head_settings(up: &Up, lane: GroupLane, base: Settings) -> Settings {
    let mut settings = base;
    settings.pylon = format!("{}-head", up.group);
    settings.label = format!("DeepSeek-V4.1 {}", up.group)
        .chars()
        .take(64)
        .collect();
    settings.relay = up.relay.clone();
    settings.allow = up.allow.clone();
    settings.slots = 1;
    settings.max_tokens = up.max_output_tokens;
    settings.decide = None;
    settings.text = None;
    settings.dedicated = true;
    settings.vram_gb = 96;
    settings.group = Some(lane);
    settings.on = true;
    settings.changed_at = now();
    settings
}

/// One machine as `cloud.json` records it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    pub name: String,
    pub zone: String,
    /// The guard's `start` record.
    pub ledger_id: Option<String>,
    pub price_per_hour: Option<f64>,
    pub expires_epoch: Option<f64>,
    pub expires_at: Option<String>,
    pub deleted: bool,
}

/// `cloud.json`: the launch this home heads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub schema: String,
    /// `starting`, `running`, `stopping`, or `stopped`.
    pub state: String,
    pub pid: u32,
    pub group: String,
    pub placement: String,
    pub manifest: String,
    pub artifact: String,
    pub machine_type: String,
    pub budget: String,
    pub guard: PathBuf,
    pub guard_env: Vec<(String, String)>,
    pub relay: String,
    pub head: Option<String>,
    /// Where the head runs, and on which machine when it is in the zone.
    #[serde(default)]
    pub head_placement: HeadPlacement,
    #[serde(default)]
    pub head_machine: Option<String>,
    #[serde(default)]
    pub transport: Transport,
    /// `pinned`, or `development` for a launch without build pins
    /// (#12410). Empty in a record from before #12410.
    #[serde(default)]
    pub launch: String,
    /// The worker build's bundle hash every stage must announce.
    #[serde(default)]
    pub worker_build: Option<String>,
    /// The head's pylon build, for a head in the zone.
    #[serde(default)]
    pub pylon_build: Option<String>,
    pub generation: Option<u64>,
    pub started_at: u64,
    /// When the group ends ahead of the first expiry.
    pub ends_at: u64,
    pub machines: Vec<Machine>,
    pub ended: Option<Value>,
}

impl Record {
    fn guard(&self) -> Guard {
        Guard {
            python: "python3".into(),
            script: self.guard.clone(),
            budget: self.budget.clone(),
            cap: None,
            env: self.guard_env.clone(),
        }
    }
}

/// The launch record in `home`.
#[must_use]
pub fn load(home: &Path) -> Option<Record> {
    serde_json::from_slice(&std::fs::read(home.join(FILE)).ok()?).ok()
}

fn save(home: &Path, record: &Record) -> Result<(), String> {
    std::fs::create_dir_all(home).map_err(|e| e.to_string())?;
    let tmp = home.join(format!("{FILE}.tmp"));
    std::fs::write(
        &tmp,
        serde_json::to_vec_pretty(record).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, home.join(FILE)).map_err(|e| e.to_string())
}

fn running(record: &Record) -> bool {
    record.state != "stopped" && crate::online::process_alive(record.pid)
}

/// A launched group, headed by a supervisor in this process.
pub struct Launched {
    pub receipt: Value,
    record: Record,
    guard: Guard,
    /// The head this computer runs.
    supervisor: Option<tokio::task::JoinHandle<()>>,
    halt: Option<oneshot::Sender<()>>,
    /// The head in the zone, over its ssh session.
    remote: Option<RemoteHead>,
}

/// The group as the head reports it.
struct Observed {
    ready: bool,
    generation: u64,
    failed: Option<String>,
    reason: String,
    head: Option<String>,
}

/// The stage group lane for `plan`.
pub fn group_lane(up: &Up, plan: &Plan) -> GroupLane {
    let artifact = plan.placement.manifest.artifact();
    let stages = plan
        .machines
        .iter()
        .flat_map(|machine| {
            machine.stages.iter().map(move |stage| {
                let mut extra = vec![
                    "--layers".to_string(),
                    format!("{}-{}", stage.from, stage.to),
                    "--gpu".into(),
                    stage.gpu.to_string(),
                    "--manifest".into(),
                    format!("sha256:{}", plan.placement.digest),
                ];
                extra.extend(["--max-context".into(), up.max_context.to_string()]);
                if let Some(n) = up.max_step_tokens {
                    extra.extend(["--max-step-tokens".into(), n.to_string()]);
                }
                if let Some(n) = up.prefill_chunk {
                    extra.extend(["--prefill-chunk".into(), n.to_string()]);
                }
                if let Some(n) = up.max_input_tokens {
                    extra.extend(["--max-input-tokens".into(), n.to_string()]);
                }
                if let Some(backend) = &up.stage_backend {
                    extra.extend(["--backend".into(), backend.clone()]);
                }
                if stage.embeddings {
                    extra.push("--embeddings".into());
                }
                if stage.head {
                    extra.push("--head".into());
                }
                let port = DATA_PORT + stage.stage;
                if up.transport == Transport::Direct {
                    // Each stage listens for the previous stage's link.
                    match &up.local_stages {
                        Some(_) => extra.extend(["--data-listen".into(), "127.0.0.1:0".into()]),
                        None => extra.extend([
                            "--data-listen".into(),
                            format!("0.0.0.0:{port}"),
                            "--data-advertise".into(),
                            format!("{}:{port}", internal(up, &machine.name)),
                        ]),
                    }
                }
                let (program, args) = match &up.local_stages {
                    Some((program, args)) => {
                        (program.clone(), args.iter().cloned().chain(extra).collect())
                    }
                    // The head runs on this machine: start the stage here.
                    None if up.head.remote() && machine.head => {
                        (PathBuf::from("/opt/openagents/stage-run"), extra)
                    }
                    // The head runs in the zone: plain ssh with the
                    // group's key to the machine's internal name.
                    None if up.head.remote() => (
                        PathBuf::from("/opt/openagents/stage-ssh"),
                        [
                            "--ssh".to_string(),
                            format!("{}/{}", cloud_head::HOME_TOKEN, cloud_head::SSH_KEY),
                            format!("{}@{}", cloud_head::SSH_USER, internal(up, &machine.name)),
                            (REMOTE_PORT + stage.stage).to_string(),
                            "--".into(),
                        ]
                        .into_iter()
                        .chain(extra)
                        .collect(),
                    ),
                    None => (
                        up.repo.join("scripts/pylon/v41-stage-ssh.sh"),
                        [
                            machine.name.clone(),
                            machine.zone.clone(),
                            up.project.clone(),
                            (REMOTE_PORT + stage.stage).to_string(),
                            "--".into(),
                        ]
                        .into_iter()
                        .chain(extra)
                        .collect(),
                    ),
                };
                GroupStage {
                    name: stage.name.clone(),
                    program,
                    args,
                    from: stage.from,
                    to: stage.to,
                }
            })
        })
        .collect();
    GroupLane {
        id: up.group.clone(),
        placement: plan.placement.name.clone(),
        model: placement::MODEL.into(),
        artifact: artifact.clone(),
        manifest: format!("sha256:{}", plan.placement.digest),
        weights: up.stage_weights.clone().unwrap_or(artifact),
        runtime: up.stage_runtime.clone(),
        protocol: crate::stage_group::PROTOCOL,
        build: up.worker_build.clone(),
        deadline_secs: up.deadline_secs,
        max_output_tokens: up.max_output_tokens,
        sampling: up.sampling,
        mode: up.mode,
        stages,
        transport: up.transport,
        links: crate::stage_group::LinkPolicy::default(),
        step_timeout_ms: millis(up.step_timeout),
        health_timeout_ms: 10_000.min(millis(up.step_timeout)),
        startup_timeout_ms: millis(up.startup_timeout),
        // A killed CUDA stage frees about 75 GB of device memory before
        // the kernel reaps it, which took longer than 3 s on a G4 machine
        // (#12399); a stage that can't be confirmed gone stops the group.
        stop_timeout_ms: STOP_MS,
        max_restarts: up.max_restarts,
        backoff_ms: 1_000,
        max_backoff_ms: 30_000,
    }
}

/// Refuses a launch without the build pins it needs, unless it is a
/// development launch, and pins on a development launch (#12410).
///
/// # Errors
///
/// Names the missing pin.
pub fn check_pins(up: &Up) -> Result<(), String> {
    if up.development {
        if up.worker_build.is_some() || up.pylon_build.is_some() {
            return Err("a --development launch runs without build pins; drop --worker-build and --pylon-build, or --development".into());
        }
        return Ok(());
    }
    if up.worker_build.is_none() {
        return Err("pin the qualified worker build with --worker-build SHA256 (its BUILD.json's SHA-256), or launch with --development".into());
    }
    if up.head.remote() && up.pylon_build.is_none() {
        return Err("pin the head's pylon build with --pylon-build SHA256 (its BUILD.json's SHA-256), or launch with --development".into());
    }
    Ok(())
}

/// What the launch record and status say about the build pins.
fn launch_kind(up: &Up) -> &'static str {
    if up.development {
        "development"
    } else {
        "pinned"
    }
}

/// The launch's build pins, as a dry run reports them: `launch` is
/// `pinned` or `development`.
#[must_use]
pub fn build_summary(up: &Up) -> Value {
    json!({
        "launch": launch_kind(up),
        "worker_build": up.worker_build,
        "pylon_build": up.pylon_build,
    })
}

/// The build pins a machine's startup script checks (`oa-build`,
/// `oa-pylon-build`), when the launch names them.
fn build_pins(up: &Up) -> Vec<(&'static str, String)> {
    let mut pins = Vec::new();
    if let Some(build) = &up.worker_build {
        pins.push(("oa-build", build.clone()));
    }
    if let Some(build) = &up.pylon_build {
        pins.push(("oa-pylon-build", build.clone()));
    }
    pins
}

fn millis(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// The guard's `start` records for `names`, newest per machine.
fn starts(guard: &Guard, names: &[String]) -> Vec<Value> {
    let records = guard.records();
    names
        .iter()
        .filter_map(|name| {
            records
                .iter()
                .rev()
                .find(|r| r["event"] == "start" && r["vm"] == name.as_str())
                .cloned()
        })
        .collect()
}

/// Launch `up`: create every machine through the guard, start the group
/// head (on this computer, or in the zone with `--head stage0|vm`), and
/// wait until every stage is ready and the group is published.
///
/// # Errors
///
/// When the plan or the guard refuses (nothing is created), when a machine
/// can't be created (the ones created are deleted), or when the group isn't
/// ready in time (everything is stopped and deleted).
pub async fn launch(up: &Up) -> Result<Launched, String> {
    if let Some(held) = load(&up.home)
        && running(&held)
    {
        return Err(format!(
            "this computer already heads group {}; `pylon cloud down` stops it first",
            held.group
        ));
    }
    if up.worker.is_none() && up.local_stages.is_none() {
        return Err(
            "name the stage worker in its bucket with --worker gs://… (W11 builds it)".into(),
        );
    }
    if up.head.remote() && up.pylon.is_none() && up.local_head.is_none() {
        return Err(
            "name the pylon binary in its bucket with --pylon gs://… for a head in the zone".into(),
        );
    }
    check_pins(up)?;
    let plan = plan(up)?;
    let lane = group_lane(up, &plan);
    // A head in the zone runs the stage programs on its own machines.
    if up.head.remote() && up.local_stages.is_none() {
        lane.validate_elsewhere()?;
    } else {
        lane.validate()?;
    }
    // Every refusal the guard can give for the set comes before anything
    // is created.
    dry_run(up, &plan)?;
    // The stage machines carry the group key's public half from birth.
    let key = if up.head.remote() {
        Some(cloud_head::make_key(&key_dir(up), &up.group)?)
    } else {
        None
    };
    let _ = std::fs::remove_file(up.home.join(DOWN));
    let mut created: Vec<String> = Vec::new();
    for machine in &plan.machines {
        if let Err(why) = up.guard.run(&machine.guard) {
            for name in &created {
                let _ = up.guard.stop(name);
            }
            return Err(format!("{} was not created: {why}", machine.name));
        }
        created.push(machine.name.clone());
    }
    let ledger = starts(&up.guard, &created);
    let machines: Vec<Machine> = plan
        .machines
        .iter()
        .map(|m| {
            let start = ledger.iter().find(|r| r["vm"] == m.name.as_str());
            Machine {
                name: m.name.clone(),
                zone: m.zone.clone(),
                ledger_id: start.and_then(|r| r["id"].as_str().map(String::from)),
                price_per_hour: start.and_then(|r| r["price_per_hour"].as_f64()),
                expires_epoch: start.and_then(|r| r["expires_epoch"].as_f64()),
                expires_at: start.and_then(|r| r["expires_at"].as_str().map(String::from)),
                deleted: false,
            }
        })
        .collect();
    let first_expiry = machines
        .iter()
        .filter_map(|m| m.expires_epoch)
        .fold(f64::INFINITY, f64::min);
    let ends_at = if first_expiry.is_finite() {
        (first_expiry as u64).saturating_sub(up.expiry_margin.as_secs())
    } else {
        now() + (up.hours * 3600.0) as u64
    };
    let mut record = Record {
        schema: SCHEMA.into(),
        state: "starting".into(),
        pid: std::process::id(),
        group: up.group.clone(),
        placement: plan.placement.name.clone(),
        manifest: lane.manifest.clone(),
        artifact: lane.artifact.clone(),
        machine_type: plan.placement.machine_type.clone(),
        budget: up.guard.budget.clone(),
        guard: up.guard.script.clone(),
        guard_env: up.guard.env.clone(),
        relay: up.relay.clone(),
        head: None,
        head_placement: up.head,
        head_machine: plan
            .machines
            .iter()
            .find(|m| m.head)
            .map(|m| m.name.clone()),
        transport: up.transport,
        launch: launch_kind(up).into(),
        worker_build: up.worker_build.clone(),
        pylon_build: up.pylon_build.clone(),
        generation: None,
        started_at: now(),
        ends_at,
        machines,
        ended: None,
    };
    save(&up.home, &record)?;

    let mut launched = Launched {
        receipt: Value::Null,
        record: record.clone(),
        guard: up.guard.clone(),
        supervisor: None,
        halt: None,
        remote: None,
    };
    if let Some(key) = key {
        // The head runs in the zone: its settings and the group's key go
        // over its session, and this computer only talks to it.
        let settings = head_settings(up, lane, Settings::off(&format!("{}-head", up.group)));
        let Some((program, args)) = head_session(up, &plan) else {
            let why = "no machine in the plan runs the head".to_string();
            launched.end(&up.home, &why).await;
            return Err(why);
        };
        let start = json!({"settings": settings, "ssh_key": key.private});
        match RemoteHead::start(&program, &args, &start).await {
            Ok(remote) => launched.remote = Some(remote),
            Err(why) => {
                launched.end(&up.home, &why).await;
                return Err(why);
            }
        }
    } else {
        // This computer heads the group: version 6 settings with the lane.
        let base =
            share::load(&up.home)?.unwrap_or_else(|| Settings::off(&format!("{}-head", up.group)));
        let settings = head_settings(up, lane, base);
        share::save(&up.home, &settings)?;
        let (halt, halted) = oneshot::channel::<()>();
        let mut options = Options::foreground();
        options.every = up.every;
        launched.supervisor = Some(tokio::spawn(crate::online::supervise(
            up.home.clone(),
            Arc::new(Dedicated),
            options,
            Arc::new(|line| eprintln!("pylon: {line}")),
            async {
                let _ = halted.await;
            },
        )));
        launched.halt = Some(halt);
    }
    let started = Instant::now();
    let (generation, head) = loop {
        let group = launched.observe(&up.home, &up.group);
        if let Some(group) = &group {
            if let Some(failed) = &group.failed {
                let why = format!("the group stopped while starting: {failed}");
                launched.end(&up.home, &why).await;
                return Err(why);
            }
            if group.ready {
                break (group.generation, group.head.clone().unwrap_or_default());
            }
        }
        if started.elapsed() > up.ready_timeout {
            let why = format!(
                "the group was not ready after {} s{}",
                up.ready_timeout.as_secs(),
                group.map(|g| format!(": {}", g.reason)).unwrap_or_default()
            );
            launched.end(&up.home, &why).await;
            return Err(why);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    };
    record.state = "running".into();
    record.head = Some(head.clone());
    record.generation = Some(generation);
    save(&up.home, &record)?;
    launched.record = record.clone();
    launched.receipt = json!({
        "launched": true,
        "group": {
            "id": up.group,
            "head": head,
            "head_placement": up.head.word(),
            "head_machine": record.head_machine,
            "transport": up.transport,
            "model": format!("{}@{}", placement::MODEL, record.artifact),
            "artifact": record.artifact,
            "manifest": record.manifest,
            "generation": generation,
            "deadline_secs": up.deadline_secs,
            "max_output_tokens": up.max_output_tokens,
            "schedule": {
                "kind": "serial", "max_context_tokens": up.max_context,
                "max_step_tokens": up.max_step_tokens.or(up.prefill_chunk).unwrap_or(up.max_context),
                "prefill_chunk_tokens": up.prefill_chunk, "max_input_tokens": up.max_input_tokens,
            },
        },
        "placement": record.placement,
        "machine_type": record.machine_type,
        "machines": record.machines,
        "ledger": {"path": up.guard.ledger(), "starts": ledger},
        "ends_at": record.ends_at,
        "build": build_summary(up),
    });
    Ok(launched)
}

impl Launched {
    /// The launch record.
    #[must_use]
    pub fn record(&self) -> &Record {
        &self.record
    }

    /// The group as its head reports it: this computer's live file, or
    /// the remote head's latest status line.
    fn observe(&self, home: &Path, group: &str) -> Option<Observed> {
        if let Some(remote) = &self.remote {
            let state = remote.state();
            let failed = state.failed.clone().or_else(|| {
                state
                    .ended
                    .clone()
                    .or_else(|| state.stopped.then(|| "the head stopped".to_string()))
            });
            return Some(Observed {
                ready: state.ready && failed.is_none(),
                generation: state.generation,
                failed,
                reason: state.reason,
                head: state.head,
            });
        }
        let live = load_live(home)?.group?;
        let head = Identity::load_or_create(&home.join("provider.key"))
            .ok()
            .map(|identity| format!("30200:{}:{group}-head", identity.pubkey()));
        Some(Observed {
            ready: live.ready,
            generation: live.generation,
            failed: live.failed,
            reason: live.reason,
            head,
        })
    }

    /// Stop the head (it drains and publishes its beacons offline), then
    /// delete every machine through the guard, and record why.
    async fn end(&mut self, home: &Path, reason: &str) -> Value {
        self.record.state = "stopping".into();
        let _ = save(home, &self.record);
        let drain = crate::provider::DRAIN + Duration::from_secs(30);
        if let Some(remote) = self.remote.as_mut() {
            remote.stop(drain).await;
        }
        if let Some(supervisor) = self.supervisor.as_mut() {
            if let Ok(Some(mut settings)) = share::load(home)
                && settings.on
            {
                settings.on = false;
                settings.changed_at = now();
                let _ = share::save(home, &settings);
            }
            // The supervisor ends once it sees `on: false`; the halt is the
            // backstop.
            let deadline = Instant::now() + drain;
            while !supervisor.is_finished() && Instant::now() < deadline {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            if let Some(halt) = self.halt.take() {
                let _ = halt.send(());
            }
            let _ = supervisor.await;
        }
        let ended = delete(&self.guard, &mut self.record, reason);
        let _ = save(home, &self.record);
        ended
    }

    /// Head the group until `stop` resolves, `down` turns it off, the
    /// machines near their expiry, the budget passes its cap, or the group
    /// stops for good; then stop everything and return the stop receipt.
    pub async fn serve(
        mut self,
        home: &Path,
        up: &Up,
        stop: impl std::future::Future<Output = ()>,
    ) -> Value {
        tokio::pin!(stop);
        let mut budget_checked = Instant::now();
        let reason = loop {
            if now() >= self.record.ends_at {
                break "the machines reached their expiry".to_string();
            }
            let turned_off = self.supervisor.is_some()
                && share::load(home).ok().flatten().is_some_and(|s| !s.on);
            if turned_off || home.join(DOWN).exists() {
                break "pylon cloud down".to_string();
            }
            let group = self.observe(home, &self.record.group);
            if let Some(failed) = group.as_ref().and_then(|g| g.failed.clone()) {
                break format!("the group was lost: {failed}");
            }
            if budget_checked.elapsed() >= up.budget_every {
                budget_checked = Instant::now();
                if let Ok(report) = self.guard.report()
                    && report["remaining"].as_f64().is_some_and(|r| r < 0.0)
                {
                    break format!(
                        "budget {} passed its cap (remaining ${:.2})",
                        self.guard.budget,
                        report["remaining"].as_f64().unwrap_or(0.0)
                    );
                }
            }
            if let Some(group) = group
                && group.generation > 0
                && self.record.generation != Some(group.generation)
            {
                self.record.generation = Some(group.generation);
                let _ = save(home, &self.record);
            }
            tokio::select! {
                () = &mut stop => break "stopped on this computer".to_string(),
                () = tokio::time::sleep(up.every) => {}
            }
        };
        eprintln!("pylon cloud: ending group {}: {reason}", self.record.group);
        let ended = self.end(home, &reason).await;
        let _ = std::fs::remove_file(home.join(DOWN));
        ended
    }
}

/// Delete each machine through the guard and record the end.
fn delete(guard: &Guard, record: &mut Record, reason: &str) -> Value {
    let mut problems = Vec::new();
    for machine in &mut record.machines {
        if machine.deleted {
            continue;
        }
        match guard.stop(&machine.name) {
            Ok(()) => machine.deleted = true,
            Err(why) if why.contains("is not running in budget") => machine.deleted = true,
            Err(why) => problems.push(format!("{}: {why}", machine.name)),
        }
    }
    let records = guard.records();
    let stops: Vec<Value> = record
        .machines
        .iter()
        .filter_map(|m| {
            let id = m.ledger_id.as_deref()?;
            records
                .iter()
                .rev()
                .find(|r| r["event"] == "stop" && r["id"] == id)
                .cloned()
        })
        .collect();
    let cost: f64 = stops.iter().filter_map(|r| r["cost"].as_f64()).sum();
    let ended = json!({
        "reason": reason,
        "at": now(),
        "stops": stops,
        "cost": (cost * 1e6).round() / 1e6,
        "problems": problems,
    });
    record.state = "stopped".into();
    record.ended = Some(ended.clone());
    ended
}

/// `pylon cloud down` at `home`: turn the head off and wait for the
/// running `up` to stop the group and delete the machines; when no `up`
/// runs, publish the beacons offline and delete the machines here.
///
/// # Errors
///
/// When there is no launch, or the running `up` doesn't finish in `wait`.
pub async fn down_at(home: &Path, wait: Duration) -> Result<Value, String> {
    let Some(mut record) = load(home) else {
        return Err("this computer heads no cloud group".into());
    };
    if record.state == "stopped" {
        return Ok(json!({"stopped": true, "already": true, "ended": record.ended}));
    }
    if running(&record) {
        // A running `up` ends the group when it sees this file; one whose
        // head runs on this computer also sees its pylon turned off.
        std::fs::write(home.join(DOWN), b"").map_err(|e| e.to_string())?;
        if record.head_placement == HeadPlacement::Local
            && let Ok(Some(mut settings)) = share::load(home)
        {
            settings.on = false;
            settings.changed_at = now();
            share::save(home, &settings)?;
        }
        let started = Instant::now();
        loop {
            if let Some(held) = load(home)
                && held.state == "stopped"
            {
                return Ok(
                    json!({"stopped": true, "ended": held.ended, "machines": held.machines}),
                );
            }
            if started.elapsed() > wait {
                return Err(
                    "turned off, but the group is still stopping; `pylon cloud status` shows when it ends"
                        .into(),
                );
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }
    // No `up` runs: stop here.
    let _ = crate::online::down_at(home, Duration::from_secs(5)).await;
    let ended = delete(&record.guard(), &mut record, "pylon cloud down");
    save(home, &record)?;
    Ok(json!({"stopped": true, "ended": ended, "machines": record.machines}))
}

/// `pylon cloud status` at `home`.
///
/// # Errors
///
/// When there is no launch.
pub fn status_at(home: &Path) -> Result<(Value, String), String> {
    let record = load(home).ok_or("this computer heads no cloud group")?;
    let live = load_live(home);
    let group = live.as_ref().and_then(|l| l.group.clone());
    let report = record.guard().report().ok();
    let state = if record.state != "stopped" && !running(&record) {
        "lost (its `pylon cloud up` is gone; run `pylon cloud down`)".to_string()
    } else {
        record.state.clone()
    };
    let mut text = format!(
        "group    {} ({}, {} on {})\nstate    {state}\nhead     {} on {}, {} transport\n",
        record.group,
        record.placement,
        record.machines.len(),
        record.machine_type,
        record.head.as_deref().unwrap_or("not published"),
        record.head_machine.as_deref().unwrap_or("this computer"),
        match record.transport {
            Transport::Front => "front",
            Transport::Direct => "direct",
        },
    );
    text.push_str(&match (record.launch.as_str(), &record.worker_build) {
        ("pinned", Some(build)) => format!("build    worker bundle {build}\n"),
        ("development", _) => "build    development launch: no build pins\n".into(),
        _ => "build    not recorded\n".into(),
    });
    if let Some(g) = &group {
        text.push_str(&format!(
            "ready    {} at generation {}{}\n",
            if g.ready { "yes" } else { "no" },
            g.generation,
            if g.ready {
                String::new()
            } else {
                format!(" ({})", g.reason)
            }
        ));
        for s in &g.stages {
            text.push_str(&format!(
                "  stage {} {}\n",
                s.name,
                if s.ready { "ready" } else { "not ready" }
            ));
        }
    }
    for m in &record.machines {
        text.push_str(&format!(
            "machine  {} {} expires {}{}\n",
            m.name,
            m.zone,
            m.expires_at.as_deref().unwrap_or("?"),
            if m.deleted { " (deleted)" } else { "" }
        ));
    }
    if let Some(r) = &report {
        text.push_str(&format!(
            "budget   {} spent ${:.2}, remaining ${:.2} of ${:.2}\n",
            record.budget,
            r["spent"].as_f64().unwrap_or(0.0),
            r["remaining"].as_f64().unwrap_or(0.0),
            r["cap"].as_f64().unwrap_or(0.0),
        ));
    }
    if let Some(ended) = &record.ended {
        text.push_str(&format!(
            "ended    {} (cost ${:.2})\n",
            ended["reason"].as_str().unwrap_or("?"),
            ended["cost"].as_f64().unwrap_or(0.0)
        ));
    }
    Ok((
        json!({"record": record, "group": group, "budget": report, "state": state}),
        text.trim_end().into(),
    ))
}

/// Usage text for `pylon cloud`.
pub const USAGE: &str = "\
usage: openagents pylon cloud up|down|status [options]

Launch a DeepSeek-V4.1 group on rented machines through the spend guard,
head it from this computer or from a machine in its zone, and stop it. Run it from an OpenAgents
checkout on the one host that allocates for the budget.

  up PLACEMENT              Create the placement's machines through
                            scripts/dev/cloud-budget.py, wait until every
                            stage is ready, publish the group, and serve it
                            until `down`, the machines' expiry, the budget's
                            cap, or the loss of a machine. Placements:
                            four-single (4 x g4-standard-48), two-dual
                            (2 x g4-standard-96), and one-quad (1 x
                            g4-standard-192).
      --dry-run             Print each machine's guard and gcloud commands
                            and the budget total; create nothing.
      --worker GS_URL       The stage worker binary in a bucket (required to
                            launch).
      --weights GS_PREFIX   The checkpoint's bucket prefix, such as a copy
                            in the zone's region (default the us-central1
                            original). Stages still check every shard's
                            hash against the pin.
      --worker-build SHA256 The SHA-256 of the qualified worker's
                            BUILD.json (required); each stage machine
                            refuses any other build, and the head any
                            stage that announces another bundle.
      --pylon-build SHA256  The SHA-256 of the head's pylon BUILD.json
                            (required with --head stage0 or vm).
      --development         Launch without build pins; the launch is
                            recorded and reported as a development one.
      --hours H             Hours until each machine's expiry (default 4).
      --zone ZONE           Default us-east1-b.
      --machine-type TYPE   Another machine type for the placement.
      --on-demand           Not spot.
      --group SLUG          The group's slug (default v41-PLACEMENT).
      --budget NAME         Default deepseek-v41.
      --cap DOLLARS         The cap a new budget opens with.
      --deadline-secs N     The deadline the head declares (default 90,
                            at most 3600; past 90 it takes only
                            pylon.deadline.v1 jobs).
      --max-output-tokens N The output limit it declares (default 1024).
      --max-context N       Prompt and answer positions (default 8192).
      --max-step-tokens N   Step-buffer capacity, independent of context.
      --prefill-chunk N     Process prompts in bounded chunks; defaults the
                            step capacity to N. No overlap between chunks.
      --max-input-tokens N  Cap prompt admission below context capacity.
      --temperature T       The head's sampling temperature (default 0.6;
                            0 samples greedily).
      --top-p P             The head's top-p (default 0.95).
      --mode MODE           Render prompts in the model's thinking mode
                            (the default) or its chat mode (chat).
      --ready-secs N        How long the launch waits for every stage to
                            load, weights copy included (default 1800).
      --step-secs N         How long one step may take, a long prompt's
                            prefill included (default 30, at most 75).
      --allow NPUB          A key that may send jobs (repeatable); default
                            any key.
      --relay URL           The relay (default wss://relay.openagents.com).
      --head WHERE          Where the group head runs: local (this
                            computer, the default), stage0 (the machine
                            that runs stage 0), or vm (a small machine of
                            its own in the zone). This computer keeps the
                            spend guard and talks only to the head.
      --head-machine-type T The head machine's type with --head vm
                            (default e2-small).
      --pylon GS_URL        The pylon binary in a bucket, for a head in the
                            zone (required to launch one).
      --transport HOW       How stage state travels: front (through the
                            head, the default) or direct (stage to stage
                            over authenticated links inside the zone).
      --backend B           The stage backend: cuda (the default) or cpu.
  down                      Drain the group, publish its beacons offline,
                            and delete its machines through the guard.
  status                    The group, its stages, its machines, and the
                            budget.
  head --home DIR           Head a group on a machine in its zone for the
                            budget host's `up` (its ssh session runs this).
  --json                    One JSON document on standard output.";

/// The OpenAgents checkout holding the guard, found from the current
/// directory up.
fn repo() -> Result<PathBuf, String> {
    let mut dir = std::env::current_dir().map_err(|e| e.to_string())?;
    loop {
        if dir.join("scripts/dev/cloud-budget.py").is_file()
            && dir.join("scripts/pylon/v41-stage-startup.sh").is_file()
        {
            return Ok(dir);
        }
        if !dir.pop() {
            return Err("run `pylon cloud` from an OpenAgents checkout: it uses scripts/dev/cloud-budget.py and scripts/pylon/".into());
        }
    }
}

/// Parse `up`'s words.
///
/// # Errors
///
/// On an unknown or malformed option.
pub fn parse_up(
    words: &[String],
    repo: &Path,
    home: &Path,
    relay: &str,
) -> Result<(Up, bool), String> {
    use crate::share::{take_flag, take_number, take_values};
    let mut words = words.to_vec();
    let dry_run = take_flag(&mut words, "--dry-run");
    let on_demand = take_flag(&mut words, "--on-demand");
    let allow_any = take_flag(&mut words, "--allow-any");
    let value = |words: &mut Vec<String>, name: &str| -> Result<Option<String>, String> {
        Ok(take_values(words, name)?.pop())
    };
    let worker = value(&mut words, "--worker")?;
    let weights = value(&mut words, "--weights")?;
    let development = take_flag(&mut words, "--development");
    let worker_build = value(&mut words, "--worker-build")?;
    let pylon_build = value(&mut words, "--pylon-build")?;
    let max_context = value(&mut words, "--max-context")?;
    let max_step_tokens = value(&mut words, "--max-step-tokens")?;
    let prefill_chunk = value(&mut words, "--prefill-chunk")?;
    let max_input_tokens = value(&mut words, "--max-input-tokens")?;
    let hours = value(&mut words, "--hours")?;
    let zone = value(&mut words, "--zone")?;
    let machine_type = value(&mut words, "--machine-type")?;
    let group = value(&mut words, "--group")?;
    let budget = value(&mut words, "--budget")?;
    let cap = value(&mut words, "--cap")?;
    let deadline = take_number(&mut words, "--deadline-secs")?;
    let max_output = take_number(&mut words, "--max-output-tokens")?;
    let ready = take_number(&mut words, "--ready-secs")?;
    let step = take_number(&mut words, "--step-secs")?;
    let temperature = value(&mut words, "--temperature")?;
    let top_p = value(&mut words, "--top-p")?;
    let mode = value(&mut words, "--mode")?;
    let allow = take_values(&mut words, "--allow")?;
    let head = value(&mut words, "--head")?;
    let head_machine_type = value(&mut words, "--head-machine-type")?;
    let pylon = value(&mut words, "--pylon")?;
    let transport = value(&mut words, "--transport")?;
    let backend = value(&mut words, "--backend")?;
    let Some(placement) = words.first().cloned() else {
        return Err("name a placement: four-single, two-dual, or one-quad".into());
    };
    if let Some(extra) = words.get(1) {
        return Err(format!("unexpected argument `{extra}`"));
    }
    let mut up = Up::new(&placement, repo, home);
    up.relay = relay.into();
    up.worker = worker;
    up.pylon = pylon;
    if let Some(weights) = weights {
        if !weights.starts_with("gs://") {
            return Err("--weights takes a gs:// prefix".into());
        }
        // The startup script appends shard names to the prefix.
        up.weights = if weights.ends_with('/') {
            weights
        } else {
            format!("{weights}/")
        };
    }
    for (name, pin) in [
        ("--worker-build", &worker_build),
        ("--pylon-build", &pylon_build),
    ] {
        if pin
            .as_deref()
            .is_some_and(|p| p.len() != 64 || !p.bytes().all(|b| b.is_ascii_hexdigit()))
        {
            return Err(format!("{name} takes a SHA-256 in hex"));
        }
    }
    up.worker_build = worker_build.map(|p| p.to_ascii_lowercase());
    up.pylon_build = pylon_build.map(|p| p.to_ascii_lowercase());
    let size = |name: &str, text: Option<String>| -> Result<Option<usize>, String> {
        text.map(|s| {
            s.parse()
                .map_err(|_| format!("{name} takes a positive number"))
        })
        .transpose()
    };
    up.max_context = size("--max-context", max_context)?.unwrap_or(8192);
    up.max_step_tokens = size("--max-step-tokens", max_step_tokens)?;
    up.prefill_chunk = size("--prefill-chunk", prefill_chunk)?;
    up.max_input_tokens = size("--max-input-tokens", max_input_tokens)?;
    up.development = development;
    if let Some(head) = head {
        up.head = HeadPlacement::parse(&head)?;
    }
    if let Some(machine_type) = head_machine_type {
        if up.head != HeadPlacement::Vm {
            return Err("--head-machine-type goes with --head vm".into());
        }
        up.head_machine_type = machine_type;
    }
    if let Some(transport) = transport {
        up.transport = match transport.as_str() {
            "front" => Transport::Front,
            "direct" => Transport::Direct,
            _ => return Err("--transport takes front or direct".into()),
        };
    }
    if let Some(backend) = backend {
        up.stage_runtime = match backend.as_str() {
            "cuda" => RUNTIME_CUDA,
            "cpu" => RUNTIME,
            _ => return Err("--backend takes cuda or cpu".into()),
        }
        .into();
        up.stage_backend = Some(backend);
    }
    up.spot = !on_demand;
    up.machine_type = machine_type;
    if let Some(hours) = hours {
        up.hours = hours.parse().map_err(|_| "--hours takes a number")?;
    }
    if let Some(zone) = zone {
        up.zone = zone;
    }
    if let Some(group) = group {
        up.group = group;
    }
    if let Some(budget) = budget {
        up.guard.budget = budget;
    }
    if let Some(cap) = cap {
        up.guard.cap = Some(cap.parse().map_err(|_| "--cap takes dollars")?);
    }
    if let Some(deadline) = deadline {
        up.deadline_secs = u64::from(deadline);
    }
    if let Some(max_output) = max_output {
        up.max_output_tokens = max_output;
    }
    if temperature.is_some() || top_p.is_some() {
        let number = |v: Option<String>, name: &str, default: f64| -> Result<f64, String> {
            v.map_or(Ok(default), |v| {
                v.parse().map_err(|_| format!("{name} takes a number"))
            })
        };
        up.sampling = Sampling::new(
            number(temperature, "--temperature", 0.6)?,
            number(top_p, "--top-p", 0.95)?,
            0,
        )?;
    }
    match mode.as_deref() {
        None => {}
        Some("thinking") => up.mode = crate::stage_group::PromptMode::Thinking,
        Some("chat") => up.mode = crate::stage_group::PromptMode::Chat,
        Some(other) => return Err(format!("--mode {other} is not thinking or chat")),
    }
    if let Some(ready) = ready {
        if ready == 0 {
            return Err("--ready-secs must be more than zero".into());
        }
        up.ready_timeout = Duration::from_secs(u64::from(ready));
    }
    if let Some(step) = step {
        // The group's own check bounds a step at 75 s.
        if !(1..=75).contains(&step) {
            return Err("--step-secs takes 1 to 75".into());
        }
        up.step_timeout = Duration::from_secs(u64::from(step));
    }
    if !allow_any && !allow.is_empty() {
        let mut keys = BTreeSet::new();
        for value in allow {
            for item in value.split(',').filter(|s| !s.is_empty()) {
                keys.insert(
                    crate::identity::hex_pubkey(item.trim())
                        .ok_or_else(|| format!("`{item}` is not a key"))?,
                );
            }
        }
        up.allow = Some(keys);
    }
    Ok((up, dry_run))
}

fn emit(json_out: bool, value: &Value, text: &str) {
    if crate::envelope::take(value) {
        return;
    }
    if json_out {
        println!("{value}");
    } else {
        println!("{text}");
    }
}

/// `pylon cloud WORDS`.
///
/// # Errors
///
/// As the subcommand.
pub async fn command(json_out: bool, words: &[String], relay: &str) -> Result<(), String> {
    let home = crate::home();
    match words.first().map(String::as_str) {
        Some("up") => {
            let repo = repo()?;
            let (up, dry) = parse_up(&words[1..], &repo, &home, relay)?;
            let plan = plan(&up)?;
            if dry {
                let value = dry_run(&up, &plan)?;
                let mut text = format!(
                    "dry run: {} for group {} ({} machines), budget {} to ${:.2} of ${:.2} if every machine runs to its expiry; nothing created\n",
                    value["placement"].as_str().unwrap_or(""),
                    up.group,
                    plan.machines.len(),
                    up.guard.budget,
                    value["total_after"].as_f64().unwrap_or(0.0),
                    value["cap"].as_f64().unwrap_or(0.0),
                );
                for m in value["machines"].as_array().into_iter().flatten() {
                    text.push_str(&format!(
                        "\n{}: {}\n  guard:  {}\n  gcloud: {}\n",
                        m["name"].as_str().unwrap_or(""),
                        m["stages"]
                            .as_array()
                            .into_iter()
                            .flatten()
                            .map(|s| format!(
                                "{} layers {}-{}",
                                s["name"].as_str().unwrap_or(""),
                                s["from"],
                                s["to"]
                            ))
                            .collect::<Vec<_>>()
                            .join(", "),
                        m["guard"].as_str().unwrap_or(""),
                        m["gcloud"].as_str().unwrap_or(""),
                    ));
                }
                let head = &value["head"];
                text.push_str(&format!(
                    "\nhead on {} ({}), {} transport\n",
                    head["machine"].as_str().unwrap_or("this computer"),
                    head["placement"].as_str().unwrap_or(""),
                    head["transport"].as_str().unwrap_or(""),
                ));
                if let Some(session) = head["session"].as_str() {
                    text.push_str(&format!("  session: {session}\n"));
                }
                for stage in head["stages"].as_array().into_iter().flatten() {
                    text.push_str(&format!(
                        "  {}: {}\n",
                        stage["name"].as_str().unwrap_or(""),
                        stage["launch"].as_str().unwrap_or("")
                    ));
                }
                text.push_str(&match up.worker_build.as_deref() {
                    _ if up.development => "\ndevelopment launch: no build pins\n".to_string(),
                    Some(build) => format!("\nworker bundle pinned to {build}\n"),
                    None => "\nno worker bundle pin; pass --worker-build or --development\n".into(),
                });
                emit(json_out, &value, text.trim_end());
                return Ok(());
            }
            let launched = launch(&up).await?;
            let receipt = launched.receipt.clone();
            let ready = format!(
                "group {} is ready at generation {} and published as {}\n{} machines; ledger {}\nCtrl-C or `pylon cloud down` stops it and deletes the machines.",
                up.group,
                receipt["group"]["generation"],
                receipt["group"]["head"].as_str().unwrap_or(""),
                launched.record().machines.len(),
                up.guard.ledger().display(),
            );
            // With --json the one document comes at the end.
            if json_out {
                eprintln!("{ready}");
            } else {
                println!("{ready}");
            }
            let ended = launched.serve(&home, &up, interrupted()).await;
            let mut value = receipt;
            value["ended"] = ended.clone();
            emit(
                json_out,
                &value,
                &format!(
                    "group {} ended: {}; machines deleted, cost ${:.2}",
                    up.group,
                    ended["reason"].as_str().unwrap_or(""),
                    ended["cost"].as_f64().unwrap_or(0.0)
                ),
            );
            Ok(())
        }
        Some("down") => {
            let value = down_at(&home, crate::provider::DRAIN + Duration::from_secs(120)).await?;
            let text = format!(
                "the group stopped; {}",
                value["ended"]["reason"]
                    .as_str()
                    .unwrap_or("its machines are deleted")
            );
            emit(json_out, &value, &text);
            Ok(())
        }
        Some("status") => {
            let (value, text) = status_at(&home)?;
            emit(json_out, &value, &text);
            Ok(())
        }
        Some("head") => {
            let mut rest = words[1..].to_vec();
            let dir = crate::share::take_values(&mut rest, "--home")?
                .pop()
                .map_or(home, PathBuf::from);
            if let Some(extra) = rest.first() {
                return Err(format!("unexpected argument `{extra}`"));
            }
            cloud_head::serve(&dir, Duration::from_secs(1)).await
        }
        Some("help" | "--help") | None => {
            emit(json_out, &json!({"usage": USAGE}), USAGE);
            Ok(())
        }
        Some(other) => Err(format!("unknown cloud command `{other}`\n\n{USAGE}")),
    }
}

/// Ctrl-C or SIGTERM.
async fn interrupted() {
    #[cfg(unix)]
    {
        let mut term =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).ok();
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            () = async { match term.as_mut() { Some(t) => { t.recv().await; } None => std::future::pending().await } } => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
