//! Frozen paired edit evaluations. Agent claims never replace independent checks.

use crate::{
    demand::Partition,
    recipes::{self, Artifact},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Arm {
    pub id: String,
    pub model: String,
    pub context: ContextKind,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextKind {
    Current,
    Summary,
    Capability,
    Recipes,
    Reviewed,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Case {
    pub id: String,
    pub family: String,
    pub partition: Partition,
    pub available_ms: u64,
    pub repository: PathBuf,
    pub base: String,
    pub task: String,
    pub index: Artifact,
    #[serde(default)]
    pub reviewed_index: Option<Artifact>,
    pub acceptance: BTreeMap<String, String>,
    pub companion_paths: Vec<String>,
    pub new_paths: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Plan {
    pub schema: String,
    pub frozen_ms: u64,
    pub index_cutoff_ms: u64,
    pub training_families: BTreeSet<String>,
    pub cases: Vec<Case>,
    pub arms: Vec<Arm>,
    pub repetitions: usize,
    pub timeout_secs: u64,
    pub max_output_bytes: u64,
    pub build_cost_usd: Option<f64>,
    pub refresh_cost_usd: Option<f64>,
    pub check_cost_usd: Option<f64>,
    pub baseline: String,
    pub candidate: String,
    pub min_families: usize,
    pub allowed_quality_loss: f64,
    pub max_cost_ratio: f64,
    pub scope: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    ProcessObserved,
    Fixture,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EngineResult {
    pub origin: Origin,
    pub model: String,
    pub exit: Option<i32>,
    pub timed_out: bool,
    pub usage: Option<(u64, u64)>,
    pub model_cost_usd: Option<f64>,
    pub navigation_calls: usize,
    pub output_digest: String,
    pub error: Option<String>,
}
pub trait Runner {
    fn run(
        &mut self,
        root: &Path,
        prompt: &str,
        arm: &Arm,
        plan: &Plan,
        output: &Path,
    ) -> Result<EngineResult, String>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Trial {
    pub case: String,
    pub family: String,
    pub arm: String,
    pub repetition: usize,
    pub plan_digest: String,
    pub index_digest: String,
    pub base: String,
    pub prompt_digest: String,
    pub result_tree: String,
    pub diff_digest: String,
    pub engine: EngineResult,
    pub accepted: bool,
    pub checks_exit: Option<i32>,
    pub checks_digest: String,
    pub missing_companions: Vec<String>,
    pub missing_new_paths: Vec<String>,
    pub elapsed_ms: u64,
    pub guide_source_bytes: u64,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub plan_digest: String,
    pub trials: Vec<Trial>,
    pub promotion_passed: bool,
    pub reasons: Vec<String>,
    pub quality_delta: f64,
    pub paired_quality_interval: Option<(f64, f64)>,
    pub cost_ratio: Option<f64>,
    pub totals: Value,
}

pub fn plan_digest(plan: &Plan) -> Result<String, String> {
    Ok(atif::digest(
        &serde_json::to_value(plan).map_err(|e| e.to_string())?,
    ))
}

pub fn validate(plan: &Plan) -> Result<(), String> {
    if plan.schema != "openagents.task-index-eval-plan.v1"
        || plan.repetitions == 0
        || plan.repetitions > 8
        || plan.arms.len() < 2
        || plan.arms.len() > 8
        || plan.cases.is_empty()
        || plan.cases.len() > 100
        || plan.timeout_secs == 0
        || plan.timeout_secs > 300
        || plan.max_output_bytes > 16 * 1024 * 1024
        || plan.index_cutoff_ms > plan.frozen_ms
    {
        return Err("Choose a bounded, frozen evaluation plan.".into());
    }
    let mut arms = BTreeSet::new();
    for arm in &plan.arms {
        if !arms.insert(&arm.id) || arm.id.is_empty() || arm.model.is_empty() {
            return Err("Each evaluation arm needs a unique ID and model.".into());
        }
    }
    if !arms.contains(&plan.baseline)
        || !arms.contains(&plan.candidate)
        || plan.baseline == plan.candidate
    {
        return Err("Choose distinct baseline and candidate arms.".into());
    }
    if !plan.allowed_quality_loss.is_finite()
        || !(0.0..=1.0).contains(&plan.allowed_quality_loss)
        || !plan.max_cost_ratio.is_finite()
        || plan.max_cost_ratio <= 0.0
        || plan.min_families < 2
    {
        return Err("Choose explicit quality, cost, and family requirements.".into());
    }
    for value in [
        plan.build_cost_usd,
        plan.refresh_cost_usd,
        plan.check_cost_usd,
    ]
    .into_iter()
    .flatten()
    {
        if !value.is_finite() || value < 0.0 {
            return Err("Measured costs must be finite and nonnegative.".into());
        }
    }
    let mut cases = BTreeSet::new();
    let mut families = BTreeMap::new();
    for case in &plan.cases {
        if !cases.insert(&case.id)
            || plan.training_families.contains(&case.family)
            || matches!(case.partition, Partition::Training | Partition::Unassigned)
            || case.available_ms < plan.index_cutoff_ms
            || case.available_ms > plan.frozen_ms
            || case.acceptance.is_empty()
        {
            return Err(
                "Evaluation tasks must be held out, dated, and independently checked.".into(),
            );
        }
        if let Some(previous) = families.insert(&case.family, case.partition) {
            if previous != case.partition {
                return Err("Related tasks cannot cross data partitions.".into());
            }
        }
        for path in case
            .acceptance
            .keys()
            .chain(&case.companion_paths)
            .chain(&case.new_paths)
        {
            recipes::relative(path)?;
        }
        if case
            .acceptance
            .keys()
            .any(|p| !p.starts_with("tests/") || !p.ends_with(".rs"))
        {
            return Err("Independent acceptance code must be a Rust integration test.".into());
        }
        let context =
            recipes::Context::open(&case.repository, &case.base, &case.index.snapshot.workspace)?;
        if context.snapshot.commit != case.index.snapshot.commit {
            return Err("An index uses source newer than its task base.".into());
        }
        context.verify(&case.index)?;
        if let Some(reviewed) = &case.reviewed_index {
            context.verify(reviewed)?;
        }
    }
    Ok(())
}

fn prompt(case: &Case, arm: &Arm) -> Result<String, String> {
    let files = recipes::git(
        &case.repository,
        &["ls-tree", "-r", "--name-only", &case.base],
    )?;
    let mut context = String::new();
    match arm.context {
        ContextKind::Current => {}
        ContextKind::Summary => {
            context = case
                .index
                .recipes
                .iter()
                .map(|r| r.description.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        }
        ContextKind::Capability => {
            context = case
                .index
                .recipes
                .iter()
                .map(|r| {
                    format!(
                        "{}: {}",
                        r.capability,
                        r.roles
                            .iter()
                            .flat_map(|r| r.references.iter().map(|r| r.path.as_str()))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        ContextKind::Recipes | ContextKind::Reviewed => {
            let index = if matches!(arm.context, ContextKind::Reviewed) {
                case.reviewed_index
                    .as_ref()
                    .ok_or("This task has no separately reviewed recipe arm.")?
            } else {
                &case.index
            };
            let request = crate::lookup::Request {
                question: case.task.clone(),
                operation: None,
                snapshot: index.snapshot.clone(),
                max_bytes: 16000,
                conditions: BTreeMap::new(),
                file_candidates: vec![],
                query_vector: None,
                recipe_vectors: BTreeMap::new(),
                decision: None,
            };
            context = serde_json::to_string(&crate::lookup::query(index, &request)?)
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(format!(
        "Implement the requested change in this isolated repository. Read the source as needed. Stay within this directory. Do not edit Cargo.toml or files under tests/. Independent acceptance tests run after you finish. Do not claim that a recipe proves the edit correct.\n\nTask:\n{}\n\nExisting files:\n{}\n\nAdditional repository guide:\n{}",
        case.task,
        String::from_utf8_lossy(&files),
        context
    ))
}

struct Worktree {
    repository: PathBuf,
    path: PathBuf,
}
impl Drop for Worktree {
    fn drop(&mut self) {
        let _ = recipes::git(
            &self.repository,
            &[
                "worktree",
                "remove",
                "--force",
                &self.path.to_string_lossy(),
            ],
        );
    }
}

pub fn run(
    plan: &Plan,
    scratch: &Path,
    target: &Path,
    runner: &mut impl Runner,
) -> Result<Report, String> {
    validate(plan)?;
    if !scratch.is_absolute() || !target.is_absolute() {
        return Err("Choose absolute scratch and Cargo target directories.".into());
    }
    crate::private::write(
        scratch,
        "run.json",
        b"{\"schema\":\"openagents.task-evaluation-run.v1\"}",
    )?;
    let mut options = std::fs::OpenOptions::new();
    options.create(true).truncate(false).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options
        .open(scratch.join("writer.lock"))
        .map_err(|e| e.to_string())?;
    lock.try_lock()
        .map_err(|_| "Another process is running this evaluation directory.")?;
    let digest = plan_digest(plan)?;
    if scratch.join("plan.json").exists() {
        let previous: Plan = crate::read_json(&scratch.join("plan.json"))?;
        if plan_digest(&previous)? != digest {
            return Err("Use a new run directory for a different frozen plan.".into());
        }
    }
    crate::private::write(
        scratch,
        "plan.json",
        &serde_json::to_vec(plan).map_err(|e| e.to_string())?,
    )?;
    let mut trials = Vec::new();
    for rep in 0..plan.repetitions {
        for case in &plan.cases {
            for arm in &plan.arms {
                let name =
                    crate::digest(format!("{}:{}:{}:{rep}", digest, case.id, arm.id).as_bytes());
                let trial_dir = scratch.join(&name);
                if trial_dir.join("result.json").exists() {
                    let previous: Trial = crate::read_json(&trial_dir.join("result.json"))?;
                    report(plan, vec![previous.clone()])?;
                    if previous.case != case.id
                        || previous.arm != arm.id
                        || previous.repetition != rep
                    {
                        return Err("A saved trial has a different identity.".into());
                    }
                    trials.push(previous);
                    continue;
                }
                crate::private::write(&trial_dir, "trial.json", b"{}")?;
                let root = trial_dir.join("worktree");
                recipes::git(
                    &case.repository,
                    &[
                        "worktree",
                        "add",
                        "--quiet",
                        "--detach",
                        &root.to_string_lossy(),
                        &case.base,
                    ],
                )?;
                let _worktree = Worktree {
                    repository: case.repository.clone(),
                    path: root.clone(),
                };
                if target.starts_with(&root) {
                    return Err(
                        "Keep the Cargo target directory outside the evaluation checkout.".into(),
                    );
                }
                let prompt = prompt(case, arm)?;
                let started = std::time::Instant::now();
                let engine = match runner.run(&root, &prompt, arm, plan, &trial_dir) {
                    Ok(result) => result,
                    Err(error) => {
                        crate::private::write(&trial_dir,"failure.json",&serde_json::to_vec(&json!({"case":case.id,"arm":arm.id,"repetition":rep,"plan_digest":digest,"error":error})).map_err(|e|e.to_string())?)?;
                        return Err(error);
                    }
                };
                let changed =
                    String::from_utf8_lossy(&recipes::git(&root, &["diff", "--name-only"])?)
                        .lines()
                        .map(str::to_owned)
                        .collect::<BTreeSet<_>>();
                let missing_companions = case
                    .companion_paths
                    .iter()
                    .filter(|p| !changed.contains(*p))
                    .cloned()
                    .collect::<Vec<_>>();
                let missing_new_paths = case
                    .new_paths
                    .iter()
                    .filter(|p| !root.join(p).is_file())
                    .cloned()
                    .collect::<Vec<_>>();
                let mut manifest_changed = changed
                    .iter()
                    .any(|p| p == "Cargo.toml" || p.ends_with("/Cargo.toml"));
                let added = String::from_utf8_lossy(&recipes::git(
                    &root,
                    &["ls-files", "--others", "--exclude-standard"],
                )?)
                .into_owned();
                manifest_changed |= added.lines().any(|p| {
                    p == "build.rs" || p.starts_with(".cargo/") || p.ends_with("Cargo.toml")
                });
                for (path, contents) in &case.acceptance {
                    let mut current = root.clone();
                    for part in Path::new(path).components() {
                        current.push(part);
                        if current.is_symlink() {
                            manifest_changed = true;
                            break;
                        }
                    }
                    if manifest_changed {
                        break;
                    }
                    let destination = root.join(path);
                    if let Some(parent) = destination.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    if destination.is_symlink() {
                        std::fs::remove_file(&destination).map_err(|e| e.to_string())?;
                    }
                    std::fs::write(destination, contents).map_err(|e| e.to_string())?;
                }
                // Check commands come from this runner, never from a trace or model reply.
                let check = if manifest_changed {
                    CommandResult {
                        exit: None,
                        timed_out: false,
                    }
                } else {
                    command(
                        &root,
                        "cargo",
                        &[
                            "test".into(),
                            "--offline".into(),
                            "--manifest-path".into(),
                            root.join("Cargo.toml").to_string_lossy().into_owned(),
                        ],
                        Some(target),
                        plan.timeout_secs,
                        plan.max_output_bytes,
                        &trial_dir,
                        "checks",
                    )?
                };
                recipes::git(&root, &["add", "-A"])?;
                let tree = String::from_utf8_lossy(&recipes::git(&root, &["write-tree"])?)
                    .trim()
                    .into();
                let diff = recipes::git(&root, &["diff", "--cached", "--binary"])?;
                crate::private::write(&trial_dir, "change.patch", &diff)?;
                let stdout = std::fs::read(trial_dir.join("checks.stdout")).unwrap_or_default();
                let stderr = std::fs::read(trial_dir.join("checks.stderr")).unwrap_or_default();
                let accepted = engine.exit == Some(0)
                    && !engine.timed_out
                    && engine.error.is_none()
                    && check.exit == Some(0)
                    && !check.timed_out
                    && !manifest_changed
                    && missing_companions.is_empty()
                    && missing_new_paths.is_empty();
                let trial = Trial {
                    case: case.id.clone(),
                    family: case.family.clone(),
                    arm: arm.id.clone(),
                    repetition: rep,
                    plan_digest: digest.clone(),
                    index_digest: atif::digest(
                        &serde_json::to_value(&case.index).map_err(|e| e.to_string())?,
                    ),
                    base: case.base.clone(),
                    prompt_digest: crate::digest(prompt.as_bytes()),
                    result_tree: tree,
                    diff_digest: crate::digest(&diff),
                    engine,
                    accepted,
                    checks_exit: check.exit,
                    checks_digest: crate::digest(&[stdout, stderr].concat()),
                    missing_companions,
                    missing_new_paths,
                    elapsed_ms: started.elapsed().as_millis() as u64,
                    guide_source_bytes: case
                        .index
                        .recipes
                        .iter()
                        .flat_map(|r| &r.roles)
                        .flat_map(|r| &r.references)
                        .map(|r| r.excerpt.len() as u64)
                        .sum(),
                    error: manifest_changed
                        .then(|| "The engine changed protected build or test paths.".into()),
                };
                crate::private::write(
                    &trial_dir,
                    "result.json",
                    &serde_json::to_vec(&trial).map_err(|e| e.to_string())?,
                )?;
                trials.push(trial);
                let partial = report(plan, trials.clone())?;
                crate::private::write(
                    scratch,
                    "report.json",
                    &serde_json::to_vec(&partial).map_err(|e| e.to_string())?,
                )?;
            }
        }
    }
    report(plan, trials)
}

pub struct CommandResult {
    pub exit: Option<i32>,
    pub timed_out: bool,
}
struct Child(std::process::Child);
impl Child {
    fn kill(&mut self) {
        #[cfg(unix)]
        unsafe {
            libc::kill(-(self.0.id() as i32), libc::SIGKILL);
        }
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
impl Drop for Child {
    fn drop(&mut self) {
        if self.0.try_wait().ok().flatten().is_none() {
            self.kill();
        }
    }
}

pub fn command(
    root: &Path,
    program: &str,
    args: &[String],
    target: Option<&Path>,
    timeout: u64,
    max_bytes: u64,
    output: &Path,
    label: &str,
) -> Result<CommandResult, String> {
    let stdout = output.join(format!("{label}.stdout"));
    let stderr = output.join(format!("{label}.stderr"));
    let create = |p: &Path| {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        options.open(p).map_err(|e| e.to_string())
    };
    let mut cmd = std::process::Command::new(program);
    cmd.current_dir(root)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(create(&stdout)?)
        .stderr(create(&stderr)?);
    if let Some(target) = target {
        cmd.env("CARGO_TARGET_DIR", target);
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            cmd.env(
                "RUSTUP_HOME",
                std::env::var_os("RUSTUP_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".rustup")),
            );
            cmd.env(
                "CARGO_HOME",
                std::env::var_os("CARGO_HOME")
                    .map(PathBuf::from)
                    .unwrap_or_else(|| home.join(".cargo")),
            );
        }
        let home = output.join("check-home");
        std::fs::create_dir_all(&home).map_err(|e| e.to_string())?;
        cmd.env("HOME", home);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        cmd.process_group(0);
    }
    let mut child = Child(
        cmd.spawn()
            .map_err(|_| "The selected evaluation command could not start.")?,
    );
    let started = std::time::Instant::now();
    loop {
        if let Some(status) = child.0.try_wait().map_err(|e| e.to_string())? {
            return Ok(CommandResult {
                exit: status.code(),
                timed_out: false,
            });
        }
        let size = std::fs::metadata(&stdout).map_or(0, |m| m.len())
            + std::fs::metadata(&stderr).map_or(0, |m| m.len());
        if started.elapsed().as_secs() >= timeout || size > max_bytes {
            child.kill();
            return Ok(CommandResult {
                exit: None,
                timed_out: true,
            });
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
}

pub struct Claude;
impl Runner for Claude {
    fn run(
        &mut self,
        root: &Path,
        prompt: &str,
        arm: &Arm,
        plan: &Plan,
        output: &Path,
    ) -> Result<EngineResult, String> {
        let capacity = std::process::Command::new("openagents")
            .args(["capacity", "check", "claude"])
            .output()
            .map_err(|_| "The Claude capacity check could not start.")?;
        if !capacity.status.success() {
            return Err("Claude capacity is unavailable; stop this evaluation run.".into());
        }
        let args = vec![
            "-p".into(),
            prompt.into(),
            "--model".into(),
            arm.model.clone(),
            "--restricted".into(),
            "--permission-mode".into(),
            "acceptEdits".into(),
            "--permission-prompts".into(),
            "none".into(),
            "--no-session-persistence".into(),
            "--setting-sources".into(),
            "".into(),
            "--strict-mcp-config".into(),
            "--mcp-config".into(),
            "{\"mcpServers\":{}}".into(),
            "--tools".into(),
            "Read,Grep,Glob,Edit,Write".into(),
            "--max-budget-usd".into(),
            "0.40".into(),
            "--output-format".into(),
            "stream-json".into(),
            "--verbose".into(),
        ];
        let result = command(
            root,
            "claude",
            &args,
            None,
            plan.timeout_secs,
            plan.max_output_bytes,
            output,
            "engine",
        )?;
        let bytes = std::fs::read(output.join("engine.stdout")).map_err(|e| e.to_string())?;
        let mut usage = None;
        let mut cost = None;
        let mut model = arm.model.clone();
        let mut navigation = 0;
        let mut error = None;
        for line in bytes.split(|b| *b == b'\n') {
            let Ok(v) = serde_json::from_slice::<Value>(line) else {
                continue;
            };
            if v["type"] == "rate_limit_event"
                && v.pointer("/rate_limit_info/status").and_then(Value::as_str) == Some("rejected")
            {
                if let Some(reset) = v
                    .pointer("/rate_limit_info/resetsAt")
                    .and_then(Value::as_u64)
                {
                    let _ = std::process::Command::new("openagents")
                        .args([
                            "capacity",
                            "record",
                            "claude",
                            "--reset",
                            &reset.to_string(),
                        ])
                        .output();
                }
                return Err("Claude reported a usage limit; stop this session and keep the partial results.".into());
            }
            if let Some(name) = v.pointer("/message/model").and_then(Value::as_str) {
                model = name.into();
            }
            if let Some(blocks) = v.pointer("/message/content").and_then(Value::as_array) {
                navigation += blocks
                    .iter()
                    .filter(|b| matches!(b["name"].as_str(), Some("Read" | "Grep" | "Glob")))
                    .count();
            }
            if v["type"] == "result" {
                usage = v.get("usage").and_then(|u| {
                    Some((
                        u.get("input_tokens")?.as_u64()?
                            + u.get("cache_read_input_tokens")
                                .and_then(Value::as_u64)
                                .unwrap_or(0)
                            + u.get("cache_creation_input_tokens")
                                .and_then(Value::as_u64)
                                .unwrap_or(0),
                        u.get("output_tokens")?.as_u64()?,
                    ))
                });
                cost = v.get("total_cost_usd").and_then(Value::as_f64);
                if v["is_error"] == true {
                    error = Some(
                        v.get("subtype")
                            .and_then(Value::as_str)
                            .unwrap_or("engine_error")
                            .into(),
                    );
                }
            }
        }
        Ok(EngineResult {
            origin: Origin::ProcessObserved,
            model,
            exit: result.exit,
            timed_out: result.timed_out,
            usage,
            model_cost_usd: cost,
            navigation_calls: navigation,
            output_digest: crate::digest(&bytes),
            error,
        })
    }
}

pub fn report(plan: &Plan, trials: Vec<Trial>) -> Result<Report, String> {
    let digest = plan_digest(plan)?;
    let mut reasons = Vec::new();
    let mut unique = BTreeSet::new();
    for trial in &trials {
        if trial
            .engine
            .model_cost_usd
            .is_some_and(|v| !v.is_finite() || v < 0.0)
            || trial.accepted
                && (trial.checks_exit != Some(0)
                    || trial.engine.exit != Some(0)
                    || trial.engine.timed_out
                    || trial.engine.error.is_some()
                    || !trial.missing_companions.is_empty()
                    || !trial.missing_new_paths.is_empty()
                    || trial.error.is_some())
        {
            return Err("A trial's acceptance or cost contradicts its recorded checks.".into());
        }
        if trial.plan_digest != digest
            || !unique.insert((&trial.case, &trial.arm, trial.repetition))
        {
            return Err("Trial identities do not match the frozen plan.".into());
        }
        let case = plan
            .cases
            .iter()
            .find(|c| c.id == trial.case)
            .ok_or("A trial names an unknown task.")?;
        let arm = plan
            .arms
            .iter()
            .find(|a| a.id == trial.arm)
            .ok_or("A trial names an unknown arm.")?;
        if trial.prompt_digest != crate::digest(prompt(case, arm)?.as_bytes()) {
            return Err("A trial used a different prompt or guide.".into());
        }
        if trial.family != case.family
            || trial.base != case.base
            || trial.index_digest
                != atif::digest(&serde_json::to_value(&case.index).map_err(|e| e.to_string())?)
            || !plan.arms.iter().any(|a| a.id == trial.arm)
            || trial.repetition >= plan.repetitions
        {
            return Err("A trial does not match its task, index, or arm.".into());
        }
    }
    let expected = plan.cases.len() * plan.arms.len() * plan.repetitions;
    if trials.len() != expected {
        reasons.push("The paired experiment is incomplete.".into());
    }
    if trials
        .iter()
        .any(|t| t.engine.origin != Origin::ProcessObserved)
    {
        reasons.push("Fixture runs do not establish model quality.".into());
    }
    let families: BTreeSet<_> = plan.cases.iter().map(|c| &c.family).collect();
    if families.len() < plan.min_families {
        reasons.push("Too few independent task families for promotion.".into());
    }
    let rate = |arm: &str, family: Option<&str>| {
        let rows: Vec<_> = trials
            .iter()
            .filter(|t| t.arm == arm && family.is_none_or(|f| t.family == f))
            .collect();
        if rows.is_empty() {
            0.0
        } else {
            rows.iter().filter(|t| t.accepted).count() as f64 / rows.len() as f64
        }
    };
    let quality_delta = rate(&plan.candidate, None) - rate(&plan.baseline, None);
    let differences: Vec<_> = families
        .iter()
        .map(|f| rate(&plan.candidate, Some(f)) - rate(&plan.baseline, Some(f)))
        .collect();
    let interval = bootstrap(&differences);
    if interval.is_none_or(|(low, _)| low < -plan.allowed_quality_loss) {
        reasons.push("The paired quality interval does not meet the frozen tolerance.".into());
    }
    let cost_per_accepted = |arm: &str, overhead: Option<f64>| -> Option<f64> {
        let rows: Vec<_> = trials.iter().filter(|t| t.arm == arm).collect();
        let accepted = rows.iter().filter(|t| t.accepted).count();
        if accepted == 0 {
            return None;
        }
        let costs = rows
            .iter()
            .map(|t| t.engine.model_cost_usd)
            .collect::<Option<Vec<_>>>()?;
        Some((costs.iter().sum::<f64>() + overhead?) / accepted as f64)
    };
    let baseline_cost = cost_per_accepted(&plan.baseline, plan.check_cost_usd);
    let overhead = plan
        .build_cost_usd
        .zip(plan.refresh_cost_usd)
        .zip(plan.check_cost_usd)
        .map(|((a, b), c)| a + b + c);
    let candidate_cost = cost_per_accepted(&plan.candidate, overhead);
    let ratio = candidate_cost
        .zip(baseline_cost)
        .and_then(|(a, b)| if b > 0.0 { Some(a / b) } else { None });
    if ratio.is_none_or(|r| r > plan.max_cost_ratio) {
        reasons.push("Complete cost per accepted task does not meet the frozen target.".into());
    }
    let mut totals = BTreeMap::new();
    for arm in &plan.arms {
        let rows: Vec<_> = trials.iter().filter(|t| t.arm == arm.id).collect();
        totals.insert(arm.id.clone(),json!({"trials":rows.len(),"accepted":rows.iter().filter(|t|t.accepted).count(),"model_cost_usd":rows.iter().map(|t|t.engine.model_cost_usd).collect::<Option<Vec<_>>>().map(|c|c.iter().sum::<f64>()),"navigation_calls":rows.iter().map(|t|t.engine.navigation_calls).sum::<usize>(),"elapsed_ms":rows.iter().map(|t|t.elapsed_ms).sum::<u64>()}));
    }
    Ok(Report {
        schema: "openagents.task-index-evaluation.v1".into(),
        plan_digest: digest,
        trials,
        promotion_passed: reasons.is_empty(),
        reasons,
        quality_delta,
        paired_quality_interval: interval,
        cost_ratio: ratio,
        totals: serde_json::to_value(totals).map_err(|e| e.to_string())?,
    })
}

fn bootstrap(values: &[f64]) -> Option<(f64, f64)> {
    if values.len() < 2 {
        return None;
    }
    let mut state = 11718u64;
    let mut samples = Vec::new();
    for _ in 0..2000 {
        let mut sum = 0.0;
        for _ in values {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            sum += values[(state >> 32) as usize % values.len()];
        }
        samples.push(sum / values.len() as f64);
    }
    samples.sort_by(f64::total_cmp);
    Some((samples[50], samples[1949]))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReplayReceipt {
    pub schema: String,
    pub plan_digest: String,
    pub trial_digest: String,
    pub case: String,
    pub family: String,
    pub base: String,
    pub diff_digest: String,
    pub result_tree: String,
    pub checks_exit: Option<i32>,
    pub verified: bool,
}

pub fn replay_trial(
    plan: &Plan,
    trial: &Trial,
    patch: &[u8],
    scratch: &Path,
    target: &Path,
) -> Result<ReplayReceipt, String> {
    validate(plan)?;
    report(plan, vec![trial.clone()])?;
    if crate::digest(patch) != trial.diff_digest {
        return Err("The replay patch does not match its recorded digest.".into());
    }
    let case = plan
        .cases
        .iter()
        .find(|c| c.id == trial.case)
        .ok_or("The replay task is unknown.")?;
    crate::private::write(scratch, "change.patch", patch)?;
    let root = scratch.join("worktree");
    recipes::git(
        &case.repository,
        &[
            "worktree",
            "add",
            "--quiet",
            "--detach",
            &root.to_string_lossy(),
            &case.base,
        ],
    )?;
    let _worktree = Worktree {
        repository: case.repository.clone(),
        path: root.clone(),
    };
    recipes::git(
        &root,
        &[
            "apply",
            "--binary",
            &scratch.join("change.patch").to_string_lossy(),
        ],
    )?;
    for (path, contents) in &case.acceptance {
        let mut at = root.clone();
        for part in Path::new(path).components() {
            at.push(part);
            if at.is_symlink() {
                return Err("A replay check cannot follow a symbolic link.".into());
            }
        }
        if std::fs::read_to_string(root.join(path)).ok().as_deref() != Some(contents.as_str()) {
            return Err("The replay changed the independent acceptance test.".into());
        }
    }
    let manifest = recipes::source(&case.repository, &case.index.snapshot, "Cargo.toml")?;
    if root.join("Cargo.toml").is_symlink()
        || std::fs::read_to_string(root.join("Cargo.toml"))
            .ok()
            .as_deref()
            != Some(&manifest)
        || root.join("build.rs").exists()
        || root.join(".cargo").exists()
    {
        return Err("The replay changed protected build inputs.".into());
    }
    recipes::git(&root, &["add", "-A"])?;
    let tree = String::from_utf8_lossy(&recipes::git(&root, &["write-tree"])?)
        .trim()
        .to_owned();
    if tree != trial.result_tree {
        return Err("The replay creates a different source tree.".into());
    }
    let checks = command(
        &root,
        "cargo",
        &[
            "test".into(),
            "--offline".into(),
            "--manifest-path".into(),
            root.join("Cargo.toml").to_string_lossy().into_owned(),
        ],
        Some(target),
        plan.timeout_secs,
        plan.max_output_bytes,
        scratch,
        "checks",
    )?;
    Ok(ReplayReceipt {
        schema: "openagents.task-index-replay.v1".into(),
        plan_digest: plan_digest(plan)?,
        trial_digest: atif::digest(&serde_json::to_value(trial).map_err(|e| e.to_string())?),
        case: trial.case.clone(),
        family: trial.family.clone(),
        base: trial.base.clone(),
        diff_digest: trial.diff_digest.clone(),
        result_tree: tree,
        checks_exit: checks.exit,
        verified: checks.exit == trial.checks_exit && !checks.timed_out,
    })
}
