//! The Gym: score a door against a suite, compare doors from the record,
//! and measure what option order does to an answer.
//!
//! Every subcommand reaches a door through one `crates/jev` client, so the
//! same command measures hosted Jev, `kev-serve`, and `lev-serve`. That is
//! the payoff of building three implementations of one contract rather than
//! three APIs.
//!
//! ```text
//! # score two doors and record the rows
//! cargo run -p gym --bin gym -- eval \
//!     --door lev=http://127.0.0.1:11436 \
//!     --door kev=http://127.0.0.1:8009 \
//!     --fit --record results/support-v2-three-way.jsonl
//!
//! # the comparison, from the rows rather than from a live run
//! cargo run -p gym --bin gym -- compare \
//!     --store results/support-v2-three-way.jsonl
//!
//! # what option order does to the answer
//! cargo run -p gym --bin gym -- permute --door lev=http://127.0.0.1:11436
//!
//! # the same items under reworded question text
//! cargo run -p gym --bin gym -- eval \
//!     --door lev=http://127.0.0.1:11436 \
//!     --questions support-v2-three-way-v2 \
//!     --record results/support-v2-three-way.jsonl
//! ```
//!
//! Hosted Jev reads `TYPESAFE_API_KEY` from the environment. Never pass a key
//! on the command line and never print one.

use std::collections::BTreeMap;
use std::time::Instant;

use gym::calibrate::{EstimatorConfig, Metrics, Record};
use gym::eval::{self, Disposition, Run};
use gym::gate::{self, Gate};
use gym::questions::QuestionSet;
use gym::row::{DoorIdentity, Row};
use gym::store::{Store, StoreError};
use gym::suite::{Item, Partition, Suite};
use jev::{Client, Config, Questions, SystemOneRequest};
use serde_json::Value;

/// The gate a run is judged by when the suite names none.
const DEFAULT_GATE: &str = "probability-v1";

/// What a door publishes about itself.
///
/// Kept apart from the client, because a run that reads recorded rows knows
/// all of this and has no door to ask.
#[derive(Clone, Debug)]
struct Facts {
    name: String,
    identity: DoorIdentity,
    estimator: String,
    samples: Option<u64>,
    seed_base: Option<u64>,
}

impl Facts {
    /// What a recorded row says about the door that produced it.
    fn of(row: &Row) -> Self {
        Self {
            name: row.door.clone(),
            identity: row.door_identity.clone(),
            estimator: row.estimator.clone(),
            samples: row.samples,
            seed_base: row.seed_base,
        }
    }
}

/// A door, and what it says it is.
struct Door {
    client: Client,
    facts: Facts,
}

/// Everything the flags said.
#[derive(Default)]
struct Options {
    doors: Vec<(String, String)>,
    jev: bool,
    suite: Option<String>,
    gate: Option<String>,
    /// The question set to serve, by id. Overrides the suite's own.
    questions: Option<String>,
    store: Option<String>,
    record: Option<String>,
    records: Option<String>,
    fit: bool,
    baseline: Option<String>,
    partition: Option<String>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let options = read_options(args);
    match command.as_str() {
        "eval" => run(eval_command(options)),
        "compare" => run(compare_command(&options)),
        "fit" => run(fit_command(&options)),
        "permute" => run(permute_command(options)),
        "" | "help" | "--help" | "-h" => {
            println!("{USAGE}");
        }
        other => {
            eprintln!("unknown command {other}\n\n{USAGE}");
            std::process::exit(2);
        }
    }
}

const USAGE: &str = "\
gym eval     score doors against a suite, fit maps, and record the rows
gym compare  compare doors from recorded rows
gym fit      fit and judge from recorded rows, asking no door
gym permute  measure how much option order moves the answer

  --door name=url     a door to ask; repeatable
  --jev               hosted Jev, from TYPESAFE_API_KEY
  --suite path        a suite file; the committed three-way suite by default
  --gate id           the acceptance rule; the suite's own by default
  --questions id      the question set to serve; the suite's own by default
  --partition name    calibration or development; both by default
  --fit               fit one map per family and judge it
  --record path       append every row to this store
  --records dir       write one calibration record per family here
  --store path        the store `compare` reads
  --baseline name     the side `compare` measures the others against";

fn run(outcome: Result<(), String>) {
    if let Err(trouble) = outcome {
        eprintln!("{trouble}");
        std::process::exit(2);
    }
}

fn read_options(args: impl Iterator<Item = String>) -> Options {
    let mut options = Options::default();
    let mut args = args.peekable();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--jev" => options.jev = true,
            "--fit" => options.fit = true,
            "--door" => {
                let Some(spec) = args.next() else { continue };
                match spec.split_once('=') {
                    Some((name, url)) => options.doors.push((name.to_string(), url.to_string())),
                    None => eprintln!("--door takes name=url, got {spec}"),
                }
            }
            "--suite" => options.suite = args.next(),
            "--gate" => options.gate = args.next(),
            "--questions" => options.questions = args.next(),
            "--store" => options.store = args.next(),
            "--record" => options.record = args.next(),
            "--records" => options.records = args.next(),
            "--baseline" => options.baseline = args.next(),
            "--partition" => options.partition = args.next(),
            other => eprintln!("unknown flag {other}"),
        }
    }
    options
}

fn load_suite(options: &Options) -> Result<Suite, String> {
    match options.suite.as_deref() {
        Some(path) => Suite::load_file(path).map_err(|error| error.to_string()),
        None => gym::suite::support_v2_three_way().map_err(|error| error.to_string()),
    }
}

/// The gate a run is judged by: the flag, then the suite's own reference,
/// then the default. A suite that names a rule is the normal case, and the
/// flag is how you judge old rows under a new rule.
fn load_gate(options: &Options, suite: &Suite) -> Result<Gate, String> {
    let id = options
        .gate
        .clone()
        .or_else(|| suite.gate.clone())
        .unwrap_or_else(|| DEFAULT_GATE.to_string());
    gate::load(&id).map_err(|error| error.to_string())
}

/// The question set a run serves: `--questions`, else the suite's own field,
/// else the text the items carry inline.
fn load_questions(options: &Options, suite: &Suite) -> Result<QuestionSet, String> {
    gym::questions::resolve(suite, options.questions.as_deref())
        .map_err(|error| error.to_string())
}

/// Which partitions a run reads. The locked partition is never one of them:
/// it is spent through a ledger, not scored by a flag.
fn partitions(options: &Options) -> Result<Vec<Partition>, String> {
    match options.partition.as_deref() {
        None => Ok(vec![Partition::Calibration, Partition::Development]),
        Some("calibration") => Ok(vec![Partition::Calibration]),
        Some("development") => Ok(vec![Partition::Development]),
        Some("locked") => Err("the locked partition is spent through a ledger, not scored \
             by a flag; see gym::suite::LockedLedger"
            .to_string()),
        Some(other) => Err(format!("unknown partition {other}")),
    }
}

fn items_of<'a>(suite: &'a Suite, wanted: &[Partition]) -> Result<Vec<&'a Item>, String> {
    let mut items = Vec::new();
    for partition in wanted {
        items.extend(suite.partition(*partition).map_err(|error| error.to_string())?);
    }
    Ok(items)
}

fn open_doors(options: &Options) -> Result<Vec<(String, Client)>, String> {
    let mut doors: Vec<(String, Client)> = Vec::new();
    if options.jev {
        match Client::from_env() {
            Ok(client) => doors.push(("jev (hosted)".to_string(), client)),
            Err(error) => eprintln!("skipping hosted Jev: {error}"),
        }
    }
    for (name, url) in &options.doors {
        let config = Config::new()
            .api_key("unused-by-a-local-door")
            .base_url(url.clone())
            .default_model(name.clone());
        match Client::new(config) {
            Ok(client) => doors.push((name.clone(), client)),
            Err(error) => eprintln!("skipping {name}: {error}"),
        }
    }
    if doors.is_empty() {
        return Err("no doors; pass --jev or --door name=url".to_string());
    }
    Ok(doors)
}

/// Reads `GET /v1/models` as raw JSON.
///
/// `jev::ModelCard` carries only name, description, and release date, and
/// drops the adapter, the base signature, and the estimator block a door
/// publishes. Every one of those belongs in a row: a run that cannot say what
/// it asked cannot be compared with another.
async fn ask_door(name: String, client: Client) -> Door {
    let unknown = Facts {
        name: name.clone(),
        identity: DoorIdentity::hosted(&name),
        estimator: "unreported".to_string(),
        samples: None,
        seed_base: None,
    };
    let published = published_facts(&client, unknown.clone()).await;
    Door { client, facts: published }
}

/// What `GET /v1/models` says, or what little is known without it.
async fn published_facts(client: &Client, unknown: Facts) -> Facts {
    let Ok(response) = client.models().list_raw(jev::ListOptions::default()).await else {
        return unknown;
    };
    let Ok(body) = serde_json::from_slice::<Value>(&response.bytes) else {
        return unknown;
    };
    let Some(model) = body.get("models").and_then(|models| models.get(0)) else {
        return unknown;
    };
    let text = |key: &str| model.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
    let number = |key: &str| model.get(key).and_then(Value::as_u64);
    let reported = text("name");
    let signature = text("base_model_signature");
    let estimator = text("estimator");
    Facts {
        identity: DoorIdentity::published(
            if reported.is_empty() { unknown.name.clone() } else { reported },
            signature,
            text("adapter"),
        ),
        estimator: if estimator.is_empty() { "unreported".to_string() } else { estimator },
        samples: number("samples"),
        seed_base: number("seed_base"),
        ..unknown
    }
}

/// How many options a Choice question serves.
fn option_count(question: &Value) -> usize {
    eval::options_of(question).map(|options| options.len()).unwrap_or_default()
}

fn question_for(question: &Value) -> Questions {
    // The suite stores each question in the shape the door reads; send it
    // through the wire unchanged so the call is the one a caller would make.
    Questions::new().with("q", jev::Question::Raw(question.clone()))
}

/// Asks one door one item, and times it.
async fn ask(client: &Client, state: &Value, question: &Value) -> (Disposition, Option<f64>) {
    let request = SystemOneRequest::new(state.clone(), question_for(question));
    let started = Instant::now();
    let response = client.system_one(request).await;
    let elapsed = started.elapsed().as_secs_f64() * 1000.0;
    match response {
        Ok(response) => match response.answers.get("q") {
            Some(answer) => (eval::read_answer(answer), Some(elapsed)),
            None => (
                Disposition::Harness("the door answered without the question".to_string()),
                None,
            ),
        },
        // The line between a door that declined and a harness that failed.
        // `gym::eval::classify` owns it and explains itself there.
        Err(error) => match eval::classify(&error) {
            Disposition::Harness(detail) => (Disposition::Harness(detail), None),
            refused => (refused, Some(elapsed)),
        },
    }
}

/// What one door's pass over the suite produced.
struct Pass {
    rows: Vec<Row>,
    lost: usize,
}

#[tokio::main(flavor = "current_thread")]
async fn eval_command(options: Options) -> Result<(), String> {
    let suite = load_suite(&options)?;
    let gate = load_gate(&options, &suite)?;
    let questions = load_questions(&options, &suite)?;
    let wanted = partitions(&options)?;
    let items = items_of(&suite, &wanted)?;
    let doors = open_doors(&options)?;
    let store = options.record.as_deref().map(Store::at);

    println!("# Suite scores: `{}`\n", suite.name);
    let counts = suite.counts();
    println!(
        "{} items, {} calibration, {} development, {} locked and unread, digest `{}`.\n",
        suite.items.len(),
        counts[&Partition::Calibration],
        counts[&Partition::Development],
        counts[&Partition::Locked],
        &suite.digest[..16]
    );
    println!("{}\n", suite.description);
    println!("Judged by `{}`, digest `{}`.\n", gate.id, gate.digest());
    println!(
        "Asked as `{}`, digest `{}`. The suite digest covers the items and the question set \
         covers the text, so a reword is a candidate against these items rather than another \
         suite.\n",
        questions.id,
        &questions.digest()[..16]
    );

    for (name, client) in doors {
        let door = ask_door(name, client).await;
        println!("## {}\n", door.facts.name);
        report_identity(&door.facts);

        let run = run_over(&suite, &gate, &questions, &door.facts);

        // Refuse a repeat before spending a door's time on it, rather than
        // after. The store refuses a duplicate row either way; doing it here
        // means a second identical run costs nothing and says so.
        let held = Held::read(store.as_ref())?;
        if let Some(store) = &store {
            refuse_repeat(&held, store, &run, &items, None)?;
        }

        let pass = score_pass(&door, &run, &questions, &items, store.as_ref()).await?;
        if let Some(store) = &store {
            println!("Recorded {} rows in `{}`.\n", pass.rows.len(), store.path().display());
        }
        report_pass(&pass, items.len());
        report_scores(&pass.rows, &wanted);

        if options.fit {
            fit_and_report(&suite, &gate, &door.facts, &pass.rows, options.records.as_deref())?;
        }
        println!();
    }
    Ok(())
}

/// What every row of one run shares.
fn run_over(suite: &Suite, gate: &Gate, questions: &QuestionSet, facts: &Facts) -> Run {
    Run {
        suite: suite.name.clone(),
        suite_digest: suite.digest.clone(),
        question_set: Some(questions.id.clone()),
        question_digest: Some(questions.digest()),
        door: facts.name.clone(),
        door_identity: facts.identity.clone(),
        estimator: facts.estimator.clone(),
        samples: facts.samples,
        seed_base: facts.seed_base,
        recorded_at: eval::now_utc(),
        gate_id: Some(gate.id.clone()),
        gate_digest: Some(gate.digest()),
    }
}

fn report_identity(facts: &Facts) {
    if facts.identity.verified {
        println!(
            "Identity: base `{}`{}. Estimator `{}`{}{}.\n",
            facts.identity.base_model_signature,
            if facts.identity.adapter.is_empty() {
                String::new()
            } else {
                format!(", adapter `{}`", facts.identity.adapter)
            },
            facts.estimator,
            facts.samples.map(|n| format!(", {n} samples")).unwrap_or_default(),
            facts.seed_base.map(|n| format!(", seed block {n}")).unwrap_or_default(),
        );
    } else {
        // A hosted closed model publishes a name and no more, and the row
        // says so rather than carrying a signature nobody checked.
        println!("Identity: not verifiable for this door.\n");
    }
}

/// Asks one door every item, recording each row as it is produced.
///
/// Each row lands in the store before the next item is asked, so a run that
/// is interrupted keeps what it measured. The duplicate check that would
/// otherwise stop it halfway has already run, before the door was asked at
/// all.
async fn score_pass(
    door: &Door,
    run: &Run,
    questions: &QuestionSet,
    items: &[&Item],
    store: Option<&Store>,
) -> Result<Pass, String> {
    let mut rows = Vec::with_capacity(items.len());
    let mut lost = 0;
    for item in items {
        let question = questions.ask(item).map_err(|error| error.to_string())?;
        let (disposition, latency) = ask(&door.client, &item.state, question).await;
        match run.row(item, None, &disposition, latency) {
            Some(row) => {
                if let Some(store) = store {
                    append(store, &row)?;
                }
                rows.push(row);
            }
            None => {
                lost += 1;
                if let Disposition::Harness(detail) = &disposition {
                    eprintln!("{}: {detail}", item.id);
                }
            }
        }
    }
    Ok(Pass { rows, lost })
}

/// The trials a store already holds, by their perturbation key.
///
/// Loaded once. A run consults it twice: to refuse a repeat before spending a
/// door's time on it, and to reuse a trial the store already records rather
/// than asking for it a second time.
struct Held {
    rows: BTreeMap<String, Row>,
}

impl Held {
    fn read(store: Option<&Store>) -> Result<Self, String> {
        let mut rows = BTreeMap::new();
        if let Some(store) = store {
            for value in store.verified_rows().map_err(|error| error.to_string())? {
                let key = gym::store::perturbation_key(&value);
                let row: Row = serde_json::from_value(value).map_err(|error| error.to_string())?;
                rows.insert(key, row);
            }
        }
        Ok(Self { rows })
    }

    /// The row this trial would be, when the store already holds it.
    fn row(&self, planned: &Row) -> Option<&Row> {
        let value = serde_json::to_value(planned).ok()?;
        self.rows.get(&gym::store::perturbation_key(&value))
    }
}

/// One planned trial, with a placeholder outcome, for a lookup or a message.
fn planned(run: &Run, item: &Item, permutation: Option<Vec<usize>>) -> Result<Row, String> {
    run.row(item, permutation, &Disposition::Refused(gym::row::RefusalCode::Busy), None)
        .ok_or_else(|| "a planned row failed to build".to_string())
}

/// What a recorded row says the door answered.
///
/// A row is the whole trace of a System One call, so a run that finds one can
/// use it instead of asking the door the same question again. It is the same
/// answer: these doors reproduce exactly within a seed block.
fn recorded_answer(row: &Row) -> Option<Disposition> {
    if let Some(code) = &row.refusal {
        return Some(Disposition::Refused(code.clone()));
    }
    let distribution = row.distribution.clone()?;
    let chosen = distribution
        .iter()
        .max_by(|left, right| left.1.total_cmp(right.1))
        .map(|(option, _)| option.clone())?;
    Some(Disposition::Answered { chosen, distribution })
}

/// Refuses a run whose rows the store already holds.
fn refuse_repeat(
    held: &Held,
    store: &Store,
    run: &Run,
    items: &[&Item],
    permutation: Option<Vec<usize>>,
) -> Result<(), String> {
    for item in items {
        if held.row(&planned(run, item, permutation.clone())?).is_some() {
            return Err(format!(
                "{} already records this perturbation for `{}` on item {}: same suite digest, \
                 question set, door identity, estimator, seed block, and option order. \
                 Re-scoring a run does not make it a second run. Move the seed block with \
                 --seed-base on the door, reword the question set with --questions, or record \
                 to another store.",
                store.path().display(),
                run.door,
                item.id
            ));
        }
    }
    Ok(())
}

fn append(store: &Store, row: &Row) -> Result<(), String> {
    row.check().map_err(|error| format!("{}: {error}", row.item_id))?;
    match store.append(row) {
        Ok(_) => Ok(()),
        Err(StoreError::DuplicatePerturbation { index, detail }) => Err(format!(
            "row {index} already records this perturbation ({detail}); the store keeps one \
             trial per perturbation, and the rows this run already wrote stay"
        )),
        Err(error) => Err(error.to_string()),
    }
}

fn report_pass(pass: &Pass, asked: usize) {
    let scored = pass.rows.iter().filter(|row| row.is_scored()).count();
    let refused = pass.rows.iter().filter(|row| row.is_refused()).count();
    // Say what did not get scored, and why. A shrinking items column with no
    // explanation is how a door gets credit for declining.
    println!(
        "{asked} items asked: {scored} scored, {refused} refused by the door, {} lost to the \
         harness and unrecorded.\n",
        pass.lost
    );
    let refusals = eval::refusals(&pass.rows);
    if !refusals.is_empty() {
        let detail: Vec<String> =
            refusals.iter().map(|(code, count)| format!("`{code}` x{count}")).collect();
        println!("Door refusals: {}.\n", detail.join(", "));
    }
}

fn metrics_row(label: &str, metrics: Metrics) -> String {
    format!(
        "| {label} | {:.2} | {:.3} | {:.3} | {:.3} | {} | {} |",
        metrics.accuracy,
        metrics.ece,
        metrics.brier,
        metrics.nll,
        metrics.confident_errors,
        metrics.items
    )
}

/// The table, as a view over the rows the run just wrote.
fn report_scores(rows: &[Row], wanted: &[Partition]) {
    println!("| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for partition in wanted {
        let inside: Vec<Row> = rows
            .iter()
            .filter(|row| row.split == partition.as_str())
            .cloned()
            .collect();
        let metrics = gym::calibrate::score(&eval::observations(&inside));
        println!("{}", metrics_row(&format!("{partition}, raw"), metrics));
    }
    println!();
}

fn fit_and_report(
    suite: &Suite,
    gate: &Gate,
    facts: &Facts,
    rows: &[Row],
    records: Option<&str>,
) -> Result<(), String> {
    let calibration: Vec<Row> = partition_rows(rows, Partition::Calibration);
    let held: Vec<Row> = partition_rows(rows, Partition::Development);
    if held.is_empty() {
        return Err("nothing to judge a map on: the development partition produced no rows"
            .to_string());
    }

    println!(
        "\n| Family | Fitted on | Raw ECE | Mapped ECE | Raw NLL | Mapped NLL | Raw Brier | \
         Mapped Brier | Verdict |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    let mut written: Vec<String> = Vec::new();
    for family in eval::families(rows) {
        let fit_on: Vec<Row> =
            calibration.iter().filter(|row| row.family == family).cloned().collect();
        let score_on: Vec<Row> = held.iter().filter(|row| row.family == family).cloned().collect();
        let fit = eval::fit_family(&family, &fit_on, &score_on, gate);
        println!(
            "| `{family}` | {} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {} |",
            fit.map.fitted_on,
            fit.raw.ece,
            fit.calibrated.ece,
            fit.raw.nll,
            fit.calibrated.nll,
            fit.raw.brier,
            fit.calibrated.brier,
            fit.verdict()
        );
        if let Some(dir) = records {
            written.push(write_record(dir, suite, gate, facts, &fit)?);
        }
    }
    for path in written {
        println!("\nWrote `{path}`.");
    }
    Ok(())
}

fn partition_rows(rows: &[Row], partition: Partition) -> Vec<Row> {
    rows.iter().filter(|row| row.split == partition.as_str()).cloned().collect()
}

fn write_record(
    dir: &str,
    suite: &Suite,
    gate: &Gate,
    facts: &Facts,
    fit: &eval::Fit,
) -> Result<String, String> {
    let record = Record {
        schema: gym::calibrate::RECORD_SCHEMA.to_string(),
        family: fit.family.clone(),
        estimator_config: EstimatorConfig::new(
            facts.estimator.clone(),
            facts.samples.unwrap_or(1),
            facts.seed_base.unwrap_or(0),
        ),
        language: "en".to_string(),
        suite: suite.name.clone(),
        suite_digest: suite.digest.clone(),
        partition_id: Partition::Calibration.as_str().to_string(),
        os_build: std::env::var("LEV_OS_BUILD").unwrap_or_default(),
        door: facts.name.clone(),
        door_identity: facts.identity.clone(),
        gate_id: Some(gate.id.clone()),
        gate_digest: Some(gate.digest()),
        // A record written by this command never reads the locked partition.
        // Spending it is a deliberate act through the ledger, and a record
        // that rests on one names the read here.
        locked_reads: Vec::new(),
        fitted: eval::now_utc()[..10].to_string(),
        map: fit.map.clone(),
        raw_metrics: fit.raw,
        calibrated_metrics: fit.calibrated,
        admitted: fit.admitted(),
        verdict: fit.verdict(),
    };
    // Namespaced by door: three doors scoring the same suite would otherwise
    // overwrite each other's records, and the last one to run would silently
    // win.
    let slug = facts.name.replace([' ', '(', ')', '/'], "_");
    let directory = format!("{dir}/{slug}");
    std::fs::create_dir_all(&directory).map_err(|error| format!("{directory}: {error}"))?;
    let path = format!("{directory}/{}.json", fit.family);
    let text = serde_json::to_string_pretty(&record).map_err(|error| error.to_string())?;
    std::fs::write(&path, text + "\n").map_err(|error| format!("{path}: {error}"))?;
    Ok(path)
}

/// The comparison, read back from the store rather than run again.
///
/// A live four-way table is a measurement nobody else can check: it exists
/// while the four doors are up and then only as a paste. The rows are the
/// evidence, and this is a view over them.
fn compare_command(options: &Options) -> Result<(), String> {
    let path = options
        .store
        .as_deref()
        .ok_or_else(|| "compare reads recorded rows; pass --store path".to_string())?;
    let store = Store::at(path);
    let values = store.verified_rows().map_err(|error| error.to_string())?;
    if values.is_empty() {
        return Err(format!("{path} holds no rows"));
    }
    let mut rows: Vec<Row> = Vec::with_capacity(values.len());
    for value in values {
        rows.push(serde_json::from_value(value).map_err(|error| error.to_string())?);
    }

    let mut sides: Vec<Side> = Vec::new();
    for row in &rows {
        let side = Side::of(row);
        if !sides.contains(&side) {
            sides.push(side);
        }
    }

    println!(
        "# One contract, {} implementation{}\n",
        sides.len(),
        if sides.len() == 1 { "" } else { "s" }
    );
    println!(
        "Read from `{path}`: {} rows over {} side{}. The chain verified, so no row was removed \
         and none was inserted.\n",
        rows.len(),
        sides.len(),
        if sides.len() == 1 { "" } else { "s" }
    );

    println!(
        "| Side | Identity | Accuracy | ECE | Brier | NLL | Confident errors | Scored | Refused \
         | Median latency |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    let mut measured: BTreeMap<String, Metrics> = BTreeMap::new();
    let mut held: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for side in &sides {
        let inside: Vec<Row> =
            rows.iter().filter(|row| &Side::of(row) == side).cloned().collect();
        let metrics = gym::calibrate::score(&eval::observations(&inside));
        let refused = inside.iter().filter(|row| row.is_refused()).count();
        println!(
            "| {} | {} | {:.2} | {:.3} | {:.3} | {:.3} | {} | {} | {} | {} |",
            side.label(),
            identity_of(&inside),
            metrics.accuracy,
            metrics.ece,
            metrics.brier,
            metrics.nll,
            metrics.confident_errors,
            metrics.items,
            refused,
            median_latency(&inside),
        );
        measured.insert(side.label(), metrics);
        held.insert(side.label(), inside);
    }
    println!(
        "\nA refused item stays in the door's denominator and out of its numerator, so a door \
         that declines the hard questions does not score better for declining them.\n"
    );

    let refusals = eval::refusals(&rows);
    if !refusals.is_empty() {
        let detail: Vec<String> =
            refusals.iter().map(|(code, count)| format!("`{code}` x{count}")).collect();
        println!("Refusals across every door: {}.\n", detail.join(", "));
    }

    let labels: Vec<String> = sides.iter().map(Side::label).collect();
    let baseline = match &options.baseline {
        Some(named) => sides
            .iter()
            .find(|side| &side.door == named || &side.label() == named)
            .map(Side::label)
            .ok_or_else(|| format!("no rows for the baseline {named}"))?,
        None => labels[0].clone(),
    };
    let Some(before) = measured.get(&baseline) else {
        return Err(format!("no rows for the baseline {baseline}"));
    };
    let gate = gate::load(options.gate.as_deref().unwrap_or("decision-v1"))
        .map_err(|error| error.to_string())?;
    println!("## Judged by `{}`, digest `{}`\n", gate.id, gate.digest());
    if labels.len() < 2 {
        println!(
            "Only `{baseline}` has rows in this store, so there is nothing to compare it \
             against yet.\n"
        );
        return Ok(());
    }
    println!("| Candidate | Against | Comparing | Verdict | Deciding criterion |");
    println!("| --- | --- | --- | --- | --- |");
    for label in labels.iter().filter(|label| *label != &baseline) {
        let Some(after) = measured.get(label) else { continue };
        // The store decides what these two sides are before the gate judges
        // them. Two sides that did not score the same items are not a worse
        // result; they are a number about two different things, and the
        // refusal travels into the table rather than being swallowed.
        let comparing = match admit(held.get(&baseline), held.get(label)) {
            Ok(comparison) => comparison.as_str().to_string(),
            Err(refusal) => {
                println!("| {label} | {baseline} | refused | not a comparison | {refusal} |");
                continue;
            }
        };
        let outcome = gate.judge(&gate::Comparison::new(
            format!("{label} against {baseline}"),
            before.scores(),
            after.scores(),
        ));
        let deciding = outcome
            .deciding()
            .map(|criterion| format!("{}: {}", criterion.name, criterion.detail))
            .unwrap_or_else(|| "nothing was judged".to_string());
        println!(
            "| {label} | {baseline} | {comparing} | {} | {deciding} |",
            outcome.verdict
        );
    }
    println!();
    Ok(())
}

/// One side of a comparison: a door, and the question set it was asked.
///
/// The door alone is not enough once a question-text variant is expressible.
/// The same door answering a reworded question is the candidate in that
/// experiment, and folding its rows into the baseline's would average a
/// comparison away.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Side {
    door: String,
    questions: Option<String>,
}

impl Side {
    fn of(row: &Row) -> Self {
        Self {
            door: row.door.clone(),
            questions: row.question_set.clone(),
        }
    }

    fn label(&self) -> String {
        match &self.questions {
            Some(set) => format!("{} asked as {set}", self.door),
            None => self.door.clone(),
        }
    }
}

/// Whether two sides are a comparison, asked of the store rather than
/// decided here.
fn admit(
    baseline: Option<&Vec<Row>>,
    candidate: Option<&Vec<Row>>,
) -> Result<gym::store::Comparison, String> {
    let values = |rows: Option<&Vec<Row>>| -> Vec<Value> {
        rows.map(Vec::as_slice)
            .unwrap_or_default()
            .iter()
            .filter_map(|row| serde_json::to_value(row).ok())
            .collect()
    };
    gym::store::admit_comparison(&values(baseline), &values(candidate))
        .map_err(|error| error.to_string())
}

/// What the rows say the door was running. Rows that disagree say so, rather
/// than the first one speaking for the rest.
fn identity_of(rows: &[Row]) -> String {
    let mut seen: Vec<String> = Vec::new();
    for row in rows {
        let identity = if row.door_identity.verified {
            let base: String = row.door_identity.base_model_signature.chars().take(8).collect();
            match row.door_identity.adapter.as_str() {
                "" => format!("base `{base}`"),
                adapter => format!("base `{base}`, adapter `{adapter}`"),
            }
        } else {
            "not verifiable".to_string()
        };
        if !seen.contains(&identity) {
            seen.push(identity);
        }
    }
    seen.join("; ")
}

fn median_latency(rows: &[Row]) -> String {
    let mut measured: Vec<f64> = rows.iter().filter_map(|row| row.latency_ms).collect();
    if measured.is_empty() {
        // Unknown, and never zero. A door whose latency nobody recorded did
        // not answer instantly.
        return "unknown".to_string();
    }
    measured.sort_by(f64::total_cmp);
    format!("{:.0} ms", measured[measured.len() / 2])
}

/// Measures how much option order moves the answer.
///
/// Order sensitivity is the quality gap that neither accuracy nor calibration
/// shows. A model that answers differently when the options are listed in a
/// different order is not reading the state; it is reading the list.
#[tokio::main(flavor = "current_thread")]
async fn permute_command(options: Options) -> Result<(), String> {
    let suite = load_suite(&options)?;
    let gate = load_gate(&options, &suite)?;
    let questions = load_questions(&options, &suite)?;
    let wanted = partitions(&options)?;
    let items = items_of(&suite, &wanted)?;
    // Only a Choice has an order to permute. A Noul's two options and a
    // Score's ordered levels both carry meaning in their order.
    let mut choices: Vec<(&Item, &Value)> = Vec::new();
    for item in items {
        let question = questions.ask(item).map_err(|error| error.to_string())?;
        if eval::options_of(question).is_some() {
            choices.push((item, question));
        }
    }
    if choices.is_empty() {
        return Err("the suite holds no Choice items, so there is no order to permute".to_string());
    }
    let doors = open_doors(&options)?;
    let store = options.record.as_deref().map(Store::at);

    println!("# Option order\n");
    println!(
        "{} Choice items from `{}`, each asked twice: once in the suite's order and once \
         reversed. A flip is an item whose winning option changed.\n",
        choices.len(),
        suite.name
    );
    println!(
        "| Door | Items | Flips | Flip rate | Accuracy forward | Accuracy reversed | Forward \
         rows reused |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- |");

    for (name, client) in doors {
        let door = ask_door(name, client).await;
        let run = run_over(&suite, &gate, &questions, &door.facts);
        // The reversed pass is what this command exists to measure, so that
        // is the pass a repeat is refused on, item by item, because each
        // item's order is its own. The forward pass is the same trial an
        // ordinary run records; where the store already holds it, it is read
        // rather than asked again.
        let held = Held::read(store.as_ref())?;
        if let Some(store) = &store {
            for (item, question) in &choices {
                let order = eval::reversed(option_count(question));
                refuse_repeat(&held, store, &run, &[*item], Some(order))?;
            }
        }

        let mut forward_rows: Vec<Row> = Vec::new();
        let mut reversed_rows: Vec<Row> = Vec::new();
        let mut reused = 0_usize;
        let mut trials = 0_usize;
        let mut flips = 0_usize;
        for (item, question) in &choices {
            let order = eval::reversed(option_count(question));
            let Some(backward) = eval::permuted(question, &order) else { continue };

            let recorded = held.row(&planned(&run, item, None)?).cloned();
            let forward_answer = match recorded.as_ref().and_then(recorded_answer) {
                Some(answer) => {
                    reused += 1;
                    forward_rows.extend(recorded);
                    answer
                }
                None => {
                    let (answer, latency) = ask(&door.client, &item.state, question).await;
                    if let Some(row) = run.row(item, None, &answer, latency) {
                        if let Some(store) = &store {
                            append(store, &row)?;
                        }
                        forward_rows.push(row);
                    }
                    answer
                }
            };

            let (reversed_answer, latency) = ask(&door.client, &item.state, &backward).await;
            if let Some(row) = run.row(item, Some(order), &reversed_answer, latency) {
                if let Some(store) = &store {
                    append(store, &row)?;
                }
                reversed_rows.push(row);
            }

            let (
                Disposition::Answered { chosen: forward, .. },
                Disposition::Answered { chosen: reversed, .. },
            ) = (&forward_answer, &reversed_answer)
            else {
                continue;
            };
            trials += 1;
            if forward != reversed {
                flips += 1;
            }
        }

        println!(
            "| `{}` | {trials} | {flips} | {:.3} | {:.2} | {:.2} | {reused} |",
            door.facts.name,
            flips as f64 / trials.max(1) as f64,
            gym::calibrate::score(&eval::observations(&forward_rows)).accuracy,
            gym::calibrate::score(&eval::observations(&reversed_rows)).accuracy,
        );
    }
    println!(
        "\nBoth passes are recorded as rows, the reversed pass carrying its permutation, so the \
         flip rate is a query over the record rather than a number in a paragraph.\n"
    );
    Ok(())
}

/// Fits and judges from rows the store already holds, without asking a door.
///
/// This is what a digested gate buys. Changing a rule does not mean running
/// the doors again: the rows carry the whole trace of every call, so
/// re-judging them under `probability-v2` is a query over the record. It is
/// also how a record is rewritten after a rule changes, which is the only
/// honest way to do it — the alternative is a record whose verdict came from
/// a rule nobody can name.
fn fit_command(options: &Options) -> Result<(), String> {
    let suite = load_suite(options)?;
    let gate = load_gate(options, &suite)?;
    let questions = load_questions(options, &suite)?;
    let path = options
        .store
        .as_deref()
        .ok_or_else(|| "fit reads recorded rows; pass --store path".to_string())?;
    let rows = read_rows(path)?;
    let scored: Vec<Row> = rows
        .iter()
        .filter(|row| row.suite_digest == suite.digest && asked_as(row, &suite, &questions))
        .cloned()
        .collect();
    if scored.is_empty() {
        return Err(format!(
            "{path} holds no rows for `{}` at digest {} asked as `{}`",
            suite.name,
            &suite.digest[..16],
            questions.id
        ));
    }

    println!("# Fitted maps, from the record\n");
    println!(
        "Read from `{path}`: {} rows of {} for `{}` at digest `{}`. No door was asked.\n",
        scored.len(),
        rows.len(),
        suite.name,
        &suite.digest[..16]
    );
    println!("Judged by `{}`, digest `{}`.\n", gate.id, gate.digest());

    let mut doors: Vec<String> = Vec::new();
    for row in &scored {
        if !doors.contains(&row.door) {
            doors.push(row.door.clone());
        }
    }

    for name in doors {
        let mine: Vec<Row> = scored.iter().filter(|row| row.door == name).cloned().collect();
        // Only the suite's own option order is fitted on. A permuted trial is
        // a measurement of order sensitivity, not a second reading of the
        // item, and pooling the two doubles an item's weight in the table.
        let mine: Vec<Row> = mine.into_iter().filter(|row| row.permutation.is_none()).collect();
        let Some(first) = mine.first() else { continue };
        let facts = Facts::of(first);
        println!("## {name}\n");
        report_identity(&facts);
        fit_and_report(&suite, &gate, &facts, &mine, options.records.as_deref())?;
        println!();
    }
    Ok(())
}

/// Whether a recorded row was served the question set a run is reading.
///
/// A store can hold a baseline and a reworded variant side by side, and
/// pooling them would fit one table on two readings of every item. So a row
/// counts when it pins this set's digest.
///
/// A row that pins no set at all counts only when the set in force is the
/// suite's own authored text. Those rows predate `question_digest`, and they
/// were served the text their suite digest already covers; they were not
/// served a variant, and reading them as one would be inventing a
/// measurement.
fn asked_as(row: &Row, suite: &Suite, questions: &QuestionSet) -> bool {
    match &row.question_digest {
        Some(digest) => digest == &questions.digest(),
        None => QuestionSet::authored(suite)
            .map(|authored| authored.digest() == questions.digest())
            .unwrap_or(false),
    }
}

/// Every row in a store, verified and typed.
fn read_rows(path: &str) -> Result<Vec<Row>, String> {
    let values = Store::at(path).verified_rows().map_err(|error| error.to_string())?;
    values
        .into_iter()
        .map(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
        .collect()
}
