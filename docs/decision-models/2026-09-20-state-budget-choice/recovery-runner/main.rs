
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use coder::classify::{Caps, STATE_BUDGET, bounded_text, state_within};
use coder::generate::{Message, Role};
use jev::{Answer, Client, Config, SystemOneRequest};
use serde_json::{Value, json};

/// Where the suite lives, relative to this crate.
fn suite_path() -> PathBuf {
    PathBuf::from("/Users/christopherdavid/work/openagents/crates/gym/suites/coder-turns-v1.json")
}

/// One real turn: the state's parts, and the truth for each family asked
/// about it.
struct Turn {
    /// The state's key in the suite, shared by every item over it.
    key: String,
    partition: String,
    task: String,
    transcript: Vec<Message>,
    repo: Vec<String>,
    /// Family to truth, over the items that read this state.
    truths: BTreeMap<String, String>,
}

/// Every turn state in the suite, one entry per state rather than per
/// item. Shell-round states are `shell::state_of`'s and are not read here.
fn turns() -> Vec<Turn> {
    let text = fs::read_to_string(suite_path()).expect("the suite is on disk");
    let suite: Value = serde_json::from_str(&text).expect("the suite is JSON");
    let mut turns: BTreeMap<String, Turn> = BTreeMap::new();
    for item in suite["items"].as_array().expect("items") {
        let state = &item["state"];
        let Some(transcript) = state["transcript"].as_array() else {
            continue;
        };
        let id = item["id"].as_str().expect("an id");
        let key = id.split('/').nth(1).expect("family/key#n").to_string();
        let turn = turns.entry(key.clone()).or_insert_with(|| Turn {
            key,
            partition: item["partition"].as_str().expect("a partition").to_string(),
            task: state["task"].as_str().expect("a task").to_string(),
            transcript: transcript
                .iter()
                .map(|message| Message {
                    role: match message["role"].as_str() {
                        Some("user") => Role::User,
                        _ => Role::Assistant,
                    },
                    text: message["text"].as_str().expect("text").to_string(),
                })
                .collect(),
            repo: state["repo_members"]
                .as_array()
                .expect("repo members")
                .iter()
                .map(|member| member.as_str().expect("a member").to_string())
                .collect(),
            truths: BTreeMap::new(),
        });
        turn.truths.insert(
            item["family"].as_str().expect("a family").to_string(),
            item["truth"].as_str().expect("a truth").to_string(),
        );
    }
    turns.into_values().collect()
}

/// The state's size as the wire carries it.
fn size(state: &Value) -> usize {
    serde_json::to_string(state)
        .expect("a state serializes")
        .len()
}

fn state(turn: &Turn, caps: &Caps) -> Value {
    state_within(&turn.task, &turn.transcript, &turn.repo, caps)
}

fn every_real_turn_state_fits_the_budget() {
    let turns = turns();
    assert_eq!(turns.len(), 40, "the suite holds 40 turn states");
    let mut largest = 0;
    for turn in &turns {
        let bytes = size(&state(turn, &Caps::PRODUCTION));
        assert!(
            bytes <= STATE_BUDGET,
            "state {} is {bytes} bytes under Caps::PRODUCTION, over the {STATE_BUDGET} budget",
            turn.key
        );
        largest = largest.max(bytes);
    }
    assert!(
        largest > STATE_BUDGET / 2,
        "the budget is far from tight: {largest}"
    );
}

fn a_shell_record_is_bounded_block_by_block() {
    let record = "ran shell commands:\n\n$ ls\nexit 0\na\nb\n\n$ cat big\nexit 0\n".to_string()
        + &"x".repeat(600)
        + "\n\n$ true\nexit 0\n";
    let caps = Caps {
        turns: 6,
        message_bytes: 4096,
        commands: 2,
        output_bytes: 8,
    };
    assert_eq!(
        bounded_text(&record, &caps),
        format!(
            "ran shell commands:\n\n$ ls\nexit 0\na\nb\n\n$ cat big\nexit 0\n{}\n\n1 more commands not shown\n",
            "x".repeat(8)
        )
    );
    assert_eq!(bounded_text(&record, &Caps::UNBUDGETED), record);
}

fn prose_is_cut_on_a_character_boundary() {
    let caps = Caps {
        message_bytes: 5,
        ..Caps::PRODUCTION
    };
    assert_eq!(bounded_text("héllo world", &caps), "héll");
    let state = state_within(
        "task",
        &[Message {
            role: Role::Assistant,
            text: "héllo world".to_string(),
        }],
        &[],
        &caps,
    );
    assert_eq!(state["transcript"][0]["text"], "héll");
}

fn the_unbudgeted_caps_reproduce_the_suite() {
    let text = fs::read_to_string(suite_path()).expect("the suite is on disk");
    let suite: Value = serde_json::from_str(&text).expect("the suite is JSON");
    let recorded: BTreeMap<String, &Value> = suite["items"]
        .as_array()
        .expect("items")
        .iter()
        .filter(|item| item["state"]["transcript"].is_array())
        .map(|item| {
            let key = item["id"].as_str().expect("an id").split('/').nth(1);
            (key.expect("a key").to_string(), &item["state"])
        })
        .collect();
    let mut over_budget = 0;
    for turn in turns() {
        let rebuilt = state(&turn, &Caps::UNBUDGETED);
        assert_eq!(&rebuilt, recorded[&turn.key], "state {}", turn.key);
        if size(&rebuilt) > STATE_BUDGET {
            over_budget += 1;
        }
    }
    assert!(
        over_budget >= 30,
        "the budget binds on real states: {over_budget} over"
    );
}

/// The rungs of the ladder, largest contributor first: command output,
/// then commands, then turns, then prose per message.
fn ladder() -> Vec<(&'static str, Caps)> {
    let base = Caps::UNBUDGETED;
    vec![
        ("unbudgeted", base),
        (
            "output 512",
            Caps {
                output_bytes: 512,
                ..base
            },
        ),
        (
            "output 256",
            Caps {
                output_bytes: 256,
                ..base
            },
        ),
        (
            "commands 3",
            Caps {
                output_bytes: 256,
                commands: 3,
                ..base
            },
        ),
        (
            "turns 8",
            Caps {
                output_bytes: 256,
                commands: 3,
                turns: 8,
                ..base
            },
        ),
        (
            "turns 6",
            Caps {
                output_bytes: 256,
                commands: 3,
                turns: 6,
                ..base
            },
        ),
        (
            "turns 4",
            Caps {
                output_bytes: 256,
                commands: 3,
                turns: 4,
                ..base
            },
        ),
        (
            "turns 6, message 1024",
            Caps {
                output_bytes: 256,
                commands: 3,
                turns: 6,
                message_bytes: 1024,
            },
        ),
        ("production: turns 6, message 768", Caps::PRODUCTION),
        (
            "turns 6, message 512",
            Caps {
                output_bytes: 256,
                commands: 3,
                turns: 6,
                message_bytes: 512,
            },
        ),
        (
            "turns 4, message 512",
            Caps {
                output_bytes: 256,
                commands: 3,
                turns: 4,
                message_bytes: 512,
            },
        ),
    ]
}

/// The option an answer names, by the same rule `gym::eval::read_answer`
/// applies: a Noul is `yes` at or above one half, a Score is the argmax
/// level with a tie going to the last level listed.
fn chosen(answer: &Answer) -> String {
    match answer {
        Answer::Noul(noul) => noul
            .selected
            .clone()
            .unwrap_or_else(|| if noul.noul >= 0.5 { "yes" } else { "no" }.to_string()),
        Answer::Choice(choice) => choice.choice.clone(),
        Answer::Score(score) => score.selected.clone().unwrap_or_else(|| {
            score
                .probabilities
                .iter()
                .max_by(|left, right| left.1.total_cmp(right.1))
                .map(|(level, _)| level.to_string())
                .unwrap_or_default()
        }),
    }
}

fn sweep() {
    let rows_path = std::env::var("STATE_SWEEP_ROWS").map_or_else(
        |_| {
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/decision-models/2026-09-20-state-budget.jsonl")
        },
        PathBuf::from,
    );
    let client = Client::new(Config::new().base_url("http://127.0.0.1:11456").api_key("local-measurement").retry(jev::RetryPolicy { max_retries: 0, ..Default::default() }).timeout(Duration::from_secs(120)))
        .expect("TYPESAFE_API_KEY builds the client");
    let runtime = tokio::runtime::Runtime::new().expect("a runtime");
    let development: Vec<Turn> = turns()
        .into_iter()
        .filter(|turn| turn.partition == "development")
        .collect();
    assert_eq!(development.len(), 16);
    let mut rows: Vec<Value> = if rows_path.exists() {
        fs::read_to_string(&rows_path).expect("retained rows")
            .lines().map(|line| serde_json::from_str(line).expect("retained JSON row")).collect()
    } else { Vec::new() };
    for (rung, caps) in ladder() {
        let mut correct: BTreeMap<String, (usize, usize)> = BTreeMap::new();
        let mut sizes = Vec::new();
        let mut refused = 0;
        for turn in &development {
            let state = state(turn, &caps);
            sizes.push(size(&state));
            let held: Vec<&Value> = rows.iter().filter(|row| row["rung"] == rung && row["state"] == turn.key).collect();
            if !held.is_empty() {
                assert_eq!(held.len(), turn.truths.len(), "a retained request must be complete");
                for row in &held {
                    let family = row["family"].as_str().expect("family");
                    assert_eq!(row["truth"].as_str(), turn.truths.get(family).map(String::as_str));
                    assert_eq!(row["state_bytes"].as_u64(), Some(size(&state) as u64));
                    let tally = correct.entry(family.to_string()).or_default();
                    tally.1 += 1;
                    if row["correct"] == true { tally.0 += 1; }
                }
                if !held[0]["refusal"].is_null() { refused += 1; }
                continue;
            }
            let started = Instant::now();
            let answered = runtime
                .block_on(client.system_one(SystemOneRequest::new(state.clone(), questions())));
            let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
            match answered {
                Ok(response) => {
                    for (family, truth) in &turn.truths {
                        let answer = response.answers.get(family.as_str());
                        let got = answer.map(chosen);
                        let right = got.as_deref() == Some(truth.as_str());
                        let tally = correct.entry(family.clone()).or_default();
                        tally.1 += 1;
                        if right {
                            tally.0 += 1;
                        }
                        rows.push(json!({
                            "schema": "openagents.coder.state_sweep_row.v1",
                            "suite": "coder-turns-v1",
                            "partition": "development",
                            "door": "lev-adapted@1",
                            "model": response.model,
                            "rung": rung,
                            "caps": { "turns": caps.turns, "message_bytes": caps.message_bytes,
                                      "commands": caps.commands, "output_bytes": caps.output_bytes },
                            "state": turn.key, "state_bytes": size(&state),
                            "family": family, "truth": truth, "chosen": got,
                            "correct": right, "refusal": Value::Null, "latency_ms": latency_ms,
                        }));
                    }
                }
                Err(error) => {
                    if let jev::Error::Api(api) = &error {
                        eprintln!("{}", json!({
                            "diagnostic": "api_response_body",
                            "rung": rung,
                            "state": turn.key,
                            "status": api.status,
                            "body": api.body.as_ref().map(ToString::to_string),
                        }));
                    }
                    refused += 1;
                    let refusal = match &error {
                        jev::Error::Api(api) => format!("api {}: {:?}", api.status, api.kind),
                        other => other.to_string(),
                    };
                    for (family, truth) in &turn.truths {
                        correct.entry(family.clone()).or_default().1 += 1;
                        rows.push(json!({
                            "schema": "openagents.coder.state_sweep_row.v1",
                            "suite": "coder-turns-v1",
                            "partition": "development",
                            "door": "lev-adapted@1",
                            "model": Value::Null,
                            "rung": rung,
                            "caps": { "turns": caps.turns, "message_bytes": caps.message_bytes,
                                      "commands": caps.commands, "output_bytes": caps.output_bytes },
                            "state": turn.key, "state_bytes": size(&state),
                            "family": family, "truth": truth, "chosen": Value::Null,
                            "correct": false, "refusal": refusal, "latency_ms": latency_ms,
                        }));
                    }
                }
            }
            let checkpoint = rows_path.with_extension("jsonl.tmp");
            fs::write(&checkpoint, rows.iter().map(|row| format!("{row}\n")).collect::<String>()).expect("checkpoint rows");
            fs::rename(&checkpoint, &rows_path).expect("replace checkpoint");
            println!("saved {rung}: {} ({} rows)", turn.key, rows.len());
        }
        fs::write(&rows_path, rows.iter().map(|row| format!("{row}\n")).collect::<String>()).expect("checkpoint rows");
        sizes.sort_unstable();
        let pooled: (usize, usize) = correct
            .values()
            .fold((0, 0), |sum, tally| (sum.0 + tally.0, sum.1 + tally.1));
        println!(
            "{rung:<22} median {:>6} B  max {:>6} B  pooled {}/{}  refused {refused}  {}",
            sizes[sizes.len() / 2],
            sizes[sizes.len() - 1],
            pooled.0,
            pooled.1,
            correct
                .iter()
                .map(|(family, (right, total))| format!("{family} {right}/{total}"))
                .collect::<Vec<_>>()
                .join("  ")
        );
    }
    let text: String = rows.iter().map(|row| format!("{row}\n")).collect();
    fs::write(&rows_path, text).expect("the rows are written");
    println!("{} rows written to {}", rows.len(), rows_path.display());
}
use indexmap::IndexMap;
use jev::{Choice, Entry, Noul, Questions, Score};
pub fn questions() -> Questions {
    Questions::new()
        .with(
            "action",
            Choice::new(
                "Based on the conversation so far, what is the single next best step?",
                IndexMap::from([
                    (
                        "respond".to_string(),
                        Some(Entry::from(
                            "Answer the user's message directly — the intent is clear and no code is needed yet",
                        )),
                    ),
                    (
                        "clarify".to_string(),
                        Some(Entry::from(
                            "Ask one short clarifying question — the request is ambiguous and a wrong answer wastes the turn",
                        )),
                    ),
                    (
                        "end_conversation".to_string(),
                        Some(Entry::from(
                            "The user is done: a goodbye, a thanks, or an explicit quit",
                        )),
                    ),
                    (
                        "none".to_string(),
                        Some(Entry::from(
                            "No listed step fits: the state does not support a next action",
                        )),
                    ),
                ]),
            ),
        )
        .with(
            "needs_code",
            Noul::new(
                "Does the user's request need code written, or files in a repository inspected or changed?",
            ),
        )
        .with(
            "risk",
            Score::new(
                "How much can the chosen next step damage?",
                vec![
                    Some(Entry::from("0: answer in prose; nothing changes")),
                    Some(Entry::from("1: reads files or runs a reversible command")),
                    Some(Entry::from("2: writes files or could break a build")),
                ],
            ),
        )
        .with(
            "progress",
            Score::new(
                "How close is the conversation to a resolved end?",
                vec![
                    Some(Entry::from("0: just started")),
                    Some(Entry::from("1: underway, intent understood")),
                    Some(Entry::from("2: resolved or resolving now")),
                ],
            ),
        )
}

fn main() { every_real_turn_state_fits_the_budget(); sweep(); }
