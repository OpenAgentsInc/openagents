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
Example: openagents agents ask Foreman \"what isn't being worked on?\"";

/// What each command does, for the command tree.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("ask", Effect::Publishes),
    Declared::computer("status", Effect::ReadOnly),
];

/// The most a question waits by default: an agent reads GitHub and its
/// tools, so it takes minutes.
const WAIT: u64 = 600;
const POLL: Duration = Duration::from_secs(3);

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
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("agents", &message, USAGE),
    };
    let allowed: &[&str] = match command.as_str() {
        "list" | "status" => &[],
        "ask" => &["on", "project", "timeout"],
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
fn connect() -> Result<(Client, Session), String> {
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
        "You have no saved agents yet. Make one in the OpenAgents app (Agent Lab › Start from Foreman), or with the SDK.".to_owned()
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
                "You have no saved agents yet. Make one in the OpenAgents app (Agent Lab › Start from Foreman).".into()
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
