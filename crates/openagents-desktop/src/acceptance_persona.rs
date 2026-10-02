//! `persona`: the simulated-user QA run's turns (`scripts/qa/simulated-users.sh`,
//! docs/qa/simulated-users.md).
//!
//! Not a release scenario: it runs only when `--only persona` names it, and
//! it checks nothing itself. A driver outside the process plays a person: it
//! appends one JSON line per message to `DIR/persona/in.jsonl`
//! (`{"text": …}`, or `{"end": true}`), and this scenario sends each one the
//! way that surface does and appends what the person saw to
//! `DIR/persona/out.jsonl`: the reply, how long it took, a capture, and, when
//! the reply started Coder, how the run ended.
//!
//! `OPENAGENTS_ACCEPTANCE_PERSONA_SURFACE` picks the surface: `desktop` (the
//! window's chat panel, the default) or `phone` (the phone's own Coder tab,
//! the shared Rust the iOS and Android apps run, paired with the gate's host
//! as a phone pairs).

use super::{Gate, Outcome, REPLY_WAIT, excerpt, follow_run, new_chat, pair_phone, pump, send};
use serde_json::{Value, json};
use std::io::Write as _;
use std::path::Path;
use std::time::{Duration, Instant};

/// How long to wait for the driver's next message before giving up.
const IDLE: Duration = Duration::from_secs(900);
/// After a reply, how long to watch for a Coder start.
const START_GRACE: Duration = Duration::from_secs(20);

/// The driver's next message, the `index`th line of `in.jsonl`: `Some(text)`,
/// or `None` at its end or when it never comes.
fn next_message(dir: &Path, index: usize) -> Option<String> {
    let deadline = Instant::now() + IDLE;
    loop {
        let text = std::fs::read_to_string(dir.join("in.jsonl")).unwrap_or_default();
        let complete: Vec<&str> = text
            .split_inclusive('\n')
            .filter(|line| line.ends_with('\n'))
            .collect();
        if let Some(line) = complete.get(index) {
            let value: Value = serde_json::from_str(line.trim()).unwrap_or(Value::Null);
            if value.get("end").and_then(Value::as_bool) == Some(true) {
                return None;
            }
            return value.get("text").and_then(Value::as_str).map(str::to_owned);
        }
        if Instant::now() >= deadline {
            return None;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn write_out(dir: &Path, value: &Value) {
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("out.jsonl"))
    {
        let _ = writeln!(file, "{value}");
    }
}

pub(super) fn persona(gate: &mut Gate) -> Outcome {
    let surface =
        std::env::var("OPENAGENTS_ACCEPTANCE_PERSONA_SURFACE").unwrap_or_else(|_| "desktop".into());
    let dir = gate.evidence("persona");
    let turns = match surface.as_str() {
        "phone" => phone(gate, &dir)?,
        _ => desktop(gate, &dir)?,
    };
    Ok(format!("{turns} persona turns on the {surface} surface"))
}

/// The window's chat panel: type, press Send, read the reply, and follow a
/// Coder run it starts.
fn desktop(gate: &mut Gate, dir: &Path) -> Result<usize, String> {
    let chat = new_chat(gate, "persona")?;
    let mut index = 0;
    while let Some(text) = next_message(dir, index) {
        let began = Instant::now();
        let mut out = json!({"turn": index, "sent": text});
        match send(gate, &text) {
            Ok(reply) => {
                out["ms"] = json!(began.elapsed().as_millis() as u64);
                out["reply"] = json!(reply.text);
                out["meta"] = serde_json::to_value(&reply.meta).unwrap_or(Value::Null);
                let started = pump(gate, START_GRACE, |gate| {
                    gate.panel().coder_run(&chat).is_some()
                });
                if started {
                    let seen = follow_run(gate, &chat, true);
                    out["coder"] = json!({
                        "task": seen.task,
                        "started": seen.started.is_some(),
                        "summary": seen.finished.as_ref().map(|f| f.summary.clone()),
                        "files_changed": seen.finished.as_ref().map(|f| f.files_changed.len()),
                        "failure": seen.failure,
                        "asked": seen.asked,
                        "ms": began.elapsed().as_millis() as u64,
                    });
                }
            }
            Err(why) => {
                out["ms"] = json!(began.elapsed().as_millis() as u64);
                out["error"] = json!(why);
            }
        }
        let file = format!("turn-{index:02}");
        gate.capture("persona", &file, 1200.0, 840.0, 1.0);
        out["capture"] = json!(format!("{file}.png"));
        out["transcript"] = json!(gate.transcript());
        write_out(dir, &out);
        index += 1;
    }
    gate.save_chat("persona");
    Ok(index)
}

/// The phone's Coder tab paired with the host: submit from its composer and
/// read the words its view shows once they stop changing.
fn phone(_gate: &mut Gate, dir: &Path) -> Result<usize, String> {
    use openagents_chat::basic_coder::Relay;
    use openagents_chat_app::coder_tab::CoderTab;
    let super::Phone {
        runtime,
        mut computers,
        host,
        ..
    } = pair_phone("acceptance-persona-phone")?;
    let chat_secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let door = Relay::new(
        openagents_chat::basic_coder::RELAY,
        openagents_chat::basic_coder::WORKER,
        chat_secret,
    )?;
    let basic = openagents_chat::basic_chats::BasicChats::new(
        Some(runtime.handle().clone()),
        Some(std::sync::Arc::new(door)),
        None,
    );
    let mut tab = CoderTab::new("coder:persona".into()).with_basic(basic);
    tab.prefer(host.clone());
    let mut chats = openagents_chat_app::chats::Chats::new(
        runtime.handle().clone(),
        chat_secret,
        Err("no store in the gate".into()),
    );
    let tick = |tab: &mut CoderTab,
                computers: &mut coder_computers::Computers,
                chats: &mut openagents_chat_app::chats::Chats| {
        tab.flush(Some(computers));
        tab.start_offered(Some(computers), chats);
        tab.render(Some(computers), chats)
    };
    let root = super::super::super::home().join(".openagents/host");
    let mut index = 0;
    while let Some(text) = next_message(dir, index) {
        let began = Instant::now();
        let view = tick(&mut tab, &mut computers, &mut chats).unwrap_or(Value::Null);
        let before = messages(&view).len();
        let tasks_before = coder::task::autostart::journal(&root).len();
        let Some(token) = super::find_composer_token(&view) else {
            write_out(
                dir,
                &json!({"turn": index, "sent": text, "error": "the phone's chat shows no composer"}),
            );
            index += 1;
            continue;
        };
        tab.submit(&token, &text, Some(&mut computers), &mut chats);
        // The reply is in once the composer is no longer busy, the view's
        // words grew past what was sent, and they held still for a moment.
        let deadline = began + REPLY_WAIT;
        let mut last: Vec<(String, String)> = Vec::new();
        let mut still_since = Instant::now();
        let mut refreshed = Instant::now();
        let shown = loop {
            let view = tick(&mut tab, &mut computers, &mut chats).unwrap_or(Value::Null);
            let now = messages(&view);
            if now != last {
                last = now.clone();
                still_since = Instant::now();
            }
            let answered = now.len() > before + 1
                && now
                    .last()
                    .is_some_and(|(role, text)| role == "assistant" && !text.is_empty());
            if (answered && !busy(&view) && still_since.elapsed() >= Duration::from_secs(3))
                || Instant::now() >= deadline
            {
                break view;
            }
            if refreshed.elapsed() >= Duration::from_secs(2) {
                let _ = computers.refresh();
                refreshed = Instant::now();
            }
            std::thread::sleep(Duration::from_millis(250));
        };
        // What the reply added: the new assistant messages and anything
        // the view shows after them (a Coder start, its buttons).
        let new: Vec<String> = messages(&shown)
            .into_iter()
            .skip(before)
            .filter(|(role, _)| role != "user")
            .map(|(_, text)| text)
            .chain(controls(&shown))
            .collect();
        let journal: Vec<Value> = coder::task::autostart::journal(&root)
            .into_iter()
            .skip(tasks_before)
            .map(|entry| serde_json::to_value(&entry).unwrap_or(Value::Null))
            .collect();
        let file = format!("turn-{index:02}-view.json");
        let _ = std::fs::write(
            dir.join(&file),
            serde_json::to_vec_pretty(&shown).unwrap_or_default(),
        );
        write_out(
            dir,
            &json!({
                "turn": index,
                "sent": text,
                "ms": began.elapsed().as_millis() as u64,
                "reply": new.join("\n"),
                "coder_started": super::has_key(&shown, "coder-start"),
                "run_coder_offered": super::has_key(&shown, "coder-run"),
                "autostart": journal,
                "capture": file,
                "excerpt": excerpt(&new.join(" ")),
            }),
        );
        index += 1;
    }
    Ok(index)
}

/// The transcript's messages, as (role, words).
fn messages(view: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        let props = &node["element"]["props"];
        if node["element"]["kind"] == "message" {
            let role = props["role"].as_str().unwrap_or("").to_owned();
            out.push((role, words_of(&json!({"root": node})).join(" ")));
            continue;
        }
        if let Some(children) = props["children"].as_array() {
            pending.extend(children.iter().rev());
        }
    }
    out
}

/// The words of the view outside the transcript's messages and the
/// header: a Coder start card, its buttons, an offer.
fn controls(view: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        let key = node["key"].as_str().unwrap_or("");
        if node["element"]["kind"] == "message"
            || node["element"]["kind"] == "composer"
            || key == "coder-chat-header"
        {
            continue;
        }
        if key.starts_with("coder-start")
            || key.starts_with("coder-run")
            || key.starts_with("coder-offer")
        {
            out.extend(words_of(&json!({"root": node})));
            continue;
        }
        if let Some(children) = node["element"]["props"]["children"].as_array() {
            pending.extend(children.iter().rev());
        }
    }
    out
}

/// The words a phone view shows, in order: text values, button labels, and
/// the spans of rendered Markdown (where the chat's replies are).
fn words_of(view: &Value) -> Vec<String> {
    fn spans(value: &Value, out: &mut Vec<String>) {
        match value {
            Value::Object(map) => {
                if let Some(text) = map.get("text").and_then(Value::as_str) {
                    out.push(text.to_owned());
                }
                for (key, child) in map {
                    if key != "text" {
                        spans(child, out);
                    }
                }
            }
            Value::Array(items) => items.iter().for_each(|item| spans(item, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        let props = &node["element"]["props"];
        if let Some(value) = props["value"].as_str() {
            out.push(value.to_owned());
        }
        if node["element"]["kind"] == "button"
            && let Some(label) = props["label"].as_str()
        {
            out.push(format!("[{label}]"));
        }
        if node["element"]["kind"] == "markdown" {
            let mut words = Vec::new();
            spans(&props["blocks"], &mut words);
            out.push(words.concat());
        }
        if let Some(children) = props["children"].as_array() {
            pending.extend(children.iter().rev());
        }
    }
    out
}

/// Whether the phone's composer says a reply is still coming.
fn busy(view: &Value) -> bool {
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        if node["element"]["kind"] == "composer" {
            return node["element"]["props"]["busy"].as_bool() == Some(true);
        }
        if let Some(children) = node["element"]["props"]["children"].as_array() {
            pending.extend(children.iter());
        }
    }
    false
}
