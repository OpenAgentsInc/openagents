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
//!
//! # did this commit move the numbers? run it before you push
//! cargo run -p gym --bin gym -- regress \
//!     --store results/support-v2-three-way.jsonl \
//!     --against results/last-week.jsonl
//! ```
//!
//! Hosted Jev reads `TYPESAFE_API_KEY` from the environment. Never pass a key
//! on the command line and never print one.

use std::collections::BTreeMap;
use std::time::Instant;

use gym::ab::Metric;
use gym::calibrate::{EstimatorConfig, Map, Metrics, Observation, Record, score};
use gym::coverage::{Coverage, Expected, RunKey, run_groups};
use gym::eval::{self, Disposition, Run};
use gym::gate::{self, Gate, Profile};
use gym::questions::QuestionSet;
use gym::regress;
use gym::row::{DoorIdentity, Row};
use gym::spread::{Draws, Spread, ece_frozen, mean_signal, stratified_bootstrap};
use gym::store::{ChainVerdict, Store, StoreError, verify_chain};
use gym::suite::{Item, Partition, Suite};
use jev::{Client, Config, Questions, SystemOneRequest};
use serde_json::Value;

/// The gate a run is judged by when the suite names none.
const DEFAULT_GATE: &str = "probability-v2";

/// How many passes `gym latency` makes when the caller names no number.
///
/// Eight, to match the eight seed blocks in
/// `docs/lev/measurements/2026-09-19-seed-variance.md`, so the two spreads
/// are read off the same number of draws.
const DEFAULT_BLOCKS: usize = 8;

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
    against: Option<String>,
    partition: Option<String>,
    blocks: Option<usize>,
    /// The one family to ask. Every family the suite holds by default.
    family: Option<String>,
    items: Option<String>,
    /// The stores `merge` folds into `--store`.
    from: Vec<String>,
    timeout: Option<u64>,
    /// The draws `spread` reads.
    draws: Option<String>,
    /// Where `report` writes the record; stdout by default.
    out: Option<String>,
    /// A door `report` was told the run was meant to ask; repeatable.
    expect: Vec<String>,
    /// Where `report` writes the report commitment, and the file `verify`
    /// checks a store against.
    commitment: Option<String>,
    /// The caller's JSONL file `build` reads.
    input: Option<String>,
    /// The suite's name, and the question set's id.
    name: Option<String>,
    /// Who the caller's labels are.
    label_source: Option<String>,
    /// How the caller's labels were produced.
    label_rule: Option<String>,
    /// Where the caller's data came from.
    source: Option<String>,
    /// The caller's licence statement for measurement use.
    licence: Option<String>,
    /// The suite's creation date; today by default.
    created: Option<String>,
    /// A description to use instead of the generated one.
    description: Option<String>,
    /// `family=ceiling` pairs for the provenance's agreement map.
    agreement: Vec<String>,
    /// Where `build` writes the suite.
    suite_out: Option<String>,
    /// Where `build` writes the question set.
    questions_out: Option<String>,
    /// The frozen admission plan `admit` judges.
    plan: Option<String>,
    /// The store `admit` reads the locked-confirmation rows from.
    locked: Option<String>,
    /// Retained report for the locked store.
    locked_commitment: Option<String>,
    /// The ledger `admit` checks the locked read was spent under.
    ledger: Option<String>,
    /// The transfer suite `admit` checks the candidate against.
    transfer_suite: Option<String>,
    /// The store `admit` reads the transfer rows from.
    transfer_store: Option<String>,
    /// Retained report for the transfer store.
    transfer_commitment: Option<String>,
    /// When `admit` dates the decision; now by default.
    at: Option<String>,
}

fn main() {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let options = read_options(args);
    match command.as_str() {
        "eval" => run(eval_command(options)),
        "compare" => run(compare_command(&options)),
        "fit" => run(fit_command(&options)),
        "merge" => run(merge_command(&options)),
        "spread" => run(spread_command(&options)),
        // One name, two sources: a store to read, or doors to ask.
        "permute" if options.store.is_some() => run(flips_command(&options)),
        "permute" => run(permute_command(options)),
        "latency" => run(latency_command(options)),
        "report" => run(report_command(&options)),
        "verify" => run(verify_command(&options)),
        "build" => run(build_command(&options)),
        "regress" => run(regress_command(&options)),
        "admit" => run(admit_command(&options)),
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
gym merge    fold one store's rows into another, re-sealing the chain
gym spread   measure how far each metric moves across seed blocks
gym permute  measure how much option order moves the answer, or read it back
             from a store with --store
gym latency  measure how much wall clock moves when nothing else does
gym report   render a measured record from recorded rows
gym verify   walk a store's receipt chain and say where it breaks
gym build    turn a caller's labelled JSONL into a suite and question set
gym regress  compare a door with its own last recorded run
gym admit    judge a frozen admission plan against recorded evidence and
             write the decision a registry activates

  --door name=url     a door to ask; repeatable
  --jev               hosted Jev, from TYPESAFE_API_KEY
  --suite path        a suite file; the committed three-way suite by default
  --gate id           the acceptance rule; the suite's own by default
  --questions id      the question set to serve; the suite's own by default
  --partition name    calibration or development; both by default
  --family name       the one family to ask; every family by default
  --expect name       a door `report` was told the run was meant to ask;
                      repeatable
  --commitment path   where `report` writes the record's commitment, or
                      the file `verify` checks the store against
  --fit               fit one map per family and judge it
  --record path       append every row to this store
  --records dir       write one calibration record per family here
  --store path        the store `compare`, `fit`, `permute`, and `regress` read
  --draws path        the draws `spread` reads, from `lev-calibration-sweep`
  --baseline name     the side `compare` measures the others against
  --against path      the store `regress` measures the rows in `--store`
                      against; the same store by default
  --items path        narrow eval or a recorded view to these item ids
  --from path         a store `merge` folds into `--store`; repeatable
  --blocks n          how many passes `latency` makes; 8 by default
  --out path          the file `report` writes; stdout by default
  --input path        the caller's JSONL `build` reads
  --name id           the suite's name, and the question set's id
  --label-source who  whose labels the records carry
  --label-rule rule   how the labels were produced, in one sentence
  --source text       where the caller's data came from
  --licence text      the caller's licence statement for measurement use
  --created date      the suite's creation date; today by default
  --description text  a description to use instead of the generated one
  --agreement f=c     the ceiling a family's labels rest on; repeatable
  --suite-out path    where `build` writes the suite; `<name>.json` here
  --questions-out p   where `build` writes the question set
  --timeout seconds   how long one call to a `--door` may take; the client's
                      ten seconds by default, and worth raising on a busy
                      machine, because a timeout loses the item entirely
  --plan path         the frozen admission plan `admit` judges
  --locked path       the store `admit` reads the locked-confirmation rows
                      from; with --ledger
  --ledger path       the ledger `admit` checks the locked read was spent
                      under; with --locked
  --transfer-suite p  the transfer suite `admit` checks the candidate against
  --transfer-store p  the store `admit` reads the transfer rows from
  --at timestamp      when `admit` dates the decision; now by default

`regress` exits 1 when something regressed, 2 when it could not compare, and
0 otherwise. Read the report either way.

`admit` writes the decision to `--out` or stdout and exits 1 when the ruling
is not `passed`: only a passed decision may activate a candidate.";

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
            "--draws" => options.draws = args.next(),
            "--record" => options.record = args.next(),
            "--records" => options.records = args.next(),
            "--baseline" => options.baseline = args.next(),
            "--against" => options.against = args.next(),
            "--partition" => options.partition = args.next(),
            "--blocks" => options.blocks = args.next().and_then(|n| n.parse().ok()),
            "--family" => options.family = args.next(),
            "--items" => options.items = args.next(),
            "--from" => options.from.extend(args.next()),
            "--expect" => options.expect.extend(args.next()),
            "--commitment" => options.commitment = args.next(),
            "--locked-commitment" => options.locked_commitment = args.next(),
            "--transfer-commitment" => options.transfer_commitment = args.next(),
            "--out" => options.out = args.next(),
            "--input" => options.input = args.next(),
            "--name" => options.name = args.next(),
            "--label-source" => options.label_source = args.next(),
            "--label-rule" => options.label_rule = args.next(),
            "--source" => options.source = args.next(),
            "--licence" => options.licence = args.next(),
            "--created" => options.created = args.next(),
            "--description" => options.description = args.next(),
            "--agreement" => options.agreement.extend(args.next()),
            "--suite-out" => options.suite_out = args.next(),
            "--questions-out" => options.questions_out = args.next(),
            "--timeout" => options.timeout = args.next().and_then(|value| value.parse().ok()),
            "--plan" => options.plan = args.next(),
            "--locked" => options.locked = args.next(),
            "--ledger" => options.ledger = args.next(),
            "--transfer-suite" => options.transfer_suite = args.next(),
            "--transfer-store" => options.transfer_store = args.next(),
            "--at" => options.at = args.next(),
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
    gym::questions::resolve(suite, options.questions.as_deref()).map_err(|error| error.to_string())
}

/// Which partitions a run reads. The locked partition is never one of them:
/// it is spent through a ledger, not scored by a flag.
fn partitions(options: &Options) -> Result<Vec<Partition>, String> {
    match options.partition.as_deref() {
        None => Ok(vec![Partition::Calibration, Partition::Development]),
        Some("calibration") => Ok(vec![Partition::Calibration]),
        Some("development") => Ok(vec![Partition::Development]),
        Some("locked") => Err(
            "the locked partition is spent through a ledger, not scored \
             by a flag; see gym::suite::LockedLedger"
                .to_string(),
        ),
        Some(other) => Err(format!("unknown partition {other}")),
    }
}

/// The item ids a view is narrowed to, one per line.
///
/// A blank line and a line beginning with `#` are skipped, so the file can
/// say where its ids came from. That provenance is the point: a subset chosen
/// after the numbers are in is how a result gets talked into existence, and a
/// file that names its rule before it is applied can be checked.
fn read_item_ids(path: &str) -> Result<std::collections::BTreeSet<String>, String> {
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let ids: std::collections::BTreeSet<String> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_string)
        .collect();
    if ids.is_empty() {
        return Err(format!("{path} names no item ids"));
    }
    Ok(ids)
}

/// Resume an interrupted measurement without widening its allowed partitions.
fn narrow_eval_items(
    items: &mut Vec<&Item>,
    wanted: &std::collections::BTreeSet<String>,
) -> Result<(), String> {
    if wanted.is_empty() {
        return Err("the eval subset names no items".to_string());
    }
    for id in wanted {
        if !items.iter().any(|item| &item.id == id) {
            return Err(format!(
                "item {id} is outside the selected open partitions and family"
            ));
        }
    }
    items.retain(|item| wanted.contains(&item.id));
    Ok(())
}

/// The items a run asks: the wanted partitions, narrowed to one family when
/// `--family` names one.
///
/// A suite holds three families and a question set holds one question per
/// family, so a reword touches one family and leaves the others word for
/// word. Asking the untouched ones anyway costs door calls and then averages
/// the change it was measuring against items that could not have moved. A
/// family that the suite does not hold is an error rather than an empty run,
/// because a typo that scores nothing reads as a door that answered nothing.
fn items_of<'a>(
    suite: &'a Suite,
    wanted: &[Partition],
    family: Option<&str>,
) -> Result<Vec<&'a Item>, String> {
    if let Some(named) = family {
        let held = suite.families();
        if !held.iter().any(|family| family == named) {
            return Err(format!(
                "`{}` holds no {named} family; it holds {}",
                suite.name,
                held.join(", ")
            ));
        }
    }
    let mut items = Vec::new();
    for partition in wanted {
        items.extend(
            suite
                .partition(*partition)
                .map_err(|error| error.to_string())?,
        );
    }
    if let Some(named) = family {
        items.retain(|item: &&Item| item.family == named);
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
        let mut config = Config::new()
            .api_key("unused-by-a-local-door")
            .base_url(url.clone())
            .default_model(name.clone());
        // A local door under load can take longer than the client's ten
        // seconds, and a timeout is a harness failure: the item leaves the
        // record entirely, and nothing downstream can tell it was ever
        // asked. Two runs that lost different items are two measurements,
        // which is what `gym regress` refuses to compare. So the timeout is
        // a flag, and a busy machine buys a complete record with wall time.
        //
        // Measured on 2026-09-19: with nine agents sharing one device, a Lev
        // call that answers in five seconds queued for twenty-five, and the
        // default turned a 157-item run into an empty file with 157 timeouts.
        if let Some(seconds) = options.timeout {
            config = config.timeout(std::time::Duration::from_secs(seconds));
        }
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
    Door {
        client,
        facts: published,
    }
}

/// What `GET /v1/models` says, or what little is known without it.
async fn published_facts(client: &Client, unknown: Facts) -> Facts {
    let Ok(response) = client.models().list_raw(jev::ListOptions::default()).await else {
        return unknown;
    };
    let Ok(body) = serde_json::from_slice::<Value>(&response.bytes) else {
        return unknown;
    };
    let Some(model) = discovered_model(&body, client.default_model()) else {
        return unknown;
    };
    let text = |key: &str| {
        model
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let number = |key: &str| model.get(key).and_then(Value::as_u64);
    let estimator = text("estimator");
    Facts {
        identity: DoorIdentity::from_model_card(model, &unknown.name),
        estimator: if estimator.is_empty() {
            "unreported".to_string()
        } else {
            estimator
        },
        samples: number("samples"),
        seed_base: number("seed_base"),
        ..unknown
    }
}

/// Select the card for the requested model, including a published alias.
fn discovered_model<'a>(body: &'a Value, selected: &str) -> Option<&'a Value> {
    let cards = body.get("models")?.as_array()?;
    cards
        .iter()
        .find(|card| {
            card.get("id").and_then(Value::as_str) == Some(selected)
                || card.get("name").and_then(Value::as_str) == Some(selected)
                || card
                    .get("aliases")
                    .and_then(Value::as_array)
                    .is_some_and(|aliases| {
                        aliases.iter().any(|alias| alias.as_str() == Some(selected))
                    })
        })
        .or_else(|| (cards.len() == 1).then(|| &cards[0]))
}

/// How many options a Choice question serves.
fn option_count(question: &Value) -> usize {
    eval::options_of(question)
        .map(|options| options.len())
        .unwrap_or_default()
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
    let mut items = items_of(&suite, &wanted, options.family.as_deref())?;
    if let Some(path) = options.items.as_deref() {
        narrow_eval_items(&mut items, &read_item_ids(path)?)?;
    }
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
    if let Some(exposure) = &suite.exposure {
        let successor = exposure
            .successor
            .as_deref()
            .map(|successor| format!(" Confirm adapted doors on `{successor}` instead."))
            .unwrap_or_default();
        println!(
            "Exposure: {} of the {} partition's items were training data, through {}. The ledger \
             refuses that partition to any door that serves an adapter.{successor}\n",
            exposure.items, exposure.partition, exposure.through
        );
    }
    let evidence = suite.evidence_counts();
    if evidence.len() > 1 {
        let detail: Vec<String> = evidence
            .iter()
            .map(|(source, count)| format!("{count} {source}"))
            .collect();
        println!("Label evidence: {}.\n", detail.join(", "));
    }
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
            println!(
                "Recorded {} rows in `{}`.\n",
                pass.rows.len(),
                store.path().display()
            );
        }
        report_pass(&pass, items.len());
        report_scores(&pass.rows, &wanted);

        if options.fit {
            fit_and_report(
                &suite,
                &gate,
                &door.facts,
                &pass.rows,
                options.records.as_deref(),
            )?;
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
    if !facts.identity.artifact_signature.is_empty() {
        println!(
            "Checkpoint: `{}`. Execution: `{:?}`.",
            facts.identity.artifact_signature, facts.identity.execution
        );
    }
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
            facts
                .samples
                .map(|n| format!(", {n} samples"))
                .unwrap_or_default(),
            facts
                .seed_base
                .map(|n| format!(", seed block {n}"))
                .unwrap_or_default(),
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
    run.row(
        item,
        permutation,
        &Disposition::Refused(gym::row::RefusalCode::Busy),
        None,
    )
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
    // The answer is the option the row names, and a row that names none was
    // written before `selected` existed — its answer is the distribution's
    // own argmax, under the shared last-of-equal-leaders convention.
    let chosen = match row.selected.clone() {
        Some(option) => option,
        None => gym::calibrate::selected(&distribution).map(|(option, _)| option.to_string())?,
    };
    Some(Disposition::Answered {
        chosen,
        distribution,
    })
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
        if held
            .row(&planned(run, item, permutation.clone())?)
            .is_some()
        {
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
    row.check()
        .map_err(|error| format!("{}: {error}", row.item_id))?;
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
        let detail: Vec<String> = refusals
            .iter()
            .map(|(code, count)| format!("`{code}` x{count}"))
            .collect();
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
///
/// Split by partition, then by what kind of evidence the labels rest on. A
/// suite whose labels are half outcomes and half readings has one accuracy
/// number that means two things, and printing only that number is how the
/// ambiguity reaches every figure downstream.
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
    let sources = evidence_of(rows);
    if sources.len() > 1 {
        println!("| Label evidence | Accuracy | ECE | Brier | NLL | Confident errors | Items |");
        println!("| --- | --- | --- | --- | --- | --- | --- |");
        for source in &sources {
            let inside: Vec<Row> = rows
                .iter()
                .filter(|row| row.label_source.label() == source)
                .cloned()
                .collect();
            let metrics = gym::calibrate::score(&eval::observations(&inside));
            println!("{}", metrics_row(source, metrics));
        }
        println!(
            "\nAn `outcome` label is what happened next in the session the state came from. An \
             `author` label is a reading of the state. They are not the same evidence, so they \
             are not pooled.\n"
        );
    }
}

/// Every kind of label evidence the rows carry, in first-seen order.
fn evidence_of(rows: &[Row]) -> Vec<String> {
    let mut sources: Vec<String> = Vec::new();
    for row in rows {
        let source = row.label_source.label().to_string();
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    sources
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
        return Err(
            "nothing to judge a map on: the development partition produced no rows".to_string(),
        );
    }

    println!(
        "\n| Family | Fitted on | Raw ECE | Mapped ECE | Raw NLL | Mapped NLL | Raw Brier | \
         Mapped Brier | Verdict |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    let mut written: Vec<String> = Vec::new();
    for family in eval::families(rows) {
        let fit_on: Vec<Row> = calibration
            .iter()
            .filter(|row| row.family == family)
            .cloned()
            .collect();
        let score_on: Vec<Row> = held
            .iter()
            .filter(|row| row.family == family)
            .cloned()
            .collect();
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
    rows.iter()
        .filter(|row| row.split == partition.as_str())
        .cloned()
        .collect()
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

/// Rows read from a store, with the narrowing flags applied.
///
/// Every view a command prints goes through this, so a narrowed table always
/// carries the sentence that says what it left out.
struct View {
    path: String,
    rows: Vec<Row>,
    /// How many rows the store held before narrowing.
    held: usize,
    /// What the narrowing did, for the line under the heading.
    narrowed: Vec<String>,
}

impl View {
    /// Reads and narrows, keeping permuted trials or dropping them.
    ///
    /// A permuted trial measures order sensitivity; it is not a second
    /// reading of the item. A table that scores rows pools the two and counts
    /// the Choice items twice, so `keep_permuted` is false for those and true
    /// for the one view whose subject is the permutation itself.
    fn read(options: &Options, keep_permuted: bool) -> Result<Self, String> {
        let path = options
            .store
            .as_deref()
            .ok_or_else(|| "this view reads recorded rows; pass --store path".to_string())?;
        let rows = read_rows(path)?;
        if rows.is_empty() {
            return Err(format!("{path} holds no rows"));
        }
        let held = rows.len();
        let mut view = Self {
            path: path.to_string(),
            rows,
            held,
            narrowed: Vec::new(),
        };
        if !keep_permuted {
            view.rows.retain(|row| row.permutation.is_none());
            if view.rows.len() < held {
                view.narrowed.push(format!(
                    "{} permuted trial(s) left out",
                    held - view.rows.len()
                ));
            }
        }
        if let Some(wanted) = options.partition.as_deref() {
            let wanted = partitions(&Options {
                partition: Some(wanted.to_string()),
                ..Options::default()
            })?;
            let named: Vec<String> = wanted.iter().map(|p| p.as_str().to_string()).collect();
            view.rows
                .retain(|row| named.iter().any(|split| split == &row.split));
            view.narrowed
                .push(format!("the {} partition only", named.join(" and ")));
        }
        if let Some(listed) = options.items.as_deref() {
            let wanted = read_item_ids(listed)?;
            view.rows.retain(|row| wanted.contains(&row.item_id));
            view.narrowed
                .push(format!("{} item ids from `{listed}`", wanted.len()));
        }
        if view.rows.is_empty() {
            return Err(format!("{path} holds no rows once the view is narrowed"));
        }
        Ok(view)
    }

    /// The sides that appear, in the order they first do.
    fn sides(&self) -> Vec<Side> {
        let mut sides: Vec<Side> = Vec::new();
        for row in &self.rows {
            let side = Side::of(row);
            if !sides.contains(&side) {
                sides.push(side);
            }
        }
        sides
    }

    /// Says what the table is not.
    ///
    /// A narrowed view that reads like the whole store is the paste this
    /// crate exists to replace.
    fn report_narrowing(&self) {
        if !self.narrowed.is_empty() {
            println!(
                "This is a view over {} recorded rows, narrowed to {}.\n",
                self.held,
                self.narrowed.join(", ")
            );
        }
    }
}

/// The comparison, read back from the store rather than run again.
///
/// A live four-way table is a measurement nobody else can check: it exists
/// while the four doors are up and then only as a paste. The rows are the
/// evidence, and this is a view over them.
fn compare_command(options: &Options) -> Result<(), String> {
    let view = View::read(options, false)?;
    let path = view.path.clone();
    let rows = view.rows.clone();
    let sides = view.sides();

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
    view.report_narrowing();

    println!(
        "| Side | Identity | Accuracy | ECE | Brier | NLL | Confident errors | Scored | Refused \
         | Median latency |"
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
    let mut measured: BTreeMap<String, Metrics> = BTreeMap::new();
    let mut held: BTreeMap<String, Vec<Row>> = BTreeMap::new();
    for side in &sides {
        let inside: Vec<Row> = rows
            .iter()
            .filter(|row| &Side::of(row) == side)
            .cloned()
            .collect();
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
        let detail: Vec<String> = refusals
            .iter()
            .map(|(code, count)| format!("`{code}` x{count}"))
            .collect();
        println!("Refusals across every door: {}.\n", detail.join(", "));
    }

    let sources = evidence_of(&rows);
    if sources.len() > 1 {
        println!("## By what the labels rest on\n");
        println!("| Side | Evidence | Accuracy | ECE | Brier | NLL | Confident errors | Items |");
        println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
        for side in &sides {
            for source in &sources {
                let inside: Vec<Row> = held
                    .get(&side.label())
                    .map(Vec::as_slice)
                    .unwrap_or_default()
                    .iter()
                    .filter(|row| row.label_source.label() == source)
                    .cloned()
                    .collect();
                if inside.is_empty() {
                    continue;
                }
                let metrics = gym::calibrate::score(&eval::observations(&inside));
                println!("| {} {}", side.label(), metrics_row(source, metrics));
            }
        }
        println!(
            "\nAn `outcome` label is what happened next in the session the state came from. An \
             `author` label is a reading of the state. A side that is better on one and worse on \
             the other is telling you something the pooled row hides.\n"
        );
    }

    let labels: Vec<String> = sides.iter().map(Side::label).collect();
    let baseline = match &options.baseline {
        Some(named) => {
            let matches: Vec<_> = sides
                .iter()
                .filter(|side| &side.door == named || &side.label() == named)
                .collect();
            match matches.as_slice() {
                [side] => side.label(),
                [] => return Err(format!("no rows for the baseline {named}")),
                _ => {
                    return Err(format!(
                        "baseline {named} names multiple checkpoints or question sets; use the full comparison label"
                    ));
                }
            }
        }
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
        let Some(after) = measured.get(label) else {
            continue;
        };
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

/// The record a store of rows reads as, rendered for whoever holds it.
///
/// `eval` reports while the run is warm; this renders the same evidence
/// later, from the receipt-chained rows alone, so a caller who only ever
/// sees the store can read the measurement and walk the chain that carries
/// it. `--suite` is enrichment, not evidence: with the suite file the
/// record can name each family's label rule and the agreement ceiling its
/// labels rest on; without it the record says what the rows say and no
/// more.
fn report_command(options: &Options) -> Result<(), String> {
    let path = options
        .store
        .as_deref()
        .ok_or_else(|| "report reads recorded rows; pass --store path".to_string())?;
    let values = Store::at(path).rows().map_err(|error| error.to_string())?;
    let head = match verify_chain(&values) {
        ChainVerdict::Ok { head, .. } => head,
        ChainVerdict::Broken { detail, .. } => return Err(detail),
    };
    let rows: Vec<Row> = values
        .into_iter()
        .map(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
        .collect::<Result<_, _>>()?;
    if rows.is_empty() {
        return Err(format!("{path} holds no rows"));
    }

    // A suite file whose digest no row pins is a different suite, and
    // lending its provenance to these rows would mislabel the record.
    let provenance = match options.suite.as_deref() {
        Some(file) => {
            let suite = Suite::load_file(file).map_err(|error| error.to_string())?;
            if !rows.iter().any(|row| row.suite_digest == suite.digest) {
                return Err(format!(
                    "{file} digests to {}, which no row in this store names",
                    suite.digest
                ));
            }
            let text = std::fs::read_to_string(file).map_err(|error| format!("{file}: {error}"))?;
            let value: Value =
                serde_json::from_str(&text).map_err(|error| format!("{file}: {error}"))?;
            Some((suite, value.get("provenance").cloned()))
        }
        None => None,
    };

    // The declared selection is the other half of completeness: which items
    // the run was meant to ask, and of which doors. `--partition`,
    // `--family`, and `--items` carry the same narrowing `eval` accepts;
    // `--expect` names a door the run was meant to ask even if it left no
    // rows. Without a suite the selection cannot be reconstructed and the
    // record is unverifiable as a completed evaluation — it still renders,
    // because a partial record is evidence, but it says so first.
    let declared = match provenance.as_ref() {
        Some((suite, _)) => {
            let subset = options.items.as_deref().map(read_item_ids).transpose()?;
            let wanted = declared_partitions(options)?;
            let mut doors = options.expect.clone();
            for door in doors_of(&rows) {
                if !doors.contains(&door) {
                    doors.push(door);
                }
            }
            Some(Expected::of(
                suite,
                &wanted,
                options.family.as_deref(),
                subset.as_ref(),
                doors,
            )?)
        }
        None => None,
    };

    let mut record = format!("# `{}` — the measured record\n\n", rows[0].suite);
    record.push_str(&format!(
        "Store `{path}` holds {held} rows; the receipt chain verifies{head}.\n",
        held = rows.len(),
        head = match &head {
            Some(head) => format!(" to head `{head}`"),
            None => String::new(),
        },
    ));
    let first = rows.iter().map(|row| &row.recorded_at).min().unwrap();
    let last = rows.iter().map(|row| &row.recorded_at).max().unwrap();
    record.push_str(&format!("Recorded {first} through {last}.\n\n"));
    if declared.is_none() {
        record.push_str(
            "**Coverage is not declared** — no `--suite` was passed, so what this run was \
             meant to ask is unknown and this record is **unverifiable as a completed \
             evaluation**. Pass `--suite` with `--partition`, `--family`, `--items`, and \
             `--expect` as the run used them.\n\n",
        );
    }
    record.push_str(
        "A row the harness never wrote is not here: items lost to timeouts or dead \
         doors leave no row, and where the selection is declared they are named as \
         missing — never scored as wrong answers. A refused item counts against the \
         door that refused it.\n\n",
    );

    for (suite, digest) in suite_groups(&rows) {
        let inside: Vec<Row> = rows
            .iter()
            .filter(|row| row.suite == suite && row.suite_digest == digest)
            .cloned()
            .collect();
        // The declaration applies to the suite it was declared against. Rows
        // pinning a different digest — another run folded into the store —
        // report their own coverage state rather than borrow this one's.
        let expected = declared.as_ref().filter(|_| {
            provenance
                .as_ref()
                .is_some_and(|(suite, _)| suite.digest == digest)
        });
        record.push_str(&suite_section(
            &suite,
            &digest,
            &inside,
            provenance.as_ref(),
            expected,
        ));
    }

    record.push_str(
        "## Checking this record\n\nEvery row pins the suite digest, the question-set digest, \
         and the gate digest, and carries a receipt over its contents chained to the row \
         before it. Every store-reading command walks that chain and refuses a broken one; \
         `gym verify --store` walks it without rendering the tables. The suite digests \
         itself on load, and the named gate lives in `crates/gym/gates/`.\n\n\
         A verified chain proves these rows were not edited or resequenced inside this \
         file; it does not prove the file is whole or that this is the only store. \
         Completeness comes from the declared selection above, and permanence comes \
         from the commitment written beside this record — check a later copy of the \
         store against it with `gym verify --store … --commitment …`.\n",
    );

    // The commitment is what a caller holds when the store is out of reach:
    // head, row count, declared selection, and the identities and digests
    // the report claimed. It is written only when the selection is
    // declared — a commitment over an unknown selection anchors nothing.
    if let Some(path) = options.commitment.as_deref() {
        let (suite, prov) = provenance.as_ref().ok_or_else(|| {
            "--commitment needs --suite so the commitment can bind the declared selection"
                .to_string()
        })?;
        let expected = declared.as_ref().expect("a suite gives a selection");
        let selection = gym::commitment::Selection {
            partitions: declared_partitions(options)?
                .iter()
                .map(|partition| partition.as_str().to_string())
                .collect(),
            family: options.family.clone(),
            items: options
                .items
                .as_deref()
                .map(read_item_ids)
                .transpose()?
                .map(|ids| ids.into_iter().collect()),
            doors: expected.doors.clone(),
        };
        let commitment = gym::commitment::Commitment::of(
            suite,
            expected,
            selection,
            &rows,
            head.clone(),
            prov.as_ref(),
        );
        let text = serde_json::to_string_pretty(&commitment).map_err(|error| error.to_string())?;
        std::fs::write(path, text + "\n").map_err(|error| format!("{path}: {error}"))?;
        println!(
            "Wrote commitment `{path}` — digest `{}`.",
            commitment.digest
        );
    }

    match options.out.as_deref() {
        Some(file) => {
            std::fs::write(file, &record).map_err(|error| format!("{file}: {error}"))?;
            println!("Wrote `{file}`.");
        }
        None => println!("{record}"),
    }
    Ok(())
}

/// The partitions a report's declared selection covers: `--partition` as a
/// comma list, defaulting to the open partitions `eval` asks. The locked
/// partition is declarable here — a run that spent it through the ledger
/// wrote rows, and coverage over them is the point of the check.
fn declared_partitions(options: &Options) -> Result<Vec<Partition>, String> {
    match options.partition.as_deref() {
        None => Ok(vec![Partition::Calibration, Partition::Development]),
        Some(list) => list
            .split(',')
            .map(|name| match name.trim() {
                "calibration" => Ok(Partition::Calibration),
                "development" => Ok(Partition::Development),
                "locked" => Ok(Partition::Locked),
                "" => Err("empty partition name".to_string()),
                other => Err(format!("unknown partition {other}")),
            })
            .collect(),
    }
}

/// The suites a store's rows name, in first-seen order: a store can hold
/// rows from more than one run.
fn suite_groups(rows: &[Row]) -> Vec<(String, String)> {
    let mut groups: Vec<(String, String)> = Vec::new();
    for row in rows {
        let group = (row.suite.clone(), row.suite_digest.clone());
        if !groups.contains(&group) {
            groups.push(group);
        }
    }
    groups
}

/// One suite's section of the record: the digests it pins, the declared
/// selection and the coverage verdict over it, then a table per door it
/// was asked of.
fn suite_section(
    suite: &str,
    digest: &str,
    rows: &[Row],
    provenance: Option<&(Suite, Option<Value>)>,
    expected: Option<&Expected>,
) -> String {
    let mut section = String::new();
    let pinned = rows.iter().find_map(|row| {
        row.question_set.as_ref().map(|id| {
            format!(
                "`{id}` — digest `{}`",
                row.question_digest.as_deref().unwrap_or("?")
            )
        })
    });
    section.push_str(&format!("Suite `{suite}` — digest `{digest}`.\n"));
    match pinned {
        Some(pinned) => section.push_str(&format!("Question set {pinned}.\n")),
        None => {
            section.push_str("Question text is inline in the items; the suite digest covers it.\n")
        }
    }
    if let Some(id) = rows.iter().find_map(|row| row.gate_id.clone()) {
        let gate_digest = rows
            .iter()
            .find_map(|row| row.gate_digest.clone())
            .unwrap_or_else(|| "?".to_string());
        section.push_str(&format!("Gate `{id}` — digest `{gate_digest}`.\n"));
    }
    section.push('\n');

    // The verdict is computed once and quoted beside the declaration, so a
    // reader learns whether this is a completed evaluation before the tables
    // that would otherwise let them assume it.
    match expected {
        Some(expected) => {
            let mut faults: Vec<String> = Vec::new();
            for door in &expected.doors {
                let asked: Vec<Row> = rows
                    .iter()
                    .filter(|row| row.door == *door && row.permutation.is_none())
                    .cloned()
                    .collect();
                if asked.is_empty() {
                    faults.push(format!("`{door}` left no rows"));
                    continue;
                }
                let groups = run_groups(&asked);
                if groups.len() > 1 {
                    faults.push(format!(
                        "`{door}` recorded under {} run identities, rendered apart below",
                        groups.len()
                    ));
                }
                for (_, group) in &groups {
                    let coverage = Coverage::of(group, expected.items());
                    if !coverage.missing.is_empty() {
                        faults.push(format!(
                            "`{door}` is missing {} of the expected items",
                            coverage.missing.len()
                        ));
                    }
                    if !coverage.duplicates.is_empty() {
                        faults.push(format!(
                            "`{door}` recorded {} of the expected items twice",
                            coverage.duplicates.len()
                        ));
                    }
                    if !coverage.unexpected.is_empty() {
                        faults.push(format!(
                            "`{door}` holds {} rows outside the declared selection",
                            coverage.unexpected.len()
                        ));
                    }
                }
            }
            if faults.is_empty() {
                section.push_str(
                    "**Coverage: complete** — every expected item of every expected door \
                     recorded exactly once.\n\n",
                );
            } else {
                section.push_str(&format!(
                    "**Coverage: incomplete** — {}. This is a partial record, not a \
                     completed evaluation.\n\n",
                    faults.join("; ")
                ));
            }
            let doors: Vec<String> = expected
                .doors
                .iter()
                .map(|door| format!("`{door}`"))
                .collect();
            section.push_str(&format!(
                "Declared selection: {} items, doors {}.\n\n",
                expected.items().len(),
                doors.join(", ")
            ));
        }
        None => section.push_str(
            "**Coverage: not declared** — the expected selection is unknown for these \
             rows, so this section cannot claim a completed evaluation.\n\n",
        ),
    }

    let doors = match expected {
        Some(expected) => expected.doors.clone(),
        None => doors_of(rows),
    };
    for door in doors {
        let asked: Vec<Row> = rows
            .iter()
            .filter(|row| row.door == door)
            .cloned()
            .collect();
        section.push_str(&format!("## `{door}`\n\n"));
        if asked.is_empty() {
            let count = expected.map(|e| e.items().len()).unwrap_or(0);
            section.push_str(&format!(
                "No rows — the door was expected to answer {count} items and the store \
                 holds none of them.\n\n"
            ));
            continue;
        }
        // Permutation probes are a second asking of the same item: evidence
        // about option order, not a second trial of the pass. They are
        // counted apart from the pass they accompany.
        let pass: Vec<Row> = asked
            .iter()
            .filter(|row| row.permutation.is_none())
            .cloned()
            .collect();
        let probes = asked.len() - pass.len();
        let groups = run_groups(&pass);
        if groups.len() > 1 {
            section.push_str(&format!(
                "These rows were recorded under {} distinct run identities — different \
                 artifacts, digests, or trial configurations. Each renders apart and \
                 they are never pooled.\n\n",
                groups.len()
            ));
        }
        for (index, (key, group)) in groups.iter().enumerate() {
            if groups.len() > 1 {
                section.push_str(&format!(
                    "### Run identity {} of {}\n\n",
                    index + 1,
                    groups.len()
                ));
            }
            section.push_str(&format!("Identity: {}.\n", identity_of(group)));
            section.push_str(&format!("Trials: {}.\n", trials_of(key)));
            if let Some(expected) = expected {
                let coverage = Coverage::of(group, expected.items());
                section.push_str(&coverage_line(&coverage));
                if !coverage.missing.is_empty() {
                    section.push_str(&format!("Missing: {}.\n", name_items(&coverage.missing)));
                }
                if !coverage.duplicates.is_empty() {
                    section.push_str(&format!(
                        "Recorded twice: {}.\n",
                        name_items(&coverage.duplicates)
                    ));
                }
                if !coverage.unexpected.is_empty() {
                    section.push_str(&format!(
                        "Outside the declared selection: {}.\n",
                        name_items(&coverage.unexpected)
                    ));
                }
                section.push('\n');
            }
            section.push_str(&door_tables(group, expected, provenance));
        }
        if probes > 0 {
            section.push_str(&format!(
                "{probes} option-order probe rows sit beside this pass, counted apart from it.\n\n"
            ));
        }
    }
    section
}

/// One run group's tables: the per-split metrics, then the per-family
/// metrics with their label ceilings, and coverage per family when the
/// selection was declared.
fn door_tables(
    asked: &[Row],
    expected: Option<&Expected>,
    provenance: Option<&(Suite, Option<Value>)>,
) -> String {
    let mut section = String::new();
    let scored = asked.iter().filter(|row| row.is_scored()).count();
    let refused = asked.iter().filter(|row| row.is_refused()).count();
    section.push_str(&format!(
        "{} items recorded: {scored} scored, {refused} refused by the door.\n",
        asked.len(),
    ));
    let refusals = eval::refusals(asked);
    if !refusals.is_empty() {
        let detail: Vec<String> = refusals
            .iter()
            .map(|(code, count)| format!("`{code}` x{count}"))
            .collect();
        section.push_str(&format!("Door refusals: {}.\n", detail.join(", ")));
    }
    section.push_str(&format!("Median latency {}.\n\n", median_latency(asked)));

    section.push_str("| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |\n");
    section.push_str("| --- | --- | --- | --- | --- | --- | --- |\n");
    for split in splits_of(asked) {
        let inside: Vec<Row> = asked
            .iter()
            .filter(|row| row.split == split)
            .cloned()
            .collect();
        let metrics = score(&eval::observations(&inside));
        section.push_str(&metrics_row(&split, metrics));
        section.push('\n');
    }
    section.push('\n');

    let ceilings = ceilings_of(asked, provenance);
    let ruled = ruled_families(asked, provenance);
    section.push_str(
        "| Family | Accuracy | ECE | Brier | NLL | Confident errors | Items | Ceiling |\n",
    );
    section.push_str("| --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for family in eval::families(asked) {
        let inside: Vec<Row> = asked
            .iter()
            .filter(|row| row.family == family)
            .cloned()
            .collect();
        let metrics = score(&eval::observations(&inside));
        let ceiling = ceilings
            .get(&family)
            .map(String::as_str)
            .unwrap_or("unstated");
        section.push_str(&format!(
            "| `{family}` | {:.2} | {:.3} | {:.3} | {:.3} | {} | {} | {ceiling} |\n",
            metrics.accuracy,
            metrics.ece,
            metrics.brier,
            metrics.nll,
            metrics.confident_errors,
            metrics.items,
        ));
    }
    section.push('\n');
    if let Some(expected) = expected {
        section.push_str("| Family | Expected | Recorded | Answered | Refused | Missing |\n");
        section.push_str("| --- | --- | --- | --- | --- | --- |\n");
        for family in expected.families() {
            let inside: Vec<Row> = asked
                .iter()
                .filter(|row| row.family == *family)
                .cloned()
                .collect();
            let coverage = Coverage::of(&inside, &expected.for_family(family));
            section.push_str(&format!(
                "| `{family}` | {} | {} | {} | {} | {} |\n",
                coverage.expected,
                coverage.recorded(),
                coverage.answered,
                coverage.refused,
                coverage.missing.len(),
            ));
        }
        section.push('\n');
    }
    if !ruled.is_empty() {
        section.push_str("Label evidence:\n\n");
        for (family, source, rule) in &ruled {
            let rule = match rule {
                Some(rule) => format!(" — rule \"{rule}\""),
                None => String::new(),
            };
            section.push_str(&format!("- `{family}` — {source}{rule}\n"));
        }
        section.push('\n');
    }
    section
}

/// The trial configuration a run group shares, stated plainly.
fn trials_of(key: &RunKey) -> String {
    let mut parts = vec![format!("estimator `{}`", key.estimator)];
    if let Some(samples) = key.samples {
        parts.push(format!("{samples} draws each"));
    }
    if let Some(seed) = key.seed_base {
        parts.push(format!("seed base `{seed}`"));
    }
    parts.join(", ")
}

/// The coverage sentence a run group earns against the selection.
fn coverage_line(coverage: &Coverage) -> String {
    format!(
        "Coverage: {} of {} expected items recorded — {} answered, {} refused; \
         {} missing or unattempted.\n",
        coverage.recorded(),
        coverage.expected,
        coverage.answered,
        coverage.refused,
        coverage.missing.len(),
    )
}

/// Up to eight `(partition, item)` names, then a count of the rest.
fn name_items(items: &[(String, String)]) -> String {
    let named: Vec<String> = items
        .iter()
        .take(8)
        .map(|(split, id)| format!("`{split}/{id}`"))
        .collect();
    match items.len() - named.len() {
        0 => named.join(", "),
        rest => format!("{}, and {rest} more", named.join(", ")),
    }
}

/// The doors a suite's rows name, in first-seen order.
fn doors_of(rows: &[Row]) -> Vec<String> {
    let mut doors: Vec<String> = Vec::new();
    for row in rows {
        if !doors.contains(&row.door) {
            doors.push(row.door.clone());
        }
    }
    doors
}

/// The splits a door's rows name, partitions in suite order first and any
/// other split names after.
fn splits_of(rows: &[Row]) -> Vec<String> {
    let mut splits: Vec<String> = Vec::new();
    for partition in Partition::ALL {
        let name = partition.as_str().to_string();
        if rows.iter().any(|row| row.split == name) && !splits.contains(&name) {
            splits.push(name);
        }
    }
    for row in rows {
        if !splits.contains(&row.split) {
            splits.push(row.split.clone());
        }
    }
    splits
}

/// The agreement ceiling each family's labels rest on, from the suite's
/// provenance: `provenance.agreement.<family>`, which the caller-suite
/// builder writes when a caller states one. A suite that publishes no
/// ceiling reports "unstated" rather than a borrowed number.
fn ceilings_of(
    rows: &[Row],
    provenance: Option<&(Suite, Option<Value>)>,
) -> BTreeMap<String, String> {
    let mut ceilings = BTreeMap::new();
    let Some((suite, Some(provenance))) = provenance else {
        return ceilings;
    };
    if suite.digest
        != rows
            .first()
            .map(|row| row.suite_digest.as_str())
            .unwrap_or("")
    {
        return ceilings;
    }
    if let Some(agreement) = provenance.get("agreement").and_then(Value::as_object) {
        for (family, ceiling) in agreement {
            let text = ceiling
                .as_str()
                .map(str::to_string)
                .or_else(|| ceiling.as_f64().map(|n| format!("{n:.3}")));
            if let Some(text) = text {
                ceilings.insert(family.clone(), text);
            }
        }
    }
    ceilings
}

/// Per family, the label evidence the rows carry and the rule the suite
/// states it by: `(<family>, <sources>, <rule>)`.
fn ruled_families(
    rows: &[Row],
    provenance: Option<&(Suite, Option<Value>)>,
) -> Vec<(String, String, Option<String>)> {
    let mut ruled = Vec::new();
    for family in eval::families(rows) {
        let sources = evidence_of(
            &rows
                .iter()
                .filter(|row| row.family == family)
                .cloned()
                .collect::<Vec<_>>(),
        );
        let rule = provenance.and_then(|(suite, _)| {
            if suite.digest != rows.first()?.suite_digest.as_str() {
                return None;
            }
            suite
                .items
                .iter()
                .filter(|item| item.family == family)
                .filter_map(|item| item.label_rule.clone())
                .next()
        });
        ruled.push((family, sources.join(", "), rule));
    }
    ruled
}

/// Whether a store's receipt chain holds, said plainly.
///
/// Every command that reads a store verifies the chain before it reads;
/// `verify` exists so the check is the point, not a side effect of a
/// table.
fn verify_command(options: &Options) -> Result<(), String> {
    let path = options
        .store
        .as_deref()
        .ok_or_else(|| "verify reads recorded rows; pass --store path".to_string())?;
    let rows = Store::at(path).rows().map_err(|error| error.to_string())?;
    match verify_chain(&rows) {
        ChainVerdict::Ok { rows, head } => {
            let head = head.as_deref().unwrap_or("none");
            println!("`{path}`: {rows} rows, chain intact, head `{head}`.");
        }
        ChainVerdict::Broken {
            index,
            fault,
            detail,
        } => {
            eprintln!(
                "`{path}`: chain broken at row {index} ({}).",
                fault.as_str()
            );
            return Err(detail);
        }
    }
    if let Some(file) = options.commitment.as_deref() {
        let commitment = gym::commitment::Commitment::load(file)?;
        let typed: Vec<Row> = rows
            .iter()
            .cloned()
            .map(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
            .collect::<Result<_, _>>()?;
        let faults = gym::commitment::check(&commitment, &typed);
        if !faults.is_empty() {
            for fault in &faults {
                eprintln!("against `{file}`: {fault}");
            }
            return Err(format!("the store does not match the commitment in {file}"));
        }
        let grown = gym::commitment::growth(&commitment, &typed);
        let note = match grown {
            0 => String::new(),
            grown => format!("; {grown} rows appended since the commitment"),
        };
        println!(
            "`{path}` matches commitment `{file}` (digest `{}`){note}.",
            commitment.digest
        );
    }
    Ok(())
}

/// A caller's labelled file becomes a pinned suite and its question set.
///
/// The validation and partitioning live in [`gym::build`]; this is the
/// file handling around it. The default output paths are the directories
/// the committed suites and question sets live in, so a suite built from a
/// checkout lands where `gym eval` looks for it.
fn build_command(options: &Options) -> Result<(), String> {
    fn needed(flag: &str, value: &Option<String>) -> Result<String, String> {
        value.clone().ok_or_else(|| format!("build needs --{flag}"))
    }
    let mut spec = gym::build::Spec::new(
        needed("input", &options.input)?,
        needed("name", &options.name)?,
        needed("label-source", &options.label_source)?,
        needed("label-rule", &options.label_rule)?,
        needed("source", &options.source)?,
        needed("licence", &options.licence)?,
    );
    if let Some(created) = &options.created {
        spec.created = created.clone();
    }
    spec.description = options.description.clone();
    if let Some(gate) = &options.gate {
        spec.gate = gate.clone();
    }
    spec.agreement = options.agreement.clone();

    let built = gym::build::build(&spec)?;
    let suite_out = options
        .suite_out
        .clone()
        .unwrap_or_else(|| format!("crates/gym/suites/{}.json", spec.name));
    let questions_out = options
        .questions_out
        .clone()
        .unwrap_or_else(|| format!("crates/gym/questions/{}.json", spec.name));
    for (path, document) in [
        (&suite_out, &built.suite),
        (&questions_out, &built.questions),
    ] {
        if let Some(parent) = std::path::Path::new(path).parent() {
            std::fs::create_dir_all(parent).map_err(|error| format!("{path}: {error}"))?;
        }
        let text = serde_json::to_string_pretty(document).map_err(|error| error.to_string())?;
        std::fs::write(path, text + "\n").map_err(|error| format!("{path}: {error}"))?;
        println!("wrote {path}");
    }
    println!("digest {}", built.digest);
    println!(
        "{} items, {} shared and {} per-item question families",
        built.items, built.family_keyed, built.item_keyed
    );
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
                // Strings, maps of strings, and a boolean serialize without a failure.
                let encoded = serde_json::to_vec(identity).expect("a model identity serializes");
                format!("{}@{:x}", self.door, sha2::Sha256::digest(encoded))
            },
        );
        match &self.questions {
            Some(set) => format!("{door} asked as {set}"),
            None => door,
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
        let mut identity = if row.door_identity.calibration_identity_complete() {
            let base: String = row
                .door_identity
                .base_model_signature
                .chars()
                .take(8)
                .collect();
            match row.door_identity.adapter.as_str() {
                "" => format!("base `{base}`"),
                adapter => format!("base `{base}`, adapter `{adapter}`"),
            }
        } else {
            "incomplete identity".to_string()
        };
        if !row.door_identity.artifact_signature.is_empty() {
            identity.push_str(&format!(
                ", checkpoint `{}`, execution `{:?}`",
                row.door_identity.artifact_signature, row.door_identity.execution
            ));
        }
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

/// The flip rate, read back from the store rather than run again.
///
/// `permute` writes both passes as rows, so which answers moved under
/// reversal is a query over the record. Reading it back is how a flip rate
/// can be narrowed the way an accuracy can — to one partition, or to the
/// items a door was not trained on — instead of being a single number that
/// arrived with the run that produced it.
fn flips_command(options: &Options) -> Result<(), String> {
    let view = View::read(options, true)?;
    println!("# Option order, from the record\n");
    println!(
        "Read from `{}`: {} rows. The chain verified, and no door was asked.\n",
        view.path,
        view.rows.len()
    );
    view.report_narrowing();
    println!("| Side | Items | Flips | Flip rate | Accuracy forward | Accuracy reversed |");
    println!("| --- | --- | --- | --- | --- | --- |");
    for side in view.sides() {
        let mine: Vec<&Row> = view
            .rows
            .iter()
            .filter(|row| Side::of(row) == side)
            .collect();
        // An item is a trial only when both passes are in the record and
        // both were answered. A pass that is missing is not a flip and not
        // an absence of one.
        let forward: BTreeMap<&str, &Row> = mine
            .iter()
            .filter(|row| row.permutation.is_none())
            .map(|row| (row.item_id.as_str(), *row))
            .collect();
        let reversed: BTreeMap<&str, &Row> = mine
            .iter()
            .filter(|row| row.permutation.is_some())
            .map(|row| (row.item_id.as_str(), *row))
            .collect();
        let mut trials = 0_usize;
        let mut flips = 0_usize;
        let mut forward_rows: Vec<Row> = Vec::new();
        let mut reversed_rows: Vec<Row> = Vec::new();
        for (item, back) in &reversed {
            let Some(front) = forward.get(item) else {
                continue;
            };
            let (Some(chosen), Some(other)) = (chosen_of(front), chosen_of(back)) else {
                continue;
            };
            trials += 1;
            if chosen != other {
                flips += 1;
            }
            forward_rows.push((*front).clone());
            reversed_rows.push((*back).clone());
        }
        if trials == 0 {
            continue;
        }
        println!(
            "| `{}` | {trials} | {flips} | {:.3} | {:.2} | {:.2} |",
            side.label(),
            flips as f64 / trials as f64,
            gym::calibrate::score(&eval::observations(&forward_rows)).accuracy,
            gym::calibrate::score(&eval::observations(&reversed_rows)).accuracy,
        );
    }
    println!(
        "\nBoth accuracy columns are over the items that carry both passes, so the two are \
         measured on the same items and the flip rate is their disagreement.\n"
    );
    Ok(())
}

/// The option a recorded row says the door answered — its `selected` when it
/// names one, else the distribution's argmax.
fn chosen_of(row: &Row) -> Option<String> {
    match recorded_answer(row) {
        Some(Disposition::Answered { chosen, .. }) => Some(chosen),
        _ => None,
    }
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
    let items = items_of(&suite, &wanted, options.family.as_deref())?;
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
            let Some(backward) = eval::permuted(question, &order) else {
                continue;
            };

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
                Disposition::Answered {
                    chosen: forward, ..
                },
                Disposition::Answered {
                    chosen: reversed, ..
                },
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
        let mine: Vec<Row> = scored
            .iter()
            .filter(|row| row.door == name)
            .cloned()
            .collect();
        // Only the suite's own option order is fitted on. A permuted trial is
        // a measurement of order sensitivity, not a second reading of the
        // item, and pooling the two doubles an item's weight in the table.
        let mine: Vec<Row> = mine
            .into_iter()
            .filter(|row| row.permutation.is_none())
            .collect();
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

/// Folds one store's rows into another, re-sealing them onto its chain.
///
/// Two runs cannot append to one chain at the same time, and two branches
/// cannot merge one as text: a receipt names the row before it, so rows that
/// were written in parallel are two chains from a shared root and neither
/// verifies after a concatenation. Recording each run to its own store and
/// folding them afterwards is the way to hold every door's rows in one chain,
/// and this is that fold. The rows are unchanged; only their place in the
/// file, and therefore their receipts, is new.
///
/// A row whose perturbation the destination already holds is left where it
/// is and counted, because the destination already records that trial and a
/// second copy of it is not a second trial.
fn merge_command(options: &Options) -> Result<(), String> {
    let destination = options
        .store
        .as_deref()
        .ok_or_else(|| "merge appends into a store; pass --store path".to_string())?;
    let sources = if options.from.is_empty() {
        return Err("merge reads stores; pass --from path, repeatable".to_string());
    } else {
        options.from.clone()
    };
    let store = Store::at(destination);
    // Verify the destination before writing to it, so a merge into a broken
    // chain fails before it lengthens one.
    let held = store.verified_rows().map_err(|error| error.to_string())?;
    let mut keys: std::collections::BTreeSet<String> =
        held.iter().map(gym::store::perturbation_key).collect();

    println!("# Merged rows\n");
    println!(
        "`{destination}` held {} verified row(s) before this merge.\n",
        held.len()
    );
    println!("| From | Rows | Appended | Already held |");
    println!("| --- | --- | --- | --- |");
    for source in &sources {
        let rows = read_rows(source)?;
        let mut appended = 0_usize;
        let mut duplicate = 0_usize;
        for row in &rows {
            let value = serde_json::to_value(row).map_err(|error| error.to_string())?;
            if !keys.insert(gym::store::perturbation_key(&value)) {
                duplicate += 1;
                continue;
            }
            append(&store, row)?;
            appended += 1;
        }
        println!("| `{source}` | {} | {appended} | {duplicate} |", rows.len());
    }
    let after = store.verified_rows().map_err(|error| error.to_string())?;
    println!(
        "\n`{destination}` now holds {} rows, and the chain verifies.\n",
        after.len()
    );
    Ok(())
}

/// Every row in a store, verified and typed.
fn read_rows(path: &str) -> Result<Vec<Row>, String> {
    let values = Store::at(path)
        .verified_rows()
        .map_err(|error| error.to_string())?;
    values
        .into_iter()
        .map(|value| serde_json::from_value(value).map_err(|error| error.to_string()))
        .collect()
}

/// What one door did over one block of the same workload.
struct Block {
    profile: Profile,
    mean_ms: f64,
    lost: usize,
}

/// Runs the same workload against one door several times and reports how
/// much the answer moves when nothing but the clock changed.
///
/// A *block* here is one pass over the items, which is the latency analogue
/// of a seed block in `lev-seed-sweep`: the door, the items, and the machine
/// are held fixed, so whatever spread comes out is the floor under any
/// latency comparison. Wall clock on a shared machine moves with whatever
/// else is running, and a gate that does not know that spread refuses doors
/// for noise.
#[tokio::main(flavor = "current_thread")]
async fn latency_command(options: Options) -> Result<(), String> {
    let suite = load_suite(&options)?;
    let questions = load_questions(&options, &suite)?;
    let wanted = partitions(&options)?;
    let items = items_of(&suite, &wanted, options.family.as_deref())?;
    let doors = open_doors(&options)?;
    let blocks = options.blocks.unwrap_or(DEFAULT_BLOCKS);
    if blocks < 2 {
        return Err("a spread needs at least two blocks; pass --blocks 2 or more".to_string());
    }

    println!("# Latency across blocks: `{}`\n", suite.name);
    println!(
        "{} items from {}, {blocks} blocks, digest `{}`.\n",
        items.len(),
        match options.partition.as_deref() {
            None => "the calibration and development partitions".to_string(),
            Some(name) => format!("the {name} partition"),
        },
        &suite.digest[..16]
    );
    println!(
        "Asked as `{}`, digest `{}`.\n",
        questions.id,
        &questions.digest()[..16]
    );

    let mut measured: BTreeMap<String, Vec<Block>> = BTreeMap::new();
    for block in 0..blocks {
        // Alternate which door goes first, so a machine that drifts over the
        // sweep does not hand one door the quiet half of it.
        let order: Vec<&(String, Client)> = if block % 2 == 0 {
            doors.iter().collect()
        } else {
            doors.iter().rev().collect()
        };
        for (name, client) in order {
            let mut latencies = Vec::with_capacity(items.len());
            let mut refusals = 0_usize;
            let mut lost = 0_usize;
            for item in &items {
                let question = questions.ask(item).map_err(|error| error.to_string())?;
                match ask(client, &item.state, question).await {
                    (Disposition::Harness(_), _) | (_, None) => lost += 1,
                    (disposition, Some(elapsed)) => {
                        if matches!(disposition, Disposition::Refused(_)) {
                            refusals += 1;
                        }
                        latencies.push(elapsed);
                    }
                }
            }
            let mean_ms = if latencies.is_empty() {
                f64::NAN
            } else {
                latencies.iter().sum::<f64>() / latencies.len() as f64
            };
            let profile = Profile::timed(&latencies).refusing(refusals);
            measured.entry(name.clone()).or_default().push(Block {
                profile,
                mean_ms,
                lost,
            });
        }
    }

    for (name, blocks) in &measured {
        println!("## `{name}`\n");
        println!("| Block | Calls | Refused | Lost | p50 | p95 | Mean |");
        println!("| --- | --- | --- | --- | --- | --- | --- |");
        for (index, block) in blocks.iter().enumerate() {
            println!(
                "| {index} | {} | {} | {} | {} | {} | {} |",
                block.profile.calls,
                block.profile.refusals.unwrap_or_default(),
                block.lost,
                milliseconds(block.profile.latency_p50_ms),
                milliseconds(block.profile.latency_p95_ms),
                milliseconds(Some(block.mean_ms)),
            );
        }
        println!();
        let read = |pick: fn(&Profile) -> Option<f64>| -> Vec<f64> {
            blocks
                .iter()
                .filter_map(|block| pick(&block.profile))
                .collect()
        };
        let p50 = read(|profile| profile.latency_p50_ms);
        let p95 = read(|profile| profile.latency_p95_ms);
        println!("| Statistic | Mean over blocks | Standard deviation | Relative | Range |");
        println!("| --- | --- | --- | --- | --- |");
        for (label, values) in [("p50", &p50), ("p95", &p95)] {
            let Some(line) = spread_line(label, values) else {
                continue;
            };
            println!("{line}");
        }
        println!();
    }
    Ok(())
}

/// One row of the spread table, or nothing when a door produced no blocks.
fn spread_line(label: &str, values: &[f64]) -> Option<String> {
    if values.len() < 2 {
        return None;
    }
    let count = values.len() as f64;
    let mean = values.iter().sum::<f64>() / count;
    // The sample standard deviation, with the Bessel correction, because
    // eight blocks are a sample of the machine's moods rather than all of
    // them.
    let variance = values
        .iter()
        .map(|value| (value - mean).powi(2))
        .sum::<f64>()
        / (count - 1.0);
    let sigma = variance.sqrt();
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let relative = if mean > 0.0 {
        sigma / mean * 100.0
    } else {
        f64::NAN
    };
    Some(format!(
        "| {label} | {mean:.1} ms | {sigma:.1} ms | {relative:.1}% | {low:.1} to {high:.1} ms |"
    ))
}

/// A latency, or a dash where there is none.
fn milliseconds(value: Option<f64>) -> String {
    match value {
        Some(ms) if ms.is_finite() => format!("{ms:.1} ms"),
        _ => "—".to_string(),
    }
}
/// Did this commit move the numbers?
///
/// The one command to run before pushing, because this repository has no CI
/// to run it for anybody. It asks no door: the rows a run already wrote hold
/// the whole trace of every call, so comparing this week's run with last
/// week's is a query over the record.
///
/// It exits 1 on a measured regression, 2 when the comparison was refused,
/// and 0 otherwise. `unverifiable` exits 0 and is not a pass: it means
/// nobody could tell, and the report says which criterion could not.
fn regress_command(options: &Options) -> Result<(), String> {
    let path = options
        .store
        .as_deref()
        .ok_or_else(|| "regress reads recorded rows; pass --store path".to_string())?;
    let latest = read_rows(path)?;
    if latest.is_empty() {
        return Err(format!("{path} holds no rows"));
    }
    let earlier = match options.against.as_deref() {
        Some(against) => {
            let rows = read_rows(against)?;
            if rows.is_empty() {
                return Err(format!("{against} holds no rows"));
            }
            Some(rows)
        }
        None => None,
    };

    let rule = gym::ab::Rule::v2();
    let findings = regress::review(earlier.as_deref(), &latest, &rule);

    println!("# Did this commit move the numbers?\n");
    match options.against.as_deref() {
        Some(against) => println!(
            "`{path}` holds {} rows and `{against}` holds {}. The chains verified, so neither \
             side's numbers were quietly rewritten. No door was asked.\n",
            latest.len(),
            earlier.as_ref().map_or(0, Vec::len),
        ),
        None => println!(
            "`{path}` holds {} rows, and each door's newest run is measured against its own \
             previous one. The chain verified, so the earlier numbers were not quietly \
             rewritten. No door was asked.\n",
            latest.len()
        ),
    }
    if findings.is_empty() {
        return Err(format!("{path} holds no run this command can read"));
    }
    for finding in &findings {
        println!("{}", regress::render(finding));
    }

    let verdicts: Vec<gym::gate::Verdict> = findings
        .iter()
        .filter_map(regress::Finding::verdict)
        .collect();
    // The provenance belongs under numbers. A run that compared nothing has
    // none, and printing a page of floors under it would read as though
    // something had been judged.
    if !verdicts.is_empty() {
        println!("{}", regress::render_floors(&rule));
    }
    let refused = findings.len() - verdicts.len();
    if refused > 0 {
        println!(
            "{refused} of {} door{} could not be compared, which is neither a pass nor a \
             regression.\n",
            findings.len(),
            if findings.len() == 1 { "" } else { "s" }
        );
    }
    if verdicts.contains(&gym::gate::Verdict::Failed) {
        std::process::exit(1);
    }
    if verdicts.is_empty() {
        std::process::exit(2);
    }
    Ok(())
}

/// One side's rows out of a store, split by the door the plan pins.
fn rows_for(rows: &[Row], door: &str) -> Vec<Row> {
    rows.iter()
        .filter(|row| row.door == door)
        .cloned()
        .collect()
}

/// A door's running cost over the rows a store recorded, stated as the
/// measured latencies and the recorded refusals. Cost is unmetered rather
/// than zero: a local lane has no meter, and a zero would be a price nobody
/// quoted.
fn profile_of(rows: &[Row]) -> Profile {
    let latencies: Vec<f64> = rows.iter().filter_map(|row| row.latency_ms).collect();
    let refusals = rows.iter().filter(|row| row.is_refused()).count();
    if latencies.len() != rows.len() || latencies.iter().any(|ms| !ms.is_finite() || *ms < 0.0) {
        return Profile::default();
    }
    // Rows do not establish a billing policy. Cost remains unknown.
    Profile::timed(&latencies).refusing(refusals)
}

/// Judges a frozen admission plan against recorded evidence and writes the
/// digested decision.
///
/// The development rows come from `--store`, split by the door names the
/// plan pins; the one-shot confirmation comes from `--locked` checked
/// against `--ledger`; the transfer check reads `--transfer-store` over
/// `--transfer-suite`. The plan, not a flag, decides what is compared: the
/// identities, the instrument, the winning metric and its declared effect,
/// and every guard's bound are all inside the plan's digest, so what this
/// command ran is what the record attests. The decision is written to
/// `--out` or stdout; the exit code follows the ruling, so a caller can
/// gate on it without parsing JSON.
fn admit_command(options: &Options) -> Result<(), String> {
    let plan_path = options
        .plan
        .as_deref()
        .ok_or_else(|| "admit judges a frozen plan; pass --plan path".to_string())?;
    let plan = gym::admission::Plan::load(plan_path).map_err(|error| error.to_string())?;
    let suite = load_suite(options)?;

    let store_path = options.store.as_deref().ok_or_else(|| {
        "admit reads the development rows from a store; pass --store path".to_string()
    })?;
    let development = read_rows(store_path)?;
    let commitment_path = options.commitment.as_deref().ok_or_else(|| {
        "admit requires a separately retained development report; pass --commitment path"
            .to_string()
    })?;
    let commitment = gym::commitment::Commitment::load(commitment_path)?;
    gym::admission::verify_report(
        &commitment,
        &development,
        &suite,
        &plan.workload,
        &[plan.base.door.clone(), plan.candidate.door.clone()],
    )?;
    let development_head = Store::at(store_path)
        .head()
        .map_err(|error| error.to_string())?;
    let dev_base = rows_for(&development, &plan.base.door);
    let dev_candidate = rows_for(&development, &plan.candidate.door);

    let locked = match (
        options.locked.as_deref(),
        options.ledger.as_deref(),
        options.locked_commitment.as_deref(),
    ) {
        (Some(path), Some(ledger_path), Some(report_path)) => {
            let rows = read_rows(path)?;
            let head = Store::at(path).head().map_err(|error| error.to_string())?;
            let ledger = gym::suite::LockedLedger::at(ledger_path);
            Some((
                rows_for(&rows, &plan.base.door),
                rows_for(&rows, &plan.candidate.door),
                head,
                ledger,
                rows,
                gym::commitment::Commitment::load(report_path)?,
            ))
        }
        (None, None, None) => None,
        _ => {
            return Err(
                "--locked, --ledger, and --locked-commitment go together; the confirmation is a read the ledger \
                 recorded, not rows a caller points at"
                    .to_string(),
            );
        }
    };

    let transfer = match (
        options.transfer_suite.as_deref(),
        options.transfer_store.as_deref(),
        options.transfer_commitment.as_deref(),
    ) {
        (Some(suite_path), Some(store_path), Some(report_path)) => {
            let transfer_suite = Suite::load_file(suite_path).map_err(|error| error.to_string())?;
            let rows = read_rows(store_path)?;
            let head = Store::at(store_path)
                .head()
                .map_err(|error| error.to_string())?;
            Some((
                transfer_suite,
                rows_for(&rows, &plan.base.door),
                rows_for(&rows, &plan.candidate.door),
                head,
                rows,
                gym::commitment::Commitment::load(report_path)?,
            ))
        }
        (None, None, None) => None,
        _ => {
            return Err(
                "--transfer-suite, --transfer-store, and --transfer-commitment go together; the check needs both the \
                 suite the plan froze and the rows scored on it"
                    .to_string(),
            );
        }
    };

    let evidence = gym::admission::Evidence {
        reports: gym::admission::Reports {
            development: Some(gym::admission::ReportEvidence {
                commitment: &commitment,
                rows: &development,
            }),
            locked: locked.as_ref().map(|(_, _, _, _, rows, commitment)| {
                gym::admission::ReportEvidence { rows, commitment }
            }),
            transfer: transfer.as_ref().map(|(_, _, _, _, rows, commitment)| {
                gym::admission::ReportEvidence { rows, commitment }
            }),
        },
        suite: &suite,
        development: gym::admission::Side {
            base: &dev_base,
            candidate: &dev_candidate,
            store_head: development_head,
        },
        locked: locked
            .as_ref()
            .map(
                |(base, candidate, head, ledger, _, _)| gym::admission::Locked {
                    base,
                    candidate,
                    ledger,
                    store_head: head.clone(),
                },
            ),
        transfer: transfer
            .as_ref()
            .map(
                |(transfer_suite, base, candidate, head, _, _)| gym::admission::Transfer {
                    suite: transfer_suite,
                    base,
                    candidate,
                    store_head: head.clone(),
                },
            ),
        deployment: Some(gym::gate::Deployment::new(
            plan.guards.deployment.budget.workload.clone(),
            profile_of(&dev_base),
            profile_of(&dev_candidate),
        )),
        decided_at: options.at.clone().unwrap_or_else(eval::now_utc),
        commitment: Some(commitment.digest.clone()),
    };
    let decision = plan.decide(&evidence).map_err(|error| error.to_string())?;

    let rendered =
        serde_json::to_string_pretty(&decision).expect("an admission decision serializes");
    match options.out.as_deref() {
        Some(path) => {
            std::fs::write(path, format!("{rendered}\n"))
                .map_err(|error| format!("{path}: {error}"))?;
            println!("wrote the decision to `{path}`: {}", decision.ruling);
        }
        None => println!("{rendered}"),
    }
    for phase in &decision.phases {
        eprintln!("{}: {}", phase.phase, phase.verdict);
        for criterion in phase.breaches() {
            eprintln!("  {} — {}", criterion.name, criterion.detail);
        }
    }
    for refusal in &decision.refusals {
        eprintln!("refused: {refusal}");
    }
    if !decision.ruling.admitted() {
        std::process::exit(1);
    }
    Ok(())
}

/// Reports how far each metric moves when only the seed block moves.
///
/// The draws come from `lev-calibration-sweep`, which asks the door and
/// records one line per item per block. Nothing here asks a door, so the
/// analysis can be corrected for the cost of a recompile rather than another
/// run of the hardware.
fn spread_command(options: &Options) -> Result<(), String> {
    let path = options
        .draws
        .as_deref()
        .ok_or_else(|| "spread reads recorded draws; pass --draws path".to_string())?;
    let text = std::fs::read_to_string(path).map_err(|error| format!("{path}: {error}"))?;
    let draws = Draws::parse(&text).map_err(|error| error.to_string())?;
    let doors = draws.doors();
    if doors.is_empty() {
        return Err(format!("{path} holds no draws"));
    }
    let families = draws.families();

    // Before a word is printed. A document that opens with the run facts and
    // then refuses halfway through reads like a result.
    //
    // Both splits have to be whole grids. Only the evaluation split has to
    // carry more than one block, because that is the one a spread is taken
    // over; the calibration split is fitted on and never reported over.
    for door in &doors {
        for split in ["evaluation", "calibration"] {
            if draws.blocks(door, split).is_empty() {
                continue;
            }
            draws
                .complete(door, split)
                .map_err(|error| error.to_string())?;
        }
        draws
            .is_a_spread(door, "evaluation")
            .map_err(|error| error.to_string())?;
    }

    println!("# The spread the calibration metrics carry across seed blocks\n");
    println!(
        "Generated by `cargo run -p gym --bin gym -- spread --draws {path}`, over draws \
         `lev-calibration-sweep` recorded from the live on-device runtime on one machine. No \
         door was asked to produce this document.\n"
    );
    report_run_facts(&draws, path);

    for door in &doors {
        let blocks = draws.blocks(door, "evaluation");
        if blocks.is_empty() {
            continue;
        }
        println!("## `{door}`, the evaluation split across every block\n");
        report_blocks(&draws, door, "evaluation", None, &blocks);
        for family in &families {
            println!("### `{door}`, `{family}`\n");
            report_blocks(&draws, door, "evaluation", Some(family), &blocks);
        }
        println!("### `{door}`, live ECE against frozen ECE\n");
        report_frozen(&draws, door, "evaluation", &families, &blocks);
        println!("### `{door}`, the item-sampling interval\n");
        report_bootstrap(&draws, door, "evaluation", &families, &blocks);
    }

    report_maps(&draws, &doors, &families);
    Ok(())
}

/// What the whole file says about how it was produced.
fn report_run_facts(draws: &Draws, path: &str) {
    let rows = draws.rows();
    let doors = draws.doors();
    println!("| Run fact | Value |");
    println!("| --- | --- |");
    println!("| Draws file | `{path}`, {} lines |", rows.len());
    if let Some(first) = rows.first() {
        println!("| Suite | `{}` |", first.suite);
        println!("| Samples per estimate | {} |", first.samples);
        println!("| Base model signature | `{}` |", first.base_signature);
    }
    for door in &doors {
        let adapter = rows
            .iter()
            .find(|row| &row.door == door)
            .and_then(|row| row.adapter.clone())
            .unwrap_or_else(|| "none".to_string());
        let evaluation = draws.blocks(door, "evaluation").len();
        let calibration = draws.blocks(door, "calibration").len();
        println!(
            "| Door `{door}` | adapter `{adapter}`, {evaluation} evaluation block(s), \
             {calibration} calibration block(s) |"
        );
    }
    println!();
}

/// The metrics a door reported on one group, block by block, and how far
/// they moved.
fn report_blocks(draws: &Draws, door: &str, split: &str, family: Option<&str>, blocks: &[u64]) {
    let mut scored = Vec::new();
    for block in blocks {
        let observations = draws.observations(door, split, family, *block);
        scored.push((*block, score(&observations)));
    }
    println!("| Seed block | Accuracy | ECE | Brier | Log loss | Confident errors | Items |");
    println!("| --- | --- | --- | --- | --- | --- | --- |");
    for (block, metrics) in &scored {
        println!(
            "| {block} | {:.3} | {:.3} | {:.3} | {:.3} | {} | {} |",
            metrics.accuracy,
            metrics.ece,
            metrics.brier,
            metrics.nll,
            metrics.confident_errors,
            metrics.items
        );
    }
    println!();
    println!("| Metric | Mean | Standard deviation | Lowest | Highest | Range |");
    println!("| --- | --- | --- | --- | --- | --- |");
    for metric in Metric::ALL {
        let values: Vec<f64> = scored
            .iter()
            .filter_map(|(_, metrics)| metric.read(&metrics.scores()))
            .collect();
        match Spread::over(&values) {
            Some(spread) => println!(
                "| `{metric}` | {:.4} | {:.4} | {:.3} | {:.3} | {:.3} |",
                spread.mean,
                spread.sd,
                spread.low,
                spread.high,
                spread.range()
            ),
            None => println!("| `{metric}` | | one block is not a spread | | | |"),
        }
    }
    println!();
}

/// How much of the ECE movement is items changing bin rather than values
/// changing.
fn report_frozen(draws: &Draws, door: &str, split: &str, families: &[String], blocks: &[u64]) {
    println!(
        "Frozen ECE bins every item by its mean signal over all {} blocks and holds that \
         membership fixed, so no block is privileged and nothing regroups. The difference \
         between the two standard deviations is what regrouping does, which can be to add \
         movement or to absorb it.\n",
        blocks.len()
    );
    println!("| Group | Live ECE sd | Frozen ECE sd | Live range | Frozen range |");
    println!("| --- | --- | --- | --- | --- |");
    let mut groups: Vec<Option<&str>> = vec![None];
    groups.extend(families.iter().map(String::as_str).map(Some));
    for group in groups {
        let per_block: Vec<Vec<Observation>> = blocks
            .iter()
            .map(|block| draws.observations(door, split, group, *block))
            .collect();
        let reference = mean_signal(&per_block);
        let live: Vec<f64> = per_block.iter().map(|block| score(block).ece).collect();
        let frozen: Vec<f64> = per_block
            .iter()
            .map(|block| ece_frozen(&reference, block))
            .collect();
        let name = group.unwrap_or("all items in one table");
        match (Spread::over(&live), Spread::over(&frozen)) {
            (Some(live), Some(frozen)) => println!(
                "| `{name}` | {:.4} | {:.4} | {:.3} | {:.3} |",
                live.sd,
                frozen.sd,
                live.range(),
                frozen.range()
            ),
            _ => println!("| `{name}` | | one block is not a spread | | |"),
        }
    }
    println!();
}

/// How far the same number would move on a different sample of items.
fn report_bootstrap(draws: &Draws, door: &str, split: &str, families: &[String], blocks: &[u64]) {
    println!(
        "A different question from the one above, and the larger half of it. Items are \
         resampled with replacement inside each family, so the suite's family mix is held \
         fixed, and each resample is scored on every block and averaged, so seed noise is \
         averaged down rather than counted twice. 2,000 resamples, seeded, so the interval \
         reproduces.\n"
    );
    println!("| Group | Metric | Mean over blocks | 95% interval | Width |");
    println!("| --- | --- | --- | --- | --- |");
    let mut groups: Vec<Option<&str>> = vec![None];
    groups.extend(families.iter().map(String::as_str).map(Some));
    for group in groups {
        let per_block: Vec<Vec<Observation>> = blocks
            .iter()
            .map(|block| draws.observations(door, split, group, *block))
            .collect();
        let strata = strata_of(draws, door, split, group, &per_block, families);
        let name = group.unwrap_or("all items in one table");
        for metric in Metric::ALL {
            let mean = mean_over_blocks(&per_block, metric);
            let interval = stratified_bootstrap(
                &strata,
                |sample: &[&Vec<Observation>]| bootstrap_statistic(sample, metric),
                2_000,
                0x5EED_B10C,
            );
            match (mean, interval) {
                (Some(mean), Some(interval)) => println!(
                    "| `{name}` | `{metric}` | {mean:.3} | {:.3} to {:.3} | {:.3} |",
                    interval.low,
                    interval.high,
                    interval.width()
                ),
                _ => println!("| `{name}` | `{metric}` | | not measured | |"),
            }
        }
    }
    println!();
}

/// One resample's value: the metric on each block, averaged over blocks.
#[allow(clippy::cast_precision_loss)]
fn bootstrap_statistic(sample: &[&Vec<Observation>], metric: Metric) -> f64 {
    let blocks = sample.first().map_or(0, |item| item.len());
    let mut total = 0.0;
    let mut seen = 0.0;
    for index in 0..blocks {
        let block: Vec<Observation> = sample.iter().map(|item| item[index].clone()).collect();
        if let Some(value) = metric.read(&score(&block).scores()) {
            total += value;
            seen += 1.0;
        }
    }
    if seen > 0.0 { total / seen } else { 0.0 }
}

/// One entry per item, each holding that item's observation in every block,
/// grouped into the strata a resample draws inside.
///
/// For a single family that is one stratum. For the whole suite it is one
/// stratum per family at its own size, because the suite fixes its mix —
/// 50 routing, 30 urgency, 18 severity — and resampling across families
/// would report an interval for a suite this one is not.
fn strata_of(
    draws: &Draws,
    door: &str,
    split: &str,
    group: Option<&str>,
    per_block: &[Vec<Observation>],
    families: &[String],
) -> Vec<Vec<Vec<Observation>>> {
    let items = per_block.first().map_or(0, Vec::len);
    let by_item: Vec<Vec<Observation>> = (0..items)
        .map(|index| per_block.iter().map(|block| block[index].clone()).collect())
        .collect();
    if group.is_some() || by_item.is_empty() {
        return vec![by_item];
    }
    let Some(first_block) = draws.blocks(door, split).first().copied() else {
        return vec![by_item];
    };
    // The same item order `observations` returns, so an item's family is the
    // family of the draw at its index.
    let order = draws.draws_of(door, split, None, first_block);
    let mut strata = Vec::new();
    for family in families {
        let inside: Vec<Vec<Observation>> = by_item
            .iter()
            .zip(order.iter())
            .filter(|(_, row)| &row.family == family)
            .map(|(item, _)| item.clone())
            .collect();
        if !inside.is_empty() {
            strata.push(inside);
        }
    }
    if strata.is_empty() {
        vec![by_item]
    } else {
        strata
    }
}

/// The mean of one metric over blocks.
#[allow(clippy::cast_precision_loss)]
fn mean_over_blocks(per_block: &[Vec<Observation>], metric: Metric) -> Option<f64> {
    let values: Option<Vec<f64>> = per_block
        .iter()
        .map(|block| metric.read(&score(block).scores()))
        .collect();
    let values = values?;
    if values.is_empty() {
        return None;
    }
    Some(values.iter().sum::<f64>() / values.len() as f64)
}

/// What a calibration map does to a door, refitted on each block's own
/// calibration draws rather than once.
fn report_maps(draws: &Draws, doors: &[String], families: &[String]) {
    report_fixed_map(draws, doors, families);
    let mut any = false;
    for door in doors {
        let calibration = draws.blocks(door, "calibration");
        let evaluation = draws.blocks(door, "evaluation");
        let shared: Vec<u64> = calibration
            .iter()
            .copied()
            .filter(|block| evaluation.contains(block))
            .collect();
        if shared.len() < 2 {
            continue;
        }
        if !any {
            println!("## The calibration map, refitted on every block\n");
            println!(
                "The published map numbers fitted one map on block 0's calibration draws and \
                 scored block 0's evaluation draws. Refitting per block moves the map as well \
                 as the scores, which is the whole spread a map claim carries.\n"
            );
            any = true;
        }
        println!("### `{door}`\n");
        println!("| Seed block | Map | ECE | Brier | Log loss | Confident errors | Fitted on |");
        println!("| --- | --- | --- | --- | --- | --- | --- |");
        let mut raw = Vec::new();
        let mut pooled = Vec::new();
        let mut banded = Vec::new();
        for block in &shared {
            let fit_on = draws.observations(door, "calibration", None, *block);
            let held = draws.observations(door, "evaluation", None, *block);
            let pooled_map = Map::fit_auto(&fit_on);
            let banded_map = Map::fit_banded(&fit_on);
            let mapped: Vec<Observation> = held
                .iter()
                .map(|o| Observation::new(pooled_map.apply(o.raw), o.correct))
                .collect();
            let conditioned: Vec<Observation> = held
                .iter()
                .map(|o| {
                    Observation::new(banded_map.apply_banded(o.raw, o.band.as_deref()), o.correct)
                })
                .collect();
            for (name, set) in [
                ("raw", &held),
                ("pooled", &mapped),
                ("band-conditioned", &conditioned),
            ] {
                let metrics = score(set);
                println!(
                    "| {block} | {name} | {:.3} | {:.3} | {:.3} | {} | {} |",
                    metrics.ece,
                    metrics.brier,
                    metrics.nll,
                    metrics.confident_errors,
                    fit_on.len()
                );
            }
            raw.push(score(&held));
            pooled.push(score(&mapped));
            banded.push(score(&conditioned));
        }
        println!();
        report_map_spread(&raw, &pooled, &banded);
    }
}

/// What one fitted map does across every evaluation block it is scored on.
///
/// This is the question a published map claim asks. The map is fitted once,
/// on the calibration draws of the first block, exactly as the published
/// numbers were; then it is scored on every evaluation block. The paired
/// gain is the improvement the map bought on that block, and the spread of
/// that gain is how much of the published improvement was the seeds.
///
/// It holds the fitting seeds fixed, so it is not the whole spread a map
/// claim carries. `The calibration map, refitted on every block` below moves
/// those too, on the blocks that have calibration draws.
fn report_fixed_map(draws: &Draws, doors: &[String], families: &[String]) {
    let mut any = false;
    let mut groups: Vec<Option<&str>> = vec![None];
    groups.extend(families.iter().map(String::as_str).map(Some));
    for door in doors {
        let Some(fitting) = draws.blocks(door, "calibration").first().copied() else {
            continue;
        };
        let evaluation = draws.blocks(door, "evaluation");
        if evaluation.len() < 2 {
            continue;
        }
        if !any {
            println!("## One map, scored on every evaluation block\n");
            println!(
                "The map is fitted once, on the calibration draws of one block, which is how \
                 every published map number here was produced. It is then scored on every \
                 evaluation block. The paired gain is what the map bought on that block, and \
                 the spread of the gain is how much of a published improvement is the seeds. \
                 The fitting seeds are held fixed, so this is not the whole spread a map claim \
                 carries.\n"
            );
            println!(
                "A map is fitted per group: over every item for the suite row, and over one \
                 family's items for a family row, which is the shape the published per-family \
                 maps were fitted in.\n"
            );
            any = true;
        }
        for group in &groups {
            report_one_fixed_map(draws, door, *group, fitting, &evaluation);
        }
    }
}

/// One group's map, fitted once and scored on every evaluation block.
fn report_one_fixed_map(
    draws: &Draws,
    door: &str,
    group: Option<&str>,
    fitting: u64,
    evaluation: &[u64],
) {
    let fit_on = draws.observations(door, "calibration", group, fitting);
    if fit_on.is_empty() {
        return;
    }
    let pooled_map = Map::fit_auto(&fit_on);
    let banded_map = Map::fit_banded(&fit_on);
    let name = group.unwrap_or("all items in one table");
    println!(
        "### `{door}`, `{name}`, map fitted on calibration block {fitting} over {} items\n",
        fit_on.len()
    );
    let bands: Vec<&String> = banded_map.by_band.keys().collect();
    println!(
        "Bands with their own table: {}.\n",
        if bands.is_empty() {
            "none; every band fell back to the pooled table".to_string()
        } else {
            bands
                .iter()
                .map(|band| format!("`{band}`"))
                .collect::<Vec<_>>()
                .join(", ")
        }
    );
    println!("| Seed block | Map | ECE | Brier | Log loss | Confident errors |");
    println!("| --- | --- | --- | --- | --- | --- |");
    let mut raw = Vec::new();
    let mut pooled = Vec::new();
    let mut banded = Vec::new();
    for block in evaluation {
        let held = draws.observations(door, "evaluation", group, *block);
        let mapped: Vec<Observation> = held
            .iter()
            .map(|o| Observation::new(pooled_map.apply(o.raw), o.correct))
            .collect();
        let conditioned: Vec<Observation> = held
            .iter()
            .map(|o| Observation::new(banded_map.apply_banded(o.raw, o.band.as_deref()), o.correct))
            .collect();
        for (label, set) in [
            ("raw", &held),
            ("pooled", &mapped),
            ("band-conditioned", &conditioned),
        ] {
            let metrics = score(set);
            println!(
                "| {block} | {label} | {:.3} | {:.3} | {:.3} | {} |",
                metrics.ece, metrics.brier, metrics.nll, metrics.confident_errors
            );
        }
        raw.push(score(&held));
        pooled.push(score(&mapped));
        banded.push(score(&conditioned));
    }
    println!();
    report_map_spread(&raw, &pooled, &banded);
}

/// The spread of each map's scores, and of the paired improvement over the
/// raw signal.
///
/// The gain is paired: both numbers come from the same block, so the spread
/// of the gain is the spread of the improvement itself rather than the two
/// levels' noise added together.
fn report_map_spread(raw: &[Metrics], pooled: &[Metrics], banded: &[Metrics]) {
    println!("| Metric | Map | Mean | Standard deviation | Paired gain over raw | Gain sd |");
    println!("| --- | --- | --- | --- | --- | --- |");
    for metric in [
        Metric::Ece,
        Metric::Brier,
        Metric::Nll,
        Metric::ConfidentErrors,
    ] {
        let read = |set: &[Metrics]| -> Vec<f64> {
            set.iter()
                .filter_map(|m| metric.read(&m.scores()))
                .collect()
        };
        let base = read(raw);
        if let Some(spread) = Spread::over(&base) {
            println!(
                "| `{metric}` | raw | {:.3} | {:.4} | | |",
                spread.mean, spread.sd
            );
        }
        for (name, set) in [("pooled", pooled), ("band-conditioned", banded)] {
            let values = read(set);
            let gains: Vec<f64> = base
                .iter()
                .zip(values.iter())
                .map(|(before, after)| metric.gain(*before, *after))
                .collect();
            match (Spread::over(&values), Spread::over(&gains)) {
                (Some(spread), Some(gain)) => println!(
                    "| `{metric}` | {name} | {:.3} | {:.4} | {:+.3} | {:.4} |",
                    spread.mean, spread.sd, gain.mean, gain.sd
                ),
                _ => println!("| `{metric}` | {name} | | one block is not a spread | | |"),
            }
        }
    }
    println!();
}

#[cfg(test)]
mod tests {
    #[test]
    fn admission_profile_does_not_invent_cost_or_drop_missing_timings() {
        let mut first = gym::row::Row::new("fixture", "digest", "one", "door");
        first.latency_ms = Some(10.0);
        let second = gym::row::Row::new("fixture", "digest", "two", "door");
        let complete = super::profile_of(&[first.clone()]);
        assert_eq!(complete.calls, 1);
        assert_eq!(complete.latency_p95_ms, Some(10.0));
        assert!(complete.cost.is_none());
        let incomplete = super::profile_of(&[first, second]);
        assert_eq!(incomplete.calls, 0);
        assert!(incomplete.latency_p95_ms.is_none());
        assert!(incomplete.cost.is_none());
    }

    use super::*;

    #[test]
    fn model_discovery_selects_the_requested_variant_or_alias() {
        let cards = serde_json::json!({"models": [
            {"id":"kev-0.5b"},
            {"id":"kev-4b", "aliases":["jev-latest", "kev-latest"]}
        ]});
        assert_eq!(discovered_model(&cards, "kev-4b").unwrap()["id"], "kev-4b");
        assert_eq!(
            discovered_model(&cards, "jev-latest").unwrap()["id"],
            "kev-4b"
        );
        assert!(discovered_model(&cards, "absent").is_none());
    }

    #[test]
    fn the_same_door_name_keeps_checkpoint_and_execution_comparisons_separate() {
        let mut before = Row {
            door: "kev-4b".to_string(),
            ..Row::default()
        };
        before.door_identity = DoorIdentity::published("kev-4b", "same-base", "");
        before.door_identity.artifact_signature = format!("sha256:{}", "a".repeat(64));
        let mut after = before.clone();
        after.door_identity.artifact_signature = format!("sha256:{}", "b".repeat(64));
        assert_ne!(Side::of(&before), Side::of(&after));
        assert_ne!(Side::of(&before).label(), Side::of(&after).label());
        after = before.clone();
        after
            .door_identity
            .execution
            .insert("dtype".to_string(), "bf16".to_string());
        assert_ne!(Side::of(&before).label(), Side::of(&after).label());
    }

    #[test]
    fn eval_subsets_preserve_order_and_reject_locked_or_unknown_items() {
        let suite = load_suite(&Options::default()).unwrap();
        let allowed = items_of(&suite, &[Partition::Calibration], None).unwrap();
        let wanted = [allowed[2].id.clone(), allowed[0].id.clone()]
            .into_iter()
            .collect();
        let mut selected = allowed.clone();
        narrow_eval_items(&mut selected, &wanted).unwrap();
        assert_eq!(
            selected.iter().map(|item| &item.id).collect::<Vec<_>>(),
            vec![&allowed[0].id, &allowed[2].id]
        );
        for forbidden in [
            "absent".to_string(),
            suite
                .items
                .iter()
                .find(|item| item.partition == Partition::Locked)
                .unwrap()
                .id
                .clone(),
        ] {
            assert!(
                narrow_eval_items(&mut allowed.clone(), &[forbidden].into_iter().collect())
                    .is_err()
            );
        }
        assert!(narrow_eval_items(&mut allowed.clone(), &Default::default()).is_err());
    }

    /// A file of item ids reads back without its provenance.
    ///
    /// The comments are the point of the format. A subset chosen after the
    /// numbers are in is how a result gets talked into existence, so the file
    /// says where its ids came from and the reader skips that.
    #[test]
    fn item_ids_skip_comments_and_blank_lines() {
        let path = std::env::temp_dir().join("gym-item-ids.txt");
        std::fs::write(
            &path,
            "# where these came from\n\nrouting/001\n  urgency/002  \n",
        )
        .unwrap();
        let ids = read_item_ids(path.to_str().unwrap()).unwrap();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains("routing/001"));
        assert!(ids.contains("urgency/002"));
        std::fs::remove_file(&path).ok();
    }

    /// A file that names nothing is refused rather than narrowing to nothing.
    #[test]
    fn a_file_of_comments_names_no_items() {
        let path = std::env::temp_dir().join("gym-item-ids-empty.txt");
        std::fs::write(&path, "# nothing here\n").unwrap();
        assert!(read_item_ids(path.to_str().unwrap()).is_err());
        std::fs::remove_file(&path).ok();
    }

    /// The flip view reads the winning option out of a recorded row.
    #[test]
    fn a_recorded_row_names_the_option_it_chose() {
        let row = Row {
            answered: true,
            correct: Some(true),
            distribution: Some(
                [
                    ("billing".to_string(), 0.25),
                    ("technical".to_string(), 0.75),
                ]
                .into_iter()
                .collect(),
            ),
            ..Row::default()
        };
        assert_eq!(chosen_of(&row).as_deref(), Some("technical"));
    }

    /// A refused row names no option, so it is neither a flip nor a match.
    #[test]
    fn a_refused_row_names_no_option() {
        let row = Row {
            refusal: Some(gym::row::RefusalCode::Guardrail),
            ..Row::default()
        };
        assert_eq!(chosen_of(&row), None);
    }

    /// One recorded row of the shape `eval` writes.
    fn recorded(family: &str, item: &str, correct: bool) -> Row {
        Row {
            recorded_at: "2026-09-20T00:00:00Z".to_string(),
            suite: "caller-v1".to_string(),
            suite_digest: "suite-digest".to_string(),
            question_set: Some("caller-v1".to_string()),
            question_digest: Some("question-digest".to_string()),
            split: "development".to_string(),
            family: family.to_string(),
            item_id: item.to_string(),
            door: "stub".to_string(),
            gate_id: Some("probability-v2".to_string()),
            gate_digest: Some("gate-digest".to_string()),
            answered: true,
            distribution: Some(
                [("yes".to_string(), 0.9), ("no".to_string(), 0.1)]
                    .into_iter()
                    .collect(),
            ),
            selected: Some("yes".to_string()),
            correct: Some(correct),
            ..Row::default()
        }
    }

    /// `report` renders the record a store carries, digests and all, and
    /// `verify` walks the same chain.
    #[test]
    fn report_renders_a_store_as_a_record() {
        let dir = std::env::temp_dir().join(format!("gym-report-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = dir.join("store.jsonl");
        let chain = Store::at(store.to_str().unwrap());
        chain
            .append(&recorded("routing", "routing/001", true))
            .unwrap();
        chain
            .append(&recorded("routing", "routing/002", false))
            .unwrap();
        chain
            .append(
                &recorded("severity", "severity/001", false)
                    .refused(gym::row::RefusalCode::Guardrail),
            )
            .unwrap();

        let out = dir.join("record.md");
        report_command(&Options {
            store: Some(store.to_str().unwrap().to_string()),
            out: Some(out.to_str().unwrap().to_string()),
            ..Options::default()
        })
        .unwrap();
        let record = std::fs::read_to_string(&out).unwrap();
        for expected in [
            "caller-v1",
            "suite-digest",
            "question-digest",
            "gate-digest",
            "receipt chain verifies",
            "`routing`",
            "`severity`",
            "1 refused by the door",
            "guardrail",
            "Checking this record",
        ] {
            assert!(record.contains(expected), "the record misses {expected}");
        }
        verify_command(&Options {
            store: Some(store.to_str().unwrap().to_string()),
            ..Options::default()
        })
        .unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    /// An edited row breaks the chain, and `verify` says so.
    #[test]
    fn verify_names_a_broken_chain() {
        let dir = std::env::temp_dir().join(format!("gym-verify-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = dir.join("store.jsonl");
        let chain = Store::at(store.to_str().unwrap());
        chain
            .append(&recorded("routing", "routing/001", true))
            .unwrap();
        chain
            .append(&recorded("routing", "routing/002", false))
            .unwrap();
        let text = std::fs::read_to_string(&store).unwrap();
        std::fs::write(
            &store,
            text.replacen("\"correct\":true", "\"correct\":false", 1),
        )
        .unwrap();
        let trouble = verify_command(&Options {
            store: Some(store.to_str().unwrap().to_string()),
            ..Options::default()
        })
        .unwrap_err();
        assert!(trouble.contains("edited"), "{trouble}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// The committed caller fixture gives report tests a real suite: its
    /// digest is what the rows pin, and its ten development items are the
    /// declared selection these tests run against.
    fn caller_suite() -> (Suite, String) {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/caller-v1/suite.json");
        let suite = Suite::load_file(path.to_str().unwrap()).unwrap();
        (suite, path.to_str().unwrap().to_string())
    }

    /// One row of the shape `eval` writes over a fixture item.
    fn fixture_row(suite: &Suite, item: &Item, door: &str) -> Row {
        Row {
            recorded_at: "2026-09-20T00:00:00Z".to_string(),
            suite: suite.name.clone(),
            suite_digest: suite.digest.clone(),
            question_set: Some("caller-v1".to_string()),
            question_digest: Some("question-digest".to_string()),
            split: item.partition.as_str().to_string(),
            family: item.family.clone(),
            item_id: item.id.clone(),
            door: door.to_string(),
            estimator: "greedy".to_string(),
            answered: true,
            distribution: Some(
                [("yes".to_string(), 0.9), ("no".to_string(), 0.1)]
                    .into_iter()
                    .collect(),
            ),
            selected: Some("yes".to_string()),
            correct: Some(true),
            ..Row::default()
        }
    }

    /// The fixture's development rows for one door, minus any skipped items.
    fn dev_rows(suite: &Suite, door: &str, skip: &[usize]) -> Vec<Row> {
        suite
            .items
            .iter()
            .filter(|item| item.partition == Partition::Development)
            .enumerate()
            .filter(|(index, _)| !skip.contains(index))
            .map(|(_, item)| fixture_row(suite, item, door))
            .collect()
    }

    /// A receipt-chained store holding exactly `rows`.
    fn chained(dir: &std::path::Path, name: &str, rows: &[Row]) -> String {
        let path = dir.join(name);
        let store = Store::at(path.to_str().unwrap());
        for row in rows {
            store.append(row).unwrap();
        }
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn admission_retained_report_refuses_changed_rows_tail_and_selection() {
        let dir = tempfile::tempdir().unwrap();
        let (suite, _) = caller_suite();
        let path = chained(dir.path(), "report.jsonl", &dev_rows(&suite, "base", &[]));
        let rows = read_rows(&path).unwrap();
        let doors = vec!["base".to_string()];
        let expected = gym::coverage::Expected::of(
            &suite,
            &[Partition::Development],
            None,
            None,
            doors.clone(),
        )
        .unwrap();
        let commitment = gym::commitment::Commitment::of(
            &suite,
            &expected,
            gym::commitment::Selection {
                partitions: vec!["development".into()],
                family: None,
                items: None,
                doors: doors.clone(),
            },
            &rows,
            rows.last().unwrap().receipt.clone(),
            None,
        );
        let workload = gym::admission::Workload {
            suite: suite.name.clone(),
            suite_digest: suite.digest.clone(),
            question_set: commitment.question_set.clone(),
            question_digest: commitment.question_digest.clone(),
            partitions: vec![Partition::Development],
            gate_digest: commitment.gate_digest.clone(),
        };
        let verify = |report: &gym::commitment::Commitment, evidence: &[Row], names: &[String]| {
            gym::admission::verify_report(report, evidence, &suite, &workload, names)
        };
        verify(&commitment, &rows, &doors).unwrap();
        assert!(verify(&commitment, &rows[..rows.len() - 1], &doors).is_err());
        let mut edited = rows.clone();
        edited[0].correct = Some(false);
        assert!(verify(&commitment, &edited, &doors).is_err());
        assert!(verify(&commitment, &rows, &["candidate".into()]).is_err());
        let mut forged = commitment.clone();
        forged.rows -= 1;
        assert!(verify(&forged, &rows, &doors).is_err());
    }

    /// Render the record for a store and return it.
    fn render(dir: &std::path::Path, store: &str, options: Options) -> String {
        let out = dir.join("record.md");
        report_command(&Options {
            store: Some(store.to_string()),
            out: Some(out.to_str().unwrap().to_string()),
            ..options
        })
        .unwrap();
        std::fs::read_to_string(&out).unwrap()
    }

    /// A full pass over the declared selection reports complete coverage.
    #[test]
    fn report_marks_a_covered_selection_complete() {
        let dir = std::env::temp_dir().join(format!("gym-report-full-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (suite, path) = caller_suite();
        let store = chained(&dir, "store.jsonl", &dev_rows(&suite, "stub", &[]));
        let record = render(
            &dir,
            &store,
            Options {
                suite: Some(path),
                partition: Some("development".to_string()),
                ..Options::default()
            },
        );
        assert!(record.contains("**Coverage: complete**"), "{record}");
        assert!(!record.contains("Coverage: incomplete"), "{record}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// An item the harness never wrote is named as missing — not scored
    /// wrong, and not allowed to pass silently.
    #[test]
    fn report_names_a_missing_middle_item() {
        let dir = std::env::temp_dir().join(format!("gym-report-gap-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (suite, path) = caller_suite();
        let store = chained(&dir, "store.jsonl", &dev_rows(&suite, "stub", &[4]));
        let missing = suite
            .items
            .iter()
            .filter(|item| item.partition == Partition::Development)
            .nth(4)
            .unwrap()
            .id
            .clone();
        let record = render(
            &dir,
            &store,
            Options {
                suite: Some(path),
                partition: Some("development".to_string()),
                ..Options::default()
            },
        );
        assert!(record.contains("**Coverage: incomplete**"), "{record}");
        assert!(
            record.contains("missing 1 of the expected items"),
            "{record}"
        );
        assert!(record.contains(&missing), "{record}");
        assert!(record.contains("not a completed evaluation"), "{record}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A dropped tail is the same finding: the last item's absence is named.
    #[test]
    fn report_names_a_missing_final_item() {
        let dir = std::env::temp_dir().join(format!("gym-report-tail-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (suite, path) = caller_suite();
        let store = chained(&dir, "store.jsonl", &dev_rows(&suite, "stub", &[9]));
        let record = render(
            &dir,
            &store,
            Options {
                suite: Some(path),
                partition: Some("development".to_string()),
                ..Options::default()
            },
        );
        assert!(record.contains("**Coverage: incomplete**"), "{record}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// An item-id file declares a subset, and covering exactly it is
    /// complete — the selection is the declaration, not the whole suite.
    #[test]
    fn report_treats_a_declared_subset_as_the_selection() {
        let dir = std::env::temp_dir().join(format!("gym-report-sub-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (suite, path) = caller_suite();
        let chosen: Vec<Item> = suite
            .items
            .iter()
            .filter(|item| item.partition == Partition::Development)
            .take(3)
            .cloned()
            .collect();
        let ids = dir.join("ids.txt");
        std::fs::write(
            &ids,
            chosen
                .iter()
                .map(|item| item.id.as_str())
                .collect::<Vec<_>>()
                .join("\n"),
        )
        .unwrap();
        let rows: Vec<Row> = chosen
            .iter()
            .map(|item| fixture_row(&suite, item, "stub"))
            .collect();
        let store = chained(&dir, "store.jsonl", &rows);
        let record = render(
            &dir,
            &store,
            Options {
                suite: Some(path),
                partition: Some("development".to_string()),
                items: Some(ids.to_str().unwrap().to_string()),
                ..Options::default()
            },
        );
        assert!(record.contains("**Coverage: complete**"), "{record}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A second recording of an item under the same run identity never
    /// reaches a report: the store refuses it on append, and `merge` folds
    /// it as a duplicate. The report's own naming of a duplicated item is
    /// coverage.rs's defense for stores written before that guard existed.
    #[test]
    fn the_store_refuses_a_second_recording_of_an_item() {
        let dir = std::env::temp_dir().join(format!("gym-report-dup-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (suite, _) = caller_suite();
        let path = dir.join("store.jsonl");
        let store = Store::at(path.to_str().unwrap());
        let rows = dev_rows(&suite, "stub", &[]);
        for row in &rows {
            store.append(row).unwrap();
        }
        let trouble = store.append(&rows[0]).unwrap_err();
        assert!(
            matches!(trouble, StoreError::DuplicatePerturbation { .. }),
            "{trouble}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Rows under two door identities render as two run groups and the
    /// verdict refuses to call either a complete pass.
    #[test]
    fn report_separates_mixed_run_identities() {
        let dir = std::env::temp_dir().join(format!("gym-report-mix-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (suite, path) = caller_suite();
        let mut rows = dev_rows(&suite, "stub", &[]);
        for row in rows.iter_mut().skip(5) {
            row.door_identity.model = "swapped".to_string();
        }
        let store = chained(&dir, "store.jsonl", &rows);
        let record = render(
            &dir,
            &store,
            Options {
                suite: Some(path),
                partition: Some("development".to_string()),
                ..Options::default()
            },
        );
        assert!(record.contains("2 run identities"), "{record}");
        assert!(record.contains("### Run identity 2 of 2"), "{record}");
        assert!(record.contains("**Coverage: incomplete**"), "{record}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A door the run was meant to ask but that left no rows is missing
    /// work, not an absent expectation.
    #[test]
    fn report_counts_an_expected_door_that_left_nothing() {
        let dir = std::env::temp_dir().join(format!("gym-report-door-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (suite, path) = caller_suite();
        let store = chained(&dir, "store.jsonl", &dev_rows(&suite, "stub", &[]));
        let record = render(
            &dir,
            &store,
            Options {
                suite: Some(path),
                partition: Some("development".to_string()),
                expect: vec!["ghost".to_string()],
                ..Options::default()
            },
        );
        assert!(record.contains("`ghost` left no rows"), "{record}");
        assert!(record.contains("**Coverage: incomplete**"), "{record}");
        std::fs::remove_dir_all(&dir).ok();
    }

    /// Without a suite the record renders, but says first that it cannot
    /// claim a completed evaluation.
    #[test]
    fn report_without_a_declared_selection_says_so_first() {
        let dir = std::env::temp_dir().join(format!("gym-report-open-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (suite, _) = caller_suite();
        let store = chained(&dir, "store.jsonl", &dev_rows(&suite, "stub", &[]));
        let record = render(&dir, &store, Options::default());
        assert!(record.contains("**Coverage is not declared**"), "{record}");
        assert!(
            record.contains("unverifiable as a completed evaluation"),
            "{record}"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A commitment written beside a record verifies the same store later,
    /// and a tail dropped after the hand-off is named against it.
    #[test]
    fn a_commitment_verifies_the_store_and_names_a_dropped_tail() {
        let dir = std::env::temp_dir().join(format!("gym-report-com-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (suite, path) = caller_suite();
        let store = chained(&dir, "store.jsonl", &dev_rows(&suite, "stub", &[]));
        let commitment = dir.join("commitment.json");
        report_command(&Options {
            store: Some(store.clone()),
            suite: Some(path),
            partition: Some("development".to_string()),
            commitment: Some(commitment.to_str().unwrap().to_string()),
            out: Some(dir.join("record.md").to_str().unwrap().to_string()),
            ..Options::default()
        })
        .unwrap();
        verify_command(&Options {
            store: Some(store.clone()),
            commitment: Some(commitment.to_str().unwrap().to_string()),
            ..Options::default()
        })
        .unwrap();

        // The record was handed over; the store lost its last row. The
        // chain still verifies — a prefix is intact — and the commitment
        // is what names the loss.
        let mut rows = read_rows(&store).unwrap();
        rows.pop();
        let trimmed = dir.join("trimmed.jsonl");
        let shorter = Store::at(trimmed.to_str().unwrap());
        for row in &rows {
            shorter.append(row).unwrap();
        }
        let trouble = verify_command(&Options {
            store: Some(trimmed.to_str().unwrap().to_string()),
            commitment: Some(commitment.to_str().unwrap().to_string()),
            ..Options::default()
        })
        .unwrap_err();
        assert!(
            trouble.contains("does not match the commitment"),
            "{trouble}"
        );
        rows.clear();
        std::fs::remove_dir_all(&dir).ok();
    }

    /// A commitment without a declared selection anchors nothing, so
    /// `report --commitment` needs `--suite`.
    #[test]
    fn a_commitment_needs_the_declared_selection() {
        let dir = std::env::temp_dir().join(format!("gym-report-noc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (suite, _) = caller_suite();
        let store = chained(&dir, "store.jsonl", &dev_rows(&suite, "stub", &[]));
        let trouble = report_command(&Options {
            store: Some(store),
            commitment: Some(dir.join("commitment.json").to_str().unwrap().to_string()),
            ..Options::default()
        })
        .unwrap_err();
        assert!(trouble.contains("--commitment needs --suite"), "{trouble}");
        std::fs::remove_dir_all(&dir).ok();
    }
}
