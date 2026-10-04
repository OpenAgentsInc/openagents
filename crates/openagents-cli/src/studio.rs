//! `openagents studio`: the Agent Studio coordinator on this computer
//! (`coder::task::studio`, docs/verse/agent-studio.md): seats bound to
//! routes, goals a lead plans, plan entries released to the task inbox as
//! their dependencies finish, shared memory, and messages to seats. The
//! host's auto-start sweep runs the same reconciliation every ten seconds;
//! these commands drive it before any Verse view exists.

use std::path::{Path, PathBuf};

use coder::task::studio::{
    self, Delivery, GoalView, MemoryKind, NewGoal, Party, PlanOutcome, Released, Repository, Role,
    Seat, Studio, View,
};
use coder::task::{Store, autostart};
use serde_json::{Value, json};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents studio COMMAND [OPTIONS]
  seat set NAME --route ROUTE [--role ROLE] [--look LOOK] [--desk N]
                  Add a seat or change one: a lead plans goals, a worker
                  (the default ROLE) works plan entries. ROUTE is PROVIDER:MODEL, the
                  form the host's auto-start routes use.
  seat list       Every seat with its route, desk, and current task.
  seat remove NAME
                  Remove a seat that holds no task waiting to start.
  goal submit TEXT --workspace LABEL [--lead SEAT]
                  Start a goal on an admitted workspace: its lead plans it.
  goal list       Every goal with its status and progress.
  plan list GOAL  The goal's plan entries with each task's progress.
  plan accept GOAL FILE
                  Deliver a plan for a goal whose lead has none, answering
                  its decision. An invalid plan becomes the decision.
  message SEAT TEXT
                  Message a seat, or every seat with `everyone`. A running
                  task reads it now when its engine reads steering;
                  otherwise its next briefing carries it.
  memory add TEXT [--kind KIND] [--goal GOAL]
                  Add shared memory every briefing carries: a convention,
                  decision, or note (the default).
  memory list     The shared memory.
  sync            Release plan entries whose dependencies are done, and
                  read finished leads' plans, now.
Every command takes --tasks DIR (the Coder task store, default
$OPENAGENTS_TASKS or ~/.openagents/tasks) and --root DIR (the host root,
default ~/.openagents/host), whose serve.json names the workspaces and
whose auto-start policy starts released tasks. A seat's route must be one
the policy admits for its tasks to start.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("seat set", Effect::LocalWrite),
    Declared::computer("seat list", Effect::ReadOnly),
    Declared::computer("seat remove", Effect::LocalWrite),
    Declared::computer("goal submit", Effect::Publishes),
    Declared::computer("goal list", Effect::ReadOnly),
    Declared::computer("plan list", Effect::ReadOnly),
    Declared::computer("plan accept", Effect::Publishes),
    Declared::computer("message", Effect::LocalWrite),
    Declared::computer("memory add", Effect::LocalWrite),
    Declared::computer("memory list", Effect::ReadOnly),
    Declared::computer("sync", Effect::Publishes),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some(first) = words.first() else {
        return output.usage("studio", "a command is required", USAGE);
    };
    if matches!(first.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("studio", &message, USAGE),
    };
    let store = args
        .option("tasks")
        .map_or_else(coder::task::local::default_store, PathBuf::from);
    let root = args.option("root").map_or_else(default_root, PathBuf::from);
    let words: Vec<&str> = args.positional().iter().map(String::as_str).collect();
    let now = autostart::unix_now();
    let result = match words.as_slice() {
        ["seat", "set", name] => seat_set(output, &store, name, &args),
        ["seat", "list"] => read(&store, &root).map(|view| seats(output, &view)),
        ["seat", "remove", name] => open(&store, &root).and_then(|(_, mut studio)| {
            studio.remove_seat(name).map_err(|e| e.to_string())?;
            output.emit(&json!({"removed": name}), |_| {
                format!("Removed seat {name}.")
            });
            Ok(())
        }),
        ["goal", "submit", text @ ..] if !text.is_empty() => {
            goal_submit(output, &store, &root, &text.join(" "), &args, now)
        }
        ["goal", "list"] => read(&store, &root).map(|view| goals(output, &view)),
        ["plan", "list", goal] => read(&store, &root).and_then(|view| plan(output, &view, goal)),
        ["plan", "accept", goal, file] => plan_accept(output, &store, &root, goal, file, now),
        ["message", seat, text @ ..] if !text.is_empty() => {
            message(output, &store, &root, seat, &text.join(" "), now)
        }
        ["memory", "add", text @ ..] if !text.is_empty() => {
            memory_add(output, &store, &root, &text.join(" "), &args)
        }
        ["memory", "list"] => read(&store, &root).map(|view| memory(output, &view)),
        ["sync"] => sync(output, &store, &root, now),
        _ => return output.usage("studio", "unknown or incomplete command", USAGE),
    };
    match result {
        Ok(()) => 0,
        Err(message) => output.fail("studio", &message),
    }
}

fn default_root() -> PathBuf {
    std::env::var_os("HOME")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents/host")
}

/// The task store and its studio, which notes releases in `root`'s
/// auto-start journal.
fn open(store: &Path, root: &Path) -> Result<(Store, Studio), String> {
    let tasks = Store::open(store).map_err(|e| e.to_string())?;
    let studio = Studio::open(store)
        .map_err(|e| e.to_string())?
        .with_host_root(root);
    Ok((tasks, studio))
}

/// The studio's view after a reconciliation, or an empty one when the
/// store holds no studio (nothing is created to read it).
fn read(store: &Path, root: &Path) -> Result<View, String> {
    if !Studio::present(store) {
        return Ok(View {
            sequence: 0,
            seats: Vec::new(),
            goals: Vec::new(),
            memory: Vec::new(),
            messages: Vec::new(),
        });
    }
    let (mut tasks, mut studio) = open(store, root)?;
    reconcile(&mut tasks, &mut studio, store, autostart::unix_now())?;
    Ok(studio.view(&tasks))
}

fn reconcile(
    tasks: &mut Store,
    studio: &mut Studio,
    store: &Path,
    now: u64,
) -> Result<Vec<Released>, String> {
    let reply =
        |task: &str| coder::task::local::result_in(Some(store), task).map(|run| run.summary);
    studio
        .reconcile(tasks, now, &reply)
        .map_err(|e| e.to_string())
}

fn seat_set(output: &Output, store: &Path, name: &str, args: &Args) -> Result<(), String> {
    let route = args
        .option("route")
        .ok_or("seat set needs --route PROVIDER:MODEL")?;
    let route = studio::parse_route(route).map_err(|e| e.to_string())?;
    let mut studio = Studio::open(store).map_err(|e| e.to_string())?;
    let existing = studio.state().seat(name).cloned();
    let desk = match args.option("desk") {
        Some(desk) => desk.parse().map_err(|_| "--desk takes a number")?,
        None => existing
            .as_ref()
            .map_or_else(|| studio.free_desk(), |seat| seat.desk),
    };
    let seat = Seat {
        name: name.to_owned(),
        role: match args.option("role").unwrap_or("worker") {
            "lead" => Role::Lead,
            "worker" => Role::Worker,
            other => return Err(format!("--role is lead or worker, not `{other}`")),
        },
        route,
        look: args
            .option("look")
            .map(str::to_owned)
            .or(existing.map(|seat| seat.look))
            .unwrap_or_else(|| "default".into()),
        desk,
    };
    studio.set_seat(seat.clone()).map_err(|e| e.to_string())?;
    output.emit(&json!({"seat": seat}), |_| {
        format!(
            "Seat {} is a {} on {}, at desk {}.",
            seat.name,
            role(seat.role),
            seat.route,
            seat.desk
        )
    });
    Ok(())
}

fn role(role: Role) -> &'static str {
    match role {
        Role::Lead => "lead",
        Role::Worker => "worker",
    }
}

fn goal_submit(
    output: &Output,
    store: &Path,
    root: &Path,
    text: &str,
    args: &Args,
    now: u64,
) -> Result<(), String> {
    let label = args
        .option("workspace")
        .ok_or("goal submit needs --workspace LABEL")?;
    let settings = coder_host::settings::ServeSettings::load(root).map_err(|e| e.to_string())?;
    let path = settings.workspaces.get(label).ok_or_else(|| {
        format!(
            "the host at {} admits no workspace `{label}`; add it with `coder host init --workspace {label}=PATH`",
            root.display()
        )
    })?;
    let (mut tasks, mut studio) = open(store, root)?;
    let (goal_id, lead) = studio
        .submit_goal(
            &mut tasks,
            NewGoal {
                text: text.to_owned(),
                repository: Repository {
                    label: label.to_owned(),
                    path: path.to_string_lossy().into_owned(),
                },
                lead: args.option("lead").map(str::to_owned),
            },
            now,
        )
        .map_err(|e| e.to_string())?;
    output.emit(&json!({"goal_id": goal_id, "lead": lead}), |_| {
        format!(
            "Goal {goal_id} submitted; seat {} plans it as task {}.",
            lead.seat, lead.task_id
        )
    });
    Ok(())
}

fn plan_accept(
    output: &Output,
    store: &Path,
    root: &Path,
    goal: &str,
    file: &str,
    now: u64,
) -> Result<(), String> {
    let bytes = if file == "-" {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut std::io::stdin().lock(), &mut bytes)
            .map_err(|e| e.to_string())?;
        bytes
    } else {
        std::fs::read(file).map_err(|e| format!("cannot read {file}: {e}"))?
    };
    let (mut tasks, mut studio) = open(store, root)?;
    match studio
        .accept_plan(&mut tasks, goal, &bytes, now)
        .map_err(|e| e.to_string())?
    {
        PlanOutcome::Accepted { released } => {
            output.emit(&json!({"accepted": true, "released": released}), |_| {
                format!(
                    "Plan accepted for goal {goal}; {} task(s) started waiting in the inbox.",
                    released.len()
                )
            });
            Ok(())
        }
        PlanOutcome::Decision(decision) => {
            output.emit(&json!({"accepted": false, "decision": decision}), |_| {
                format!(
                    "The plan is not valid; goal {goal} waits on a decision:\n{}",
                    decision
                        .reasons
                        .iter()
                        .map(|reason| format!("- {reason}"))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            });
            Err("the plan is not valid".into())
        }
    }
}

fn message(
    output: &Output,
    store: &Path,
    root: &Path,
    seat: &str,
    text: &str,
    now: u64,
) -> Result<(), String> {
    let to = match seat.trim_start_matches('@') {
        "everyone" | "all" => Party::Everyone,
        name => Party::Seat { name: name.into() },
    };
    let (tasks, mut studio) = open(store, root)?;
    let sent = studio
        .message(&tasks, Party::Person, to, text, now)
        .map_err(|e| e.to_string())?;
    output.emit(&json!({"messages": sent}), |_| {
        sent.iter()
            .map(|message| match &message.delivery {
                Delivery::Steered { task_id } => {
                    format!("{}: sent to its running task {task_id}.", message.to)
                }
                _ => format!("{}: its next task's briefing carries it.", message.to),
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(())
}

fn memory_add(
    output: &Output,
    store: &Path,
    root: &Path,
    text: &str,
    args: &Args,
) -> Result<(), String> {
    let kind = match args.option("kind").unwrap_or("note") {
        "convention" => MemoryKind::Convention,
        "decision" => MemoryKind::Decision,
        "note" => MemoryKind::Note,
        other => {
            return Err(format!(
                "--kind is convention, decision, or note, not `{other}`"
            ));
        }
    };
    let (_, mut studio) = open(store, root)?;
    let sequence = studio
        .remember(kind, Party::Person, args.option("goal"), text)
        .map_err(|e| e.to_string())?;
    output.emit(&json!({"sequence": sequence}), |_| "Remembered.".into());
    Ok(())
}

fn sync(output: &Output, store: &Path, root: &Path, now: u64) -> Result<(), String> {
    let (mut tasks, mut studio) = open(store, root)?;
    let released = reconcile(&mut tasks, &mut studio, store, now)?;
    output.emit(&json!({"released": released}), |_| {
        if released.is_empty() {
            "Nothing new to release.".into()
        } else {
            released
                .iter()
                .map(|item| format!("Released {} to seat {}.", item.task_id, item.seat))
                .collect::<Vec<_>>()
                .join("\n")
        }
    });
    Ok(())
}

fn word(value: &Value) -> String {
    value.as_str().unwrap_or("").to_owned()
}

fn seats(output: &Output, view: &View) {
    let value = json!({"seats": view.seats});
    output.emit(&value, |_| {
        if view.seats.is_empty() {
            return "No seats yet: `openagents studio seat set NAME --route ROUTE`.".into();
        }
        let mut rows = vec![vec![
            "SEAT".into(),
            "ROLE".into(),
            "ROUTE".into(),
            "DESK".into(),
            "TASK".into(),
        ]];
        for item in &view.seats {
            rows.push(vec![
                item.seat.name.clone(),
                role(item.seat.role).into(),
                item.seat.route.to_string(),
                item.seat.desk.to_string(),
                match (&item.task_id, item.progress) {
                    (Some(task), Some(progress)) => {
                        format!("{task} ({})", word(&json!(progress)))
                    }
                    _ => "idle".into(),
                },
            ]);
        }
        crate::out::table(&rows)
    });
}

fn goals(output: &Output, view: &View) {
    output.emit(&json!({"goals": view.goals}), |_| {
        if view.goals.is_empty() {
            return "No goals yet.".into();
        }
        view.goals
            .iter()
            .map(|goal| {
                let mut line = format!(
                    "{}  {}  {}/{} tasks over  {}",
                    goal.goal_id,
                    word(&json!(goal.status)),
                    goal.final_tasks,
                    goal.total_tasks,
                    goal.text.lines().next().unwrap_or("")
                );
                if let Some(decision) = &goal.decision {
                    for reason in &decision.reasons {
                        line.push_str(&format!("\n    decision: {reason}"));
                    }
                }
                line
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
}

fn plan(output: &Output, view: &View, goal: &str) -> Result<(), String> {
    let found: &GoalView = view
        .goals
        .iter()
        .find(|item| item.goal_id == goal)
        .ok_or_else(|| format!("no goal is `{goal}`"))?;
    output.emit(&json!({"goal": found}), |_| {
        if found.entries.is_empty() {
            return format!(
                "Goal {goal} has no plan yet; its lead task {} is {}.",
                found.lead_task_id,
                word(&json!(found.lead_progress))
            );
        }
        let mut rows = vec![vec![
            "ID".into(),
            "SEAT".into(),
            "PROGRESS".into(),
            "AFTER".into(),
            "TITLE".into(),
        ]];
        for entry in &found.entries {
            rows.push(vec![
                entry.id.clone(),
                entry.seat.clone(),
                word(&json!(entry.progress)),
                entry.depends_on.join(","),
                entry.title.clone(),
            ]);
        }
        crate::out::table(&rows)
    });
    Ok(())
}

fn memory(output: &Output, view: &View) {
    output.emit(&json!({"memory": view.memory}), |_| {
        if view.memory.is_empty() {
            return "No shared memory yet.".into();
        }
        view.memory
            .iter()
            .map(|entry| {
                format!(
                    "{}  {}  {}",
                    entry.sequence,
                    word(&json!(entry.kind)),
                    entry.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
}
