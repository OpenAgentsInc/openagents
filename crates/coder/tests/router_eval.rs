//! The chat router's live eval over the labeled route set (#9925).
//!
//! Ignored by default: each test asks a live service. Run one with
//!
//! ```sh
//! set -a; . ~/work/.secrets/typesafe.env; set +a    # TYPESAFE_API_KEY for Jev
//! CODER_AI_GATEWAY_KEY=... \                      # embeddings for the baseline
//! ROUTER_EVAL_SPLIT=held_out \
//!   cargo test -p coder --test router_eval -- --ignored --nocapture --test-threads 1
//! ```
//!
//! `ROUTER_EVAL_SPLIT` is `held_out` (the default), `tune`, or `all`.
//! `ROUTER_EVAL_ROWS` is `all` (the default), `v1` (the rows of
//! `routes-v1.json`, for comparing with the v1 measurements), `gym` (the
//! rows `routes-v2.json` added), `capability` (the rows `routes-v3.json`
//! added), `presentation` (the rows `routes-v4.json` added), or
//! `delegation` (the delegation rows and near misses #10073 added), or
//! `engine` (the engine requests and near misses #10076 added), or `map`
//! (asking to see the route map, and its near misses, #10085), or
//! `plugins` (which plugins there are, what one does, testing one, and
//! "the map" against Project map, #10090), or `coder_followup` (a
//! follow-up after Coder's run in the chat ended: a question about the run
//! for the chat, or more work for Coder's next turn, and near misses,
//! #10094), or `essays` (our essays and the ideas in them, and near
//! misses, #10099), or `plugin_create` (asking to make a new plugin, and
//! ordinary coding asks near it, #10177), or `standing` (background rules
//! from conversation and their near misses, #10157). Every run
//! also prints how the dispatch offers named engines (#10076).
//! `ROUTER_EVAL_SURFACE=desktop` asks as the desktop app does, with the
//! `deck` question over the decks it ships; unset is the set's default
//! phone context.
//! Each system prints a Markdown report ([`coder::router_eval::Report`]) and
//! writes its JSON to `ROUTER_EVAL_OUT` (default `target/router-eval/`).
//!
//! `ROUTER_EVAL_PUBLISH=1` makes `live_router` the published eval (#9959):
//! it reads the held-out rows (but the `coder_followup` ones, which would
//! pass NIP-EVAL's 256 cases) and the calibration partition, fits the
//! calibration maps on the latter ([`coder::router::calibration`]), scores
//! the held-out split raw and calibrated, and writes the evidence record
//! ([`coder::router_claim`]: `report.json` and its artifacts, an
//! `openagents.eval-report.v1` on the router as a decision service) and
//! `calibration-v2.json` to `target/router-eval/<date>/`. Commit the
//! calibration file as `crates/coder/fixtures/chat-router/calibration-v2.json`
//! and the numbers to a measurement under `docs/coder/measurements/`.
//!
//! Systems:
//!
//! - `chat-router-v5` (`live_router`): the router's Jev question set and
//!   policy table, in `Mode::Router`, with the `tool` question over the
//!   product corpus's tool catalog as the deployed worker asks it
//!   (`ROUTER_EVAL_GYM=off` leaves it out) and the `capability` question
//!   over the admitted set (the built-ins and that catalog; no adoption
//!   is read offline).
//! - `first-response-legacy`: the same judgment decided in `Mode::Legacy`,
//!   which is what a turn that asks only for `opener` gets: today's
//!   opener-and-prepared-answer path, the baseline the router must beat.
//! - `embedding`: nearest neighbor over the bank's `when` texts and the
//!   route descriptions, with its canned threshold fit on the tune split.
//!
//! Rows run one at a time, so each latency is one request's.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use coder::generate::{DEFAULT_DOOR_URL, Message, Role};
use coder::router;
use coder::router_eval::{
    CANNED_TARGET, Reading, Report, Row, Set, fit_threshold, nearest_reading, partition_of,
    route_descriptions, routed_reading,
};
use knowledge::search::{Embed, cosine};

fn split() -> String {
    std::env::var("ROUTER_EVAL_SPLIT").unwrap_or_else(|_| "held_out".to_string())
}

/// Whether this run is the published eval.
fn publishing() -> bool {
    std::env::var_os("ROUTER_EVAL_PUBLISH").is_some()
}

fn out_dir() -> PathBuf {
    std::env::var_os("ROUTER_EVAL_OUT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/router-eval"),
        PathBuf::from,
    )
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or_default()
}

/// The rows of `split` that `ROUTER_EVAL_ROWS` keeps, and the label the
/// report's split carries.
fn rows<'a>(set: &'a Set, split: &str) -> (Vec<&'a Row>, String) {
    let which = std::env::var("ROUTER_EVAL_ROWS").unwrap_or_else(|_| "all".to_string());
    let rows = set.rows(split);
    let gym = |row: &Row| row.tags.iter().any(|tag| tag == "gym");
    let capability = |row: &Row| row.tags.iter().any(|tag| tag == "capability");
    let presentation = |row: &Row| row.tags.iter().any(|tag| tag == "presentation");
    let delegation = |row: &Row| row.tags.iter().any(|tag| tag == "delegation");
    let engine = |row: &Row| row.tags.iter().any(|tag| tag == "engine");
    let map = |row: &Row| row.tags.iter().any(|tag| tag == "map");
    let plugins = |row: &Row| row.tags.iter().any(|tag| tag == "plugins");
    let followup = |row: &Row| row.tags.iter().any(|tag| tag == "coder_followup");
    let essays = |row: &Row| row.tags.iter().any(|tag| tag == "essays");
    let plugin_create = |row: &Row| row.tags.iter().any(|tag| tag == "plugin_create");
    let standing = |row: &Row| row.tags.iter().any(|tag| tag == "standing");
    match which.as_str() {
        "v1" => (
            rows.into_iter()
                .filter(|r| {
                    !gym(r)
                        && !capability(r)
                        && !presentation(r)
                        && !delegation(r)
                        && !engine(r)
                        && !map(r)
                        && !plugins(r)
                        && !followup(r)
                        && !essays(r)
                        && !plugin_create(r)
                        && !standing(r)
                })
                .collect(),
            format!("{split}-v1-rows"),
        ),
        "engine" => (
            rows.into_iter().filter(|r| engine(r)).collect(),
            format!("{split}-engine-rows"),
        ),
        "map" => (
            rows.into_iter().filter(|r| map(r)).collect(),
            format!("{split}-map-rows"),
        ),
        "plugins" => (
            rows.into_iter().filter(|r| plugins(r)).collect(),
            format!("{split}-plugins-rows"),
        ),
        "essays" => (
            rows.into_iter().filter(|r| essays(r)).collect(),
            format!("{split}-essays-rows"),
        ),
        "plugin_create" => (
            rows.into_iter().filter(|r| plugin_create(r)).collect(),
            format!("{split}-plugin-create-rows"),
        ),
        "coder_followup" => (
            rows.into_iter().filter(|r| followup(r)).collect(),
            format!("{split}-coder-followup-rows"),
        ),
        "gym" => (
            rows.into_iter().filter(|r| gym(r)).collect(),
            format!("{split}-gym-rows"),
        ),
        "capability" => (
            rows.into_iter().filter(|r| capability(r)).collect(),
            format!("{split}-capability-rows"),
        ),
        "presentation" => (
            rows.into_iter().filter(|r| presentation(r)).collect(),
            format!("{split}-presentation-rows"),
        ),
        "delegation" => (
            rows.into_iter().filter(|r| delegation(r)).collect(),
            format!("{split}-delegation-rows"),
        ),
        "standing" => (
            rows.into_iter().filter(|r| standing(r)).collect(),
            format!("{split}-standing-rows"),
        ),
        _ => (rows, split.to_string()),
    }
}

fn transcript(row: &Row) -> Vec<Message> {
    row.messages
        .iter()
        .map(|m| Message {
            role: if m.role == "assistant" {
                Role::Assistant
            } else {
                Role::User
            },
            text: m.text.clone(),
        })
        .collect()
}

fn publish(report: &Report) {
    println!("{}", report.markdown());
    let dir = out_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{}-{}.json", report.system, report.split));
    let _ = std::fs::write(
        &path,
        serde_json::to_string_pretty(&report.json()).unwrap_or_default(),
    );
    println!("wrote {}", path.display());
}

/// Asks the `chat-router-v1` set for every row of the split and decides
/// each tier in `mode`, as the deployed worker does for a turn with no
/// computer state: the CLI route is wired, so its command groups are asked
/// as `cli_group` (`ROUTER_EVAL_CLI=off` leaves them out, as before the
/// worker wired it).
async fn run_router(name: &str, mode: router::Mode) {
    let judge = coder::decision::from_env()
        .expect("a decision profile")
        .expect("TYPESAFE_API_KEY or another profile");
    let bank = router::Bank::builtin();
    let mut seams = router::Seams::default();
    if std::env::var("ROUTER_EVAL_CLI").as_deref() != Ok("off") {
        seams.cli = std::sync::Arc::new(coder::cli_route::CommandRoute::new(
            judge.clone(),
            std::sync::Arc::new(coder::cli_route::NoFill),
        ));
    }
    let facts = router::worker_facts("google/gemini-3.8-flash", Some(DEFAULT_DOOR_URL), &seams);
    let tools = if std::env::var("ROUTER_EVAL_GYM").as_deref() == Ok("off") {
        Vec::new()
    } else {
        let root = knowledge::product::repository();
        let corpus =
            knowledge::product::Corpus::load(&knowledge::product::default_dir(), Some(&root))
                .expect("the product corpus loads");
        coder::gym_kb::tools(&corpus)
    };
    let admitted = router::Admitted::of(&tools, &[]);
    let desktop = std::env::var("ROUTER_EVAL_SURFACE").as_deref() == Ok("desktop");
    // The desktop app's entries (`.desktop` variants, #10085) show only there.
    let facts = facts.on_desktop(desktop);
    let context = router::Context {
        surface: desktop.then_some(router::Surface::Desktop),
        ..router::Context::default()
    };
    let decks: &[openagents_deck::DeckEntry] = if desktop { router::decks() } else { &[] };
    let situation = router::Situation {
        mode,
        context: &context,
        personalize: true,
        draft: false,
        earlier: false,
        plugin: false,
    };
    let set = Set::fixture();
    let (rows, split_label) = if publishing() {
        // The held-out rows the record is on, and the calibration
        // partition the maps are fitted on.
        let rows: Vec<&Row> = set
            .rows
            .iter()
            .filter(|row| matches!(partition_of(row), "locked" | "calibration"))
            // NIP-EVAL's report names at most 256 cases, and the held-out
            // split passed that with the follow-up rows (#10094): those are
            // measured on their own (`ROUTER_EVAL_ROWS=coder_followup`),
            // and their calibration-partition rows still fit the maps.
            .filter(|row| {
                partition_of(row) != "locked" || !row.tags.iter().any(|tag| tag == "coder_followup")
            })
            .collect();
        (rows, "held_out+calibration".to_string())
    } else {
        rows(&set, &split())
    };
    let started_at = now();
    let mut readings = Vec::new();
    let mut traces = Vec::new();
    // `eval.run`: whether the reply offers `start_eval` for the row's tool
    // (the default tool when it names none), against the starter test
    // sets as the deployed worker reads them (#9943).
    let starter = starter_records(&tools);
    let mut offers = Offers::default();
    let mut engines = Engines::default();
    // `ROUTER_EVAL_READINGS`: replay a run's readings file instead of
    // asking the judge (refits the maps from a finished run).
    let replay: Option<Vec<Reading>> = std::env::var_os("ROUTER_EVAL_READINGS").map(|path| {
        let bytes = std::fs::read(&path).expect("the readings file");
        serde_json::from_slice(&bytes).expect("a readings file")
    });
    if let Some(replayed) = &replay {
        readings = rows
            .iter()
            .map(|row| {
                replayed
                    .iter()
                    .find(|reading| reading.id == row.id)
                    .cloned()
                    .unwrap_or_else(|| Reading {
                        id: row.id.clone(),
                        error: Some("not in the replayed readings".to_string()),
                        ..Reading::default()
                    })
            })
            .collect();
    }
    for row in rows.iter().filter(|_| replay.is_none()) {
        let started = Instant::now();
        let asked = router::ask(
            &judge,
            router::split(
                row.latest(),
                &transcript(row),
                bank,
                &facts,
                &seams.cli.groups(),
                &tools,
                &admitted,
                // The set's default phone context asks no `deck` question.
                decks,
            ),
        )
        .await;
        let ms = started.elapsed().as_millis();
        readings.push(match asked {
            Ok(response) => {
                let routing = router::reading(&response, bank, &facts, &admitted);
                // A multi-turn row has messages before its latest (#10138).
                let situation = router::Situation {
                    earlier: row.messages.len() > 1,
                    ..situation
                };
                let tier = router::decide(&routing, bank, &facts, &situation);
                let offered = offers.count(row, &tier, &starter, bank, &facts);
                engines.count(row, &tier);
                let mut traced = trace(row, &routing, &tier);
                traced["start_eval"] = serde_json::json!(offered);
                traces.push(traced);
                routed_reading(&row.id, &routing, &tier, ms)
            }
            Err(error) => Reading {
                id: row.id.clone(),
                error: Some(error.to_string()),
                ms,
                ..Reading::default()
            },
        });
    }
    let ended_at = now();
    publish(&Report::of(name, &split_label, &rows, &readings));
    if offers.rows > 0 {
        println!(
            "eval.run start_eval ({split_label}): offered on {} of {} rows, for the right tool on {}",
            offers.offered, offers.rows, offers.right
        );
    }
    engines.print(&split_label);
    write_traces(name, &split_label, &traces);
    write_readings(name, &split_label, &readings);
    print_thresholds(&split_label, &rows, &readings);
    if publishing() && mode == router::Mode::Router {
        let profile = coder::decision::profile_from_env()
            .ok()
            .flatten()
            .map_or("unknown".to_string(), |profile| {
                format!("{}:jev", profile.name())
            });
        record(&set, &rows, &readings, &profile, started_at, ended_at);
    }
}

/// The published eval's evidence record (#9959): the held-out report as
/// an `openagents.eval-report.v1`, with the calibration maps fitted on the
/// calibration partition and scored on the held-out split.
fn record(
    set: &Set,
    rows: &[&Row],
    readings: &[Reading],
    judge: &str,
    started_at: u64,
    ended_at: u64,
) {
    use coder::router::calibration::{Calibration, Question, SCHEMA};
    use coder::router_claim::{Claim, GATE};
    use coder::router_eval::observations;

    let held: Vec<&Row> = rows
        .iter()
        .copied()
        .filter(|row| partition_of(row) == "locked")
        .collect();
    let fit: Vec<&Row> = rows
        .iter()
        .copied()
        .filter(|row| partition_of(row) == "calibration")
        .collect();
    let held_readings: Vec<Reading> = readings
        .iter()
        .filter(|r| held.iter().any(|row| row.id == r.id))
        .cloned()
        .collect();
    let fit_readings: Vec<Reading> = readings
        .iter()
        .filter(|r| fit.iter().any(|row| row.id == r.id))
        .cloned()
        .collect();
    let report = Report::of(router::SET, "held_out", &held, &held_readings);
    publish(&report);
    publish(&Report::of(router::SET, "calibration", &fit, &fit_readings));

    let probability = gym::gate::load("probability-v2").expect("probability-v2 loads");
    let (fit_route, fit_answer) = observations(&fit, &fit_readings);
    let (held_route, held_answer) = observations(&held, &held_readings);
    let bank = router::Bank::builtin();
    let date = gym::eval::utc_from_unix(ended_at)[..10].to_string();
    let calibration = Calibration {
        schema: SCHEMA.to_string(),
        set: router::set_id(),
        bank: bank.id(),
        created: date.clone(),
        fitted_on: "calibration".to_string(),
        fitted_rows: fit.len(),
        route: Question::fit(&fit_route, &held_route, &probability),
        answer: Question::fit(&fit_answer, &held_answer, &probability),
    };
    for (name, question) in [
        ("route", &calibration.route),
        ("answer", &calibration.answer),
    ] {
        println!(
            "calibration {name}: fitted on {} rows, {} bins; held out {} rows: ECE {:.3} -> {:.3}, Brier {:.3} -> {:.3}, NLL {:.3} -> {:.3}, confident errors {} -> {}; probability-v2 {}",
            question.map.fitted_on,
            question.map.bins.len(),
            question.held_out_items,
            question.raw.ece,
            question.calibrated.ece,
            question.raw.brier,
            question.calibrated.brier,
            question.raw.nll,
            question.calibrated.nll,
            question.raw.confident_errors,
            question.calibrated.confident_errors,
            question.verdict,
        );
    }

    let gym_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../gym");
    let suite_bytes = std::fs::read(gym_dir.join("suites/chat-router-v2.json")).expect("the suite");
    let gate_bytes = std::fs::read(gym_dir.join(format!("gates/{GATE}.json"))).expect("the gate");
    let gate = gym::gate::load(GATE).expect("router-v1 loads");
    let commit = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .ok()
        .filter(|out| out.status.success())
        .map(|out| String::from_utf8_lossy(&out.stdout).trim().to_string());
    let worker = format!("coder@{}", env!("CARGO_PKG_VERSION"));
    let claim = Claim {
        set,
        suite_bytes: &suite_bytes,
        gate_bytes: &gate_bytes,
        rows: &held,
        readings: &held_readings,
        report: &report,
        calibration: &calibration,
        gate: &gate,
        bank,
        judge,
        worker: &worker,
        commit: commit.as_deref(),
        started_at,
        ended_at,
    };
    // The maps first: they stand on their own held-out gate, whether or
    // not the evidence record below is well formed.
    let dir = out_dir().join(&date);
    let _ = std::fs::create_dir_all(&dir);
    let calibration_file = std::env::var("ROUTER_EVAL_CALIBRATION_FILE")
        .unwrap_or_else(|_| "calibration-v2.json".to_string());
    std::fs::write(
        dir.join(&calibration_file),
        serde_json::to_string_pretty(&calibration).unwrap_or_default() + "\n",
    )
    .expect("the calibration is written");
    println!("wrote {}", dir.join(&calibration_file).display());
    let record = claim.record();
    let bytes = record.bytes();
    nostr::eval_ext::parse_report(&bytes).expect("the record is a NIP-EVAL report");
    let outcome = gate.judge_router(&claim.comparison());
    println!(
        "gate {} ({}): {}",
        outcome.gate_id, outcome.gate_digest, outcome.verdict
    );
    for criterion in &outcome.criteria {
        println!(
            "  {} {}: {}",
            criterion.name, criterion.verdict, criterion.detail
        );
    }
    record.write(&dir).expect("the record is written");
    println!(
        "wrote {} ({} bytes of report, {} files) and calibration-v2.json",
        dir.display(),
        bytes.len(),
        record.files.len()
    );
}

/// The Gym's records with a starter test set for every catalog tool, as a
/// refresh reads them from the hosted runner's releases: six tests each,
/// the tool named by its starter catalog reference.
fn starter_records(tools: &[router::gym::Tool]) -> router::gym::Records {
    let suites = tools
        .iter()
        .enumerate()
        .filter_map(|(n, tool)| {
            let release = router::gym::EventPointer {
                id: format!("{:064x}", n + 1),
                pubkey: nostr::eval_ext::hosted::RUNNER.to_string(),
                kind: 3184,
            };
            Some(router::gym::SuiteRecord {
                release: release.clone(),
                tool: Some(tool.id.clone()),
                tool_name: tool.name.clone(),
                author: release.pubkey.clone(),
                subject: coder::gym_kb::catalog_definition(tool)?,
                cases: 6,
                at: 1_790_667_164,
                source: release,
            })
        })
        .collect();
    router::gym::Records {
        tools: tools.to_vec(),
        suites,
        ..router::gym::Records::default()
    }
}

/// `eval.run` rows and the offers their replies made.
#[derive(Default)]
struct Offers {
    rows: usize,
    offered: usize,
    right: usize,
}

impl Offers {
    /// Counts `row` when it is labeled `eval.run`: the tool the reply's
    /// `start_eval` names, if it made one.
    fn count(
        &mut self,
        row: &coder::router_eval::Row,
        tier: &router::Tier,
        records: &router::gym::Records,
        bank: &router::Bank,
        facts: &router::Facts,
    ) -> Option<String> {
        if row.route != "eval.run" {
            return None;
        }
        self.rows += 1;
        let router::Tier::Gym { route, tool, .. } = tier else {
            return None;
        };
        let grounding = router::gym::Grounding {
            records: records.clone(),
            news: Vec::new(),
        };
        let reply = router::gym::reply(*route, tool.as_deref(), &grounding, bank, facts);
        let router::gym::Reply::Bank {
            offer:
                Some(router::Offer::StartEval {
                    subject: nostr::cj_conversation::SubjectSource::Definition(subject),
                    ..
                }),
            ..
        } = reply
        else {
            return None;
        };
        self.offered += 1;
        let offered = records
            .tools
            .iter()
            .find(|tool| coder::gym_kb::catalog_definition(tool).as_ref() == Some(&*subject))
            .map(|tool| tool.id.clone());
        let wanted = row
            .tool
            .clone()
            .unwrap_or_else(|| coder::gym_kb::DEFAULT_TOOL.to_string());
        if offered.as_deref() == Some(wanted.as_str()) {
            self.right += 1;
        }
        offered
    }
}

/// How dispatch offers named engines (#10076).
#[derive(Default)]
struct Engines {
    /// Rows labeled with an engine, and those whose dispatch offer named it.
    asked: usize,
    named_right: usize,
    /// Rows labeled with an engine whose offer named another one.
    named_wrong: usize,
    /// Dispatch offers that named an engine where the row asks for none:
    /// a `work.dispatch` row without one, or any other row.
    false_named: usize,
    /// Dispatch offers that named an engine, in all.
    named: usize,
}

impl Engines {
    fn count(&mut self, row: &Row, tier: &router::Tier) {
        let offered = match tier {
            router::Tier::CannedStem {
                offer: Some(router::Offer::RunCoder { engine, .. }),
                ..
            }
            | router::Tier::CannedFinal {
                offer: Some(router::Offer::RunCoder { engine, .. }),
                ..
            } => *engine,
            _ => None,
        };
        let wanted = row.engine.as_deref().and_then(router::CodingEngine::parse);
        if offered.is_some() {
            self.named += 1;
        }
        match (wanted, offered) {
            (Some(want), Some(got)) if want == got => {
                self.asked += 1;
                self.named_right += 1;
            }
            (Some(_), Some(_)) => {
                self.asked += 1;
                self.named_wrong += 1;
            }
            (Some(_), None) => self.asked += 1,
            (None, Some(_)) => self.false_named += 1,
            (None, None) => {}
        }
    }

    fn print(&self, split: &str) {
        let precision = if self.named == 0 {
            1.0
        } else {
            self.named_right as f64 / self.named as f64
        };
        println!(
            "engine ({split}): {} rows ask for one; the offer named it on {}, another on {}; \
             {} offers named one where none was asked; engine precision {precision:.3} ({}/{})",
            self.asked,
            self.named_right,
            self.named_wrong,
            self.false_named,
            self.named_right,
            self.named
        );
    }
}

/// One row's reading in full, for tuning on the tune split: ids and
/// probabilities, and the row's own labels.
fn trace(row: &Row, routing: &router::Routing, tier: &router::Tier) -> serde_json::Value {
    serde_json::json!({
        "id": row.id,
        "message": row.latest(),
        "label": { "route": row.route, "answer": row.answer, "also": row.also,
                   "tier": row.tier, "cli_group": row.cli_group },
        "route": routing.route.word(),
        "route_p": routing.route_p,
        "runner_up": routing.runner_up.map(|(r, p)| (r.word(), p)),
        "answer": routing.answer.as_ref().map(|(e, p)| (e.id.clone(), *p)),
        "needs_specifics": routing.needs_specifics,
        "lane": format!("{:?}", routing.lane),
        "lane_p": routing.lane_p,
        "cli_group": routing.cli_group,
        "tool": routing.tool,
        "capability": routing.capability.as_ref().map(|(c, p)| (c.id.clone(), *p)),
        "capability_missing_p": routing.capability_missing_p,
        "capability_closest": routing.capability_closest.as_ref().map(|(c, p)| (c.id.clone(), *p)),
        "deck": routing.deck,
        "engine": routing.engine.map(|(engine, p)| (engine.word(), p)),
        "engine_label": row.engine,
        "risk": routing.risk.word(),
        "risk_p": routing.risk_p,
        "tier": tier.word(),
        "served": tier.answer().map(|e| e.id.clone()),
    })
}

/// The raw readings, so a threshold or map can be refitted later without
/// asking Jev again (#10386).
fn write_readings(name: &str, split: &str, readings: &[Reading]) {
    let dir = std::env::var_os("ROUTER_EVAL_OUT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/router-eval"),
        PathBuf::from,
    );
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join(format!("{name}-{split}-readings.json")),
        serde_json::to_string_pretty(readings).unwrap_or_default(),
    );
}

/// The `answer` and `route` thresholds on these rows (#10386): the raw
/// policy thresholds against the committed map with the cost-derived
/// calibrated threshold, each with its served, wrong and fell-through
/// counts and its mean cost under the written costs.
fn print_thresholds(split: &str, rows: &[&Row], readings: &[Reading]) {
    use router::thresholds::{ANSWER_COSTS, CALIBRATED_ANSWER_CONFIDENCE, ROUTE_COSTS, operate};
    let Ok(calibration) = router::calibration::Calibration::builtin() else {
        return;
    };
    let (route, answer) = coder::router_eval::observations(rows, readings);
    let show = |question: &str, point: router::thresholds::Operating| {
        println!(
            "threshold ({split}) {question} {}{:.2}: n={} served={} wrong={} fell_through={} precision={} cost/item={:.3}",
            if point.calibrated {
                "calibrated>="
            } else {
                "raw>="
            },
            point.threshold,
            point.items,
            point.acted,
            point.wrong,
            point.fell_through,
            point
                .precision()
                .map_or("-".to_string(), |p| format!("{p:.3}")),
            point.cost_per_item,
        );
    };
    show(
        "answer",
        operate(
            &answer,
            None,
            router::policy::ANSWER_CONFIDENCE,
            ANSWER_COSTS,
        ),
    );
    show(
        "answer",
        operate(
            &answer,
            Some(&calibration.answer.map),
            CALIBRATED_ANSWER_CONFIDENCE,
            ANSWER_COSTS,
        ),
    );
    show(
        "route",
        operate(&route, None, router::policy::ROUTE_CONFIDENCE, ROUTE_COSTS),
    );
}

fn write_traces(name: &str, split: &str, traces: &[serde_json::Value]) {
    let dir = std::env::var_os("ROUTER_EVAL_OUT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/router-eval"),
        PathBuf::from,
    );
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join(format!("{name}-{split}-rows.json")),
        serde_json::to_string_pretty(traces).unwrap_or_default(),
    );
}

#[tokio::test]
#[ignore = "calls the live judge"]
async fn live_router() {
    run_router(router::SET, router::Mode::Router).await;
}

#[tokio::test]
#[ignore = "calls the live judge"]
async fn live_first_response_baseline() {
    run_router("first-response-legacy", router::Mode::Legacy).await;
}

/// Embeds `texts` in batches.
async fn embed_all<E: Embed>(embedder: &E, texts: Vec<String>) -> Vec<Vec<f32>> {
    let mut out = Vec::with_capacity(texts.len());
    for batch in texts.chunks(96) {
        let (vectors, _) = embedder
            .embed(batch.to_vec())
            .await
            .expect("the embeddings call");
        out.extend(vectors);
    }
    out
}

#[tokio::test]
#[ignore = "calls a live embeddings provider"]
async fn live_embedding_baseline() {
    let embedder = coder::codebase::embedder().expect("an embeddings key");
    let bank: Vec<(String, String, String)> = router::Bank::builtin()
        .answers
        .iter()
        .filter_map(|a| Some((a.id.clone(), a.routes.first()?.clone(), a.when.clone())))
        .collect();
    let routes = route_descriptions();
    let bank_vectors = embed_all(&embedder, bank.iter().map(|b| b.2.clone()).collect()).await;
    let route_vectors = embed_all(
        &embedder,
        routes.iter().map(|(_, d)| (*d).to_string()).collect(),
    )
    .await;
    let set = Set::fixture();
    let mut scored = BTreeMap::new();
    let mut latency = BTreeMap::new();
    for row in &set.rows {
        let started = Instant::now();
        let (vectors, _) = embedder
            .embed(vec![row.latest().to_string()])
            .await
            .expect("the embeddings call");
        latency.insert(row.id.clone(), started.elapsed().as_millis());
        let query = &vectors[0];
        let answers: Vec<(String, String, f64)> = bank
            .iter()
            .zip(&bank_vectors)
            .map(|((id, route, _), v)| (id.clone(), route.clone(), cosine(v, query)))
            .collect();
        let near_routes: Vec<(String, f64)> = routes
            .iter()
            .zip(&route_vectors)
            .map(|((route, _), v)| ((*route).to_string(), cosine(v, query)))
            .collect();
        scored.insert(row.id.clone(), (answers, near_routes));
    }
    let tune = set.rows("tune");
    let threshold = fit_threshold(&tune, &scored, CANNED_TARGET);
    println!("embedding baseline: canned threshold {threshold:.2}, fit on the tune split");
    let (rows, split) = rows(&set, &split());
    let readings: Vec<Reading> = rows
        .iter()
        .map(|row| {
            let (answers, near) = &scored[&row.id];
            let mut reading = nearest_reading(&row.id, answers, near, threshold);
            reading.ms = latency[&row.id];
            reading
        })
        .collect();
    publish(&Report::of(
        &format!("embedding-nn@{threshold:.2}"),
        &split,
        &rows,
        &readings,
    ));
}
