//! `openagents land`: one landing queue for every machine (#11227).
//!
//! Agents anywhere submit a branch; one integrator (`land work`, on a cloud
//! environment) lands entries on `main` one at a time: rebase, the
//! touched-crate checks, push with retries on a race, a bounded repair turn
//! on a conflict, else a bounce back to the author; then the issue is
//! closed or commented. `coder::task::land_queue` is the queue;
//! `docs/cloud/land-queue.md` is the guide.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use coder::cli_route::tree::{Declared, Effect};
use coder::task::land_queue::{
    self, Entry, Heartbeat, Instance, Integrator, Live, Queue, State, Store,
};
use serde_json::{Value, json};

use crate::out::table;
use crate::{Args, Output};

pub(crate) const USAGE: &str = "usage: openagents land COMMAND [--queue URL]
  submit [--branch B] [--issue N] [--no-close] [--summary TEXT] [--target T] [--no-wake]
              Queue this checkout's commits for landing on T (main): push HEAD
              to land/<id> on origin (or name a branch already there with
              --branch) and add it to the queue. With --issue N the landing
              closes issue N (--no-close only comments) and moves the board.
              When the integrator's environment is stopped, this starts it
              unless --no-wake.
  status [--all]
              The integrator (where, when last seen, what it lands now) and
              the open entries, oldest first; --all adds landed, bounced and
              withdrawn ones with each one's commit or reason. Bare
              `openagents land` is status.
  show ID     One entry and every try at landing it: checks, landing
              attempts, repair, outcome.
  withdraw ID Take a queued entry out of the queue.
  work [--once] [--every SECONDS] [--repair COMMAND|none]
              Run the integrator in this checkout: take the oldest entry, land
              or bounce it, record the try, repeat (polling every 20 s).
              --repair names the one conflict-repair turn's command (claude
              by default); none bounces every conflict.
The queue is OPENAGENTS_LAND_QUEUE or --queue (gs://bucket/prefix or a
folder); by default gs://openagentsgemini-coder-artifacts/land-queue/openagents.
See docs/cloud/land-queue.md.";

/// What each command does, for the chat router's command tree.
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("submit", Effect::Publishes),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("withdraw", Effect::Publishes),
    Declared::computer("work", Effect::LongRunning),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &["all", "once", "no-close", "no-wake"]) {
        Ok(args) => args,
        Err(message) => return output.usage("land", &message, USAGE),
    };
    let positional = args.positional();
    let command = positional.first().map_or("status", String::as_str);
    let known: &[&str] = match command {
        "submit" => &["queue", "branch", "issue", "summary", "target"],
        "status" | "show" | "withdraw" => &["queue"],
        "work" => &["queue", "every", "repair"],
        _ => return output.usage("land", &format!("unknown command `{command}`"), USAGE),
    };
    if let Some(name) = args.option_names().into_iter().find(|n| !known.contains(n)) {
        return output.usage("land", &format!("unknown option `--{name}`"), USAGE);
    }
    let store = match land_queue::open_default(args.option("queue")) {
        Ok(store) => store,
        Err(why) => return output.fail("land", &why),
    };
    let queue = Queue {
        store: store.as_ref(),
    };
    let result = match command {
        "status" => status(&queue, args.switch("all")),
        "show" => match positional.get(1) {
            Some(id) => show(&queue, id),
            None => return output.usage("land", "show takes an entry id", USAGE),
        },
        "withdraw" => match positional.get(1) {
            Some(id) => withdraw(&queue, id),
            None => return output.usage("land", "withdraw takes an entry id", USAGE),
        },
        "submit" => submit(&queue, &args),
        "work" => return work(output, store.as_ref(), &args),
        _ => unreachable!("checked above"),
    };
    match result {
        Ok(value) => {
            output.emit(&value, |value| render(command, value));
            0
        }
        Err(why) => output.fail(&format!("land {command}"), &why),
    }
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|_| "cannot run git".to_owned())?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
    }
}

fn top() -> Result<PathBuf, String> {
    git(Path::new("."), &["rev-parse", "--show-toplevel"])
        .map(PathBuf::from)
        .map_err(|_| "Run this inside a checkout of the repository.".to_owned())
}

/// `OWNER/NAME` of `origin`.
fn repository(top: &Path) -> String {
    let url = git(top, &["remote", "get-url", "origin"]).unwrap_or_default();
    let tail = url
        .trim_end_matches(".git")
        .rsplit(|c| c == ':' || c == '/')
        .take(2)
        .collect::<Vec<_>>();
    match tail.as_slice() {
        [name, owner] if !owner.is_empty() && !name.is_empty() => format!("{owner}/{name}"),
        _ => "OpenAgentsInc/openagents".into(),
    }
}

fn submit(queue: &Queue<'_>, args: &Args) -> Result<Value, String> {
    let top = top()?;
    let at = land_queue::now();
    let machine = land_queue::machine();
    let id = land_queue::new_id(at, &machine);
    let target = args.option("target").unwrap_or("main").to_owned();
    let issue = match args.option("issue") {
        Some(text) => Some(
            text.trim_start_matches('#')
                .parse::<u64>()
                .map_err(|_| format!("--issue {text} is not an issue number"))?,
        ),
        None => None,
    };
    let (branch, head) = if let Some(branch) = args.option("branch") {
        let listed = git(
            &top,
            &["ls-remote", "origin", &format!("refs/heads/{branch}")],
        )?;
        let head = listed
            .split_whitespace()
            .next()
            .ok_or_else(|| format!("origin has no branch `{branch}`; push it first"))?
            .to_owned();
        (branch.to_owned(), head)
    } else {
        if !git(&top, &["status", "--porcelain", "--untracked-files=no"])?.is_empty() {
            return Err("This checkout has uncommitted changes; commit them first.".into());
        }
        git(&top, &["fetch", "-q", "origin", &target])?;
        let ahead = git(
            &top,
            &["rev-list", "--count", &format!("origin/{target}..HEAD")],
        )?;
        if ahead == "0" {
            return Err(format!("HEAD has no commits that origin/{target} lacks."));
        }
        let branch = format!("land/{id}");
        git(
            &top,
            &["push", "-q", "origin", &format!("HEAD:refs/heads/{branch}")],
        )?;
        (branch, git(&top, &["rev-parse", "HEAD"])?)
    };
    let summary = match args.option("summary") {
        Some(text) => text.to_owned(),
        None => git(&top, &["log", "-1", "--format=%s", &head]).unwrap_or_else(|_| branch.clone()),
    };
    let entry = Entry {
        id,
        branch,
        target,
        issue,
        close: !args.switch("no-close"),
        author: git(&top, &["config", "user.name"]).unwrap_or_default(),
        machine,
        summary,
        head,
        submitted_at: at,
        state: State::Queued,
        updated_at: at,
        tries: 0,
        commit: None,
        reason: None,
        worker: None,
    };
    queue.submit(&entry)?;
    let ahead = queue
        .entries()?
        .iter()
        .filter(|e| e.state.open() && e.id < entry.id)
        .count();
    let woke = if args.switch("no-wake") {
        None
    } else {
        wake(queue)
    };
    Ok(json!({
        "entry": entry,
        "ahead": ahead,
        "queue": queue.store.location(),
        "woke": woke,
    }))
}

/// Starts the integrator's instance when its heartbeat is stale; what was
/// done, in a sentence.
fn wake(queue: &Queue<'_>) -> Option<String> {
    let beat = queue.worker().ok().flatten()?;
    if land_queue::now().saturating_sub(beat.at) < land_queue::WORKER_STALE_SECS {
        return None;
    }
    let instance = beat.instance?;
    let out = Command::new("gcloud")
        .args([
            "compute",
            "instances",
            "start",
            &instance.name,
            "--zone",
            &instance.zone,
            "--project",
            &instance.project,
            "--async",
            "--quiet",
        ])
        .stdin(Stdio::null())
        .output();
    Some(match out {
        Ok(out) if out.status.success() => format!(
            "The integrator ({}) was not running; started it.",
            instance.name
        ),
        _ => format!(
            "The integrator ({}) is not running and could not be started from here.",
            instance.name
        ),
    })
}

fn status(queue: &Queue<'_>, all: bool) -> Result<Value, String> {
    let entries = queue.entries()?;
    let shown: Vec<&Entry> = entries.iter().filter(|e| all || e.state.open()).collect();
    let worker = queue.worker()?;
    let alive = worker
        .as_ref()
        .is_some_and(|w| land_queue::now().saturating_sub(w.at) < land_queue::WORKER_STALE_SECS);
    Ok(json!({
        "queue": queue.store.location(),
        "worker": worker,
        "worker_alive": alive,
        "open": entries.iter().filter(|e| e.state.open()).count(),
        "entries": shown,
    }))
}

fn show(queue: &Queue<'_>, id: &str) -> Result<Value, String> {
    let entry = queue
        .entry(id)?
        .ok_or_else(|| format!("No entry `{id}` in the queue."))?;
    let records = queue.records(id)?;
    Ok(json!({ "entry": entry, "records": records }))
}

fn withdraw(queue: &Queue<'_>, id: &str) -> Result<Value, String> {
    let mut entry = queue
        .entry(id)?
        .ok_or_else(|| format!("No entry `{id}` in the queue."))?;
    if entry.state != State::Queued {
        return Err(format!(
            "Entry `{id}` is {}, not queued.",
            entry.state.word()
        ));
    }
    entry.state = State::Withdrawn;
    entry.updated_at = land_queue::now();
    queue.put(&entry)?;
    Ok(json!({ "entry": entry }))
}

fn ago(at: u64) -> String {
    let secs = land_queue::now().saturating_sub(at);
    match secs {
        0..=89 => format!("{secs}s"),
        90..=5399 => format!("{}m", secs / 60),
        5400..=172_799 => format!("{}h", secs / 3600),
        _ => format!("{}d", secs / 86_400),
    }
}

fn render(command: &str, value: &Value) -> String {
    match command {
        "status" => {
            let mut text = format!("queue: {}\n", value["queue"].as_str().unwrap_or(""));
            match value["worker"].as_object() {
                Some(w) => text.push_str(&format!(
                    "integrator: {} ({}, seen {} ago){}\n",
                    w.get("machine").and_then(Value::as_str).unwrap_or("?"),
                    if value["worker_alive"].as_bool() == Some(true) {
                        "running"
                    } else {
                        "not running"
                    },
                    ago(w.get("at").and_then(Value::as_u64).unwrap_or(0)),
                    w.get("current")
                        .and_then(Value::as_str)
                        .map(|c| format!(", landing {c}"))
                        .unwrap_or_default()
                )),
                None => text.push_str("integrator: none has run\n"),
            }
            let entries = value["entries"].as_array().cloned().unwrap_or_default();
            if entries.is_empty() {
                text.push_str("Nothing waiting to land.");
                return text;
            }
            let mut rows = vec![vec![
                "ID".to_owned(),
                "STATE".into(),
                "FROM".into(),
                "ISSUE".into(),
                "AGE".into(),
                "TRIES".into(),
                "RESULT".into(),
            ]];
            for e in &entries {
                let result = e["commit"]
                    .as_str()
                    .map(|c| c.get(..10).unwrap_or(c).to_owned())
                    .or_else(|| e["reason"].as_str().map(|r| clip(r, 70)))
                    .unwrap_or_else(|| clip(e["summary"].as_str().unwrap_or(""), 70));
                rows.push(vec![
                    e["id"].as_str().unwrap_or("").to_owned(),
                    e["state"].as_str().unwrap_or("").to_owned(),
                    e["machine"].as_str().unwrap_or("").to_owned(),
                    e["issue"]
                        .as_u64()
                        .map(|n| format!("#{n}"))
                        .unwrap_or_default(),
                    ago(e["submitted_at"].as_u64().unwrap_or(0)),
                    e["tries"].as_u64().unwrap_or(0).to_string(),
                    result,
                ]);
            }
            text.push_str(&table(&rows));
            text
        }
        "show" => {
            let e = &value["entry"];
            let mut text = format!(
                "{} {} ({} from {}, {}): {}\n",
                e["id"].as_str().unwrap_or(""),
                e["state"].as_str().unwrap_or(""),
                e["branch"].as_str().unwrap_or(""),
                e["machine"].as_str().unwrap_or(""),
                e["author"].as_str().unwrap_or(""),
                e["summary"].as_str().unwrap_or("")
            );
            if let Some(c) = e["commit"].as_str() {
                text.push_str(&format!("landed as {c}\n"));
            }
            if let Some(r) = e["reason"].as_str() {
                text.push_str(&format!("reason: {r}\n"));
            }
            for r in value["records"].as_array().cloned().unwrap_or_default() {
                text.push_str(&format!(
                    "\ntry {} on {}: {}{}\n",
                    r["number"],
                    r["worker"].as_str().unwrap_or(""),
                    r["outcome"].as_str().unwrap_or(""),
                    if r["repaired"].as_bool() == Some(true) {
                        " (after a conflict repair turn)"
                    } else {
                        ""
                    }
                ));
                for key in ["checks", "landing", "problems"] {
                    for line in r[key].as_array().cloned().unwrap_or_default() {
                        text.push_str(&format!("  {key}: {}\n", line.as_str().unwrap_or("")));
                    }
                }
            }
            text
        }
        "submit" => {
            let e = &value["entry"];
            let mut text = format!(
                "Queued {} ({}), {} ahead of it, in {}.",
                e["id"].as_str().unwrap_or(""),
                e["branch"].as_str().unwrap_or(""),
                value["ahead"],
                value["queue"].as_str().unwrap_or("")
            );
            if let Some(w) = value["woke"].as_str() {
                text.push_str(&format!("\n{w}"));
            }
            text
        }
        "withdraw" => format!("Withdrew {}.", value["entry"]["id"].as_str().unwrap_or("")),
        _ => value.to_string(),
    }
}

fn clip(text: &str, max: usize) -> String {
    let line = text.lines().next().unwrap_or("");
    if line.chars().count() <= max {
        line.to_owned()
    } else {
        format!("{}…", line.chars().take(max).collect::<String>())
    }
}

/// This VM, when it is a GCE instance.
fn instance() -> Option<Instance> {
    let md = |path: &str| -> Option<String> {
        let out = Command::new("curl")
            .args([
                "-sf",
                "-m",
                "2",
                "-H",
                "Metadata-Flavor: Google",
                &format!("http://metadata.google.internal/computeMetadata/v1/{path}"),
            ])
            .stdin(Stdio::null())
            .output()
            .ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    Some(Instance {
        name: md("instance/name")?,
        zone: md("instance/zone")?.rsplit('/').next()?.to_owned(),
        project: md("project/project-id")?,
    })
}

/// The marker the environment's idle stop reads: present with this
/// process's id while an entry is being landed.
fn busy_marker() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default()
        .join(".openagents/land-queue/busy")
}

fn work(output: &Output, store: &dyn Store, args: &Args) -> u8 {
    let top = match top() {
        Ok(top) => top,
        Err(why) => return output.fail("land work", &why),
    };
    let every = match args.number("every", 20u64) {
        Ok(every) => every.max(1),
        Err(why) => return output.usage("land", &why, USAGE),
    };
    let repair = match args.option("repair") {
        Some("none") => None,
        Some(command) => Some(command.to_owned()),
        None => Some("claude".to_owned()),
    };
    let machine = land_queue::machine();
    let here = instance();
    let queue = Queue { store };
    if let Ok(Some(beat)) = queue.worker()
        && beat.machine != machine
        && land_queue::now().saturating_sub(beat.at) < land_queue::WORKER_STALE_SECS
    {
        return output.fail(
            "land work",
            &format!(
                "Another integrator ({}) is taking this queue; one runs at a time.",
                beat.machine
            ),
        );
    }
    let checks = land_queue::gate();
    let mut effects = Live {
        repo: repository(&top),
        repair,
    };
    let state = busy_marker();
    let _ = std::fs::create_dir_all(state.parent().unwrap_or(Path::new(".")));
    let worktree = state.with_file_name("work");
    eprintln!(
        "land: integrator {machine} on {} (queue {})",
        top.display(),
        store.location()
    );
    let mut failed = false;
    loop {
        let mut beat = Heartbeat {
            machine: machine.clone(),
            at: land_queue::now(),
            current: None,
            instance: here.clone(),
        };
        if let Err(why) = queue.beat(&beat) {
            eprintln!("land: the heartbeat was not written: {why}");
        }
        let next = queue.next(&machine);
        let taken = match next {
            Ok(Some(entry)) => {
                beat.current = Some(entry.id.clone());
                let _ = queue.beat(&beat);
                let _ = std::fs::write(&state, std::process::id().to_string());
                let mut integrator = Integrator {
                    queue: Queue { store },
                    top: top.clone(),
                    worktree: worktree.clone(),
                    machine: machine.clone(),
                    checks: &checks,
                    effects: &mut effects,
                    attempts: coder::task::landing::Plan::ATTEMPTS,
                    backoff: coder::task::landing::Backoff::LANDING,
                };
                let stepped = integrator.step();
                let _ = std::fs::remove_file(&state);
                match stepped {
                    Ok(Some((entry, outcome))) => {
                        let line = json!({
                            "entry": entry.id,
                            "state": entry.state.word(),
                            "commit": entry.commit,
                            "reason": entry.reason,
                        });
                        output.line(&line, |_| format!("{} {:?}", entry.id, outcome));
                        failed |= entry.state == State::Bounced;
                        true
                    }
                    Ok(None) => false,
                    Err(why) => {
                        eprintln!("land: {why}");
                        false
                    }
                }
            }
            Ok(None) => false,
            Err(why) => {
                eprintln!("land: the queue could not be read: {why}");
                false
            }
        };
        if args.switch("once") && !taken {
            break;
        }
        if !taken {
            std::thread::sleep(Duration::from_secs(every));
        }
    }
    if failed { crate::EXIT_FAILURE } else { 0 }
}
