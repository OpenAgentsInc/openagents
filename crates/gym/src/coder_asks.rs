//! `gym coder asks`: the questions `coder-one ask` answered, with what
//! each cost and how many of its citations checked.
//!
//! Every ask records itself under `~/.openagents/coder-one/asks/ask-<ms>/`:
//! a manifest (`openagents.coder-one.ask.v1`), the ATIF invocation log, the
//! briefing, the executor's stream, and the answer. This view reads the
//! manifests; `gym coder asks ID` prints one answer with its claims and
//! the marks the citation check gave them.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use crate::runs::{clip_words, duration, money, now_ms, when};

/// The manifest schema an ask writes.
pub const ASK_SCHEMA: &str = "openagents.coder-one.ask.v1";

/// This view's JSON schema.
pub const SCHEMA: &str = "openagents.gym.coder-asks.v1";

const HELP: &str = "\
gym coder asks [ID|latest] [--dir PATH] [--limit N] [--json]

The questions `coder-one ask` answered, newest first: when, the executor,
whether it answered, how many citations checked and claims were verified,
the cost, the time, and the question. ID (or a prefix, or `latest`) prints
one ask's answer with each claim marked ✓ when its citations checked and ?
when one didn't. Asks are read from ~/.openagents/coder-one/asks unless
--dir names another directory.";

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
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            "--json" => json_output = true,
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
    let dir = dir.ok_or("no --dir and no HOME")?;
    let asks = load(&dir);
    let write = |out: &mut dyn std::io::Write, text: &str| {
        writeln!(out, "{text}").map_err(|error| error.to_string())
    };
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
