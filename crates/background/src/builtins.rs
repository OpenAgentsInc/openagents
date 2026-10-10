//! The other built-in background processes (phase 3), in the spec's
//! order, and the steps their actions take. Each ships off, except
//! keeping `~/openagents` on `main` on CoderOS and pruning stale Coder
//! worktrees on CoderOS and cloud pool hosts (#10292), and the nightly
//! decision recalibration there (#10387); a person turns one on with
//! `openagents background resume ID`, `/background`, or the desktop's
//! Background settings, and edits it like any rule.
//!
//! None is a limit on anyone's use: the usage summary only says what ran.

use std::collections::BTreeMap;
use std::io::Write as _;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::{Answer, Judge, Question, Setting, Step, StepOutcome};
use crate::paths::{self, Layout, bytes, show};
use crate::plan::Env;
use crate::rule::{
    self, ALWAYS, Action, Classes, Level, Origin, Rule, Safety, Trigger, Watched, disk,
};
use crate::services::{Claim, CoderRun, Failure, Services};
use crate::store::{State, write_atomic};

/// `background.flake`: how sure Jev must be that a failure is the same
/// intermittent failure as an earlier one. Unmeasured.
pub const FLAKE: Setting = Setting::new("background.flake", 0.8);

/// The prompt of the nightly simulated-user QA run.
pub const QA_PROMPT: &str = "Run the simulated-user QA described in docs/qa/simulated-users.md \
(scripts/qa/simulated-users.sh) against this checkout, and file each confirmed problem as a \
GitHub issue labeled `qa`, as that document says. Do not change product code in this run.";

/// Whether this computer runs CoderOS.
#[must_use]
pub fn on_coderos() -> bool {
    Path::new("/etc/coderos").is_dir()
}

/// The file a cloud pool host's setup writes (a GCE pool VM, a Boat
/// sandbox from the Coder host template): `scripts/cloud/coder-host-setup.sh`
/// and the pool host agent create it.
pub const POOL_HOST_MARKER: &str = "/etc/openagents/pool-host";

/// Whether this computer is a cloud pool host.
#[must_use]
pub fn on_pool_host() -> bool {
    Path::new(POOL_HOST_MARKER).is_file()
}

/// Whether this computer is a host that runs Coder for others to come
/// back to: CoderOS or a cloud pool host, not a person's own computer.
/// Stale worktree pruning ships on there (#10292); disk cleanup stays a
/// person's choice everywhere.
#[must_use]
pub fn coder_host() -> bool {
    on_coderos() || on_pool_host()
}

fn base(id: &str, name: &str) -> Rule {
    let mut rule = disk();
    rule.id = id.into();
    rule.name = name.into();
    rule.origin = Origin::BuiltIn;
    rule.enabled = false;
    rule.triggers = Vec::new();
    rule.actions = Vec::new();
    rule.cooldown_secs = 0;
    rule.classes = Classes {
        agent_targets: Vec::new(),
        checkouts: Vec::new(),
        claude_checkouts: Vec::new(),
        ..disk().classes
    };
    rule.safety = Safety {
        allow: Vec::new(),
        deny: Vec::new(),
        report: Vec::new(),
    };
    rule
}

/// The built-in rule `id` other than `disk`.
#[must_use]
pub fn rule(id: &str) -> Option<Rule> {
    Some(match id {
        "worktrees" => {
            let mut rule = base(id, "Stale worktree pruning");
            rule.enabled = coder_host();
            rule.triggers = vec![Trigger::Daily { at: "03:30".into() }];
            rule.actions = vec![Action::PruneWorktrees];
            rule.goal.start = Level {
                bytes: ALWAYS,
                percent: 0,
            };
            rule.goal.stop = rule.goal.start;
            rule.goal.emergency = Level {
                bytes: 0,
                percent: 0,
            };
            rule.classes.worktree_days = 7;
            rule.safety.allow = vec!["~/.openagents/worktrees".into()];
            rule
        }
        "claims" => {
            let mut rule = base(id, "Stale issue claims");
            rule.triggers = vec![Trigger::Interval { every_secs: 3600 }];
            rule.actions = vec![Action::ReleaseStaleClaims { idle_hours: 6 }];
            rule
        }
        "checkout" => {
            let mut rule = base(id, "Keep ~/openagents on main");
            rule.enabled = on_coderos();
            rule.triggers = vec![Trigger::Interval { every_secs: 3600 }];
            rule.actions = vec![Action::GitFastForward {
                repo: "~/openagents".into(),
                branch: Some("main".into()),
            }];
            rule
        }
        "health" => {
            let mut rule = base(id, "Relay and host health");
            rule.triggers = vec![Trigger::Interval { every_secs: 300 }];
            rule.actions = vec![
                Action::HealthWatch {
                    target: Watched::Relay,
                    failures: 3,
                },
                Action::HealthWatch {
                    target: Watched::Host,
                    failures: 3,
                },
            ];
            rule
        }
        "flakes" => {
            let mut rule = base(id, "Flaky-test watch");
            rule.triggers = vec![Trigger::Interval { every_secs: 1800 }];
            rule.actions = vec![Action::FlakeWatch];
            rule
        }
        "qa" => {
            let mut rule = base(id, "Nightly simulated-user QA");
            rule.triggers = vec![Trigger::Daily { at: "02:00".into() }];
            rule.actions = vec![Action::StartCoderRun {
                prompt: QA_PROMPT.into(),
                workspace: None,
                chat: None,
            }];
            rule
        }
        "usage" => {
            let mut rule = base(id, "Daily usage summary");
            rule.triggers = vec![Trigger::Daily { at: "21:00".into() }];
            rule.actions = vec![Action::UsageSummary];
            rule
        }
        "rotate" => {
            let mut rule = base(id, "Log and trace rotation");
            rule.triggers = vec![Trigger::Daily { at: "04:00".into() }];
            rule.actions = vec![Action::RotateLogs {
                compress_days: 7,
                keep_days: 30,
            }];
            rule
        }
        "calibration" => {
            let mut rule = base(id, "Nightly decision recalibration");
            rule.enabled = coder_host();
            rule.triggers = vec![Trigger::Daily { at: "04:30".into() }];
            rule.actions = vec![Action::Recalibrate];
            rule
        }
        _ => return None,
    })
}

/// What the host's powers are for one evaluation: the judge and the
/// services, either of which may be missing.
#[derive(Clone, Copy, Default)]
pub struct Powers<'a> {
    pub judge: Option<&'a dyn Judge>,
    pub services: Option<&'a dyn Services>,
}

fn step(kind: &str, target: Option<String>, outcome: StepOutcome, detail: String) -> Step {
    Step {
        kind: kind.into(),
        target,
        outcome,
        detail,
    }
}

fn missing(kind: &str) -> Step {
    step(
        kind,
        None,
        StepOutcome::Failed,
        "the host cannot do this here".into(),
    )
}

/// The steps of one phase 3 action. `None` for an action this module
/// does not take.
#[must_use]
pub fn steps(
    env: &Env<'_>,
    rule: &Rule,
    action: &Action,
    powers: Powers<'_>,
    dry_run: bool,
) -> Option<Vec<Step>> {
    let services = powers.services;
    Some(match action {
        Action::StartCoderRun {
            prompt,
            workspace,
            chat,
        } => {
            let Some(services) = services else {
                return Some(vec![missing("start_coder_run")]);
            };
            if dry_run {
                return Some(vec![step(
                    "start_coder_run",
                    workspace.clone(),
                    StepOutcome::Would,
                    match chat {
                        Some(chat) => format!(
                            "would post into chat {}: {}",
                            short(chat),
                            first_line(prompt)
                        ),
                        None => format!("would start a Coder run: {}", first_line(prompt)),
                    },
                )]);
            }
            let run = CoderRun {
                title: rule.name.clone(),
                prompt: prompt.clone(),
                workspace: workspace
                    .as_ref()
                    .map(|w| rule::expand(w, &env.layout.home).display().to_string()),
                chat: chat.clone(),
            };
            vec![match services.start_coder_run(&run) {
                Ok(task) => step(
                    "start_coder_run",
                    Some(task.clone()),
                    StepOutcome::Done,
                    match &run.chat {
                        Some(chat) => {
                            format!("Posted into chat {}: {}.", short(chat), rule.name)
                        }
                        None => format!("Started Coder run {}: {}.", short(&task), rule.name),
                    },
                ),
                Err(why) => step("start_coder_run", None, StepOutcome::Failed, why),
            }]
        }
        Action::RunPlugin { plugin, input } => {
            let Some(services) = services else {
                return Some(vec![missing("run_plugin")]);
            };
            if dry_run {
                return Some(vec![step(
                    "run_plugin",
                    Some(plugin.clone()),
                    StepOutcome::Would,
                    format!("would run {plugin}"),
                )]);
            }
            vec![match services.run_plugin(plugin, input) {
                Ok(reply) => step(
                    "run_plugin",
                    Some(plugin.clone()),
                    StepOutcome::Done,
                    first_line(&reply),
                ),
                Err(why) => step("run_plugin", Some(plugin.clone()), StepOutcome::Failed, why),
            }]
        }
        Action::ReleaseStaleClaims { idle_hours } => {
            let Some(services) = services else {
                return Some(vec![missing("release_claim")]);
            };
            claims(services, *idle_hours, env.now, dry_run)
        }
        Action::HealthWatch { target, failures } => {
            let Some(services) = services else {
                return Some(vec![missing("health")]);
            };
            vec![health(
                env.layout, rule, services, *target, *failures, dry_run,
            )]
        }
        Action::FlakeWatch => {
            let Some(services) = services else {
                return Some(vec![missing("flake")]);
            };
            flakes(env.layout, rule, services, powers.judge, env.now, dry_run)
        }
        Action::UsageSummary => vec![summary(env.layout, services, env.now)],
        Action::Recalibrate => {
            let Some(services) = services else {
                return Some(vec![missing("recalibrate")]);
            };
            vec![match services.recalibrate(dry_run) {
                Ok(line) => step(
                    "recalibrate",
                    None,
                    if dry_run {
                        StepOutcome::Would
                    } else {
                        StepOutcome::Done
                    },
                    line,
                ),
                Err(why) => step("recalibrate", None, StepOutcome::Failed, why),
            }]
        }
        Action::RotateLogs {
            compress_days,
            keep_days,
        } => rotate(env, *compress_days, *keep_days, dry_run),
        _ => return None,
    })
}

fn first_line(text: &str) -> String {
    let line = text.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    line.chars().take(200).collect()
}

fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

fn claims(services: &dyn Services, idle_hours: u64, now: u64, dry_run: bool) -> Vec<Step> {
    let found: Vec<Claim> = match services.stale_claims(idle_hours, now) {
        Ok(found) => found,
        Err(why) => return vec![step("release_claim", None, StepOutcome::Failed, why)],
    };
    found
        .into_iter()
        .map(|claim| {
            let target = Some(format!("{}#{}", claim.repository, claim.number));
            if dry_run {
                return step(
                    "release_claim",
                    target,
                    StepOutcome::Would,
                    format!("would release #{}: {}", claim.number, claim.why),
                );
            }
            match services.release_claim(&claim) {
                Ok(()) => step(
                    "release_claim",
                    target,
                    StepOutcome::Done,
                    format!("Released #{}: {}.", claim.number, claim.why),
                ),
                Err(why) => step("release_claim", target, StepOutcome::Failed, why),
            }
        })
        .collect()
}

fn health(
    layout: &Layout,
    rule: &Rule,
    services: &dyn Services,
    target: Watched,
    failures: u32,
    dry_run: bool,
) -> Step {
    let name = target.name();
    if dry_run {
        return step(
            "health",
            Some(name.into()),
            StepOutcome::Would,
            format!("would probe the {name}"),
        );
    }
    let key = name.to_owned();
    match services.probe(target) {
        Ok(()) => {
            State::update(layout, &rule.id, |state| {
                state.failures.remove(&key);
            });
            step(
                "health",
                Some(key),
                StepOutcome::Skipped,
                format!("the {name} answers"),
            )
        }
        Err(why) => {
            let mut count = 0;
            State::update(layout, &rule.id, |state| {
                let n = state.failures.entry(key.clone()).or_default();
                *n += 1;
                count = *n;
            });
            if count < failures {
                return step(
                    "health",
                    Some(key),
                    StepOutcome::Skipped,
                    format!("the {name} did not answer ({count} of {failures}): {why}"),
                );
            }
            State::update(layout, &rule.id, |state| {
                state.failures.remove(&key);
            });
            match services.restart(target) {
                Ok(done) => step(
                    "health",
                    Some(key),
                    StepOutcome::Done,
                    format!("The {name} did not answer {count} times; {done}"),
                ),
                Err(restart) => step(
                    "health",
                    Some(key),
                    StepOutcome::Failed,
                    format!(
                        "The {name} did not answer {count} times and could not be restarted: {restart}"
                    ),
                ),
            }
        }
    }
}

/// A failure seen more than once: a known flake.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flake {
    pub test: String,
    /// The first failure's output, its signature.
    pub output: String,
    pub tasks: Vec<String>,
    pub seen: u64,
    /// The issue it was reported in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<String>,
}

/// What the flake watch remembers: known flakes, and the failures seen
/// once, by test.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Flakes {
    #[serde(default)]
    pub known: BTreeMap<String, Flake>,
    #[serde(default)]
    pub once: BTreeMap<String, Failure>,
    /// The newest failure time already looked at.
    #[serde(default)]
    pub through: u64,
}

impl Flakes {
    #[must_use]
    pub fn load(layout: &Layout) -> Self {
        std::fs::read(layout.flakes())
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    fn save(&self, layout: &Layout) {
        if let Ok(bytes) = serde_json::to_vec_pretty(self) {
            let _ = write_atomic(&layout.flakes(), &bytes);
        }
    }
}

/// The question the flake watch asks about two failures of one test.
pub const SAME_FAILURE: &str = "Do these two failures of the same test show the same \
intermittent failure (the same error, in another run), rather than two different problems?";

/// The state Jev reads for two failures of one test: code-built.
#[must_use]
pub fn flake_state(test: &str, earlier: &str, now: &str) -> String {
    format!(
        "Test: {test}\nEarlier failure (another Coder run):\n{}\nThis failure:\n{}",
        clip(earlier),
        clip(now)
    )
}

fn clip(text: &str) -> String {
    text.lines().take(30).collect::<Vec<_>>().join("\n")
}

fn same(judge: &dyn Judge, test: &str, earlier: &str, now: &str) -> Result<f64, String> {
    let answers = judge.ask(
        &flake_state(test, earlier, now),
        &[("same".into(), Question::Noul(SAME_FAILURE.into()))],
    )?;
    answers
        .get("same")
        .and_then(|answer: &Answer| answer.noul)
        .ok_or_else(|| "the judge did not answer".into())
}

fn flakes(
    layout: &Layout,
    rule: &Rule,
    services: &dyn Services,
    judge: Option<&dyn Judge>,
    now: u64,
    dry_run: bool,
) -> Vec<Step> {
    let mut memory = Flakes::load(layout);
    let since = if memory.through == 0 {
        now.saturating_sub(86_400)
    } else {
        memory.through + 1
    };
    let failures = match services.failures(since) {
        Ok(failures) => failures,
        Err(why) => return vec![step("flake", None, StepOutcome::Failed, why)],
    };
    let Some(judge) = judge else {
        return vec![step(
            "flake",
            None,
            StepOutcome::Skipped,
            format!("{}: no judge is set up here", FLAKE.name),
        )];
    };
    let mut steps = Vec::new();
    for failure in failures {
        memory.through = memory.through.max(failure.at);
        let earlier = memory
            .known
            .get(&failure.test)
            .map(|flake| (flake.output.clone(), flake.tasks.clone()))
            .or_else(|| {
                memory
                    .once
                    .get(&failure.test)
                    .map(|f| (f.output.clone(), vec![f.task.clone()]))
            });
        let Some((output, tasks)) = earlier else {
            memory.once.insert(failure.test.clone(), failure);
            continue;
        };
        if tasks.contains(&failure.task) {
            continue;
        }
        let p = match same(judge, &failure.test, &output, &failure.output) {
            Ok(p) => p,
            Err(why) => {
                steps.push(step("flake", Some(failure.test), StepOutcome::Failed, why));
                continue;
            }
        };
        let known = memory.known.contains_key(&failure.test);
        if !FLAKE.yes(p) {
            if !known {
                // A different failure: it becomes the one remembered.
                memory.once.insert(failure.test.clone(), failure);
            }
            continue;
        }
        if dry_run {
            steps.push(step(
                "flake",
                Some(failure.test.clone()),
                StepOutcome::Would,
                format!("would report {} as flaky ({p:.2})", failure.test),
            ));
            continue;
        }
        let flake = memory
            .known
            .entry(failure.test.clone())
            .or_insert_with(|| Flake {
                test: failure.test.clone(),
                output: output.clone(),
                tasks: tasks.clone(),
                seen: 1,
                issue: None,
            });
        flake.seen += 1;
        flake.tasks.push(failure.task.clone());
        memory.once.remove(&failure.test);
        let title = format!("Flaky test: {}", failure.test);
        let body = format!(
            "{} failed the same way in {} Coder runs ({}); {} {:.2}.\n\n```\n{}\n```",
            failure.test,
            flake.seen,
            flake.tasks.join(", "),
            FLAKE.name,
            p,
            clip(&failure.output)
        );
        match services.report_issue(&title, &body, flake.issue.as_deref()) {
            Ok(issue) => {
                let detail = if flake.issue.is_some() {
                    format!(
                        "{} failed again the same way; noted on {issue}.",
                        failure.test
                    )
                } else {
                    format!("{} looks flaky; reported in {issue}.", failure.test)
                };
                flake.issue = Some(issue);
                steps.push(step("flake", Some(failure.test), StepOutcome::Done, detail));
            }
            Err(why) => steps.push(step("flake", Some(failure.test), StepOutcome::Failed, why)),
        }
    }
    if !dry_run {
        memory.save(layout);
        State::update(layout, &rule.id, |_| {});
    }
    steps
}

/// One line: what Coder ran and finished today, what it cost, and what
/// the background rules freed. Never a limit.
fn summary(layout: &Layout, services: Option<&dyn Services>, now: u64) -> Step {
    let since = now.saturating_sub(86_400);
    let mut parts = Vec::new();
    match services.map(|s| s.usage(since)) {
        Some(Ok(usage)) => {
            let mut runs = format!(
                "{} Coder run{} ended ({} finished, {} failed)",
                usage.ended,
                if usage.ended == 1 { "" } else { "s" },
                usage.succeeded,
                usage.failed
            );
            if let Some(cost) = usage.cost_microusd {
                runs.push_str(&format!(", {}", dollars(cost)));
                if usage.unpriced > 0 {
                    runs.push_str(&format!(" ({} unpriced)", usage.unpriced));
                }
            }
            parts.push(runs);
        }
        Some(Err(why)) => parts.push(format!("Coder runs unknown: {why}")),
        None => {}
    }
    let records = crate::view::log(layout, None, Some(since), usize::MAX);
    let freed: u64 = records.iter().map(|r| r.freed_sum).sum();
    let ran = records.len();
    if ran > 0 {
        parts.push(format!(
            "background rules ran {ran} time{}{}",
            if ran == 1 { "" } else { "s" },
            if freed > 0 {
                format!(" and freed {}", bytes(freed))
            } else {
                String::new()
            }
        ));
    }
    let line = if parts.is_empty() {
        "Today: nothing ran.".to_owned()
    } else {
        let mut line = format!("Today: {}.", parts.join("; "));
        if let Some(first) = line.get_mut(7..8) {
            first.make_ascii_uppercase();
        }
        line
    };
    step("usage", None, StepOutcome::Done, line)
}

/// Micro-dollars as `$1.23`.
#[must_use]
pub fn dollars(micro: u64) -> String {
    let cents = (micro + 5_000) / 10_000;
    format!("${}.{:02}", cents / 100, cents % 100)
}

/// The folders log rotation works in, under `~/.openagents`: traces, the
/// host's and Coder's logs, run artifacts, and gate logs.
#[must_use]
pub fn log_roots(layout: &Layout) -> Vec<PathBuf> {
    ["traces", "logs", "log", "run-artifacts", "gate/logs"]
        .iter()
        .map(|name| layout.openagents.join(name))
        .collect()
}

/// A file log rotation may touch: a log, trace, or text artifact.
fn rotatable(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    [".log", ".jsonl", ".txt", ".out", ".err", ".diff", ".json"]
        .iter()
        .any(|ext| name.ends_with(ext) || name.ends_with(&format!("{ext}.gz")))
}

fn files(root: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_dir() {
            if depth < 4 && !crate::paths::mount_point(&path) {
                files(&path, depth + 1, found);
            }
        } else if meta.is_file() && rotatable(&path) {
            found.push(path);
        }
    }
}

fn rotate(env: &Env<'_>, compress_days: u64, keep_days: u64, dry_run: bool) -> Vec<Step> {
    let layout = env.layout;
    let rule = disk();
    let deny = layout.deny(&rule);
    let snapshot = env.processes.snapshot();
    let mut found = Vec::new();
    for root in log_roots(layout) {
        if paths::real_dir(&root) {
            files(&root, 0, &mut found);
        }
    }
    found.sort();
    let (mut compressed, mut removed, mut freed, mut failed) = (0u64, 0u64, 0u64, Vec::new());
    for path in found {
        if deny.iter().any(|d| path.starts_with(d)) {
            continue;
        }
        let Ok(meta) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        let age = env
            .now
            .saturating_sub(u64::try_from(meta.mtime()).unwrap_or(0));
        let in_use = match &snapshot {
            Ok(snapshot) => snapshot.inside(&path) > 0,
            Err(_) => true,
        };
        if in_use {
            continue;
        }
        let size = meta.blocks() * 512;
        if age >= keep_days * 86_400 {
            if dry_run {
                removed += 1;
                freed += size;
                continue;
            }
            match std::fs::remove_file(&path) {
                Ok(()) => {
                    removed += 1;
                    freed += size;
                }
                Err(error) => failed.push(format!("{}: {error}", show(&path, &layout.home))),
            }
        } else if age >= compress_days * 86_400 && path.extension().is_none_or(|ext| ext != "gz") {
            if dry_run {
                compressed += 1;
                continue;
            }
            match gzip(&path, &meta) {
                Ok(saved) => {
                    compressed += 1;
                    freed += saved;
                }
                Err(error) => failed.push(format!("{}: {error}", show(&path, &layout.home))),
            }
        }
    }
    let mut steps = Vec::new();
    if compressed + removed > 0 {
        let verb = if dry_run { "would" } else { "" };
        let detail = if dry_run {
            format!(
                "{verb} compress {compressed} and remove {removed} old log and trace files ({})",
                bytes(freed)
            )
        } else {
            format!(
                "Compressed {compressed} and removed {removed} old log and trace files, {}.",
                bytes(freed)
            )
        };
        steps.push(step(
            "rotate",
            None,
            if dry_run {
                StepOutcome::Would
            } else {
                StepOutcome::Done
            },
            detail,
        ));
    }
    for why in failed.into_iter().take(3) {
        steps.push(step("rotate", None, StepOutcome::Failed, why));
    }
    steps
}

/// Compress `path` to `path.gz` with the same modification time, then
/// remove it. The bytes saved.
fn gzip(path: &Path, meta: &std::fs::Metadata) -> std::io::Result<u64> {
    let mut out = path.as_os_str().to_owned();
    out.push(".gz");
    let out = PathBuf::from(out);
    if out.exists() {
        return Err(std::io::Error::other("a compressed copy already exists"));
    }
    let input = std::fs::read(path)?;
    let file = std::fs::File::create_new(&out)?;
    let mut encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let written = encoder.write_all(&input).and_then(|()| encoder.finish());
    let file = match written {
        Ok(file) => file,
        Err(error) => {
            let _ = std::fs::remove_file(&out);
            return Err(error);
        }
    };
    file.sync_all()?;
    if let Ok(modified) = meta.modified() {
        let _ = file.set_modified(modified);
    }
    let after = std::fs::symlink_metadata(&out)?.blocks() * 512;
    std::fs::remove_file(path)?;
    Ok((meta.blocks() * 512).saturating_sub(after))
}
