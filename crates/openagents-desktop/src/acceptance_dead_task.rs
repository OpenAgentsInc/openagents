//! `phone-dead-task` (#10124): a Coder run whose process dies never blocks
//! its project. The phone's own Coder tab starts a task; once the host has
//! admitted it, the gate kills the task's owner process and everything it
//! started, as a crash or a restart would. The phone must then read that
//! run as ended, in plain words ("Coder's process ended unexpectedly"),
//! and a second coding message from the phone must start Coder in the same
//! project and finish, with no tap and no "Couldn't start". The first
//! run's record stays in the task store.

use super::{Gate, Outcome, Phone, excerpt, find_composer_token, has_key, pair_phone, view_texts};
use serde_json::Value;
use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

const FIRST_ASK: &str =
    "Can you look through the code in my project and summarize what it implements?";
const SECOND_ASK: &str =
    "Can you look through calc.py in my project and tell me what its function does?";
/// How long a reply and the start it leads to may take.
const START_WAIT: Duration = Duration::from_secs(270);
/// How long the host may take to read the killed run as ended.
const ENDED_WAIT: Duration = Duration::from_secs(60);
/// How long the second run may take to finish.
const RUN_WAIT: Duration = Duration::from_secs(600);

pub(super) fn phone_dead_task(gate: &mut Gate) -> Outcome {
    if let Some(skip) = gate.need(true, false) {
        return skip;
    }
    let evidence = gate.evidence("phone-dead-task");
    let mut phone = pair_phone("acceptance-phone-dead-task")?;
    let home = crate::home();
    let root = home.join(".openagents/host");
    let store = home.join(".openagents/tasks");
    let mut log = Vec::new();
    let outcome = dead_task(&mut phone, &root, &store, &mut log);
    let _ = std::fs::write(evidence.join("steps.txt"), log.join("\n"));
    let journal: Vec<String> = coder::task::autostart::journal(&root)
        .iter()
        .map(|entry| serde_json::to_string(entry).unwrap_or_default())
        .collect();
    let _ = std::fs::write(evidence.join("autostart.jsonl"), journal.join("\n"));
    outcome
}

fn dead_task(phone: &mut Phone, root: &Path, store: &Path, log: &mut Vec<String>) -> Outcome {
    // 1. A task from the phone, admitted and running.
    let first = start(phone, FIRST_ASK, root, "first")?;
    log.push(format!(
        "first task {} started, owner process {}",
        first.task, first.owner
    ));
    let deadline = Instant::now() + START_WAIT;
    loop {
        let task = show(store, &first.task)?;
        match task.status {
            coder::task::Status::Running => break,
            coder::task::Status::Queued => {}
            other => {
                return Err(format!(
                    "the first task ended before the gate could kill it: {other:?}"
                ));
            }
        }
        if Instant::now() >= deadline {
            return Err("the first task was never admitted".into());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    // 2. Its process dies, as in a crash: the owner and everything under it.
    let killed = kill_tree(first.owner);
    log.push(format!("killed {killed:?}"));
    if killed.is_empty() {
        return Err(format!(
            "the first task's owner process {} was already gone",
            first.owner
        ));
    }
    // 3. The phone reads the run as ended, in plain words.
    let deadline = Instant::now() + ENDED_WAIT;
    let headline = loop {
        let _ = phone.computers.refresh();
        let summary = phone
            .computers
            .snapshot()
            .activity
            .iter()
            .filter(|s| s.host == phone.host && s.subject == first.task)
            .max_by_key(|s| s.sequence)
            .cloned();
        if let Some(summary) = summary.filter(|s| {
            matches!(
                s.phase,
                nostr::activity_summary::Phase::Cancelled | nostr::activity_summary::Phase::Failed
            )
        }) {
            break summary.headline;
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "the phone never read the killed run as ended (task {}: {:?})",
                first.task,
                show(store, &first.task).map(|t| (t.status, t.execution))
            ));
        }
        std::thread::sleep(Duration::from_secs(1));
    };
    log.push(format!("the phone read: {headline}"));
    if headline != coder::task::owner::OWNER_ENDED_TEXT {
        return Err(format!(
            "the phone said {headline:?} for the killed run, not {:?}",
            coder::task::owner::OWNER_ENDED_TEXT
        ));
    }
    // 4. A second task from the phone, in the same project, starts and
    // finishes.
    let second = start(phone, SECOND_ASK, root, "second")?;
    log.push(format!("second task {} started", second.task));
    let deadline = Instant::now() + RUN_WAIT;
    let ended = loop {
        let task = show(store, &second.task)?;
        if matches!(
            task.status,
            coder::task::Status::Finished | coder::task::Status::Cancelled
        ) {
            break task;
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "the second task did not finish within {}s ({:?})",
                RUN_WAIT.as_secs(),
                task.status
            ));
        }
        std::thread::sleep(Duration::from_secs(1));
    };
    let refused: Vec<String> = coder::task::autostart::journal(root)
        .into_iter()
        .filter(|entry| entry.task.as_deref() == Some(second.task.as_str()))
        .filter(|entry| matches!(entry.event.as_str(), "unadmitted" | "not_started"))
        .map(|entry| format!("{} {}", entry.event, entry.detail.unwrap_or_default()))
        .collect();
    if !refused.is_empty() {
        return Err(format!("the second task was refused first: {refused:?}"));
    }
    if ended.execution != coder::task::Execution::Finished {
        return Err(format!(
            "the second task ended {:?} / {:?} ({:?})",
            ended.status, ended.execution, ended.cancellation_reason
        ));
    }
    // The killed run's record stays.
    let first_record = show(store, &first.task)?;
    let kept = first_record
        .run
        .as_ref()
        .and_then(|run| run.result.as_ref())
        .is_some_and(|result| result.ending == coder::task::owner::OWNER_ENDED);
    if !kept {
        return Err(format!(
            "the killed run's record is not kept as ended: {:?}",
            first_record.run.as_ref().map(|run| &run.result)
        ));
    }
    Ok(format!(
        "killed task {}'s owner and its {} processes; the phone read \"{headline}\"; the next phone task {} started in the same project and finished",
        &first.task[..first.task.len().min(12)],
        killed.len(),
        &second.task[..second.task.len().min(12)],
    ))
}

struct Started {
    task: String,
    owner: u32,
}

/// Ask from a new chat in the phone's own Coder tab, never tapping, and
/// wait for the host to start Coder for it.
fn start(phone: &mut Phone, ask: &str, root: &Path, name: &str) -> Result<Started, String> {
    use openagents_chat::basic_coder::Relay;
    use openagents_chat_app::coder_tab::CoderTab;
    let before: BTreeSet<String> = coder::task::autostart::journal(root)
        .into_iter()
        .filter_map(|entry| entry.task)
        .collect();
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
    let mut tab = CoderTab::new(format!("coder:acceptance-dead-{name}")).with_basic(basic);
    tab.prefer(phone.host.clone());
    let mut chats = openagents_chat_app::chats::Chats::new(
        phone.runtime.handle().clone(),
        chat_secret,
        Err("no store in the gate".into()),
    );
    let computers = &mut phone.computers;
    let tick = |tab: &mut CoderTab,
                computers: &mut coder_computers::Computers,
                chats: &mut openagents_chat_app::chats::Chats| {
        tab.flush(Some(computers));
        tab.start_offered(Some(computers), chats);
        tab.render(Some(computers), chats).unwrap_or(Value::Null)
    };
    let view = tick(&mut tab, computers, &mut chats);
    let token = find_composer_token(&view).ok_or("the phone's chat shows no composer")?;
    tab.submit(&token, ask, Some(computers), &mut chats);
    let deadline = Instant::now() + START_WAIT;
    let mut last_refresh = Instant::now();
    loop {
        let view = tick(&mut tab, computers, &mut chats);
        if has_key(&view, "coder-run") {
            return Err(format!(
                "{name}: the reply offered Run Coder instead of starting Coder"
            ));
        }
        let new: Vec<coder::task::autostart::Entry> = coder::task::autostart::journal(root)
            .into_iter()
            .filter(|entry| entry.task.as_ref().is_some_and(|t| !before.contains(t)))
            .collect();
        if let Some(entry) = new.iter().find(|entry| {
            matches!(
                entry.event.as_str(),
                "refused" | "no_capacity" | "not_started" | "skipped" | "unadmitted"
            )
        }) {
            return Err(format!(
                "{name}: the computer did not start the task: {} {}",
                entry.event,
                entry.detail.clone().unwrap_or_default()
            ));
        }
        if let Some(entry) = new.iter().find(|entry| entry.event == "started") {
            return Ok(Started {
                task: entry.task.clone().unwrap_or_default(),
                owner: entry
                    .owner_process
                    .ok_or("the start names no owner process")?,
            });
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "{name}: no Coder start within {}s of asking (texts: {})",
                START_WAIT.as_secs(),
                excerpt(&view_texts(&view).join(" | "))
            ));
        }
        if last_refresh.elapsed() >= Duration::from_secs(2) {
            let _ = computers.refresh();
            last_refresh = Instant::now();
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

fn show(store: &Path, task: &str) -> Result<coder::task::Task, String> {
    coder::task::Store::open(store)
        .and_then(|store| store.show(task))
        .map_err(|e| format!("the task store: {e}"))
}

/// Kill `pid` and every process under it with SIGKILL, as a crash would:
/// the descendants are found first, so none is reparented away. Returns
/// the processes signalled.
fn kill_tree(pid: u32) -> Vec<u32> {
    let mut all = vec![pid];
    let mut index = 0;
    while index < all.len() {
        let parent = all[index];
        index += 1;
        let children = std::process::Command::new("pgrep")
            .args(["-P", &parent.to_string()])
            .output()
            .map(|out| String::from_utf8_lossy(&out.stdout).into_owned())
            .unwrap_or_default();
        all.extend(
            children
                .lines()
                .filter_map(|line| line.trim().parse::<u32>().ok()),
        );
    }
    all.into_iter()
        .filter(|pid| {
            std::process::Command::new("kill")
                .args(["-9", &pid.to_string()])
                .status()
                .is_ok_and(|status| status.success())
        })
        .collect()
}
