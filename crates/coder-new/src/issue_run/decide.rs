//! The decision steps: from the issue to the files and the briefing.
//!
//! 1. The context finder of #11210 (`scripts/filefind/filefind.py query
//!    --json`) runs its deterministic candidate stages and its learned
//!    scorer at the base commit. Each stage shows as a card with its
//!    candidates and their reasons, then the scorer's ranking.
//! 2. System One questions through `crates/jev`: what kind of change the
//!    issue asks for (a Choice), and, for each of the scorer's top files,
//!    whether the fix needs to change it (a Noul). Each card shows the
//!    question, the options and their probabilities, the answer, the door
//!    and model that answered, the latency and the cost.
//! 3. The ranked files the briefing will list, then the briefing itself,
//!    built by #11211's generator (`scripts/coder/issue_run_briefing.py`).
//!
//! When the finder's script is missing or fails, the briefing falls back to
//! `briefing.py`'s own `lite` finder, and the card says so.

use std::{path::Path, time::Instant};

use serde_json::{Value, json};

use super::{Decider, Options, Transcript, setup::Base, setup::output};

/// A per-file probability below this drops the file from the briefing.
const KEEP_AT: f64 = 0.35;
/// The fewest files a briefing keeps whatever the answers say.
const MIN_FILES: usize = 3;

/// One check the briefing names.
#[derive(Clone, Debug)]
pub struct Check {
    pub id: String,
    pub argv: Vec<String>,
    pub filter: bool,
    pub what: String,
}

/// What the decision steps hand the agent.
#[derive(Clone, Debug, Default)]
pub struct Briefing {
    pub files: Vec<String>,
    pub checks: Vec<Check>,
    pub system: String,
    pub prompt: String,
    pub markdown: String,
    /// What the System One questions cost.
    pub decision_usd: f64,
}

/// The finder's candidate stages, in the order it runs them, each with
/// what it looks at.
const STAGES: [(&str, &str); 9] = [
    (
        "emb",
        "Embedding similarity between the issue and each file's path and head",
    ),
    (
        "sym",
        "Paths, file and crate names, identifiers and strings the issue names",
    ),
    (
        "co",
        "Files that changed together with the strongest candidates",
    ),
    (
        "sim",
        "Files the fixes of the most similar past issues changed",
    ),
    ("hist", "Commit subjects like the issue's text"),
    (
        "pair",
        "Paired files: tests, mod lines, Cargo.toml, the crate's tests folder",
    ),
    ("dir", "Other files in a candidate's folder"),
    ("recent", "Files changed in the newest commits"),
    (
        "stage2",
        "A second scorer pass over the first pass's neighbors",
    ),
];

/// Runs every decision step; `None` when the run cannot go on.
pub async fn decide(
    options: &Options,
    base: &Base,
    folder: &Path,
    transcript: &mut Transcript,
) -> Option<Briefing> {
    let found = finder(base, folder, transcript).await;
    let ranked: Vec<Value> = found
        .as_ref()
        .and_then(|found| found["files"].as_array().cloned())
        .unwrap_or_default();
    let mut decision_usd = 0.0;
    let kind = issue_kind(options, base, transcript, &mut decision_usd).await;
    let judged = file_questions(options, base, &ranked, transcript, &mut decision_usd).await;
    let chosen = choose(options, &ranked, &judged, transcript);
    let mut briefing = brief(options, base, &chosen, kind.as_deref(), folder, transcript).await?;
    briefing.decision_usd = decision_usd;
    Some(briefing)
}

/// The finder's answer, with a card per stage and one for the scorer.
async fn finder(base: &Base, folder: &Path, transcript: &mut Transcript) -> Option<Value> {
    let script = super::tools_root(&base.repo).join("scripts/filefind/filefind.py");
    let card = json!({
        "kind": "stage",
        "title": "Context finder",
        "detail": "Deterministic candidate stages over the repository's history, then a learned scorer (#11210)",
        "rows": [],
    });
    let index = transcript.card(card.clone());
    let started = Instant::now();
    if !script.exists() {
        let mut card = card;
        card["rows"] = json!([{"label": "fallback", "text": "scripts/filefind/filefind.py is not in this checkout; the briefing uses its own lite finder"}]);
        transcript.finish(index, card, json!({"ok": true}));
        return None;
    }
    let mut command = tokio::process::Command::new("python3");
    command
        .arg(&script)
        .args(["query", "--repo"])
        .arg(&base.repo)
        .args([
            "--rev",
            &base.base,
            "--issue",
            &base.issue.to_string(),
            "--k",
            "40",
            "--json",
        ])
        .current_dir(&base.repo)
        .stdin(std::process::Stdio::null());
    if let Some(key) = super::secret("OPENROUTER_API_KEY", "openrouter.env") {
        command.env("OPENROUTER_API_KEY", key);
    }
    let result = command.output().await;
    let ms = started.elapsed().as_millis() as u64;
    let found = result
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| serde_json::from_slice::<Value>(&out.stdout).ok());
    let Some(found) = found else {
        let mut card = card;
        card["ms"] = json!(ms);
        card["rows"] = json!([{"label": "fallback", "text": "The finder did not answer; the briefing uses its own lite finder"}]);
        transcript.finish(index, card, json!({"ok": true}));
        return None;
    };
    let _ = std::fs::write(
        folder.join("filefind.json"),
        serde_json::to_vec_pretty(&found).unwrap_or_default(),
    );
    let files = found["files"].as_array().cloned().unwrap_or_default();
    let timing = &found["timing_ms"];
    let mut card = card;
    card["ms"] = json!(ms);
    card["rows"] = json!([
        {"label": "candidates", "text": format!("{} files ranked at {}", files.len(), super::setup::short(&base.base))},
        {"label": "time", "text": format!(
            "index {} ms · issue embedding {} ms · stages and scorer {} ms",
            timing["load"].as_u64().unwrap_or(0) + timing["tree"].as_u64().unwrap_or(0) + timing["refresh_index"].as_u64().unwrap_or(0),
            timing["embed_query"].as_u64().unwrap_or(0),
            timing["total"].as_u64().unwrap_or(0).saturating_sub(timing["embed_query"].as_u64().unwrap_or(0)),
        )},
    ]);
    transcript.finish(index, card, json!({"ok": true}));
    for (stage, what) in STAGES {
        let hits: Vec<&Value> = files
            .iter()
            .filter(|file| {
                file["stages"]
                    .as_array()
                    .is_some_and(|stages| stages.iter().any(|s| s.as_str() == Some(stage)))
            })
            .collect();
        if hits.is_empty() {
            continue;
        }
        let rows: Vec<Value> = hits
            .iter()
            .take(6)
            .map(|file| {
                json!({
                    "label": file["path"],
                    "text": reason_for(stage, file),
                })
            })
            .collect();
        let card = json!({
            "kind": "stage",
            "title": format!("Stage {stage}"),
            "detail": what,
            "count": hits.len(),
            "more": hits.len().saturating_sub(6),
            "ms": timing[stage].as_u64(),
            "rows": rows,
        });
        let index = transcript.card(card.clone());
        transcript.finish(index, card, json!({"ok": true}));
    }
    let rows: Vec<Value> = files
        .iter()
        .take(12)
        .map(|file| {
            json!({
                "rank": file["rank"],
                "path": file["path"],
                "p": file["confidence"],
            })
        })
        .collect();
    let card = json!({
        "kind": "ranked",
        "title": "Scorer ranks the candidates",
        "detail": format!("Learned scorer ({}): the probability that the fix touches each file", found["model"].as_str().unwrap_or("model.json")),
        "ms": timing["score"].as_u64(),
        "rows": rows,
        "more": files.len().saturating_sub(12),
    });
    let index = transcript.card(card.clone());
    transcript.finish(index, card, json!({"ok": true}));
    Some(found)
}

/// The reason a file gave for `stage`, else its first reason.
fn reason_for(stage: &str, file: &Value) -> String {
    let reasons: Vec<&str> = file["reasons"]
        .as_array()
        .map(|reasons| reasons.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    // Display only: the finder writes each stage's short name into the
    // reason it gives for that stage.
    let marker = match stage {
        "emb" => "embedding",
        "co" => "changes with",
        "sim" => "similar #",
        "sym" => "named",
        "pair" => "pair",
        "hist" => "commit",
        "dir" => "folder",
        "recent" => "recent",
        _ => "",
    };
    reasons
        .iter()
        .find(|reason| !marker.is_empty() && reason.contains(marker))
        .or(reasons.first())
        .map_or_else(|| "found by this stage".to_owned(), |r| (*r).to_owned())
}

/// A System One client and how to name it on a card.
struct Door {
    client: jev::Client,
    label: String,
    model: String,
}

fn jev_door() -> Option<Door> {
    let key = super::secret("TYPESAFE_API_KEY", "typesafe.env")?;
    let config = jev::Config::new()
        .api_key(key)
        .base_url("https://api.typesafe.ai")
        .default_model("jev-latest")
        .timeout(std::time::Duration::from_secs(30))
        .retry(jev::RetryPolicy {
            max_retries: 1,
            ..Default::default()
        });
    Some(Door {
        client: jev::Client::new(config).ok()?,
        label: "Jev, TypeSafe API".into(),
        model: "jev-latest".into(),
    })
}

/// Our decision API (#11225): connected Pylons first, then Gemini on
/// Vertex, keyless.
fn ours_door() -> Option<Door> {
    let env = |name: &str| std::env::var(name).ok();
    let resolved = jev_hosted::ours(
        &env,
        &jev_hosted::Door {
            url: jev_hosted::OPENAGENTS,
            model: "jev-latest",
        },
        &|config| config.timeout(std::time::Duration::from_secs(30)),
    )
    .ok()?;
    Some(Door {
        client: resolved.client,
        label: "OpenAgents decisions".into(),
        model: "jev-latest".into(),
    })
}

fn clef_door() -> Option<Door> {
    let address = std::net::SocketAddr::from(([127, 0, 0, 1], 11434));
    std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_millis(500)).ok()?;
    let config = jev::Config::local("http://127.0.0.1:11434", "clef-flash")
        .timeout(std::time::Duration::from_secs(120));
    Some(Door {
        client: jev::Client::new(config).ok()?,
        label: "Clef-Flash, Ollama on this computer".into(),
        model: "clef-flash".into(),
    })
}

/// The door for the issue's kind, then the one for the per-file questions.
fn doors(decider: Decider) -> (Option<Door>, Option<Door>) {
    match decider {
        Decider::Off => (None, None),
        Decider::Jev => (jev_door(), jev_door()),
        Decider::Clef => (clef_door(), clef_door()),
        Decider::Auto => (ours_door(), ours_door()),
    }
}

fn issue_state(base: &Base) -> String {
    let body: String = base.body.chars().take(4000).collect();
    format!("Issue #{}: {}\n\n{}", base.issue, base.title, body)
}

/// The door's own name for itself when it relays, else the card's label.
fn answered_by(door: &Door, response: &jev::SystemOneResponse) -> String {
    response
        .service()
        .and_then(|service| service["door"].as_str().map(str::to_owned))
        .map_or_else(
            || door.label.clone(),
            |relay| format!("{} via {relay}", door.label),
        )
}

/// The issue's kind, as a Choice.
async fn issue_kind(
    options: &Options,
    base: &Base,
    transcript: &mut Transcript,
    spent: &mut f64,
) -> Option<String> {
    const KINDS: [(&str, &str); 5] = [
        ("bug", "Something that should work does not: fix it"),
        ("feature", "New behavior or a new surface"),
        ("docs", "Documentation or text only"),
        ("refactor", "Restructure code without changing behavior"),
        ("config", "Build, release, settings or dependency change"),
    ];
    let question = "What kind of change does this issue ask for?";
    let (door, _) = doors(options.decider);
    let mut card = json!({
        "kind": "model",
        "title": "Issue kind",
        "question": question,
        "used_for": "Recorded with the run; the agent's instructions are the same for every kind today",
        "options": KINDS.iter().map(|(name, _)| json!({"name": name})).collect::<Vec<_>>(),
    });
    let Some(door) = door else {
        card["skipped"] =
            json!("No System One door is set up (TYPESAFE_API_KEY or Ollama with clef-flash)");
        let index = transcript.card(card.clone());
        transcript.finish(index, card, json!({"ok": true}));
        return None;
    };
    card["door"] = json!(door.label);
    card["model"] = json!(door.model);
    let index = transcript.card(card.clone());
    let mut choice = jev::Choice::default();
    for (name, about) in KINDS {
        choice = choice.option(name, about);
    }
    choice.instructions = Some(question.into());
    let request = jev::SystemOneRequest::new(
        issue_state(base),
        jev::Questions::new().with("kind", choice),
    );
    let started = Instant::now();
    let result = door.client.system_one(request).await;
    card["ms"] = json!(started.elapsed().as_millis() as u64);
    match result {
        Ok(response) => {
            card["door"] = json!(answered_by(&door, &response));
            card["model"] = json!(response.model);
            let cost = response.usage.cost_usd();
            *spent += cost.unwrap_or(0.0);
            card["cost_usd"] = json!(cost);
            card["tokens"] = json!(response.usage.input_tokens);
            let Ok(answer) = response.choice("kind") else {
                transcript.finish(index, card, json!({"error": "The answer had no kind."}));
                return None;
            };
            card["options"] = json!(
                KINDS
                    .iter()
                    .map(|(name, _)| json!({
                        "name": name,
                        "p": answer.probabilities.get(*name).copied().unwrap_or(0.0),
                        "chosen": answer.choice == *name,
                    }))
                    .collect::<Vec<_>>()
            );
            card["chosen"] = json!(answer.choice);
            let chosen = answer.choice.clone();
            transcript.finish(index, card, json!({"ok": true}));
            Some(chosen)
        }
        Err(error) => {
            transcript.finish(index, card, json!({"error": error.to_string()}));
            None
        }
    }
}

/// For the scorer's top files, whether the fix needs each one (a Noul per
/// file). Returns `path -> probability` for the files that were answered.
async fn file_questions(
    options: &Options,
    base: &Base,
    ranked: &[Value],
    transcript: &mut Transcript,
    spent: &mut f64,
) -> Vec<(String, f64)> {
    let question = "Does the fix for this issue need to change this file?";
    let asked: Vec<String> = ranked
        .iter()
        .take((options.files * 2).clamp(6, 12))
        .filter_map(|file| file["path"].as_str().map(str::to_owned))
        .collect();
    if asked.is_empty() {
        return Vec::new();
    }
    let (_, door) = doors(options.decider);
    let mut card = json!({
        "kind": "model",
        "title": "Does the fix need each file?",
        "question": question,
        "used_for": format!("A file under {KEEP_AT} leaves the briefing (at least {MIN_FILES} stay)"),
        "files": asked.iter().map(|path| json!({"name": path})).collect::<Vec<_>>(),
    });
    let Some(door) = door else {
        card["skipped"] = json!("No System One door is set up; the scorer's ranking stands");
        let index = transcript.card(card.clone());
        transcript.finish(index, card, json!({"ok": true}));
        return Vec::new();
    };
    card["door"] = json!(door.label);
    card["model"] = json!(door.model);
    let index = transcript.card(card.clone());
    let started = Instant::now();
    let mut answers = Vec::new();
    let mut rows: Vec<Value> = asked.iter().map(|path| json!({"name": path})).collect();
    let mut cost_total = 0.0;
    let mut tokens = 0u64;
    let mut last_door = door.label.clone();
    for (row, path) in asked.iter().enumerate() {
        if transcript.cancelled() {
            break;
        }
        let head = output(
            "git",
            &["show", &format!("{}:{path}", base.base)],
            &base.repo,
        )
        .await
        .map(|text| text.lines().take(80).collect::<Vec<_>>().join("\n"))
        .unwrap_or_else(|_| "(the file does not exist at this commit)".into());
        let state = format!("{}\n\nFile: {path}\n\n{head}", issue_state(base));
        let request = jev::SystemOneRequest::new(
            state,
            jev::Questions::new().with("relevant", jev::Noul::new(question)),
        );
        let asked_at = Instant::now();
        match door.client.system_one(request).await {
            Ok(response) => {
                let ms = asked_at.elapsed().as_millis() as u64;
                last_door = answered_by(&door, &response);
                let cost = response.usage.cost_usd().unwrap_or(0.0);
                cost_total += cost;
                tokens += response.usage.input_tokens.unwrap_or(0);
                if let Ok(answer) = response.noul("relevant") {
                    rows[row] = json!({
                        "name": path,
                        "p": answer.noul,
                        "chosen": if answer.noul >= KEEP_AT { "yes" } else { "no" },
                        "ms": ms,
                    });
                    answers.push((path.clone(), answer.noul));
                }
            }
            Err(error) => {
                rows[row] = json!({"name": path, "error": error.to_string()});
            }
        }
        card["files"] = json!(rows);
        card["door"] = json!(last_door);
        card["ms"] = json!(started.elapsed().as_millis() as u64);
        card["cost_usd"] = json!(cost_total);
        card["tokens"] = json!(tokens);
        transcript.set(
            index,
            crate::live::Entry::Tool {
                name: super::DECISION.into(),
                input: card.clone(),
                output: Value::Null,
                running: true,
            },
        );
    }
    *spent += cost_total;
    transcript.finish(index, card, json!({"ok": true}));
    answers
}

/// The ranked files the briefing lists: the scorer's order, minus files
/// the per-file answers ruled out.
fn choose(
    options: &Options,
    ranked: &[Value],
    judged: &[(String, f64)],
    transcript: &mut Transcript,
) -> Vec<Value> {
    let p_of = |path: &str| judged.iter().find(|(p, _)| p == path).map(|(_, p)| *p);
    // When files were asked about, only they compete: an unasked file
    // further down the scorer's list never jumps over an asked one.
    let pool: Vec<&Value> = if judged.is_empty() {
        ranked.iter().collect()
    } else {
        ranked
            .iter()
            .filter(|file| p_of(file["path"].as_str().unwrap_or_default()).is_some())
            .collect()
    };
    let mut kept = Vec::new();
    let mut dropped = Vec::new();
    for file in pool {
        let path = file["path"].as_str().unwrap_or_default();
        match p_of(path) {
            Some(p) if p < KEEP_AT => dropped.push(file),
            _ => kept.push(file),
        }
    }
    // Keep the fewest files the briefing needs, refilling from the dropped
    // ones in the scorer's order.
    while kept.len() < MIN_FILES.min(ranked.len()) && !dropped.is_empty() {
        kept.push(dropped.remove(0));
    }
    kept.truncate(options.files);
    let rows: Vec<Value> = kept
        .iter()
        .enumerate()
        .map(|(rank, file)| {
            let path = file["path"].as_str().unwrap_or_default();
            json!({
                "rank": rank + 1,
                "path": path,
                "p": file["confidence"],
                "judged": p_of(path),
            })
        })
        .collect();
    let card = json!({
        "kind": "ranked",
        "title": "Files for the briefing",
        "detail": format!("The scorer's order; {} ruled out by the answers", dropped.len()),
        "rows": rows,
    });
    let index = transcript.card(card.clone());
    transcript.finish(index, card, json!({"ok": true}));
    kept.into_iter()
        .map(|file| {
            let reasons = file["reasons"].clone();
            json!({
                "path": file["path"],
                "score": file["confidence"],
                "why": reasons,
            })
        })
        .collect()
}

/// The briefing, from #11211's generator.
async fn brief(
    options: &Options,
    base: &Base,
    files: &[Value],
    kind: Option<&str>,
    folder: &Path,
    transcript: &mut Transcript,
) -> Option<Briefing> {
    let mut card = json!({
        "kind": "briefing",
        "title": "Briefing",
        "detail": "Plan, files with excerpts, similar past changes, checks and repo rules (#11211's generator)",
    });
    let index = transcript.card(card.clone());
    let started = Instant::now();
    let script = super::tools_root(&base.repo).join("scripts/coder/issue_run_briefing.py");
    let request = json!({
        "repo": base.repo,
        "issue": base.issue,
        "title": base.title,
        "body": base.body,
        "base": base.base,
        "files": files,
        "kind": kind,
        "levers": if files.is_empty() { json!({"finder": "lite", "briefing_files": options.files}) } else { json!({}) },
    });
    let answer = run_with_input(&script, &request, &base.repo).await;
    card["ms"] = json!(started.elapsed().as_millis() as u64);
    let answer = match answer {
        Ok(answer) => answer,
        Err(why) => {
            transcript.finish(index, card, json!({"error": why}));
            return None;
        }
    };
    let built = &answer["briefing"];
    let markdown = answer["markdown"].as_str().unwrap_or_default().to_owned();
    let _ = std::fs::write(folder.join("briefing.md"), &markdown);
    let checks: Vec<Check> = built["checks"]
        .as_array()
        .map(|checks| {
            checks
                .iter()
                .map(|check| Check {
                    id: check["id"].as_str().unwrap_or_default().to_owned(),
                    argv: check["argv"]
                        .as_array()
                        .map(|argv| {
                            argv.iter()
                                .filter_map(|a| a.as_str().map(str::to_owned))
                                .collect()
                        })
                        .unwrap_or_default(),
                    filter: check["filter"].as_bool().unwrap_or(false),
                    what: check["what"].as_str().unwrap_or_default().to_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    let listed: Vec<String> = built["files"]
        .as_array()
        .map(|files| {
            files
                .iter()
                .filter_map(|file| file["path"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default();
    card["finder"] = built["finder"].clone();
    card["chars"] = json!(markdown.chars().count());
    card["files"] = json!(listed);
    card["checks"] = json!(checks.iter().map(|c| c.id.clone()).collect::<Vec<_>>());
    card["plan"] = built["plan"].clone();
    card["history"] = json!(
        built["history"]
            .as_array()
            .map(|h| h
                .iter()
                .map(|c| format!(
                    "{} {}",
                    c["commit"].as_str().unwrap_or(""),
                    c["subject"].as_str().unwrap_or("")
                ))
                .collect::<Vec<_>>())
            .unwrap_or_default()
    );
    card["path"] = json!(folder.join("briefing.md").display().to_string());
    transcript.finish(index, card, json!({"ok": true}));
    Some(Briefing {
        files: listed,
        checks,
        system: answer["system"].as_str().unwrap_or_default().to_owned(),
        prompt: answer["prompt"].as_str().unwrap_or_default().to_owned(),
        markdown,
        decision_usd: 0.0,
    })
}

/// Runs `python3 script` with `input` as JSON on standard input and reads
/// its JSON answer.
async fn run_with_input(script: &Path, input: &Value, cwd: &Path) -> Result<Value, String> {
    use tokio::io::AsyncWriteExt;
    if !script.exists() {
        return Err(format!("{} is not in this checkout.", script.display()));
    }
    let mut child = tokio::process::Command::new("python3")
        .arg(script)
        .current_dir(cwd)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|_| "Cannot run python3.".to_owned())?;
    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(input.to_string().as_bytes()).await;
    }
    let out = child
        .wait_with_output()
        .await
        .map_err(|_| "The briefing script did not finish.".to_owned())?;
    if !out.status.success() {
        let error = String::from_utf8_lossy(&out.stderr);
        let line = error
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("no message");
        return Err(format!("The briefing script failed: {line}"));
    }
    serde_json::from_slice(&out.stdout)
        .map_err(|_| "The briefing script's answer did not parse.".to_owned())
}
