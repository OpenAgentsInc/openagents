//! `phone-closed-loop` (#10118): the owner develops OpenAgents and ships
//! TestFlight from the phone. The phone's own Coder tab, paired with the
//! gate's host, asks for a small change in a scratch clone of this
//! repository (`OPENAGENTS_ACCEPTANCE_LOOP_PROJECT`, whose `origin` is a
//! local bare repository, `OPENAGENTS_ACCEPTANCE_LOOP_REMOTE`); Coder must
//! start at once, edit, commit, and push to that remote's `main` with no
//! question or approval. Then it asks for a TestFlight build as a dry run;
//! Coder must run `scripts/release/testflight.sh --validate-only` to the
//! end (an archive App Store Connect validates, nothing uploaded). The
//! host runs as the owner's does: the clone is its project and auto-start
//! runs tasks with full access. Every distinct thing the phone's chat
//! showed is kept with its time in `phone-closed-loop/timeline.jsonl`; the
//! phone must have shown the run going, then its outcome, in words.

use super::{Gate, Outcome, excerpt, find_composer_token, has_key, pair_phone, remote_main};
use openagents_desktop::control::SocketControl;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How long the small change may take, from the message.
const CHANGE_WAIT: Duration = Duration::from_secs(20 * 60);
/// How long the archive and validation may take, from the message.
const SHIP_WAIT: Duration = Duration::from_secs(90 * 60);

const CHANGE_ASK: &str = "In my openagents project, create docs/release/phone-loop-check.md \
with the single line \"Checked from the phone.\", commit it, and push it to main.";
const SHIP_ASK: &str = "Ship a TestFlight build of the OpenAgents iOS app from my openagents \
project as a dry run: archive it and validate it with App Store Connect, but don't upload it.";

/// What the phone's chat showed of one run.
struct Seen {
    task: Option<String>,
    /// Each distinct card text, with the seconds since the message.
    timeline: Vec<(u64, String)>,
    /// The card's words once the run ended.
    ended: Option<String>,
    /// What the open Coder chat said at the end.
    reply: String,
    /// Whether the card ever showed Coder waiting for an answer.
    asked: bool,
}

pub(super) fn phone_closed_loop(gate: &mut Gate) -> Outcome {
    if let Some(skip) = gate.need(true, false) {
        return skip;
    }
    let var = |name: &str| {
        std::env::var(name).map_err(|_| {
            format!("the gate set no {name} (run it through scripts/release/acceptance.sh)")
        })
    };
    if std::env::var_os("OPENAGENTS_ACCEPTANCE_ASC").is_none() {
        return Ok("SKIP no App Store Connect key to validate a build with".into());
    }
    let project = var("OPENAGENTS_ACCEPTANCE_LOOP_PROJECT")?;
    let remote = var("OPENAGENTS_ACCEPTANCE_LOOP_REMOTE")?;
    let ship = PathBuf::from(var("OPENAGENTS_SHIP_DIR")?);
    let coder = var("OPENAGENTS_ACCEPTANCE_CODER")?;
    let microcoder = var("OPENAGENTS_ACCEPTANCE_MICROCODER")?;
    let shown = std::env::var("OPENAGENTS_ACCEPTANCE_LABEL").ok();
    let evidence = gate.evidence("phone-closed-loop");
    // The owner's setup: the clone is the host's project, and auto-start
    // runs its tasks with full access (`coder host autostart on
    // --full-access`), which signing and the upload need.
    let socket = crate::platform::control_path().ok_or("no control socket")?;
    let mut control = SocketControl::new(socket.clone());
    openagents_desktop::control::pick_project(&mut control, &project, shown.as_deref(), true)
        .map_err(|e| format!("the host would not take the clone as its project: {e:?}"))?;
    let label = "openagents";
    let root = crate::home().join(".openagents/host");
    let on = std::process::Command::new(&coder)
        .args([
            "host",
            "autostart",
            "on",
            "--workspace",
            label,
            "--controller",
            &microcoder,
        ])
        .args([
            "--route",
            "codex:gpt-6.1-sol",
            "--route",
            "claude:claude-opus-5-5",
        ])
        .args(["--full-access", "--root"])
        .arg(&root)
        .output()
        .map_err(|e| format!("coder host autostart on: {e}"))?;
    if !on.status.success() {
        return Err(format!(
            "coder host autostart on --full-access failed: {}",
            String::from_utf8_lossy(&on.stderr).trim()
        ));
    }
    let outcome = closed_loop(&project, &remote, &ship, &evidence);
    // Give the scenarios' project back, as it was.
    if let Some(shown) = &shown {
        let acceptance = std::env::var("OPENAGENTS_ACCEPTANCE_PROJECT").unwrap_or_default();
        let _ =
            openagents_desktop::control::pick_project(&mut control, &acceptance, Some(label), true);
        let _ = shown;
    }
    outcome
}

fn closed_loop(project: &str, remote: &str, ship: &Path, evidence: &Path) -> Outcome {
    let mut phone = pair_phone("acceptance-phone-loop")?;
    if phone.workspace != "openagents" {
        return Err(format!(
            "the phone would start Coder in {}, not the openagents clone",
            phone.workspace
        ));
    }
    let _ = project;
    // The app pairs its Coder chats with the computer as it pairs, so a
    // task's messages read back on the phone.
    let chat_secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let mut chats = openagents_chat_app::chats::Chats::new(
        phone.runtime.handle().clone(),
        chat_secret,
        Err("no store in the gate".into()),
    );
    let invitation = phone
        .chats
        .clone()
        .ok_or("pairing handed the phone no Coder chats invitation")?;
    chats.pair(invitation, phone.label.clone(), phone.host.clone(), None);
    let deadline = Instant::now() + Duration::from_secs(90);
    while chats.coder_client(&phone.host).is_none() {
        if Instant::now() >= deadline {
            return Err("the phone's Coder chats never paired with the computer".into());
        }
        chats.settle();
        std::thread::sleep(Duration::from_millis(500));
    }
    // 1. A small change, committed and pushed.
    let before = remote_main(remote).ok_or_else(|| format!("{remote} has no main"))?;
    let change = run(
        &mut phone,
        &mut chats,
        CHANGE_ASK,
        CHANGE_WAIT,
        evidence,
        "change",
    )?;
    let after = remote_main(remote).unwrap_or_default();
    let mut problems = Vec::new();
    if change.asked {
        problems.push("the change: Coder asked a question".to_owned());
    }
    if after == before {
        problems.push(format!("the change: the remote's main is still {before}"));
    } else {
        let changed = git(remote, &["diff", "--name-only", &before, &after]);
        if !changed
            .lines()
            .any(|path| path == "docs/release/phone-loop-check.md")
        {
            problems.push(format!(
                "the change: the remote's new main {after} does not add docs/release/phone-loop-check.md ({:?})",
                changed.trim()
            ));
        }
    }
    check_shown(&change, "the change", &mut problems);
    // 2. A TestFlight build as a dry run.
    let started = std::time::SystemTime::now();
    let shipped = run(
        &mut phone, &mut chats, SHIP_ASK, SHIP_WAIT, evidence, "ship",
    )?;
    if shipped.asked {
        problems.push("the ship: Coder asked a question".to_owned());
    }
    check_shown(&shipped, "the ship", &mut problems);
    match release(ship, started) {
        Ok(release) => {
            let _ = std::fs::write(evidence.join("release.txt"), &release.progress);
            if release.status != "done" {
                problems.push(format!(
                    "the ship: the release is {} ({})",
                    release.status,
                    excerpt(release.progress.lines().last().unwrap_or_default())
                ));
            } else if !release.progress.contains("Nothing was uploaded") {
                problems.push(format!(
                    "the ship: the release was not a dry run ({})",
                    excerpt(&release.progress)
                ));
            }
        }
        Err(why) => problems.push(format!("the ship: {why}")),
    }
    // Build output stays out of the host's workspace, or the next task
    // there is refused at admission (#10118).
    let projects = crate::home().join(".openagents/host/projects");
    for entry in std::fs::read_dir(&projects).into_iter().flatten().flatten() {
        if entry.path().join("target").exists() {
            problems.push(format!(
                "a Cargo target directory was left in the host's workspace {}",
                entry.path().display()
            ));
        }
    }
    if problems.is_empty() {
        Ok(format!(
            "from the phone, Coder pushed {} to main with no question, then archived and validated a TestFlight build with nothing uploaded; the phone showed {:?} then {:?} (timeline.jsonl)",
            &after[..after.len().min(10)],
            shipped
                .timeline
                .iter()
                .map(|(_, text)| text.as_str())
                .find(|text| !text.is_empty())
                .unwrap_or_default(),
            shipped.ended.unwrap_or_default()
        ))
    } else {
        Err(problems.join("; "))
    }
}

/// The phone must show the run going, then how it ended, in words.
fn check_shown(seen: &Seen, what: &str, problems: &mut Vec<String>) {
    if seen.task.is_none() {
        problems.push(format!("{what}: Coder never started"));
        return;
    }
    if seen.timeline.len() < 2 {
        problems.push(format!(
            "{what}: the phone's chat showed only {:?} while Coder ran",
            seen.timeline
        ));
    }
    match &seen.ended {
        None => problems.push(format!("{what}: the phone never showed the run ending")),
        Some(ended) if !ended.contains("Done") => {
            problems.push(format!("{what}: the phone showed {ended:?}"))
        }
        // The card says how it ended, in a line of Coder's reply.
        Some(ended) if !ended.contains(" · ") || ended.trim_end().ends_with(':') => problems.push(
            format!("{what}: the phone's card showed no outcome line ({ended:?})"),
        ),
        Some(_) => {}
    }
    if seen.reply.trim().is_empty() {
        problems.push(format!("{what}: the open Coder chat showed no reply"));
    }
}

/// One message from a new phone chat, followed to the run's end as the
/// phone shows it: the tab is ticked as the app ticks it, and the
/// computers are refreshed as a wake would.
fn run(
    phone: &mut super::Phone,
    chats: &mut openagents_chat_app::chats::Chats,
    ask: &str,
    wait: Duration,
    evidence: &Path,
    name: &str,
) -> Result<Seen, String> {
    use openagents_chat::basic_coder::Relay;
    use openagents_chat_app::coder_tab::CoderTab;
    let chat_secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let door = Relay::new(
        openagents_chat::basic_coder::RELAY,
        openagents_chat::basic_coder::WORKER,
        chat_secret,
    )?;
    let basic = openagents_chat::basic_chats::BasicChats::new(
        Some(phone.runtime.handle().clone()),
        Some(std::sync::Arc::new(door)),
        None,
    );
    let mut tab = CoderTab::new(format!("coder:acceptance-{name}")).with_basic(basic);
    tab.prefer(phone.host.clone());
    let computers = &mut phone.computers;
    // The app's tick: send pending commands, start what a reply offers at
    // once, then the view.
    let tick = |tab: &mut CoderTab,
                computers: &mut coder_computers::Computers,
                chats: &mut openagents_chat_app::chats::Chats| {
        tab.flush(Some(computers));
        tab.start_offered(Some(computers), chats);
        tab.render(Some(computers), chats).unwrap_or(Value::Null)
    };
    let view = tick(&mut tab, computers, chats);
    let token = find_composer_token(&view).ok_or("the phone's chat shows no composer")?;
    tab.submit(&token, ask, Some(computers), chats);
    let began = Instant::now();
    let deadline = began + wait;
    let mut seen = Seen {
        task: None,
        timeline: Vec::new(),
        ended: None,
        reply: String::new(),
        asked: false,
    };
    let mut last_refresh = Instant::now();
    let mut view = Value::Null;
    let root = crate::home().join(".openagents/host");
    let before: std::collections::BTreeSet<String> = coder::task::autostart::journal(&root)
        .into_iter()
        .filter_map(|entry| entry.task)
        .collect();
    while Instant::now() < deadline {
        view = tick(&mut tab, computers, chats);
        if seen.task.is_none() {
            seen.task = coder::task::autostart::journal(&root)
                .into_iter()
                .find(|entry| {
                    entry.event == "started"
                        && entry
                            .task
                            .as_ref()
                            .is_some_and(|task| !before.contains(task))
                })
                .and_then(|entry| entry.task);
        }
        let card = card_text(&view);
        if !card.is_empty() && seen.timeline.last().is_none_or(|(_, last)| *last != card) {
            seen.timeline
                .push((began.elapsed().as_secs(), card.clone()));
        }
        if has_key(&view, "coder-run") {
            return Err(format!(
                "{name}: the reply offered Run Coder instead of starting Coder"
            ));
        }
        if card.contains("Waiting for you") {
            seen.asked = true;
        }
        if has_key(&view, "coder-start")
            && !has_key(&view, "coder-start-stop")
            && seen.task.is_some()
        {
            // The card's outcome line follows the end, as the phone keeps
            // showing the chat.
            let ended_at = Instant::now();
            let mut card = card;
            while !card.contains(" · ") && ended_at.elapsed() < Duration::from_secs(30) {
                std::thread::sleep(Duration::from_millis(500));
                let _ = computers.refresh();
                view = tick(&mut tab, computers, chats);
                card = card_text(&view);
            }
            if seen.timeline.last().is_none_or(|(_, last)| *last != card) {
                seen.timeline
                    .push((began.elapsed().as_secs(), card.clone()));
            }
            seen.ended = Some(card);
            break;
        }
        if last_refresh.elapsed() >= Duration::from_secs(2) {
            let _ = computers.refresh();
            last_refresh = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    // What the run said, as the phone opens it.
    if seen.ended.is_some() && has_key(&view, "coder-start-open") {
        let activation = rust_native::Activation {
            instance: view["instance"].as_str().unwrap_or_default().into(),
            revision: view["revision"].as_u64().unwrap_or_default(),
            node: "coder-start-open".into(),
        };
        tab.activate(&activation, Some(computers), chats);
        let opened = Instant::now();
        while opened.elapsed() < Duration::from_secs(60) {
            let _ = computers.refresh();
            view = tick(&mut tab, computers, chats);
            // Coder's reply: the words after the message that asked.
            let words = said(&view);
            if let Some(at) = words.iter().position(|text| text.contains(ask)) {
                let reply = words[at + 1..].join("\n");
                if !reply.trim().is_empty() {
                    seen.reply = reply;
                    break;
                }
            }
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    let timeline: Vec<String> = seen
        .timeline
        .iter()
        .map(|(at, text)| json!({ "step": name, "seconds": at, "card": text }).to_string())
        .collect();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(evidence.join("timeline.jsonl"))
        .map_err(|e| e.to_string())?;
    use std::io::Write as _;
    let _ = writeln!(file, "{}", timeline.join("\n"));
    let _ = std::fs::write(
        evidence.join(format!("{name}-view.json")),
        serde_json::to_vec_pretty(&view).unwrap_or_default(),
    );
    let _ = std::fs::write(evidence.join(format!("{name}-reply.txt")), &seen.reply);
    Ok(seen)
}

/// The start card's words: its title, note, and anything else in it.
fn card_text(view: &Value) -> String {
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        if node["key"] == "coder-start" {
            return super::view_texts(&json!({ "root": node })).join(" · ");
        }
        if let Some(children) = node["element"]["props"]["children"].as_array() {
            pending.extend(children.iter());
        }
    }
    String::new()
}

struct Release {
    status: String,
    progress: String,
}

/// The release `testflight.sh` ran under the gate's ship directory since
/// `since`.
fn release(ship: &Path, since: std::time::SystemTime) -> Result<Release, String> {
    let newest = std::fs::read_dir(ship)
        .map_err(|_| "Coder never ran scripts/release/testflight.sh".to_owned())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|dir| dir.join("status").is_file())
        .filter(|dir| {
            std::fs::metadata(dir.join("progress.log"))
                .and_then(|m| m.modified())
                .is_ok_and(|at| at >= since)
        })
        .max_by_key(|dir| {
            std::fs::metadata(dir.join("status"))
                .and_then(|m| m.modified())
                .ok()
        })
        .ok_or("Coder never ran scripts/release/testflight.sh")?;
    Ok(Release {
        status: std::fs::read_to_string(newest.join("status"))
            .unwrap_or_default()
            .trim()
            .to_owned(),
        progress: std::fs::read_to_string(newest.join("progress.log")).unwrap_or_default(),
    })
}

fn git(dir: &str, args: &[&str]) -> String {
    std::process::Command::new("git")
        .args(["--git-dir", dir])
        .args(args)
        .output()
        .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
        .unwrap_or_default()
}

/// The words a phone view's messages say, in order: markdown blocks'
/// spans and code, and plain text values.
fn said(view: &Value) -> Vec<String> {
    let mut out = Vec::new();
    let mut pending = vec![&view["root"]];
    while let Some(node) = pending.pop() {
        let props = &node["element"]["props"];
        if let Some(blocks) = props["blocks"].as_array() {
            let mut text = String::new();
            for block in blocks {
                if let Some(code) = block["text"].as_str() {
                    text.push_str(code);
                }
                for span in block["spans"].as_array().into_iter().flatten() {
                    text.push_str(span["text"].as_str().unwrap_or_default());
                }
                text.push('\n');
            }
            out.push(text);
        }
        if let Some(children) = props["children"].as_array() {
            pending.extend(children.iter().rev());
        }
    }
    out
}
