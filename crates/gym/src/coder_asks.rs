//! `gym coder asks`: the questions `coder-one ask` answered, with what
//! each cost and how many of its citations checked.
//!
//! Every ask records itself under `~/.openagents/coder-one/asks/ask-<ms>/`:
//! a manifest (`openagents.coder-one.ask.v1`), the ATIF invocation log, the
//! briefing, the executor's stream, and the answer. This view reads the
//! manifests; `gym coder asks ID` prints one answer with its claims and
//! the marks the citation check gave them.
//!
//! `coder-one ask study` measures the ask on a question set whose answers
//! are written down, and records an `openagents.coder-one.ask-study.v1`
//! result under `~/.openagents/coder-one/ask-studies/` or retained under
//! `bench/terminal-bench/asks/studies/`. `gym coder asks --studies` lists
//! them side by side, and `--study ID` prints one study's rows, so a change
//! to the probes, the questions, or the executor compares against the
//! baseline like any other component.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::runs::{clip_words, duration, money, now_ms, when};

/// The manifest schema an ask writes.
pub const ASK_SCHEMA: &str = "openagents.coder-one.ask.v1";

/// This view's JSON schema.
pub const SCHEMA: &str = "openagents.gym.coder-asks.v1";

/// An ask study's result schema.
pub const STUDY_SCHEMA: &str = "openagents.coder-one.ask-study.v1";

const HELP: &str = "\
gym coder asks [ID|latest] [--dir PATH] [--limit N] [--json]
gym coder asks --studies [--study ID] [--studies-dir PATH] [--json]

The questions `coder-one ask` answered, newest first: when, the executor,
whether it answered, how many citations checked and claims were verified,
the cost, the time, and the question. ID (or a prefix, or `latest`) prints
one ask's answer with each claim marked ✓ when its citations checked and ?
when one didn't. Asks are read from ~/.openagents/coder-one/asks unless
--dir names another directory.

--studies lists the ask studies `coder-one ask study` recorded, local ones
from ~/.openagents/coder-one/ask-studies and retained ones from
bench/terminal-bench/asks/studies: the question set, the executor, the
citation validity, the recall of the expected tasks and runs, the cost, and
the mean time. --study ID prints one study's rows.";

/// Where asks are recorded.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/asks"))
}

/// Every ask's manifest under `dir`, newest first.
#[must_use]
pub fn load(dir: &Path) -> Vec<Value> {
    let mut asks: Vec<Value> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let text = std::fs::read_to_string(entry.path().join("manifest.json")).ok()?;
            let value: Value = serde_json::from_str(&text).ok()?;
            (value["schema"] == ASK_SCHEMA).then_some(value)
        })
        .collect();
    asks.sort_by(|a, b| b["id"].as_str().cmp(&a["id"].as_str()));
    asks
}

/// The local and retained study directories.
#[must_use]
pub fn study_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = std::env::var_os("HOME").filter(|home| !home.is_empty()) {
        dirs.push(PathBuf::from(home).join(".openagents/coder-one/ask-studies"));
    }
    dirs.push(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/asks/studies"),
    );
    dirs
}

/// Every ask study under `dirs`, one per study id, local first, then
/// newest first.
#[must_use]
pub fn load_studies(dirs: &[PathBuf]) -> Vec<Value> {
    let mut studies: Vec<Value> = Vec::new();
    for dir in dirs {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path().join("result.json"))
            .filter(|path| path.is_file())
            .collect();
        paths.sort();
        for path in paths {
            let Some(value) = std::fs::read_to_string(&path)
                .ok()
                .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            else {
                continue;
            };
            if value["schema"] == STUDY_SCHEMA
                && !studies.iter().any(|study| study["study"] == value["study"])
            {
                studies.push(value);
            }
        }
    }
    studies.sort_by(|a, b| b["study"].as_str().cmp(&a["study"].as_str()));
    studies
}

fn fraction(value: &Value) -> String {
    value
        .as_f64()
        .map_or_else(|| "—".to_owned(), |v| format!("{v:.2}"))
}

fn studies_lines(studies: &[Value]) -> Vec<String> {
    let mut lines = vec![
        format!("Coder One ask studies: {}", studies.len()),
        String::new(),
        format!(
            "{:<26} {:<18} {:<18} {:>8} {:>9} {:>6} {:>6} {:>8} {:>7}  {}",
            "study",
            "set",
            "executor",
            "answered",
            "citations",
            "tasks",
            "runs",
            "cost",
            "mean",
            "implementation"
        ),
    ];
    for study in studies {
        let t = &study["totals"];
        lines.push(format!(
            "{:<26} {:<18} {:<18} {:>8} {:>9} {:>6} {:>6} {:>8} {:>6.1}s  {}",
            study["study"].as_str().unwrap_or("?"),
            study["set"].as_str().unwrap_or("?"),
            format!(
                "{} {}",
                study["executor"].as_str().unwrap_or("?"),
                study["model"].as_str().unwrap_or("")
            ),
            format!("{}/{}", t["answered"], t["questions"]),
            fraction(&t["citation_validity"]),
            fraction(&t["mean_task_recall"]),
            fraction(&t["mean_run_recall"]),
            t["cost_usd"].as_f64().map_or_else(|| "—".to_owned(), money),
            t["mean_milliseconds"].as_f64().unwrap_or(0.0) / 1000.0,
            study["implementation_digest"]
                .as_str()
                .map_or("?", |digest| &digest[..digest.len().min(12)]),
        ));
    }
    lines.push(String::new());
    lines.push(
        "citations is the share of citations the check held; tasks and runs are the mean recall of the expected ones."
            .to_owned(),
    );
    lines
}

fn study_lines(study: &Value) -> Vec<String> {
    let mut lines = vec![
        format!(
            "{} · {} · {} {}",
            study["study"].as_str().unwrap_or("?"),
            study["set"].as_str().unwrap_or("?"),
            study["executor"].as_str().unwrap_or("?"),
            study["model"].as_str().unwrap_or("")
        ),
        String::new(),
        format!(
            "{:<32} {:>9} {:>6} {:>6} {:>8} {:>7}  missed tasks",
            "question", "citations", "tasks", "runs", "cost", "time"
        ),
    ];
    for row in study["rows"].as_array().into_iter().flatten() {
        let missed: Vec<&str> = row["tasks_missed"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        lines.push(format!(
            "{:<32} {:>9} {:>6} {:>6} {:>8} {:>6.1}s  {}",
            clip_words(row["id"].as_str().unwrap_or("?"), 32),
            format!("{}/{}", row["valid_citations"], row["citations"]),
            fraction(&row["task_recall"]),
            fraction(&row["run_recall"]),
            row["cost_usd"]
                .as_f64()
                .map_or_else(|| "—".to_owned(), money),
            row["milliseconds"].as_f64().unwrap_or(0.0) / 1000.0,
            if row["answered"] == true {
                missed.join(", ")
            } else {
                format!("no answer: {}", row["error"].as_str().unwrap_or("?"))
            }
        ));
    }
    let t = &study["totals"];
    lines.push(String::new());
    lines.push(format!(
        "{} of {} answered · citations {}/{} valid · task recall {} · run recall {} · {} · {} under a minute",
        t["answered"],
        t["questions"],
        t["valid_citations"],
        t["citations"],
        fraction(&t["mean_task_recall"]),
        fraction(&t["mean_run_recall"]),
        t["cost_usd"].as_f64().map_or_else(|| "—".to_owned(), money),
        t["under_a_minute"],
    ));
    lines
}

fn started_ms(ask: &Value) -> Option<i64> {
    ask["id"].as_str()?.strip_prefix("ask-")?.parse().ok()
}

/// One ask as the list's JSON carries it.
#[must_use]
pub fn summary(ask: &Value) -> Value {
    json!({
        "id": ask["id"],
        "question": ask["question"],
        "scope": ask["scope"],
        "executor": ask["executor"],
        "status": ask["status"],
        "claims": ask["citations"]["claims"],
        "verified": ask["citations"]["verified"],
        "citations": ask["citations"]["citations"],
        "valid_citations": ask["citations"]["valid_citations"],
        "validity": ask["citations"]["validity"],
        "cost_usd": ask["cost"]["usd"],
        "jev_usd": ask["cost"]["jev_usd"],
        "executor_usd": ask["cost"]["executor_usd"],
        "milliseconds": ask["milliseconds"],
        "dir": ask["dir"],
    })
}

fn list_lines(asks: &[Value], now: i64) -> Vec<String> {
    let mut lines = vec![format!("Coder One asks, newest first: {}", asks.len())];
    lines.push(String::new());
    for ask in asks {
        let citations = &ask["citations"];
        lines.push(format!(
            "{:<10} {:<5} {:<9} {:>5} {:>5} {:>8} {:>7}  {}",
            started_ms(ask).map_or_else(|| "—".to_owned(), |ms| when(ms, now)),
            ask["executor"].as_str().unwrap_or("?"),
            ask["status"].as_str().unwrap_or("?"),
            format!(
                "{}/{}",
                citations["valid_citations"], citations["citations"]
            ),
            format!("{}/{}", citations["verified"], citations["claims"]),
            ask["cost"]["usd"]
                .as_f64()
                .map_or_else(|| "—".to_owned(), money),
            ask["milliseconds"]
                .as_u64()
                .map_or_else(String::new, duration),
            clip_words(ask["question"].as_str().unwrap_or(""), 80),
        ));
    }
    lines.push(String::new());
    lines.push(
        "Columns: when, executor, status, citations checked, claims verified, cost, time, question."
            .to_owned(),
    );
    lines.push("See one: gym coder asks ID".to_owned());
    lines
}

fn one_lines(ask: &Value) -> Vec<String> {
    let mut lines = vec![
        format!(
            "{} · {}",
            ask["id"].as_str().unwrap_or("?"),
            ask["executor"].as_str().unwrap_or("?")
        ),
        format!("Question: {}", ask["question"].as_str().unwrap_or("")),
        String::new(),
    ];
    match ask["answer"].as_str() {
        Some(answer) => lines.extend(answer.lines().map(str::to_owned)),
        None => lines.push(format!(
            "No answer: {}",
            ask["status"].as_str().unwrap_or("?")
        )),
    }
    lines.push(String::new());
    for (index, claim) in ask["claims"].as_array().into_iter().flatten().enumerate() {
        let mark = if claim["verified"] == true {
            '✓'
        } else {
            '?'
        };
        lines.push(format!(
            "{mark} {}. {}",
            index + 1,
            claim["claim"].as_str().unwrap_or("")
        ));
        for problem in claim["problems"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            lines.push(format!("     unverified: {problem}"));
        }
    }
    let proposals = ask["proposals"].as_array().cloned().unwrap_or_default();
    if !proposals.is_empty() {
        lines.push(String::new());
        lines.push("Proposals (✓ validated, ✗ refused); see gym coder proposals ID:".to_owned());
        for proposal in &proposals {
            lines.push(format!(
                "{} {} [{}] {}",
                if proposal["valid"] == true {
                    '✓'
                } else {
                    '✗'
                },
                proposal["id"].as_str().unwrap_or("?"),
                proposal["kind"].as_str().unwrap_or("?"),
                proposal["title"].as_str().unwrap_or("")
            ));
        }
    }
    let citations = &ask["citations"];
    lines.push(String::new());
    lines.push(format!(
        "{} of {} citations check; {} of {} claims verified. Cost {}; time {}.",
        citations["valid_citations"],
        citations["citations"],
        citations["verified"],
        citations["claims"],
        ask["cost"]["usd"]
            .as_f64()
            .map_or_else(|| "unknown".to_owned(), money),
        ask["milliseconds"]
            .as_u64()
            .map_or_else(String::new, duration),
    ));
    lines.push(format!("Recorded: {}", ask["dir"].as_str().unwrap_or("")));
    lines
}

/// `gym coder asks …`.
///
/// # Errors
///
/// Returns a message when an argument doesn't parse or the named ask isn't
/// found.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let mut dir = default_dir();
    let mut json_output = false;
    let mut limit = 40usize;
    let mut id: Option<String> = None;
    let mut studies = false;
    let mut study: Option<String> = None;
    let mut studies_dirs = study_dirs();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            "--json" => json_output = true,
            "--studies" => studies = true,
            "--study" => {
                studies = true;
                study = Some(args.get(index + 1).ok_or("--study needs a value")?.clone());
                index += 1;
            }
            "--studies-dir" => {
                studies = true;
                studies_dirs = vec![PathBuf::from(
                    args.get(index + 1).ok_or("--studies-dir needs a value")?,
                )];
                index += 1;
            }
            "--dir" => {
                dir = Some(PathBuf::from(
                    args.get(index + 1).ok_or("--dir needs a value")?,
                ));
                index += 1;
            }
            "--limit" => {
                limit = args
                    .get(index + 1)
                    .and_then(|n| n.parse().ok())
                    .ok_or("--limit needs a number")?;
                index += 1;
            }
            other if !other.starts_with("--") && id.is_none() => id = Some(other.to_owned()),
            other => return Err(format!("unknown option {other}\n\n{HELP}")),
        }
        index += 1;
    }
    let write = |out: &mut dyn std::io::Write, text: &str| {
        writeln!(out, "{text}").map_err(|error| error.to_string())
    };
    if studies {
        let loaded = load_studies(&studies_dirs);
        if let Some(id) = study {
            let one = loaded
                .iter()
                .find(|s| {
                    s["study"]
                        .as_str()
                        .is_some_and(|name| name.starts_with(&id))
                })
                .ok_or_else(|| format!("no ask study matches {id}"))?;
            if json_output {
                write(
                    out,
                    &serde_json::to_string_pretty(one).map_err(|e| e.to_string())?,
                )?;
            } else {
                for line in study_lines(one) {
                    write(out, &line)?;
                }
            }
            return Ok(0);
        }
        if json_output {
            let value = json!({
                "schema": "openagents.gym.coder-ask-studies.v1",
                "studies": loaded.iter().map(|s| json!({
                    "study": s["study"],
                    "set": s["set"],
                    "set_digest": s["set_digest"],
                    "executor": s["executor"],
                    "model": s["model"],
                    "implementation_digest": s["implementation_digest"],
                    "totals": s["totals"],
                })).collect::<Vec<_>>(),
            });
            write(
                out,
                &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
            )?;
        } else {
            for line in studies_lines(&loaded) {
                write(out, &line)?;
            }
        }
        return Ok(0);
    }
    let dir = dir.ok_or("no --dir and no HOME")?;
    let asks = load(&dir);
    if let Some(id) = id {
        let ask = if id == "latest" {
            asks.first()
        } else {
            asks.iter()
                .find(|ask| ask["id"].as_str().is_some_and(|name| name.starts_with(&id)))
        }
        .ok_or_else(|| format!("no ask matches {id} in {}", dir.display()))?;
        if json_output {
            write(
                out,
                &serde_json::to_string_pretty(ask).map_err(|e| e.to_string())?,
            )?;
        } else {
            for line in one_lines(ask) {
                write(out, &line)?;
            }
        }
        return Ok(0);
    }
    let shown: Vec<Value> = asks.into_iter().take(limit).collect();
    if json_output {
        let value = json!({
            "schema": SCHEMA,
            "dir": dir,
            "asks": shown.iter().map(summary).collect::<Vec<_>>(),
        });
        write(
            out,
            &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
        )?;
        return Ok(0);
    }
    for line in list_lines(&shown, now_ms()) {
        write(out, &line)?;
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn studies_list_side_by_side_and_one_prints_its_rows() {
        let dir = tempfile::tempdir().unwrap();
        for (id, validity) in [("ask-study-1", 1.0), ("ask-study-2", 0.9)] {
            let study = dir.path().join(id);
            std::fs::create_dir_all(&study).unwrap();
            let result = json!({
                "schema": STUDY_SCHEMA,
                "study": id,
                "set": "ask-questions-v1",
                "executor": "luna",
                "model": "gpt-6-luna",
                "implementation_digest": "0123456789abcdef",
                "rows": [{"id": "unearned-success", "answered": true, "valid_citations": 9, "citations": 9,
                          "task_recall": 0.67, "run_recall": 0.33, "cost_usd": 0.01, "milliseconds": 50_000,
                          "tasks_missed": ["vba-userform-port"]}],
                "totals": {"questions": 1, "answered": 1, "valid_citations": 9, "citations": 9,
                           "citation_validity": validity, "mean_task_recall": 0.67, "mean_run_recall": 0.33,
                           "cost_usd": 0.01, "mean_milliseconds": 50_000.0, "under_a_minute": 1},
            });
            std::fs::write(study.join("result.json"), result.to_string()).unwrap();
        }
        let run = |extra: &[&str]| {
            let mut args: Vec<String> = extra.iter().map(|s| (*s).to_owned()).collect();
            args.extend(["--studies-dir".to_owned(), dir.path().display().to_string()]);
            let mut out = Vec::new();
            command(&args, &mut out).unwrap();
            String::from_utf8(out).unwrap()
        };
        let text = run(&["--studies"]);
        assert!(text.contains("Coder One ask studies: 2"), "{text}");
        assert!(text.find("ask-study-2").unwrap() < text.find("ask-study-1").unwrap());
        assert!(text.contains("0.90"), "{text}");
        let one = run(&["--study", "ask-study-1"]);
        assert!(one.contains("unearned-success"), "{one}");
        assert!(one.contains("vba-userform-port"), "{one}");
        let value: Value = serde_json::from_str(&run(&["--studies", "--json"])).unwrap();
        assert_eq!(value["studies"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn asks_list_newest_first_and_one_reads_with_its_marks() {
        let dir = tempfile::tempdir().unwrap();
        for (id, verified) in [("ask-1790000000000", true), ("ask-1790000100000", false)] {
            let ask = dir.path().join(id);
            std::fs::create_dir_all(&ask).unwrap();
            let manifest = json!({
                "schema": ASK_SCHEMA,
                "id": id,
                "question": "why?",
                "executor": "luna",
                "status": "answered",
                "answer": "Because.",
                "claims": [{"claim": "It did.", "verified": verified, "problems": if verified { json!([]) } else { json!(["the Gym has no run x/y"]) }}],
                "citations": {"claims": 1, "verified": u8::from(verified), "citations": 2, "valid_citations": if verified { 2 } else { 1 }},
                "cost": {"usd": 0.0085},
                "milliseconds": 50_100,
                "dir": ask,
            });
            std::fs::write(ask.join("manifest.json"), manifest.to_string()).unwrap();
        }
        std::fs::create_dir_all(dir.path().join("not-an-ask")).unwrap();
        let base = ["--dir".to_owned(), dir.path().display().to_string()];
        let run = |extra: &[&str]| {
            let mut args: Vec<String> = extra.iter().map(|s| (*s).to_owned()).collect();
            args.extend(base.iter().cloned());
            let mut out = Vec::new();
            command(&args, &mut out).unwrap();
            String::from_utf8(out).unwrap()
        };
        let text = run(&[]);
        assert!(text.contains("Coder One asks, newest first: 2"), "{text}");
        let first = text.find("1/2").unwrap();
        let second = text.find("2/2").unwrap();
        assert!(first < second, "{text}");
        let value: Value = serde_json::from_str(&run(&["--json"])).unwrap();
        assert_eq!(value["asks"][0]["id"], "ask-1790000100000");
        assert_eq!(value["asks"][1]["cost_usd"], 0.0085);
        let one = run(&["latest"]);
        assert!(one.contains("? 1. It did."), "{one}");
        assert!(one.contains("unverified: the Gym has no run x/y"), "{one}");
        let one = run(&["ask-1790000000"]);
        assert!(one.contains("✓ 1. It did."), "{one}");
    }
}
