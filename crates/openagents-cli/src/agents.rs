//! `openagents agents`: your saved agents (Foreman and the ones you make in
//! the app or with the SDK). List them, and ask one a question on a computer
//! of yours (#11934). Asking is an ordinary assignment through the
//! OpenAgents SDK, the same the phone's agent picker makes: the agent runs
//! with its saved instructions and access on a computer that's online with
//! its runtime, and its conversation is in the app.

use std::path::PathBuf;
use std::time::{Duration, Instant};

use openagents_sdk::agents::AgentDefinition;
use openagents_sdk::agents::assignments::{Assignment, AssignmentSpec, AssignmentState};
use openagents_sdk::agents::place::{Unplaced, place};
use openagents_sdk::{CancellationToken, Client, Deployment, Session};
use serde_json::{Value, json};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents agents COMMAND
  list
        Your saved agents: name, runtime, model, and what each is for.
  ask NAME QUESTION [--on COMPUTER] [--project PROJECT] [--timeout SECONDS]
        Ask one of your agents and print its answer. It runs on a computer
        of yours that is online in the OpenAgents app with the agent's
        runtime: --on picks the computer and --project its project, by name
        or id. Without --project, the project named like this folder's Git
        repository comes first. It waits up to --timeout seconds (default
        600); a dropped wait doesn't stop the agent, and its conversation
        stays in the app. QUESTION `-` reads it from stdin.
  status [ID]
        Your recent questions to agents and their state, or one by its id.
  bind AGENT_ID --chat CHAT --on COMPUTER --project PROJECT [--task TEXT]
        Bind an existing chat to a saved agent, by its definition ID (never
        its name): from its next turn the chat runs the agent's current
        revision with its instructions, and a host acts on what the
        definition declares (check-ins, standing authority). The chat must
        be a plain chat in PROJECT on COMPUTER. --task is the chat's next
        turn (default: a note that it now runs the agent).
  revise AGENT_ID --on COMPUTER --project PROJECT [--template NAME]
         [--instructions FILE] [--promote] [--timeout SECONDS]
        Draft a new revision of a saved agent by its definition ID: its
        current settings, or Agent Lab template NAME's (keeping the agent's
        name), with instructions from FILE. Runs the 2026-10-17 replay suite
        on COMPUTER and prints each case. With --promote, saves the revision
        only when every case passed. Waits up to --timeout (default 5400).
Example: openagents agents ask Foreman \"what isn't being worked on?\"";

/// What each command does, for the command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("ask", Effect::Publishes),
    Declared::computer("status", Effect::ReadOnly),
    Declared::computer("bind", Effect::Publishes),
    Declared::computer("revise", Effect::Publishes),
];

/// The most a question waits by default: an agent reads GitHub and its
/// tools, so it takes minutes.
const WAIT: u64 = 600;
const POLL: Duration = Duration::from_secs(3);
/// The longest `revise` waits for its replay run: six scenarios of up to
/// fifteen minutes each.
const REVISE_WAIT: u64 = 5400;

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some(command) = words.first() else {
        println!("{USAGE}");
        return 2;
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let rest = &words[1..];
    let args = match Args::parse(rest, &["promote"]) {
        Ok(args) => args,
        Err(message) => return output.usage("agents", &message, USAGE),
    };
    let allowed: &[&str] = match command.as_str() {
        "list" | "status" => &[],
        "ask" => &["on", "project", "timeout"],
        "bind" => &["chat", "on", "project", "task"],
        "revise" => &["on", "project", "template", "instructions", "timeout"],
        // `agents` was an alias of `agent` (the workshop agent) before it
        // listed saved agents; its other commands still go there.
        _ => return crate::agent::run(output, words),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|n| !allowed.contains(n))
    {
        return output.usage("agents", &format!("unknown option `--{name}`"), USAGE);
    }
    let result = match command.as_str() {
        "list" => list(),
        "status" => status(args.positional().first().map(String::as_str)),
        "bind" | "revise" => {
            let positional = args.positional();
            let [agent] = positional else {
                return output.usage(
                    "agents",
                    &format!("{command} needs exactly one agent definition ID"),
                    USAGE,
                );
            };
            let (Some(on), Some(project)) = (args.option("on"), args.option("project")) else {
                return output.usage("agents", "--on and --project are required", USAGE);
            };
            if command == "bind" {
                let Some(chat) = args.option("chat") else {
                    return output.usage("agents", "--chat is required", USAGE);
                };
                bind(agent, chat, on, project, args.option("task"))
            } else {
                let timeout = match args.option("timeout").map(str::parse::<u64>) {
                    None => REVISE_WAIT,
                    Some(Ok(seconds)) if seconds > 0 => seconds,
                    _ => {
                        return output.usage(
                            "agents",
                            "--timeout is a positive number of seconds",
                            USAGE,
                        );
                    }
                };
                revise(
                    agent,
                    on,
                    project,
                    args.option("template"),
                    args.option("instructions"),
                    args.switch("promote"),
                    Duration::from_secs(timeout),
                )
            }
        }
        _ => {
            let positional = args.positional();
            let (Some(name), Some(question)) = (positional.first(), positional.get(1)) else {
                return output.usage("agents", "ask needs an agent name and a question", USAGE);
            };
            if positional.len() > 2 {
                return output.usage("agents", "put the question in quotes", USAGE);
            }
            let timeout = match args.option("timeout").map(str::parse::<u64>) {
                None => WAIT,
                Some(Ok(seconds)) if seconds > 0 => seconds,
                _ => {
                    return output.usage(
                        "agents",
                        "--timeout is a positive number of seconds",
                        USAGE,
                    );
                }
            };
            let question = if question == "-" {
                let mut text = String::new();
                if std::io::Read::read_to_string(&mut std::io::stdin(), &mut text).is_err() {
                    return output.fail("agents", "Couldn't read the question from stdin.");
                }
                text
            } else {
                question.clone()
            };
            ask(
                name,
                question.trim(),
                args.option("on"),
                args.option("project"),
                Duration::from_secs(timeout),
            )
        }
    };
    match result {
        Ok((value, text)) => {
            output.emit(&value, |_| text);
            if value["state"] == "failed" || value["state"] == "stopped" {
                1
            } else {
                0
            }
        }
        Err(message) => output.fail("agents", &message),
    }
}

/// The signed-in account through the OpenAgents SDK, at the site this
/// computer is signed in to.
pub(crate) fn connect() -> Result<(Client, Session), String> {
    let site = crate::keys::Site::open(None)?;
    let origin = site.saved.origin.trim_end_matches('/').to_owned();
    let deployment = if origin == "https://openagents.com" {
        Deployment::production()
    } else {
        Deployment::new(&format!("{origin}/v1"), &origin)
            .map_err(|_| format!("{origin} isn't an OpenAgents site this command can use."))?
    };
    let client = Client::new(deployment).map_err(|e| format!("Couldn't start the client: {e}"))?;
    let session = crate::runtime()
        .block_on(client.authenticate(site.saved.token()))
        .map_err(|_| "Not logged in: log in again with coder login.".to_owned())?;
    Ok((client, session))
}

fn agents(client: &Client, session: &Session) -> Result<Vec<AgentDefinition>, String> {
    let api = client.agents(session).map_err(|e| e.to_string())?;
    let cancel = CancellationToken::new();
    crate::runtime().block_on(async {
        let mut all = Vec::new();
        let mut after: Option<String> = None;
        loop {
            let page = api
                .list(after.as_deref(), 100, &cancel)
                .await
                .map_err(|e| format!("Couldn't read your agents: {e}"))?;
            all.extend(page.data.into_iter().filter(|a| !a.deleted));
            match page.next {
                Some(next) => after = Some(next),
                None => return Ok(all),
            }
        }
    })
}

fn list() -> Result<(Value, String), String> {
    let (client, session) = connect()?;
    let agents = agents(&client, &session)?;
    let text = if agents.is_empty() {
        "You have no saved agents yet. Make one in the OpenAgents app (in Agent Lab, start from Foreman), or with the SDK.".to_owned()
    } else {
        crate::out::table(
            &agents
                .iter()
                .map(|a| {
                    vec![
                        a.spec.name.clone(),
                        a.spec.runtime.clone(),
                        a.spec.model.clone().unwrap_or_default(),
                        a.spec.description.clone(),
                    ]
                })
                .collect::<Vec<_>>(),
        )
    };
    Ok((json!({ "agents": agents }), text))
}

/// The name of the Git repository this command runs in, to prefer the
/// computer project of the same name.
fn repository_name() -> Option<String> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let top = String::from_utf8(out.stdout).ok()?;
    PathBuf::from(top.trim())
        .file_name()?
        .to_str()
        .map(str::to_owned)
}

fn runtime_words(runtime: &str) -> &str {
    match runtime {
        "claude-code" => "Claude Code",
        "codex" => "Codex",
        "sdk" => "its installed Rust runner",
        other => other,
    }
}

fn ask(
    name: &str,
    question: &str,
    computer: Option<&str>,
    project: Option<&str>,
    timeout: Duration,
) -> Result<(Value, String), String> {
    if question.is_empty() {
        return Err("Ask a question.".into());
    }
    let (client, session) = connect()?;
    let mine = agents(&client, &session)?;
    let matching: Vec<&AgentDefinition> = mine
        .iter()
        .filter(|a| a.spec.name.eq_ignore_ascii_case(name))
        .collect();
    let agent = match matching.as_slice() {
        [one] => (*one).clone(),
        [] => {
            let names: Vec<&str> = mine.iter().map(|a| a.spec.name.as_str()).collect();
            return Err(if names.is_empty() {
                "You have no saved agents yet. Make one in the OpenAgents app (in Agent Lab, start from Foreman).".into()
            } else {
                format!(
                    "You have no agent named {name}. Your agents: {}.",
                    names.join(", ")
                )
            });
        }
        _ => {
            return Err(format!(
                "You have more than one agent named {name}; rename one in the app."
            ));
        }
    };
    let cancel = CancellationToken::new();
    let hosts = crate::runtime()
        .block_on(
            client
                .agent_lab(&session)
                .map_err(|e| e.to_string())?
                .hosts(&cancel),
        )
        .map_err(|e| format!("Couldn't check your computers: {e}"))?;
    let prefer = repository_name();
    let (host, project) = place(&hosts, &agent, computer, project, prefer.as_deref()).map_err(|why| match why {
        Unplaced::NoComputer => format!(
            "No computer of yours is online with {} for {}. Open the OpenAgents desktop app on it, then ask again.",
            runtime_words(&agent.spec.runtime),
            agent.spec.name
        ),
        Unplaced::NoProject => "That computer has no matching project. Add the folder as a project in the OpenAgents app, or choose one with --project.".into(),
    })?;
    let api = client.agents(&session).map_err(|e| e.to_string())?;
    let request = api
        .assignment_request(AssignmentSpec {
            agent: agent.id.clone(),
            agent_revision: agent.revision,
            host: host.id.clone(),
            project: Some(project),
            task: question.to_owned(),
        })
        .map_err(|_| "That question is too long. Ask it in fewer words.".to_owned())?;
    let saved = keep(&request)?;
    let assigned = crate::runtime().block_on(api.assign(&request, &cancel));
    let mut current = match assigned {
        Ok(a) => {
            let _ = std::fs::remove_file(&saved);
            a
        }
        Err(e) => {
            return Err(format!(
                "Couldn't tell whether {} got the question ({e}). It's kept at {}; check `openagents agents status` before asking again.",
                agent.spec.name,
                saved.display()
            ));
        }
    };
    eprintln!(
        "Asked {} on {}. Waiting for its answer…",
        agent.spec.name, host.name
    );
    let started = Instant::now();
    while matches!(
        current.state,
        AssignmentState::Queued | AssignmentState::Running
    ) && started.elapsed() < timeout
    {
        std::thread::sleep(POLL);
        if let Ok(a) = crate::runtime().block_on(api.assignment(&current.id, &cancel)) {
            current = a;
        }
    }
    Ok(shown(&agent.spec.name, &host.name, &current))
}

/// Save the request privately before sending, as the SDK asks, so an
/// uncertain send can be checked instead of repeated.
fn keep<T: serde::Serialize>(request: &T) -> Result<PathBuf, String> {
    use std::io::Write;
    let home = std::env::var_os("HOME").ok_or("HOME isn't set.")?;
    let dir = PathBuf::from(home).join(".openagents/agents/requests");
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't save the request: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700));
    }
    let path = dir.join(format!(
        "{}.json",
        openagents_sdk::agents::request_id().map_err(|e| e.to_string())?
    ));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|e| format!("Couldn't save the request: {e}"))?;
    file.write_all(&serde_json::to_vec(request).map_err(|e| e.to_string())?)
        .and_then(|_| file.sync_all())
        .map_err(|e| format!("Couldn't save the request: {e}"))?;
    Ok(path)
}

fn state_word(state: AssignmentState) -> &'static str {
    match state {
        AssignmentState::Queued => "queued",
        AssignmentState::Running => "running",
        AssignmentState::Completed => "completed",
        AssignmentState::Failed => "failed",
        AssignmentState::Stopped => "stopped",
        AssignmentState::Unknown => "unknown",
    }
}

fn shown(agent: &str, computer: &str, a: &Assignment) -> (Value, String) {
    let value = json!({
        "id": a.id,
        "agent": agent,
        "computer": computer,
        "state": state_word(a.state),
        "answer": a.result,
        "chat": a.chat,
    });
    let text = match a.state {
        AssignmentState::Completed => a.result.clone().unwrap_or_default().trim().to_owned(),
        AssignmentState::Queued | AssignmentState::Running => format!(
            "{agent} is still working on {computer}. Its answer will be in its conversation in the OpenAgents app, and in `openagents agents status {}`.",
            a.id
        ),
        _ => format!(
            "{agent} didn't finish on {computer}. Open its conversation in the OpenAgents app to see what happened."
        ),
    };
    (value, text)
}

fn status(id: Option<&str>) -> Result<(Value, String), String> {
    let (client, session) = connect()?;
    let api = client.agents(&session).map_err(|e| e.to_string())?;
    let cancel = CancellationToken::new();
    if let Some(id) = id {
        let a = crate::runtime()
            .block_on(api.assignment(id, &cancel))
            .map_err(|e| format!("Couldn't read that question: {e}"))?;
        return Ok(shown(&a.definition.spec.name, "your computer", &a));
    }
    let page = crate::runtime()
        .block_on(api.assignments(None, None, &cancel))
        .map_err(|e| format!("Couldn't read your questions: {e}"))?;
    let rows: Vec<Vec<String>> = page
        .data
        .iter()
        .map(|a| {
            vec![
                a.id.clone(),
                a.definition.spec.name.clone(),
                state_word(a.state).into(),
                a.spec.task.chars().take(60).collect(),
            ]
        })
        .collect();
    let text = if rows.is_empty() {
        "No questions to your agents yet.".to_owned()
    } else {
        crate::out::table(&rows)
    };
    Ok((json!({ "assignments": page.data }), text))
}

/// One of your saved agents by its definition ID, at its latest revision.
fn agent_by_id(client: &Client, session: &Session, id: &str) -> Result<AgentDefinition, String> {
    if !openagents_sdk::agents::valid_id(id) {
        return Err(
            "Use the agent's definition ID (64 hex characters), not its name: `openagents agents list --json` shows it."
                .into(),
        );
    }
    let api = client.agents(session).map_err(|e| e.to_string())?;
    let agent = crate::runtime()
        .block_on(api.get(id, None, &CancellationToken::new()))
        .map_err(|e| format!("Couldn't read that agent: {e}"))?;
    if agent.deleted {
        return Err("That agent was deleted.".into());
    }
    Ok(agent)
}

/// The computer and project named exactly (by ID or name), as Agent Lab sees them.
fn host_and_project(
    client: &Client,
    session: &Session,
    computer: &str,
    project: &str,
) -> Result<(openagents_sdk::agent_lab::Host, String), String> {
    let hosts = crate::runtime()
        .block_on(
            client
                .agent_lab(session)
                .map_err(|e| e.to_string())?
                .hosts(&CancellationToken::new()),
        )
        .map_err(|e| format!("Couldn't check your computers: {e}"))?;
    let host = hosts
        .into_iter()
        .find(|h| h.id == computer || h.name == computer)
        .ok_or_else(|| {
            format!("No computer of yours named {computer} is online in the OpenAgents app.")
        })?;
    let project = host
        .projects
        .iter()
        .find(|p| p.id == project || p.name == project)
        .map(|p| p.id.clone())
        .ok_or_else(|| format!("{} has no project named {project}.", host.name))?;
    Ok((host, project))
}

fn bind(
    agent: &str,
    chat: &str,
    computer: &str,
    project: &str,
    task: Option<&str>,
) -> Result<(Value, String), String> {
    let (client, session) = connect()?;
    let agent = agent_by_id(&client, &session, agent)?;
    let (host, project) = host_and_project(&client, &session, computer, project)?;
    let task = task.map(str::to_owned).unwrap_or_else(|| {
        format!(
            "This chat now runs your saved definition {} at revision {}. Carry on from your working state; your next check-in has it.",
            agent.id, agent.revision
        )
    });
    let api = client.agents(&session).map_err(|e| e.to_string())?;
    let request = api
        .assignment_request_for_chat(
            AssignmentSpec {
                agent: agent.id.clone(),
                agent_revision: agent.revision,
                host: host.id.clone(),
                project: Some(project),
                task,
            },
            chat,
        )
        .map_err(|_| "Use the chat's full ID and a shorter task.".to_owned())?;
    let saved = keep(&request)?;
    let a = crate::runtime()
        .block_on(api.assign(&request, &CancellationToken::new()))
        .map_err(|e| {
            format!(
                "Couldn't tell whether the chat was bound ({e}). The request is kept at {}; check `openagents agents status` before trying again.",
                saved.display()
            )
        })?;
    let _ = std::fs::remove_file(&saved);
    let text = format!(
        "Bound chat {} to {} (definition {}, revision {}) on {}. The computer adopts it within seconds; its next turn runs the agent.",
        a.chat, agent.spec.name, agent.id, agent.revision, host.name
    );
    Ok((
        json!({
            "assignment": a.id,
            "chat": a.chat,
            "agent": agent.id,
            "revision": agent.revision,
            "computer": host.id,
            "state": state_word(a.state),
        }),
        text,
    ))
}

/// Whether a finished run lets a revision be promoted: every planned case
/// ran and passed, with nothing failed, unfinished or waiting for review.
fn suite_passed(c: &openagents_sdk::agent_lab::Counts) -> bool {
    c.planned > 0
        && c.passed == c.planned
        && c.failed == 0
        && c.pending == 0
        && c.review == 0
        && c.incomplete == 0
}

/// The draft for a new revision of `agent`: the template's settings when
/// one is named (keeping the agent's own name), else the agent's current
/// settings, then the instructions file if given.
fn revision_draft(
    agent: &AgentDefinition,
    template: Option<&str>,
    instructions: Option<String>,
) -> Result<openagents_sdk::agent_lab::Draft, String> {
    use openagents_sdk::agent_lab::{Draft, Pin, templates};
    let mut spec = match template {
        None => agent.spec.clone(),
        Some("foreman") => {
            // The skills are folded into the instructions, as Agent Lab saves it.
            let mut spec = templates::foreman()
                .draft
                .agent_spec()
                .map_err(|e| e.message)?;
            spec.name = agent.spec.name.clone();
            spec
        }
        Some(other) => {
            return Err(format!(
                "No Agent Lab template named {other}. Templates: foreman."
            ));
        }
    };
    if let Some(text) = instructions {
        if text.trim().is_empty() {
            return Err("The instructions file is empty.".into());
        }
        spec.instructions = text;
    }
    Ok(Draft {
        spec,
        source: Some(Pin {
            id: agent.id.clone(),
            revision: agent.revision,
        }),
        components: vec![],
    })
}

fn revise(
    agent: &str,
    computer: &str,
    project: &str,
    template: Option<&str>,
    instructions: Option<&str>,
    promote: bool,
    timeout: Duration,
) -> Result<(Value, String), String> {
    use openagents_sdk::agent_lab::{Content, Promote, SaveDocument, StartRun, replay};
    let instructions = instructions
        .map(|path| std::fs::read_to_string(path).map_err(|e| format!("Couldn't read {path}: {e}")))
        .transpose()?;
    let (client, session) = connect()?;
    let agent = agent_by_id(&client, &session, agent)?;
    let (host, project) = host_and_project(&client, &session, computer, project)?;
    let mut experiment = replay::experiment(revision_draft(&agent, template, instructions)?);
    experiment.host = host.id.clone();
    experiment.project = project;
    let labs = client.agent_lab(&session).map_err(|e| e.to_string())?;
    let cancel = CancellationToken::new();
    let id = || openagents_sdk::agents::request_id().map_err(|e| e.to_string());
    let save = labs.bind(SaveDocument {
        request_id: id()?,
        expected_revision: 0,
        content: Content::Experiment { experiment },
        summary: format!(
            "Revision of {} after revision {}",
            agent.spec.name, agent.revision
        ),
    });
    let saved = keep(&save)?;
    let document = crate::runtime()
        .block_on(labs.save(None, &save, &cancel))
        .map_err(|e| {
            format!(
                "Couldn't save the draft ({e}); the request is kept at {}.",
                saved.display()
            )
        })?;
    let _ = std::fs::remove_file(&saved);
    let version = document
        .versions
        .last()
        .ok_or("The saved draft has no version.")?
        .number;
    let start = labs.bind(StartRun {
        request_id: id()?,
        version,
    });
    let mut run = crate::runtime()
        .block_on(labs.start(&document.id, &start, &cancel))
        .map_err(|e| format!("Couldn't start the replay run: {e}"))?;
    eprintln!(
        "Running the replay suite ({} cases) for {} on {}: run {}…",
        run.trials.len(),
        agent.spec.name,
        host.name,
        run.id
    );
    let started = Instant::now();
    while !run.trials.iter().all(|t| t.outcome.finished()) && started.elapsed() < timeout {
        std::thread::sleep(Duration::from_secs(15));
        if let Ok(r) = crate::runtime().block_on(labs.run(&run.id, &cancel)) {
            run = r;
        }
    }
    let counts = run.counts(0);
    let cases: Vec<Value> = run
        .trials
        .iter()
        .map(|t| {
            json!({
                "case": run.experiment.suite.cases.get(t.case).map(|c| c.id.clone()),
                "outcome": t.outcome,
            })
        })
        .collect();
    let passed = suite_passed(&counts);
    let promoted = if promote && passed {
        let request = labs.bind(Promote {
            request_id: id()?,
            version,
        });
        let saved = keep(&request)?;
        let updated = crate::runtime()
            .block_on(labs.promote(&document.id, &request, &cancel))
            .map_err(|e| {
                format!(
                    "The suite passed but promoting failed ({e}); the request is kept at {}.",
                    saved.display()
                )
            })?;
        let _ = std::fs::remove_file(&saved);
        Some(updated.revision)
    } else {
        None
    };
    let text = format!(
        "{} passed, {} failed, {} need review, {} did not finish, of {}. {}",
        counts.passed,
        counts.failed,
        counts.review,
        counts.incomplete,
        counts.planned,
        match (promoted, promote, passed) {
            (Some(r), _, _) => format!("Promoted: {} is now at revision {r}.", agent.spec.name),
            (None, true, _) => format!(
                "Not promoted: {} stays at revision {}.",
                agent.spec.name, agent.revision
            ),
            (None, false, true) => "Passed; run again with --promote to save it.".into(),
            (None, false, false) => "Not passed.".into(),
        }
    );
    Ok((
        json!({
            "agent": agent.id,
            "from_revision": agent.revision,
            "document": document.id,
            "version": version,
            "run": run.id,
            "cases": cases,
            "counts": counts,
            "passed": passed,
            "promoted_revision": promoted,
        }),
        text,
    ))
}

#[cfg(test)]
mod revise_tests {
    use super::*;
    use openagents_sdk::agent_lab::Counts;

    fn agent() -> AgentDefinition {
        AgentDefinition {
            id: "a".repeat(64),
            revision: 3,
            deleted: false,
            spec: openagents_sdk::agents::AgentSpec {
                name: "Night shift".into(),
                description: "Keeps watch".into(),
                instructions: "Old".into(),
                runtime: "claude-code".into(),
                model: None,
                access: openagents_sdk::agents::Access::HostDefault,
                fallbacks: vec![],
                autonomy: None,
            },
        }
    }

    /// #12057: only a run where every case passed may be promoted.
    #[test]
    fn only_a_clean_run_promotes() {
        let all = Counts {
            planned: 6,
            passed: 6,
            ..Counts::default()
        };
        assert!(suite_passed(&all));
        for broken in [
            Counts {
                failed: 1,
                passed: 5,
                ..all.clone()
            },
            Counts {
                review: 1,
                passed: 5,
                ..all.clone()
            },
            Counts {
                incomplete: 1,
                passed: 5,
                ..all.clone()
            },
            Counts {
                pending: 1,
                passed: 5,
                ..all.clone()
            },
            Counts::default(),
        ] {
            assert!(!suite_passed(&broken), "{broken:?}");
        }
    }

    /// A revision pins the agent it revises; a template keeps the agent's
    /// name and brings its declared autonomy.
    #[test]
    fn a_revision_pins_its_agent_and_keeps_its_name() {
        let a = agent();
        let draft = revision_draft(&a, Some("foreman"), Some("New".into())).unwrap();
        let source = draft.source.as_ref().unwrap();
        assert_eq!((source.id.as_str(), source.revision), (a.id.as_str(), 3));
        assert_eq!(draft.spec.name, "Night shift");
        assert_eq!(draft.spec.instructions, "New");
        assert!(
            draft
                .spec
                .autonomy
                .as_ref()
                .is_some_and(|x| x.check_ins && x.authority)
        );
        // Without an instructions file, the template brings its skills (#12048).
        let templated = revision_draft(&a, Some("foreman"), None).unwrap();
        assert!(
            templated
                .spec
                .instructions
                .contains("Dispatching issue work:\n")
        );
        let plain = revision_draft(&a, None, None).unwrap();
        assert_eq!(plain.spec, a.spec);
        assert!(revision_draft(&a, Some("nope"), None).is_err());
        assert!(revision_draft(&a, None, Some("  ".into())).is_err());
    }
}
