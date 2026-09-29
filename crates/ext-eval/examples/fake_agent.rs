//! A stand-in for `coder -p`, for the runner's tests.
//!
//! It takes the arguments the runner passes (`--prompt-file FILE --trace
//! PATH --json`), reads its pinned door from `CODER_DOOR_URL` and
//! `CODER_DOOR_KEY`, asks it once (`POST /v1/responses`, no streaming)
//! with the appended guidance as instructions, records the turn as an ATIF
//! log, and prints a `--json` summary. When `CODER_PROGRAMS` admits
//! programs and the prompt asks about the repository or its callers, it
//! records one call to each step it finds under
//! `$HOME/.openagents/programs`, so `operation_used` graders see the
//! subject arm reach the extension. `OA_EVAL_FAKE` picks extra behavior:
//! `env` dumps the environment to stderr and to `env.txt`; `canary`
//! reports whether it could read `OA_EVAL_CANARY`; `sleep` prints its
//! process id and sleeps until it is stopped; `write` writes the reply to `summary.md`.

use std::io::Write;
use std::path::PathBuf;

use serde_json::{Value, json};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value = |flag: &str| {
        args.iter()
            .position(|arg| arg == flag)
            .and_then(|at| args.get(at + 1))
            .cloned()
    };
    let prompt = value("--prompt-file")
        .and_then(|path| std::fs::read_to_string(path).ok())
        .or_else(|| value("-p"))
        .unwrap_or_default();
    let trace = value("--trace").map(PathBuf::from);
    let mode = std::env::var("OA_EVAL_FAKE").unwrap_or_default();
    let model = std::env::var("CODER_MODEL").unwrap_or_else(|_| "fake".into());
    let cwd = std::env::current_dir().unwrap_or_default();
    let session = atif::Session::opening(
        "fake-session",
        &model,
        "fake-door",
        &cwd.display().to_string(),
        "0.0.0",
    );
    let mut log = trace
        .as_ref()
        .and_then(|path| atif::Log::create_at(path, &session).ok());
    let mut append = |step: atif::Step| {
        if let Some(log) = &mut log {
            let _ = log.append(&step);
        }
    };
    append(atif::Step::said(atif::Source::User, &prompt));

    if mode == "env" {
        let mut dump = String::new();
        for (key, value) in std::env::vars() {
            dump.push_str(&format!("{key}={value}\n"));
        }
        eprint!("{dump}");
        let _ = std::fs::write(cwd.join("env.txt"), &dump);
    }
    if mode == "sleep" {
        println!("{}", json!({ "pid": std::process::id() }));
        let _ = std::io::stdout().flush();
        loop {
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }

    let guidance = std::env::var("CODER_GUIDANCE")
        .ok()
        .and_then(|path| std::fs::read_to_string(path).ok())
        .unwrap_or_default();
    if let Ok(expected) = std::env::var("CODER_GUIDANCE_DIGEST") {
        let found = nostr::contracts::digest_bytes(guidance.as_bytes());
        if found != expected {
            summary(None, "failed", Some("guidance digest mismatch"));
            std::process::exit(1);
        }
    }

    let programs = std::env::var("CODER_PROGRAMS").unwrap_or_default();
    let asks_for_a_map = prompt.to_ascii_lowercase().contains("repository")
        || prompt.to_ascii_lowercase().contains("callers");
    if !programs.is_empty() && programs != "none" && asks_for_a_map {
        let dir =
            PathBuf::from(std::env::var("HOME").unwrap_or_default()).join(".openagents/programs");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&dir)
            .map(|entries| entries.filter_map(Result::ok).map(|e| e.path()).collect())
            .unwrap_or_default();
        files.sort();
        for file in files {
            let Ok(program) = std::fs::read(&file)
                .map_err(|_| ())
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).map_err(|_| ()))
            else {
                continue;
            };
            for step in program
                .pointer("/definition/steps")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                let name = step["name"].as_str().unwrap_or_default();
                let mut extra = serde_json::Map::new();
                extra.insert("step".into(), json!(name));
                append(atif::Step::called(atif::Call {
                    id: format!("call-{name}"),
                    name: name.to_string(),
                    arguments: json!({}),
                    output: String::new(),
                    outcome: atif::Outcome::Completed,
                    milliseconds: 1,
                    purpose: None,
                    extra,
                }));
            }
        }
    }

    let mut reply = ask(&guidance, &prompt);
    if mode == "canary" {
        let canary = std::env::var("OA_EVAL_CANARY").unwrap_or_default();
        let readable = std::fs::read(&canary).is_ok();
        let seen = std::fs::metadata(&canary).is_ok();
        reply = Ok(format!(
            "canary: {}; home canary listed: {seen}",
            if readable { "readable" } else { "unreadable" }
        ));
    }
    let reply = match reply {
        Ok(reply) => reply,
        Err(why) => {
            append(atif::Step::said(atif::Source::System, &why));
            if let Some(log) = &mut log {
                let _ = log.finish(atif::log::ENDED);
            }
            summary(None, "failed", Some(&why));
            std::process::exit(1);
        }
    };
    if mode == "write" {
        let _ = std::fs::write(cwd.join("summary.md"), &reply);
    }
    append(atif::Step::said(atif::Source::Agent, &reply));
    if let Some(log) = &mut log {
        let _ = log.finish(atif::log::ENDED);
    }
    summary(Some(&reply), "answered", None);
}

fn ask(guidance: &str, prompt: &str) -> Result<String, String> {
    let url = std::env::var("CODER_DOOR_URL").map_err(|_| "no door".to_string())?;
    let key = std::env::var("CODER_DOOR_KEY").map_err(|_| "no door key".to_string())?;
    let body = json!({
        "model": std::env::var("CODER_MODEL").unwrap_or_default(),
        "instructions": guidance,
        "input": [{"type": "message", "role": "user",
                   "content": [{"type": "input_text", "text": prompt}]}],
        "stream": false,
    });
    let response = reqwest::blocking::Client::new()
        .post(format!("{url}/v1/responses"))
        .bearer_auth(key)
        .json(&body)
        .send()
        .map_err(|error| format!("door: {error}"))?;
    if !response.status().is_success() {
        return Err(format!("door answered {}", response.status()));
    }
    let value: Value = response.json().map_err(|error| error.to_string())?;
    Ok(value
        .pointer("/output/0/content/0/text")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string())
}

fn summary(reply: Option<&str>, outcome: &str, error: Option<&str>) {
    let mut out = std::io::stdout();
    let _ = writeln!(
        out,
        "{}",
        json!({
            "reply": reply,
            "outcome": outcome,
            "cost_usd": 0.001,
            "error": error,
        })
    );
}
