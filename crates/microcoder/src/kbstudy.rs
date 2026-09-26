//! A bounded runner for one frozen knowledge-snapshot comparison.
//!
//! Dispatch uses the shared supervisor. Every assignment is reserved before
//! execution, and an interrupted assignment is retained rather than rerun.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use knowledge::study::{
    self, Arm, Assignment, Case, Configuration, Finished, Partition, Plan, Store,
};
use serde_json::{Value, json};

const USAGE: &str = "microcoder kb study draft SNAPSHOT.json TASKS_DIR PLAN.json TASK [TASK ...]\nmicrocoder kb study freeze PLAN.json SNAPSHOT.json STUDY_DIR\nmicrocoder kb study run STUDY_DIR\nmicrocoder kb study report STUDY_DIR\nDrafting/freezing/reporting run no model. Running executes the frozen assignments and their declared model budgets.";
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}
fn io(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Run the study CLI. Only `run` can invoke models.
pub async fn main(args: &[String]) -> u8 {
    match command(args).await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("knowledge study: {error}");
            2
        }
    }
}
async fn command(args: &[String]) -> Result<(), String> {
    match args.first().map(String::as_str) {
        Some("draft") if args.len() >= 5 => draft(
            Path::new(&args[1]),
            Path::new(&args[2]),
            Path::new(&args[3]),
            &args[4..],
        ),
        Some("freeze") if args.len() == 4 => {
            let plan: Plan = serde_json::from_value(
                nostr::contracts::parse_strict(&std::fs::read(&args[1]).map_err(io)?)
                    .map_err(io)?,
            )
            .map_err(io)?;
            let candidate = std::fs::read(&args[2]).map_err(io)?;
            if study::file_digest(&plan.binary)? != plan.binary_digest {
                return Err("executable changed before freeze".into());
            }
            check_tasks(&plan)?;
            let store = Store::freeze(Path::new(&args[3]), &plan, &candidate)?;
            println!(
                "Frozen {} assignments under {}",
                store.assignments.len(),
                store.plan_digest
            );
            Ok(())
        }
        Some("run") if args.len() == 2 => run(Path::new(&args[1])).await,
        Some("report") if args.len() == 2 => {
            let store = Store::open(Path::new(&args[1]))?;
            let report = report(&store)?;
            println!("{}", serde_json::to_string_pretty(&report).map_err(io)?);
            Ok(())
        }
        _ => Err(USAGE.into()),
    }
}
fn draft(snapshot: &Path, tasks: &Path, output: &Path, names: &[String]) -> Result<(), String> {
    let verified = knowledge::snapshot::read(snapshot)?;
    let candidate = std::fs::read(snapshot).map_err(io)?;
    let binary = std::env::current_exe()
        .map_err(io)?
        .canonicalize()
        .map_err(io)?;
    let tasks_root = tasks.canonicalize().map_err(io)?;
    let mut random = [0_u8; 16];
    std::fs::File::open("/dev/urandom")
        .map_err(io)?
        .read_exact(&mut random)
        .map_err(io)?;
    let study = format!(
        "study-{}",
        random
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    let mut cases = Vec::new();
    for task in names {
        let found = crate::tbench::find(&tasks_root, task)?;
        cases.push(Case {
            task: task.clone(),
            group: task.clone(),
            partition: Partition::Confirmation,
            workload_digest: study::tree_digest(&found.dir)?,
            environment_digest: study::tree_digest(&found.dir.join("environment"))?,
        });
    }
    let plan = Plan {
        schema: study::SCHEMA.into(),
        study,
        owner: "local:operator".into(),
        binary_digest: study::file_digest(&binary)?,
        binary,
        tasks_root,
        candidate_digest: knowledge::digest(&candidate),
        configuration: Configuration {
            provider: "codex".into(),
            cost_basis: "list_price".into(),
            retrieval_mode: "lexical".into(),
            decision_base_url: jev::defaults::BASE_URL.into(),
            decision_model: std::env::var(jev::env::DEFAULT_MODEL)
                .ok()
                .filter(|m| !m.trim().is_empty())
                .unwrap_or_else(|| jev::defaults::MODEL.into()),
            model: crate::MODEL.into(),
            effort: "medium".into(),
            strong_model: crate::STRONG_MODEL.into(),
            max_steps: 30,
            max_seconds: 600,
            max_usd: 0.25,
            command_seconds: 60,
            test_seconds: 30,
            prompt: crate::run::USER_PROMPT.into(),
            network: "none".into(),
            acceptance: true,
        },
        max_total_usd: names.len() as f64 * 0.5,
        cases,
        repetitions: 1,
        first_subject: true,
        source_tasks: Vec::new(),
    };
    plan.validate(&verified.base.entries)?;
    knowledge::snapshot::write_new(output, &plan)?;
    println!(
        "Drafted {} assignments. Review task families, source exclusions, network, and the ${:.2} nominal reservation before freezing.",
        plan.assignments().len(),
        plan.max_total_usd
    );
    Ok(())
}
fn check_tasks(plan: &Plan) -> Result<(), String> {
    for case in &plan.cases {
        let task = crate::tbench::find(&plan.tasks_root, &case.task)?;
        if study::tree_digest(&task.dir)? != case.workload_digest
            || study::tree_digest(&task.dir.join("environment"))? != case.environment_digest
        {
            return Err(format!(
                "task or environment sources changed: {}",
                case.task
            ));
        }
    }
    Ok(())
}
fn container_name(store: &Store, assignment: &Assignment) -> String {
    format!("kb-{}-{}", store.plan.study, assignment.id).to_ascii_lowercase()
}
fn argv(store: &Store, assignment: &Assignment) -> Vec<String> {
    let config = &store.plan.configuration;
    let mut args = vec![
        assignment.task.clone(),
        "--provider".into(),
        config.provider.clone(),
        "--kb-lexical".into(),
        "--model".into(),
        config.model.clone(),
        "--effort".into(),
        config.effort.clone(),
        "--strong-model".into(),
        config.strong_model.clone(),
        "--route".into(),
        "never".into(),
        "--max-steps".into(),
        config.max_steps.to_string(),
        "--max-minutes".into(),
        (config.max_seconds as f64 / 60.0).to_string(),
        "--max-usd".into(),
        config.max_usd.to_string(),
        "--command-seconds".into(),
        config.command_seconds.to_string(),
        "--test-seconds".into(),
        config.test_seconds.to_string(),
        "--network".into(),
        config.network.clone(),
        "--prompt".into(),
        config.prompt.clone(),
        "--run-dir".into(),
        store
            .root
            .join(format!("{}.run", assignment.id))
            .display()
            .to_string(),
        "--container-name".into(),
        container_name(store, assignment),
        "--kb".into(),
        if assignment.arm == Arm::Subject {
            "candidates".into()
        } else {
            "off".into()
        },
    ];
    if assignment.arm == Arm::Subject {
        args.extend([
            "--kb-snapshot".into(),
            store.root.join("candidate.json").display().to_string(),
            "--kb-cache".into(),
            store
                .root
                .join(format!("{}.embeddings.json", assignment.id))
                .display()
                .to_string(),
        ]);
    }
    if !config.acceptance {
        args.push("--no-acceptance".into());
    }
    args
}
async fn run(root: &Path) -> Result<(), String> {
    let store = Store::open(root)?;
    if study::file_digest(&store.plan.binary)? != store.plan.binary_digest {
        return Err("frozen executable changed".into());
    }
    check_tasks(&store.plan)?;
    let prior = store.results()?;
    for (assignment, retained) in store.assignments.iter().zip(prior) {
        if retained["state"] == "finished" {
            if retained["finished"]["problems"]
                .as_array()
                .is_some_and(|items| items.iter().any(|p| p == "container_cleanup_unknown"))
            {
                return Err(
                    "a prior container cleanup is unknown; inspect it before continuing".into(),
                );
            }
            continue;
        }
        if retained["state"] == "unknown" {
            return Err(format!(
                "{} already started; inspect its retained execution instead of rerunning it",
                assignment.id
            ));
        }
        if study::file_digest(&store.plan.binary)? != store.plan.binary_digest {
            return Err("frozen executable changed".into());
        }
        check_tasks(&store.plan)?;
        let args = argv(&store, assignment);
        let started = store.begin(assignment, args.clone())?;
        println!(
            "{}: {} {:?}; reserved before dispatch",
            assignment.id, assignment.task, assignment.arm
        );
        let task = crate::tbench::find(&store.plan.tasks_root, &assignment.task)?;
        let deadline = store
            .plan
            .configuration
            .max_seconds
            .saturating_add(task.verifier_seconds)
            .saturating_add(600);
        let mut command = std::process::Command::new(&store.plan.binary);
        command
            .args(&args)
            .env("MICROCODER_TASKS", &store.plan.tasks_root)
            .env(
                jev::env::BASE_URL,
                &store.plan.configuration.decision_base_url,
            )
            .env(
                jev::env::DEFAULT_MODEL,
                &store.plan.configuration.decision_model,
            );
        let clock = Instant::now();
        let ended = supervise::Job::from_command(command)
            .bounded(supervise::Limits::within(Duration::from_secs(deadline)).keeping(1024 * 1024))
            .run()
            .await;
        std::fs::write(
            store.root.join(format!("{}.stdout.txt", assignment.id)),
            &ended.stdout.text,
        )
        .map_err(io)?;
        std::fs::write(
            store.root.join(format!("{}.stderr.txt", assignment.id)),
            &ended.stderr.text,
        )
        .map_err(io)?;
        let cleanup = cleanup(&container_name(&store, assignment)).await;
        knowledge::snapshot::write_new(
            &store.root.join(format!("{}.cleanup.json", assignment.id)),
            &cleanup,
        )?;
        let clean = cleanup["absence_confirmed"] == true;
        let mut finished = audit(&store, &started.output, assignment)?;
        finished.started_digest = study::started_digest(&started)?;
        finished.finished_at_ms = now();
        finished.exit_code = ended.ending.code();
        finished.ending = ended.ending.to_string();
        finished.wall_seconds = clock.elapsed().as_secs_f64();
        for name in ["stdout.txt", "stderr.txt", "cleanup.json"] {
            finished.evidence.insert(
                name.into(),
                study::file_digest(&store.root.join(format!("{}.{}", assignment.id, name)))?,
            );
        }
        if ended.truncated() {
            finished
                .problems
                .push("subprocess_output_truncated; full events retained when available".into());
        }
        if !clean {
            finished.problems.push("container_cleanup_unknown".into());
        }
        store.finish(&finished)?;
        println!(
            "{}: reward {:?}; {}",
            assignment.id, finished.reward, finished.ending
        );
        if !clean {
            return Err("container cleanup could not be confirmed; cohort stopped".into());
        }
    }
    let result = report(&store)?;
    let bytes = serde_json::to_vec_pretty(&result).map_err(io)?;
    let path = store.root.join(format!(
        "result-{}.json",
        knowledge::digest(&bytes).trim_start_matches("sha256:")
    ));
    if !path.exists() {
        knowledge::snapshot::write_new(&path, &result)?;
    }
    println!("Retained result: {}", path.display());
    Ok(())
}
async fn cleanup(name: &str) -> Value {
    let started_at_ms = now();
    let mut remove = std::process::Command::new("docker");
    remove.args(["rm", "-f", name]);
    let removed = supervise::Job::from_command(remove)
        .bounded(supervise::Limits::within(Duration::from_secs(30)).keeping(16 * 1024))
        .run()
        .await;
    let mut inspect = std::process::Command::new("docker");
    inspect.args(["ps", "-aq", "--filter", &format!("name=^/{name}$")]);
    let ended = supervise::Job::from_command(inspect)
        .bounded(supervise::Limits::within(Duration::from_secs(30)).keeping(16 * 1024))
        .run()
        .await;
    json!({"schema":"openagents.kb-study-cleanup.v1","container":name,"started_at_ms":started_at_ms,"finished_at_ms":now(),
        "remove":{"ending":removed.ending.to_string(),"stdout":removed.stdout.text,"stderr":removed.stderr.text},
        "inspect":{"ending":ended.ending.to_string(),"stdout":ended.stdout.text,"stderr":ended.stderr.text},
        "absence_confirmed":ended.ending.success() && !ended.truncated() && ended.stdout.text.trim().is_empty()})
}
fn audit(store: &Store, output: &Path, assignment: &Assignment) -> Result<Finished, String> {
    let intake = knowledge::evidence::read_run(output);
    let known_cost_lower_bound_usd = intake.cost.known_lower_bound_usd;
    let mut costs = intake.cost.components;
    let mut problems = intake.problems;
    let summary = intake
        .summary_bytes
        .as_ref()
        .and_then(|bytes| nostr::contracts::parse_strict(bytes).ok());
    let config = &store.plan.configuration;
    let configuration_matches = summary.as_ref().is_some_and(|value| {
        value["provider"] == config.provider
            && value["cost_basis"] == config.cost_basis
            && value["effort"] == config.effort
            && value["retrieval_mode"]
                == if assignment.arm == Arm::Subject {
                    "lexical"
                } else {
                    "off"
                }
            && value["decision"]["base_url"] == config.decision_base_url
            && value["decision"]["model"] == config.decision_model
    });
    if !configuration_matches {
        problems.push("summary_configuration_unknown_or_mismatched".into());
    }
    let mut served = BTreeSet::new();
    let event_path = output.join("events.jsonl");
    let events = std::fs::symlink_metadata(&event_path)
        .ok()
        .filter(|metadata| metadata.is_file() && metadata.len() <= 128 * 1024 * 1024)
        .and_then(|_| std::fs::read(&event_path).ok());
    if let Some(bytes) = &events {
        for line in bytes.split(|b| *b == b'\n').filter(|line| !line.is_empty()) {
            let Ok(value) = nostr::contracts::parse_strict(line) else {
                problems.push("malformed_event".into());
                for cost in costs.values_mut() {
                    *cost = None;
                }
                continue;
            };
            if value["event"] == "generated" {
                if let Some(model) = value["generated"]["model"].as_str() {
                    served.insert(model.to_string());
                }
                // Earlier producers wrote zero when the provider omitted a charge.
                // Without a receipt distinguishing a true free call, retain unknown.
                if value["generated"]["usd"].as_f64().is_none_or(|cost| {
                    cost < 0.0 || (cost == 0.0 && !known_zero(&value["generated"]))
                }) {
                    costs.insert("model_usd".into(), None);
                }
            }
            if value.get("judgment").is_some()
                && value["judgment"]["usd"].as_f64().is_none_or(|cost| {
                    cost < 0.0 || (cost == 0.0 && !known_zero(&value["judgment"]))
                })
            {
                costs.insert("jev_usd".into(), None);
            }
            if value["event"] == "retrieved" && value["retrieval"]["cached"] != true {
                let retrieval = &value["retrieval"];
                if retrieval["error"].is_string() {
                    costs.insert("jev_usd".into(), None);
                }
                if retrieval["lexical_only"].is_null()
                    && retrieval["embedding_usd"]
                        .as_f64()
                        .is_none_or(|cost| cost <= 0.0)
                {
                    costs.insert("embedding_usd".into(), None);
                }
            }
        }
    } else {
        problems.push("events_unavailable".into());
        for cost in costs.values_mut() {
            *cost = None;
        }
    }
    if served.is_empty() {
        problems.push("served_model_identity_unknown".into());
    }
    if served
        .iter()
        .any(|model| model != &store.plan.configuration.model)
    {
        problems.push("served_model_differs_from_frozen_model".into());
    }
    Ok(Finished {
        schema: "openagents.kb-study-finish.v1".into(),
        plan_digest: store.plan_digest.clone(),
        assignment: assignment.id.clone(),
        started_digest: String::new(),
        finished_at_ms: 0,
        exit_code: None,
        ending: String::new(),
        wall_seconds: 0.0,
        summary_digest: intake.summary_digest,
        events_digest: study::file_digest(&event_path).ok(),
        reward: if configuration_matches
            && intake.task == assignment.task
            && intake.model == store.plan.configuration.model
        {
            intake.reward
        } else {
            problems.push("summary_task_or_model_mismatch".into());
            None
        },
        costs,
        known_cost_lower_bound_usd,
        evidence: BTreeMap::new(),
        served_models: served.into_iter().collect(),
        problems,
    })
}
fn known_zero(receipt: &Value) -> bool {
    receipt["known_usd"].as_f64() == Some(0.0)
        && receipt.get("cost_unknown").is_some_and(Value::is_null)
}
/// Report every frozen assignment, including unfinished and failed attempts.
///
/// # Errors
/// Corrupt retained receipts refuse instead of shrinking the denominator.
pub fn report(store: &Store) -> Result<Value, String> {
    let results = store.results()?;
    let rows: Vec<_> = store.assignments.iter().zip(&results).collect();
    let arms = arm_totals(&rows);
    let mut partitions = BTreeMap::new();
    for (name, partition) in [
        ("development", Partition::Development),
        ("confirmation", Partition::Confirmation),
    ] {
        let selected: Vec<_> = rows
            .iter()
            .copied()
            .filter(|(a, _)| a.partition == partition)
            .collect();
        partitions.insert(name, arm_totals(&selected));
    }
    let mut tasks = BTreeMap::new();
    for case in &store.plan.cases {
        let selected: Vec<_> = rows
            .iter()
            .copied()
            .filter(|(a, _)| a.task == case.task)
            .collect();
        tasks.insert(
            &case.task,
            json!({"partition":case.partition,"group":case.group,"arms":arm_totals(&selected)}),
        );
    }
    Ok(
        json!({"schema":"openagents.kb-study-result.v1","nip_opt":"openagents.optimization-result.v1",
        "profile":"fixed candidate snapshot vs no knowledge; paired assigned order", "plan_digest":store.plan_digest,
        "candidate_digest":store.plan.candidate_digest,"configuration":store.plan.configuration,
        "cases":store.plan.cases,"assignments":results,"arms":arms,"partitions":partitions,"tasks":tasks,"verdict":"inconclusive","promotion_eligible":false,
        "limitations":["Source-family independence is declared by the operator, not proven by filenames.",
        "Wilson intervals describe graded attempts only and assume independent trials; repeated tasks and shared source families limit that interpretation. Missing-outcome bounds include every assigned attempt.",
        "Task and environment source digests do not prove identical remote package repositories or provider deployment.",
        "Zero-priced model events without an attributable zero-charge receipt are conservatively unknown.",
        "Small fixed cohorts describe outcomes; they do not establish reliable transfer or authorize automatic admission.",
        "A nominal loop spend bound can overshoot by an in-flight call; unknown charges remain unknown."]}),
    )
}

fn arm_totals(rows: &[(&Assignment, &Value)]) -> BTreeMap<&'static str, Value> {
    let mut arms = BTreeMap::new();
    for (name, arm) in [("subject", Arm::Subject), ("baseline", Arm::Baseline)] {
        let mut planned = 0;
        let mut passed = 0;
        let mut graded = 0;
        let mut completed = 0;
        let mut known = 0.0;
        let mut cost_unknown = 0;
        let mut seconds = 0.0;
        for (_, row) in rows.iter().copied().filter(|(a, _)| a.arm == arm) {
            planned += 1;
            if row["state"] == "finished" {
                completed += 1;
            }
            if let Some(reward) = row["finished"]["reward"].as_f64() {
                graded += 1;
                passed += usize::from(reward == 1.0);
            }
            seconds += row["finished"]["wall_seconds"].as_f64().unwrap_or(0.0);
            known += row["finished"]["known_cost_lower_bound_usd"]
                .as_f64()
                .unwrap_or(0.0);
            let recorded_costs = row["finished"]["costs"].as_object();
            let missing = recorded_costs.is_none_or(|costs| {
                ["model_usd", "jev_usd", "embedding_usd"]
                    .iter()
                    .any(|key| !costs.contains_key(*key))
                    || costs.values().any(|value| value.as_f64().is_none())
            });
            cost_unknown += usize::from(missing);
        }
        arms.insert(name, json!({"planned":planned,"finished":completed,"graded":graded,"passed":passed,
            "outcome_unknown":planned-graded,"graded_pass_fraction_wilson_95":wilson(passed,graded),"known_cost_lower_bound_usd":known,
            "total_usd":if cost_unknown == 0 {Some(known)} else {None},"cost_unknown_attempts":cost_unknown,
            "known_wall_seconds":seconds,"wall_unknown_attempts":planned-completed,
            "completion_fraction_bounds":if planned>0 {Some((passed as f64/planned as f64,(passed+planned-graded) as f64/planned as f64))} else {None}}));
    }
    arms
}
fn wilson(passed: usize, graded: usize) -> Option<(f64, f64)> {
    if graded == 0 {
        return None;
    }
    let n = graded as f64;
    let p = passed as f64 / n;
    let z = 1.959_963_984_540_054;
    let d = 1.0 + z * z / n;
    let center = (p + z * z / (2.0 * n)) / d;
    let radius = z * (p * (1.0 - p) / n + z * z / (4.0 * n * n)).sqrt() / d;
    Some(((center - radius).max(0.0), (center + radius).min(1.0)))
}

#[cfg(test)]
#[path = "kbstudy/tests.rs"]
mod tests;
