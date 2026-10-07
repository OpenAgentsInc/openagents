//! `openagents studio`: the Agent Studio coordinator on this computer
//! (`coder::task::studio`, docs/verse/agent-studio.md): seats bound to
//! routes, goals a lead plans, plan entries released to the task inbox as
//! their dependencies finish, shared memory, and messages to seats. The
//! host's auto-start sweep runs the same reconciliation every ten seconds;
//! these commands drive it before any Verse view exists.
//!
//! Every action Everglade's panels take on the running host (decisions,
//! reviews, merges, steering, and the live view) goes through the host's
//! control socket in [`crate::studio_host`], as do `goal submit` and
//! `message` when a host answers there.

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
  up [--repo PATH] [--workspace LABEL] [--team TEAM] [--sim]
     [--no-verse] [--no-host] [--controller PATH] [--coder PATH]
     [--verse PATH] [--control-socket PATH] [--full-access]
                  Launch the studio on a repository: admit it as a host
                  workspace, turn auto-start on for the team's routes,
                  seat a team, start the host if none runs, and open
                  Verse in Everglade. The default team is a lead and two
                  workers on the signed-in coding agents; TEAM is
                  NAME=ROUTE,NAME=ROUTE,... with the lead first, ROUTE
                  being PROVIDER:MODEL or a provider alone. --sim opens
                  the simulated team on a scratch repository with no
                  model spend. --full-access gives the team's runs the
                  host user's reads and network, which session and sdk
                  seats need. Options are remembered for the next up.
  down            Stop and undo only what up started and changed.
  host [--coder PATH] [--control-socket PATH]
                  Start this computer's host as up does, with no
                  repository, team, or Verse, when none answers. Verse
                  runs it when you confirm starting a host at the
                  workshop agent's desk; down stops it.
  seat set NAME --route ROUTE [--role ROLE] [--look LOOK] [--desk N]
                  Add a seat or change one: a lead plans goals, a worker
                  (the default ROLE) works plan entries. ROUTE is
                  PROVIDER[/ENGINE]:MODEL, the form the host's auto-start
                  routes use; a claude or codex route may name its engine,
                  session or loop, which overrides the owner's host-wide
                  engine setting for the seat's tasks.
  seat list       Every seat with its route, desk, and current task.
  seat remove NAME
                  Remove a seat that holds no task waiting to start.
  lead-review on|off|status
                  Whether the lead reviews a worker's green change before
                  the person's merge decision (on by default), or print
                  the current setting.
  seat pause|resume|stop SEAT
                  Pause a seat (it keeps its task and takes no new one),
                  resume it, or stop it: cancel its task, return that task
                  to the board, and pause the seat.
  goal submit TEXT --workspace LABEL [--lead SEAT]
                  Start a goal on an admitted workspace: its lead plans it.
                  Goes through the running host when one answers.
  goal list       Every goal with its status and progress.
  plan list GOAL  The goal's plan entries with each task's progress.
  plan accept GOAL FILE
                  Deliver a plan for a goal whose lead has none, answering
                  its decision. An invalid plan becomes the decision.
  message SEAT TEXT
                  Message a seat, or every seat with `everyone`. A running
                  task reads it now when its engine reads steering;
                  otherwise its next briefing carries it. Goes through the
                  running host when one answers.
  status          The studio on the running host: goals, each seat's
                  activity, station, and task, and how many decisions wait.
  tasks [GOAL]    Every studio task, or one goal's: its identity, plan
                  entry, seat, status, and the entries it waits on.
  log SEAT        The seat's log tail: what its engine did last.
  decisions       The open decisions: questions, approvals with the step
                  they ask to take, and goals waiting on a plan.
  answer DECISION [TEXT] [--file PATH] [--always]
                  Answer a decision: `allow` or `deny` an approval, answer
                  a question, or give a goal its plan (--file PATH, `-`
                  for stdin). --always approves the step and keeps the
                  standing rule the approval offers for its seat.
  review TASK [--diff]
                  The task's change: its revisions, files, and line counts;
                  --diff prints the diff.
  merge TASK [--head REV]
                  Merge the task's reviewed change into its checkout's
                  branch. Nothing is pushed. --head refuses when the change
                  moved past the revision you reviewed.
  request-changes TASK TEXT [--head REV]
                  Send the change back to the task's seat with TEXT.
  reject TASK [REASON] [--head REV]
                  Close the task; its worktree stays until it is archived.
  task cancel|retry|prioritize TASK
                  Cancel a planned or running task, plan a failed or
                  cancelled one again under a new identity, or move a
                  planned task ahead of its goal's other planned tasks.
  task reassign TASK SEAT
                  Give a planned task to another seat.
  watch [--interval SECONDS] [--limit N]
                  Print the studio, then each change as it happens: one
                  JSON line each under --json. Stops after N lines.
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
the policy admits for its tasks to start.
The commands that act on the running host (status through watch, and goal
submit and message when a host answers) reach it through its control
socket: --control-socket PATH, by default the one the OpenAgents app and
`openagents host serve --control` open. They print the host's refusal code
and message. A task, goal, or decision may be named by a unique prefix of
its identity.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("up", Effect::LocalWrite),
    Declared::computer("down", Effect::LocalWrite),
    Declared::computer("host", Effect::LocalWrite),
    Declared::computer("seat set", Effect::LocalWrite),
    Declared::computer("seat list", Effect::ReadOnly),
    Declared::computer("seat remove", Effect::LocalWrite),
    Declared::computer("lead-review on", Effect::LocalWrite),
    Declared::computer("lead-review off", Effect::LocalWrite),
    Declared::computer("lead-review status", Effect::ReadOnly),
    Declared::computer("seat pause", Effect::Publishes),
    Declared::computer("seat resume", Effect::Publishes),
    Declared::computer("seat stop", Effect::Publishes),
    Declared::computer("goal submit", Effect::Publishes),
    Declared::computer("goal list", Effect::ReadOnly),
    Declared::computer("plan list", Effect::ReadOnly),
    Declared::computer("plan accept", Effect::Publishes),
    Declared::computer("message", Effect::LocalWrite),
    Declared::computer("memory add", Effect::LocalWrite),
    Declared::computer("memory list", Effect::ReadOnly),
    Declared::computer("sync", Effect::Publishes),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("tasks", Effect::ReadOnly),
    Declared::computer("log", Effect::ReadOnly),
    Declared::computer("decisions", Effect::ReadOnly),
    Declared::computer("answer", Effect::Publishes),
    Declared::computer("review", Effect::ReadOnly),
    Declared::computer("merge", Effect::LocalWrite),
    Declared::computer("request-changes", Effect::Publishes),
    Declared::computer("reject", Effect::Publishes),
    Declared::computer("task cancel", Effect::Publishes),
    Declared::computer("task retry", Effect::Publishes),
    Declared::computer("task prioritize", Effect::Publishes),
    Declared::computer("task reassign", Effect::Publishes),
    Declared::computer("watch", Effect::LongRunning),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some(first) = words.first() else {
        return output.usage("studio", "a command is required", USAGE);
    };
    if matches!(first.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let switches: Vec<&str> = crate::studio_up::SWITCHES
        .iter()
        .chain(crate::studio_host::SWITCHES)
        .copied()
        .collect();
    let args = match Args::parse(words, &switches) {
        Ok(args) => args,
        Err(message) => return output.usage("studio", &message, USAGE),
    };
    let store = args
        .option("tasks")
        .map_or_else(coder::task::local::default_store, PathBuf::from);
    let root = args.option("root").map_or_else(default_root, PathBuf::from);
    let words: Vec<&str> = args.positional().iter().map(String::as_str).collect();
    let now = autostart::unix_now();
    if let ["up" | "down" | "host"] = words.as_slice() {
        let own = args.option("root").is_none();
        let paths = crate::studio_up::Paths::new(
            root,
            store,
            args.option("control-socket").map(PathBuf::from),
            own,
        );
        return match words[0] {
            "up" => crate::studio_up::up(output, &args, &paths),
            "host" => crate::studio_up::host_up(output, &args, &paths),
            _ => crate::studio_up::down(output, &paths),
        };
    }
    if let Some(code) = crate::studio_host::dispatch(output, &words, &args) {
        return code;
    }
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
        ["lead-review", "status"] => read_studio(&store, &root).map(|studio| {
            lead_review(output, studio.as_ref().is_none_or(Studio::lead_review));
        }),
        ["lead-review", value @ ("on" | "off")] => {
            open(&store, &root).and_then(|(_, mut studio)| {
                let on = *value == "on";
                studio.set_lead_review(on).map_err(|e| e.to_string())?;
                lead_review(output, on);
                Ok(())
            })
        }
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
/// auto-start journal and gives each its own worktree under `root`.
fn open(store: &Path, root: &Path) -> Result<(Store, Studio), String> {
    let tasks = Store::open(store).map_err(|e| e.to_string())?;
    let studio = Studio::open(store)
        .map_err(|e| e.to_string())?
        .with_host_root(root)
        .with_worktrees(coder::task::studio::git::worktrees_dir(root));
    Ok((tasks, studio))
}

/// The studio the store holds, or none when it holds no studio (nothing
/// is created to read it).
fn read_studio(store: &Path, root: &Path) -> Result<Option<Studio>, String> {
    if !Studio::present(store) {
        return Ok(None);
    }
    open(store, root).map(|(_, studio)| Some(studio))
}

fn lead_review(output: &Output, on: bool) {
    output.emit(&json!({"lead_review": on}), |_| {
        if on {
            "The lead reviews each green change before the merge decision.".into()
        } else {
            "Green changes go to the merge decision without the lead's review.".into()
        }
    });
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
            spend: Default::default(),
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
            "SPENT".into(),
            "TASK".into(),
        ]];
        for item in &view.seats {
            rows.push(vec![
                item.seat.name.clone(),
                role(item.seat.role).into(),
                item.seat.route.to_string(),
                item.seat.desk.to_string(),
                item.spend.label(),
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
    output.emit(&json!({"goals": view.goals, "spend": view.spend}), |_| {
        if view.goals.is_empty() {
            return "No goals yet.".into();
        }
        let mut lines: Vec<String> = view
            .goals
            .iter()
            .map(|goal| {
                let mut line = format!(
                    "{}  {}  {}/{} tasks over  {} spent  {}",
                    goal.goal_id,
                    word(&json!(goal.status)),
                    goal.final_tasks,
                    goal.total_tasks,
                    goal.spend.label(),
                    goal.text.lines().next().unwrap_or("")
                );
                if let Some(decision) = &goal.decision {
                    for reason in &decision.reasons {
                        line.push_str(&format!("\n    decision: {reason}"));
                    }
                }
                line
            })
            .collect();
        lines.push(format!("Studio spend: {}", view.spend.label()));
        lines.join("\n")
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
            "SPENT".into(),
            "TITLE".into(),
        ]];
        for entry in &found.entries {
            rows.push(vec![
                entry.id.clone(),
                entry.seat.clone(),
                word(&json!(entry.progress)),
                entry.depends_on.join(","),
                entry.spend.label(),
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
