//! `openagents eval`: score doors against a Gym suite, read a recorded
//! store back as a report, and compare its sides under a gate.
//!
//! Every command is a view over `crates/gym`: the suite, the gate, the
//! receipt-chained store, and the comparison rule are the crate's own, and
//! nothing here changes a schema or a gate. `run` is the only command that
//! reaches a network, and it asks the doors named with `--door` over HTTP
//! for as long as `--timeout` allows. `report` and `compare` read a store
//! and never contact a door.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use gym::eval::{self, Disposition, Run};
use gym::gate::{self, Gate};
use gym::row::{DoorIdentity, Row};
use gym::store::{ChainVerdict, Store};
use gym::suite::{Item, Partition, Suite};
use jev::{Client, Config, Questions, RetryPolicy, SystemOneRequest};
use serde_json::{Value, json};

use crate::{Args, Output};

const USAGE: &str = "usage: openagents eval COMMAND [OPTIONS]
  run --door NAME=URL... --timeout SECONDS [--suite FILE] [--gate ID]
      [--questions ID] [--partition calibration|development] [--record FILE]
                          Ask each door every open item and score the rows.
  report --store FILE [--suite FILE]
                          Read a receipt-chained store back as a measured record.
  compare --store FILE [--baseline SIDE] [--gate ID]
      [--partition calibration|development]
                          Score each side of a store and judge them under a gate.
Options:
  --door NAME=URL         A door that answers POST /v1/systemone. Repeat for more.
  --timeout SECONDS       Wall time allowed for each door call, retries included.
  --suite FILE            A suite file; run defaults to the bundled support suite.
  --gate ID               A gate under crates/gym/gates/; defaults to the suite's.
  --record FILE           Append each row to this receipt-chained store.
  --store FILE            The store report and compare read.
  --baseline SIDE         The side compare judges the others against; default the first.
The locked partition is never scored by a flag. A door that fails to answer
leaves no row; the lost items are listed beside the rows and the run exits 1.";

const DEFAULT_GATE: &str = "probability-v2";

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("eval", "a command is required", USAGE);
    };
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("eval", &message, USAGE),
    };
    match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            0
        }
        "run" => {
            let plan = match RunPlan::from_args(&args) {
                Ok(plan) => plan,
                Err(message) => return output.usage("eval run", &message, USAGE),
            };
            match crate::runtime().block_on(run_command(plan)) {
                Ok(value) => {
                    let complete = value["complete"].as_bool().unwrap_or(false);
                    output.emit(&value, render_run);
                    if complete { 0 } else { crate::EXIT_FAILURE }
                }
                Err(message) => output.fail("eval run", &message),
            }
        }
        "report" => {
            let Some(store) = args.option("store") else {
                return output.usage("eval report", "--store FILE is required", USAGE);
            };
            match report_command(store, args.option("suite")) {
                Ok(value) => {
                    output.emit(&value, render_report);
                    0
                }
                Err(message) => output.fail("eval report", &message),
            }
        }
        "compare" => {
            let Some(store) = args.option("store") else {
                return output.usage("eval compare", "--store FILE is required", USAGE);
            };
            let partition = match args.option("partition").map(partition_named).transpose() {
                Ok(partition) => partition,
                Err(message) => return output.usage("eval compare", &message, USAGE),
            };
            match compare_command(
                store,
                args.option("baseline"),
                args.option("gate"),
                partition,
            ) {
                Ok(value) => {
                    output.emit(&value, render_compare);
                    0
                }
                Err(message) => output.fail("eval compare", &message),
            }
        }
        other => output.usage("eval", &format!("unknown command `{other}`"), USAGE),
    }
}

/// Everything `run` needs, checked before a door is opened.
struct RunPlan {
    doors: Vec<(String, String)>,
    timeout: Duration,
    suite: Option<String>,
    gate: Option<String>,
    questions: Option<String>,
    partitions: Vec<Partition>,
    record: Option<String>,
}

impl RunPlan {
    fn from_args(args: &Args) -> Result<Self, String> {
        let doors = args
            .options("door")
            .into_iter()
            .map(parse_door)
            .collect::<Result<Vec<_>, _>>()?;
        if doors.is_empty() {
            return Err("at least one --door NAME=URL is required".to_owned());
        }
        let Some(timeout) = args.option("timeout") else {
            return Err("--timeout SECONDS is required for every network operation".to_owned());
        };
        let timeout: u64 = timeout
            .parse()
            .ok()
            .filter(|seconds| *seconds > 0)
            .ok_or_else(|| "--timeout must be a positive number of seconds".to_owned())?;
        let partitions = match args.option("partition") {
            None => vec![Partition::Calibration, Partition::Development],
            Some(named) => vec![partition_named(named)?],
        };
        Ok(Self {
            doors,
            timeout: Duration::from_secs(timeout),
            suite: args.option("suite").map(str::to_owned),
            gate: args.option("gate").map(str::to_owned),
            questions: args.option("questions").map(str::to_owned),
            partitions,
            record: args.option("record").map(str::to_owned),
        })
    }
}

fn parse_door(text: &str) -> Result<(String, String), String> {
    match text.split_once('=') {
        Some((name, url)) if !name.is_empty() && url.starts_with("http") => {
            Ok((name.to_owned(), url.to_owned()))
        }
        _ => Err(format!(
            "--door expects NAME=URL with an http(s) URL, got `{text}`"
        )),
    }
}

/// Which open partition a flag names. The locked partition is spent through
/// a ledger and no flag scores it.
fn partition_named(named: &str) -> Result<Partition, String> {
    match named {
        "calibration" => Ok(Partition::Calibration),
        "development" => Ok(Partition::Development),
        "locked" => Err(
            "the locked partition is read once, through a ledger; no flag can score it".to_owned(),
        ),
        other => Err(format!("unknown partition `{other}`")),
    }
}

fn load_suite(path: Option<&str>) -> Result<Suite, String> {
    match path {
        Some(path) => Suite::load_file(path).map_err(|error| format!("{path}: {error}")),
        None => gym::suite::support_v2_three_way().map_err(|error| error.to_string()),
    }
}

fn load_gate(flag: Option<&str>, suite: Option<&Suite>) -> Result<Gate, String> {
    let id = flag
        .map(str::to_owned)
        .or_else(|| suite.and_then(|suite| suite.gate.clone()))
        .unwrap_or_else(|| DEFAULT_GATE.to_owned());
    gate::load(&id).map_err(|error| format!("gate `{id}`: {error}"))
}

fn read_rows(path: &str) -> Result<(Vec<Row>, Option<String>), String> {
    let values = Store::at(path)
        .rows()
        .map_err(|error| format!("{path}: {error}"))?;
    let head = match gym::store::verify_chain(&values) {
        ChainVerdict::Ok { head, .. } => head,
        ChainVerdict::Broken { detail, .. } => return Err(format!("{path}: {detail}")),
    };
    let rows: Vec<Row> = values
        .into_iter()
        .map(|value| serde_json::from_value(value).map_err(|error| format!("{path}: {error}")))
        .collect::<Result<_, _>>()?;
    if rows.is_empty() {
        return Err(format!("{path} holds no rows"));
    }
    Ok((rows, head))
}

/// The scores over a set of rows, in the shape every command emits.
fn metrics_of(rows: &[Row]) -> Value {
    let metrics = gym::calibrate::score(&eval::observations(rows));
    let mut latencies: Vec<f64> = rows.iter().filter_map(|row| row.latency_ms).collect();
    latencies.sort_by(f64::total_cmp);
    let median = (!latencies.is_empty()).then(|| latencies[latencies.len() / 2]);
    json!({
        "rows": rows.len(),
        "scored": metrics.items,
        "refused": rows.iter().filter(|row| row.is_refused()).count(),
        "accuracy": metrics.accuracy,
        "ece": metrics.ece,
        "brier": metrics.brier,
        "nll": metrics.nll,
        "confident_errors": metrics.confident_errors,
        "median_latency_ms": median,
        "refusals": eval::refusals(rows),
    })
}

/// One side of a store: a door, the question set it was asked as, and the
/// checkpoint it served when the rows carry one.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Side {
    door: String,
    questions: Option<String>,
    identity: Option<DoorIdentity>,
}

impl Side {
    fn of(row: &Row) -> Self {
        Self {
            door: row.door.clone(),
            questions: row.question_set.clone(),
            identity: (!row.door_identity.artifact_signature.is_empty())
                .then(|| row.door_identity.clone()),
        }
    }

    fn label(&self) -> String {
        use sha2::Digest;
        let door = self.identity.as_ref().map_or_else(
            || self.door.clone(),
            |identity| {
                let encoded = serde_json::to_vec(identity).expect("a door identity serializes");
                let digest = sha2::Sha256::digest(encoded);
                let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
                format!("{}@{}", self.door, &hex[..16])
            },
        );
        match &self.questions {
            Some(set) => format!("{door} asked as {set}"),
            None => door,
        }
    }
}

fn sides_of(rows: &[Row]) -> Vec<Side> {
    let mut sides: Vec<Side> = Vec::new();
    for row in rows {
        let side = Side::of(row);
        if !sides.contains(&side) {
            sides.push(side);
        }
    }
    sides
}

async fn run_command(plan: RunPlan) -> Result<Value, String> {
    let suite = load_suite(plan.suite.as_deref())?;
    let gate = load_gate(plan.gate.as_deref(), Some(&suite))?;
    let questions = gym::questions::resolve(&suite, plan.questions.as_deref())
        .map_err(|error| error.to_string())?;
    let mut items: Vec<&Item> = Vec::new();
    for partition in &plan.partitions {
        items.extend(
            suite
                .partition(*partition)
                .map_err(|error| error.to_string())?,
        );
    }
    if items.is_empty() {
        return Err(format!(
            "`{}` holds no items in the selected partitions",
            suite.name
        ));
    }
    let store = plan.record.as_deref().map(Store::at);

    let mut doors = Vec::new();
    for (name, url) in &plan.doors {
        let config = Config::new()
            .api_key("unused-by-a-local-door")
            .base_url(url.clone())
            .default_model(name.clone())
            .timeout(plan.timeout)
            .retry(RetryPolicy {
                budget: Some(plan.timeout),
                ..RetryPolicy::default()
            });
        let client = Client::new(config).map_err(|error| format!("door `{name}`: {error}"))?;
        doors.push((name.clone(), url.clone(), client));
    }

    let mut results = Vec::new();
    for (name, url, client) in doors {
        let facts = published_facts(&client, &name).await;
        let run = Run {
            suite: suite.name.clone(),
            suite_digest: suite.digest.clone(),
            question_set: Some(questions.id.clone()),
            question_digest: Some(questions.digest()),
            door: name.clone(),
            door_identity: facts.identity.clone(),
            estimator: facts.estimator.clone(),
            samples: facts.samples,
            seed_base: facts.seed_base,
            recorded_at: eval::now_utc(),
            gate_id: Some(gate.id.clone()),
            gate_digest: Some(gate.digest()),
        };
        let mut rows = Vec::with_capacity(items.len());
        let mut lost = Vec::new();
        for item in &items {
            let question = questions.ask(item).map_err(|error| error.to_string())?;
            let (disposition, latency) = ask(&client, &item.state, question).await;
            match run.row(item, None, &disposition, latency) {
                Some(row) => {
                    if let Some(store) = &store {
                        append(store, &row)?;
                    }
                    rows.push(row);
                }
                None => {
                    let Disposition::Harness(detail) = disposition else {
                        unreachable!("only a harness failure yields no row");
                    };
                    lost.push(json!({ "item": item.id, "detail": detail }));
                }
            }
        }
        let mut by_partition = serde_json::Map::new();
        for partition in &plan.partitions {
            let inside: Vec<Row> = rows
                .iter()
                .filter(|row| row.split == partition.as_str())
                .cloned()
                .collect();
            by_partition.insert(partition.as_str().to_owned(), metrics_of(&inside));
        }
        results.push(json!({
            "door": name,
            "url": url,
            "identity": facts.identity,
            "estimator": facts.estimator,
            "metrics": metrics_of(&rows),
            "partitions": by_partition,
            "asked": items.len(),
            "lost": lost,
        }));
    }
    let complete = results
        .iter()
        .all(|door| door["lost"].as_array().is_none_or(Vec::is_empty));
    Ok(json!({
        "complete": complete,
        "suite": suite.name,
        "suite_digest": suite.digest,
        "questions": questions.id,
        "questions_digest": questions.digest(),
        "gate": gate.id,
        "gate_digest": gate.digest(),
        "partitions": plan.partitions.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        "items": items.len(),
        "timeout_seconds": plan.timeout.as_secs(),
        "recorded_to": plan.record,
        "doors": results,
    }))
}

/// What a door publishes about itself, or the little that is known without it.
struct Facts {
    identity: DoorIdentity,
    estimator: String,
    samples: Option<u64>,
    seed_base: Option<u64>,
}

async fn published_facts(client: &Client, name: &str) -> Facts {
    let unknown = Facts {
        identity: DoorIdentity::hosted(name),
        estimator: "unreported".to_owned(),
        samples: None,
        seed_base: None,
    };
    let Ok(response) = client.models().list_raw(jev::ListOptions::default()).await else {
        return unknown;
    };
    let Ok(body) = serde_json::from_slice::<Value>(&response.bytes) else {
        return unknown;
    };
    let Some(cards) = body.get("models").and_then(Value::as_array) else {
        return unknown;
    };
    let Some(model) = cards
        .iter()
        .find(|card| {
            card.get("id").and_then(Value::as_str) == Some(name)
                || card.get("name").and_then(Value::as_str) == Some(name)
        })
        .or_else(|| (cards.len() == 1).then(|| &cards[0]))
    else {
        return unknown;
    };
    let estimator = model
        .get("estimator")
        .and_then(Value::as_str)
        .unwrap_or_default();
    Facts {
        identity: DoorIdentity::from_model_card(model, name),
        estimator: if estimator.is_empty() {
            unknown.estimator
        } else {
            estimator.to_owned()
        },
        samples: model.get("samples").and_then(Value::as_u64),
        seed_base: model.get("seed_base").and_then(Value::as_u64),
    }
}

async fn ask(client: &Client, state: &Value, question: &Value) -> (Disposition, Option<f64>) {
    let questions = Questions::new().with("q", jev::Question::Raw(question.clone()));
    let request = SystemOneRequest::new(state.clone(), questions);
    let started = Instant::now();
    let response = client.system_one(request).await;
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    match response {
        Ok(response) => match response.answers.get("q") {
            Some(answer) => (eval::read_answer(answer), Some(elapsed)),
            None => (
                Disposition::Harness("the door replied without an answer to the question".into()),
                None,
            ),
        },
        Err(error) => match eval::classify(&error) {
            Disposition::Harness(detail) => (Disposition::Harness(detail), None),
            refused => (refused, Some(elapsed)),
        },
    }
}

fn append(store: &Store, row: &Row) -> Result<(), String> {
    row.check()
        .map_err(|error| format!("{}: {error}", row.item_id))?;
    store
        .append(row)
        .map(|_| ())
        .map_err(|error| format!("{}: {error}", store.path().display()))
}

fn report_command(path: &str, suite: Option<&str>) -> Result<Value, String> {
    let (rows, head) = read_rows(path)?;
    let suite = match suite {
        Some(file) => {
            let suite = Suite::load_file(file).map_err(|error| format!("{file}: {error}"))?;
            if !rows.iter().any(|row| row.suite_digest == suite.digest) {
                return Err(format!(
                    "{file} digests to {}, which no row in this store names",
                    suite.digest
                ));
            }
            Some(suite)
        }
        None => None,
    };
    let mut suites: Vec<(String, String)> = Vec::new();
    for row in &rows {
        let key = (row.suite.clone(), row.suite_digest.clone());
        if !suites.contains(&key) {
            suites.push(key);
        }
    }
    let sections: Vec<Value> = suites
        .iter()
        .map(|(name, digest)| {
            let inside: Vec<Row> = rows
                .iter()
                .filter(|row| &row.suite == name && &row.suite_digest == digest)
                .cloned()
                .collect();
            let sides: Vec<Value> = sides_of(&inside)
                .iter()
                .map(|side| {
                    let held: Vec<Row> = inside
                        .iter()
                        .filter(|row| &Side::of(row) == side)
                        .cloned()
                        .collect();
                    json!({
                        "side": side.label(),
                        "door": side.door,
                        "questions": side.questions,
                        "metrics": metrics_of(&held),
                        "families": eval::families(&held),
                    })
                })
                .collect();
            json!({
                "suite": name,
                "suite_digest": digest,
                "rows": inside.len(),
                "sides": sides,
            })
        })
        .collect();
    let first = rows.iter().map(|row| &row.recorded_at).min();
    let last = rows.iter().map(|row| &row.recorded_at).max();
    Ok(json!({
        "store": path,
        "rows": rows.len(),
        "chain": "verified",
        "head": head,
        "recorded_from": first,
        "recorded_through": last,
        "coverage_declared": suite.is_some(),
        "declared_suite": suite.as_ref().map(|suite| json!({
            "name": suite.name,
            "digest": suite.digest,
            "items": suite.items.len(),
            "families": suite.families(),
        })),
        "suites": sections,
    }))
}

fn compare_command(
    path: &str,
    baseline: Option<&str>,
    gate: Option<&str>,
    partition: Option<Partition>,
) -> Result<Value, String> {
    let (mut rows, _) = read_rows(path)?;
    let held = rows.len();
    rows.retain(|row| row.permutation.is_none());
    if let Some(partition) = partition {
        rows.retain(|row| row.split == partition.as_str());
    }
    if rows.is_empty() {
        return Err(format!("{path} holds no rows once the view is narrowed"));
    }
    let sides = sides_of(&rows);
    let mut measured: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    let mut table = Vec::new();
    for side in &sides {
        let inside: Vec<Row> = rows
            .iter()
            .filter(|row| &Side::of(row) == side)
            .cloned()
            .collect();
        table.push(json!({
            "side": side.label(),
            "door": side.door,
            "questions": side.questions,
            "metrics": metrics_of(&inside),
        }));
        measured.insert(side.label(), inside);
    }
    let labels: Vec<String> = sides.iter().map(Side::label).collect();
    let baseline = match baseline {
        Some(named) => {
            let matches: Vec<&Side> = sides
                .iter()
                .filter(|side| side.door == named || side.label() == named)
                .collect();
            match matches.as_slice() {
                [side] => side.label(),
                [] => return Err(format!("no rows for the baseline `{named}`")),
                _ => {
                    return Err(format!(
                        "baseline `{named}` names several checkpoints or question sets; use the full side label"
                    ));
                }
            }
        }
        None => labels[0].clone(),
    };
    let gate = load_gate(gate, None)?;
    let before = &measured[&baseline];
    let before_scores = gym::calibrate::score(&eval::observations(before)).scores();
    let mut judgements = Vec::new();
    for label in labels.iter().filter(|label| **label != baseline) {
        let after = &measured[label];
        let comparison = gym::store::admit_comparison(&values_of(before), &values_of(after));
        match comparison {
            Err(refusal) => judgements.push(json!({
                "candidate": label,
                "baseline": baseline,
                "admitted": false,
                "refusal": refusal.to_string(),
            })),
            Ok(kind) => {
                let after_scores = gym::calibrate::score(&eval::observations(after)).scores();
                let outcome = gate.judge(&gate::Comparison::new(
                    format!("{label} compared with {baseline}"),
                    before_scores,
                    after_scores,
                ));
                judgements.push(json!({
                    "candidate": label,
                    "baseline": baseline,
                    "admitted": true,
                    "comparison": kind.as_str(),
                    "verdict": outcome.verdict.as_str(),
                    "deciding": outcome.deciding().map(|c| json!({
                        "criterion": c.name,
                        "detail": c.detail,
                    })),
                    "outcome": outcome,
                }));
            }
        }
    }
    Ok(json!({
        "store": path,
        "rows_held": held,
        "rows": rows.len(),
        "partition": partition.map(|p| p.as_str()),
        "gate": gate.id,
        "gate_digest": gate.digest(),
        "baseline": baseline,
        "sides": table,
        "judgements": judgements,
    }))
}

fn values_of(rows: &[Row]) -> Vec<Value> {
    rows.iter()
        .filter_map(|row| serde_json::to_value(row).ok())
        .collect()
}

fn text(value: &Value, key: &str) -> String {
    value[key].as_str().unwrap_or("").to_owned()
}

fn metrics_line(metrics: &Value) -> String {
    format!(
        "accuracy {:.2}  ece {:.3}  brier {:.3}  nll {:.3}  confident errors {}  scored {}  refused {}",
        metrics["accuracy"].as_f64().unwrap_or(0.0),
        metrics["ece"].as_f64().unwrap_or(0.0),
        metrics["brier"].as_f64().unwrap_or(0.0),
        metrics["nll"].as_f64().unwrap_or(0.0),
        metrics["confident_errors"].as_u64().unwrap_or(0),
        metrics["scored"].as_u64().unwrap_or(0),
        metrics["refused"].as_u64().unwrap_or(0),
    )
}

fn render_run(value: &Value) -> String {
    let mut lines = vec![format!(
        "suite {} ({}), {} items, gate {}",
        text(value, "suite"),
        &text(value, "suite_digest")[..16.min(text(value, "suite_digest").len())],
        value["items"].as_u64().unwrap_or(0),
        text(value, "gate"),
    )];
    if let Some(path) = value["recorded_to"].as_str() {
        lines.push(format!("recorded to {path}"));
    }
    if value["complete"] == Value::Bool(false) {
        lines.push(
            "incomplete: a door lost items, so this run is not a full measurement".to_owned(),
        );
    }
    for door in value["doors"].as_array().into_iter().flatten() {
        lines.push(format!(
            "{}: {}  lost {}",
            text(door, "door"),
            metrics_line(&door["metrics"]),
            door["lost"].as_array().map_or(0, Vec::len),
        ));
    }
    lines.join("\n")
}

fn render_report(value: &Value) -> String {
    let mut lines = vec![format!(
        "{}: {} rows, chain {}{}",
        text(value, "store"),
        value["rows"].as_u64().unwrap_or(0),
        text(value, "chain"),
        value["head"]
            .as_str()
            .map(|head| format!(" to head {head}"))
            .unwrap_or_default(),
    )];
    if !value["coverage_declared"].as_bool().unwrap_or(false) {
        lines.push(
            "coverage not declared: pass --suite to check the record as a completed evaluation"
                .to_owned(),
        );
    }
    for suite in value["suites"].as_array().into_iter().flatten() {
        lines.push(format!(
            "suite {} ({} rows)",
            text(suite, "suite"),
            suite["rows"].as_u64().unwrap_or(0)
        ));
        for side in suite["sides"].as_array().into_iter().flatten() {
            lines.push(format!(
                "  {}: {}",
                text(side, "side"),
                metrics_line(&side["metrics"])
            ));
        }
    }
    lines.join("\n")
}

fn render_compare(value: &Value) -> String {
    let mut lines = vec![format!(
        "{}: {} rows, gate {}, baseline {}",
        text(value, "store"),
        value["rows"].as_u64().unwrap_or(0),
        text(value, "gate"),
        text(value, "baseline"),
    )];
    for side in value["sides"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {}: {}",
            text(side, "side"),
            metrics_line(&side["metrics"])
        ));
    }
    let judgements = value["judgements"].as_array();
    if judgements.is_none_or(Vec::is_empty) {
        lines.push("nothing to compare the baseline against yet".to_owned());
    }
    for judgement in judgements.into_iter().flatten() {
        if judgement["admitted"].as_bool().unwrap_or(false) {
            lines.push(format!(
                "{} vs {}: {} ({})",
                text(judgement, "candidate"),
                text(judgement, "baseline"),
                text(judgement, "verdict"),
                judgement["deciding"]["criterion"]
                    .as_str()
                    .unwrap_or("nothing was judged"),
            ));
        } else {
            lines.push(format!(
                "{} vs {}: refused, {}",
                text(judgement, "candidate"),
                text(judgement, "baseline"),
                text(judgement, "refusal"),
            ));
        }
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Args {
        let words: Vec<String> = words.iter().map(|w| (*w).to_owned()).collect();
        Args::parse(&words, &[]).unwrap()
    }

    #[test]
    fn run_requires_doors_and_timeout() {
        let error = RunPlan::from_args(&args(&["--timeout", "5"]))
            .map(drop)
            .unwrap_err();
        assert!(error.contains("--door"));
        let error = RunPlan::from_args(&args(&["--door", "a=http://127.0.0.1:1"]))
            .map(drop)
            .unwrap_err();
        assert!(error.contains("--timeout"));
        let error = RunPlan::from_args(&args(&["--door", "a=http://x", "--timeout", "0"]))
            .map(drop)
            .unwrap_err();
        assert!(error.contains("positive"));
    }

    #[test]
    fn run_plan_reads_every_flag() {
        let plan = RunPlan::from_args(&args(&[
            "--door",
            "a=http://127.0.0.1:1",
            "--door",
            "b=http://127.0.0.1:2",
            "--timeout",
            "30",
            "--partition",
            "development",
            "--record",
            "rows.jsonl",
        ]))
        .unwrap();
        assert_eq!(plan.doors.len(), 2);
        assert_eq!(plan.timeout, Duration::from_secs(30));
        assert_eq!(plan.partitions, vec![Partition::Development]);
        assert_eq!(plan.record.as_deref(), Some("rows.jsonl"));
    }

    #[test]
    fn the_locked_partition_is_refused() {
        assert!(partition_named("locked").unwrap_err().contains("ledger"));
        assert!(partition_named("other").is_err());
    }

    #[test]
    fn door_needs_a_name_and_an_http_url() {
        assert!(parse_door("a=http://x").is_ok());
        assert!(parse_door("=http://x").is_err());
        assert!(parse_door("a=x").is_err());
        assert!(parse_door("a").is_err());
    }

    #[test]
    fn report_and_compare_refuse_a_missing_store() {
        let dir = std::env::temp_dir().join(format!("oa-eval-{}", std::process::id()));
        let missing = dir.join("none.jsonl").display().to_string();
        assert!(report_command(&missing, None).is_err());
        assert!(compare_command(&missing, None, None, None).is_err());
    }

    #[test]
    fn eval_group_usage_is_exit_64() {
        let output = Output::new(true);
        assert_eq!(run(&output, &[]), crate::EXIT_USAGE);
        assert_eq!(
            run(
                &output,
                &["run".to_owned(), "--timeout".to_owned(), "5".to_owned()]
            ),
            crate::EXIT_USAGE
        );
        assert_eq!(run(&output, &["report".to_owned()]), crate::EXIT_USAGE);
        assert_eq!(run(&output, &["nope".to_owned()]), crate::EXIT_USAGE);
    }
}
