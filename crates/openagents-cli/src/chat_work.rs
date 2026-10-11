//! `openagents chat work --issues SPEC`: hand several GitHub issues to
//! Coder on this computer, one issue flow each
//! ([`coder::task::issue_run`]).
//!
//! Each issue gets a thread of its own, titled with the issue, bound to
//! its flow's task, so the desktop and a paired phone show it like any
//! chat's Coder run. Issues run one at a time, or `--parallel N` at once,
//! each in its own worktree; landing is serialized so parallel flows
//! rebase onto each other. Each flow is handed to a process of its own,
//! so it keeps working if this command or its shell ends. An issue another claim holds (a claim comment
//! within the repository's claim window, not released) is skipped, and so
//! is a closed one. Every flow's events stream here, marked with the
//! issue, and each issue ends with one `issue` line saying what happened.

use std::io::Write;
use std::path::Path;
use std::time::Duration;

use coder::task::issue_run::{self, Handed, Land, Reference, Refused, Runner, Tracker};
use coder::task::local::{self, Local, State};
use openagents_chat::coder_events::{self, CoderEvent, Line};
use openagents_chat::service::Command;
use openagents_chat::tool_groups::Stream;
use serde_json::{Value, json};
use tokio::sync::mpsc;

use openagents_chat::client::{Client, new_id};

use super::{Failure, Printer, event, failed};
use crate::out::Output;
use crate::{Args, EXIT_FAILURE};

/// The most flows a queue runs at once.
const MAX_PARALLEL: u64 = 4;
/// How often a flow's task is read.
const POLL: Duration = Duration::from_millis(300);

/// What a queue's workers tell the printing loop.
enum Told {
    Started {
        issue: u64,
        thread: String,
        task: String,
        project: String,
        url: String,
    },
    Line {
        issue: u64,
        line: Box<Line>,
    },
    Done {
        issue: u64,
        outcome: String,
        message: String,
        thread: Option<String>,
        task: Option<String>,
        commits: Vec<String>,
    },
}

/// `chat work`.
pub(super) async fn work(output: &Output, args: &Args) -> Result<u8, Failure> {
    let spec = args
        .option("issues")
        .ok_or_else(|| {
            Failure::Usage("`chat work` needs --issues NUMBERS or --issues LABEL".into())
        })?
        .to_owned();
    let target = super::placement::Target::parse(args.option("on")).map_err(Failure::Usage)?;
    let on_boat = target == super::placement::Target::Boat;
    let engine_fallback = args.switch("engine-fallback");
    // The briefed agent is the default engine (#11258); `--engine bare`
    // keeps the Coder issue flow.
    let briefed = match args.option("engine").unwrap_or("briefed") {
        "briefed" => true,
        "bare" => false,
        other => {
            return Err(Failure::Usage(format!(
                "--engine is `briefed` or `bare`, not `{other}`"
            )));
        }
    };
    if briefed {
        return briefed_work(output, args, &spec, target).await;
    }
    if target != super::placement::Target::Here
        && !engine_fallback
        && !(on_boat
            && (args.option("engine-logins") == Some("boat")
                || (args.option("engine-logins").is_none()
                    && std::env::var("OA_BOAT_ENGINE_LOGINS").ok().as_deref() == Some("boat"))))
    {
        super::boat::codex_chatgpt_login().map_err(|why| Failure::Refused(format!(
            "Cloud Codex cannot use your ChatGPT login: {why}. No engine was started. Use --engine-fallback to allow an API key or Grok Build."
        )))?;
    }

    let most = match target {
        super::placement::Target::Boat => super::boat::MAX_PARALLEL,
        super::placement::Target::Gce => super::gce::MAX_PARALLEL,
        super::placement::Target::Here => MAX_PARALLEL,
    };
    let parallel: u64 = args.number("parallel", 1).map_err(Failure::Usage)?;
    if !(1..=most).contains(&parallel) {
        return Err(Failure::Usage(format!("--parallel is 1 to {most}")));
    }
    if !on_boat && (args.option("engine-logins").is_some() || args.option("template").is_some()) {
        return Err(Failure::Usage(
            "--engine-logins and --template go with --on boat".into(),
        ));
    }
    let land = args
        .option("land")
        .map(Land::parse)
        .transpose()
        .map_err(Failure::Usage)?;
    let here =
        std::env::current_dir().map_err(|_| failed("This command has no working directory."))?;
    let checkout = local::checkout(&here).map_err(failed)?;
    let repository = issue_run::Gh.repository(&checkout.top).map_err(failed)?;
    let project = issue_run::Policy::load(&checkout.top)
        .map_err(failed)?
        .project;
    let numbers = tokio::task::spawn_blocking({
        let (repository, spec) = (repository.clone(), spec.clone());
        move || issue_run::select_in(&issue_run::Gh, &repository, &spec, &project)
    })
    .await
    .map_err(|_| failed("the issues could not be listed"))?
    .map_err(failed)?;
    if numbers.is_empty() {
        return Err(failed(format!(
            "No open issue in {repository} matches `{spec}`."
        )));
    }
    if target == super::placement::Target::Gce {
        let request = super::gce::Request {
            repository,
            numbers,
            parallel,
            land,
            engine_fallback,
        };
        return super::gce::work(output, request).await;
    }
    if on_boat {
        let logins = args
            .option("engine-logins")
            .map(str::to_owned)
            .or_else(|| std::env::var("OA_BOAT_ENGINE_LOGINS").ok())
            .map_or(Ok(super::boat::EngineLogins::ApiKeys), |word| {
                super::boat::EngineLogins::parse(&word)
            })
            .map_err(Failure::Usage)?;
        let request = super::boat::Request {
            repository,
            numbers,
            parallel,
            land,
            engine_fallback,
            logins,
            template: args.option("template").map(str::to_owned).or_else(|| {
                std::env::var("OA_BOAT_TEMPLATE")
                    .ok()
                    .filter(|t| !t.is_empty())
            }),
            build: std::env::var("OA_BOAT_BUILD").is_ok_and(|v| v == "1"),
        };
        return super::boat::work(output, request).await;
    }
    let mut backend = super::open(args, None, false, &mut Printer::new(output)).await?;
    let store = local::default_store();
    event(
        output,
        json!({"event": "queue", "repository": repository, "issues": numbers, "parallel": parallel}),
    );
    if !output.json() {
        eprintln!(
            "Coder works {} issue{} of {repository}, {} at a time: {}",
            numbers.len(),
            if numbers.len() == 1 { "" } else { "s" },
            parallel,
            numbers
                .iter()
                .map(|n| format!("#{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    let (sender, mut receiver) = mpsc::unbounded_channel::<Told>();
    let queue = std::sync::Arc::new(std::sync::Mutex::new(
        numbers
            .iter()
            .copied()
            .collect::<std::collections::VecDeque<u64>>(),
    ));
    let mut workers = Vec::new();
    for _ in 0..parallel {
        let (queue, sender, store, dir) = (
            std::sync::Arc::clone(&queue),
            sender.clone(),
            store.clone(),
            here.clone(),
        );
        let repository = repository.clone();
        workers.push(std::thread::spawn(move || {
            loop {
                let next = queue.lock().ok().and_then(|mut queue| queue.pop_front());
                let Some(issue) = next else {
                    return;
                };
                worker(&store, &dir, &repository, issue, land, &sender);
            }
        }));
    }
    drop(sender);

    let mut active: Vec<String> = Vec::new();
    let mut results: Vec<Value> = Vec::new();
    let mut tools: std::collections::HashMap<u64, Stream> = std::collections::HashMap::new();
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    let mut stopping = false;
    loop {
        let told = tokio::select! {
            told = receiver.recv() => told,
            _ = &mut interrupt, if !stopping => {
                stopping = true;
                queue.lock().map(|mut queue| queue.clear()).ok();
                let local = Local::here(store.clone());
                for task in &active {
                    let _ = local.stop(task);
                }
                eprintln!(
                    "Stopping: no more issues start, and each running flow ends at its next step \
                     and says so on its issue."
                );
                continue;
            }
        };
        let Some(told) = told else {
            break;
        };
        match told {
            Told::Started {
                issue,
                thread,
                task,
                project,
                url,
            } => {
                if stopping {
                    let _ = Local::here(store.clone()).stop(&task);
                }
                active.push(task.clone());
                bind(&mut backend, &thread, &task, &project, issue).await;
                event(
                    output,
                    json!({"event": "coder", "issue": issue, "thread": thread, "accepted": true,
                        "task": {"host": local::LOCAL_HOST, "task": task, "project": project,
                            "issue": issue, "issue_url": url}}),
                );
                if !output.json() {
                    eprintln!("#{issue}: Coder took {url} as task {task} (thread {thread}).");
                }
            }
            Told::Line { issue, line } => {
                show(output, issue, tools.entry(issue).or_default(), &line);
            }
            Told::Done {
                issue,
                outcome,
                message,
                thread,
                task,
                commits,
            } => {
                if let Some(task) = &task {
                    active.retain(|running| running != task);
                }
                let record = json!({"event": "issue", "issue": issue, "outcome": outcome,
                    "message": message, "thread": thread, "task": task, "commits": commits});
                event(output, record.clone());
                if !output.json() {
                    println!("#{issue}: {outcome}. {message}");
                    let _ = std::io::stdout().flush();
                }
                results.push(record);
            }
        }
    }
    for worker in workers {
        let _ = tokio::task::spawn_blocking(move || worker.join()).await;
    }
    let good = |record: &Value| {
        matches!(
            record["outcome"].as_str(),
            Some("landed" | "pull_request" | "skipped" | "closed")
        )
    };
    let landed = results
        .iter()
        .filter(|r| matches!(r["outcome"].as_str(), Some("landed" | "pull_request")))
        .count();
    event(
        output,
        json!({"event": "queue_done", "issues": results.len(), "landed": landed}),
    );
    if !output.json() {
        eprintln!("Coder landed {landed} of {} issue(s).", results.len());
    }
    Ok(if results.iter().all(good) {
        0
    } else {
        EXIT_FAILURE
    })
}

/// `chat work` with the briefed agent ([`super::work_briefed`]).
async fn briefed_work(
    output: &Output,
    args: &Args,
    spec: &str,
    target: super::placement::Target,
) -> Result<u8, Failure> {
    if args.option("engine-logins").is_some() || args.option("template").is_some() {
        return Err(Failure::Usage(
            "--engine-logins and --template go with --engine bare --on boat".into(),
        ));
    }
    let here =
        std::env::current_dir().map_err(|_| failed("This command has no working directory."))?;
    let checkout = local::checkout(&here).map_err(failed)?;
    let repository = issue_run::Gh.repository(&checkout.top).map_err(failed)?;
    let policy = issue_run::Policy::load(&checkout.top).map_err(failed)?;
    let land = super::work_briefed::land_word(args.option("land"), policy.land == Land::Main)
        .map_err(Failure::Usage)?;
    let parallel: u64 = args.number("parallel", 1).map_err(Failure::Usage)?;
    if !(1..=MAX_PARALLEL).contains(&parallel) {
        return Err(Failure::Usage(format!("--parallel is 1 to {MAX_PARALLEL}")));
    }
    let project = policy.project;
    let numbers = tokio::task::spawn_blocking({
        let (repository, spec) = (repository.clone(), spec.to_owned());
        move || issue_run::select_in(&issue_run::Gh, &repository, &spec, &project)
    })
    .await
    .map_err(|_| failed("the issues could not be listed"))?
    .map_err(failed)?;
    if numbers.is_empty() {
        return Err(failed(format!(
            "No open issue in {repository} matches `{spec}`."
        )));
    }
    let output = *output;
    let top = checkout.top.clone();
    tokio::task::spawn_blocking(move || {
        if target == super::placement::Target::Here {
            super::work_briefed::work_here(&output, &top, &repository, &numbers, parallel, land)
        } else {
            super::work_briefed::work_cloud(&output, &repository, &numbers, land)
        }
    })
    .await
    .map_err(|_| failed("the work stopped"))?
}

/// Names the issue's thread and binds its task, so the apps show the run.
async fn bind(backend: &mut Client, thread: &str, task: &str, project: &str, issue: u64) {
    let steps = [
        Command::Create {
            chat: thread.to_owned(),
        },
        Command::Rename {
            chat: thread.to_owned(),
            title: format!("Coder: issue #{issue}"),
        },
        Command::BindCoder {
            chat: thread.to_owned(),
            host: local::LOCAL_HOST.into(),
            task: task.to_owned(),
            project: Some(project.to_owned()),
        },
    ];
    for step in steps {
        if let Err(why) = backend.apply(step).await {
            eprintln!("openagents chat: #{issue}'s thread could not record its run ({why}).");
            return;
        }
    }
}

/// Keep the cloud's announced Codex route as its only admitted provider.
fn cloud_codex_only(mut settings: coder::task::settings::Coder) -> coder::task::settings::Coder {
    use coder::task::{capacity::Provider, settings::Choice};
    settings
        .providers
        .retain(|choice| choice.provider == Provider::Codex);
    if settings.providers.is_empty() {
        settings.providers.push(Choice::new(Provider::Codex));
    }
    for provider in [
        Provider::Claude,
        Provider::Grok,
        Provider::Devin,
        Provider::OpenCode,
    ] {
        if !settings.disabled.contains(&provider) {
            settings.disabled.push(provider);
        }
    }
    settings
}

#[cfg(test)]
mod cloud_tests {
    #[test]
    fn strict_cloud_runs_admit_only_codex() {
        use coder::task::{capacity::Provider, settings};
        let pinned = super::cloud_codex_only(settings::Coder::default());
        assert_eq!(pinned.provider_list(), vec![Provider::Codex]);
        pinned.validate().unwrap();
        let disabled = settings::Coder {
            disabled: vec![Provider::Codex],
            ..settings::Coder::default()
        };
        assert!(super::cloud_codex_only(disabled).validate().is_err());
    }
}

/// One issue's flow, on a worker thread: begin, follow, finish.
fn worker(
    store: &Path,
    dir: &Path,
    repository: &str,
    issue: u64,
    land: Option<Land>,
    sender: &mpsc::UnboundedSender<Told>,
) {
    let mut runner = Runner::new(store.to_path_buf());
    if std::env::var("OA_CLOUD_CODEX_ONLY").ok().as_deref() == Some("1") {
        let local = Local::here(store.to_path_buf());
        let mut settings = match local.settings() {
            Ok(settings) => settings.clone(),
            Err(message) => {
                let _ = sender.send(Told::Done {
                    issue,
                    outcome: "not_started".into(),
                    message,
                    thread: None,
                    task: None,
                    commits: Vec::new(),
                });
                return;
            }
        };
        settings = cloud_codex_only(settings);
        runner.local = std::sync::Arc::new(local.with_settings(settings));
    }
    runner.land = land;
    runner.skip_claimed = true;
    let thread = new_id();
    let reference = Reference {
        repository: Some(repository.to_owned()),
        number: issue,
    };
    let lost_task = (std::env::var("OPENAGENTS_CODER_PLACEMENT").as_deref() == Ok("gce"))
        .then(|| std::env::var("OPENAGENTS_CODER_RECOVER_TASK").ok())
        .flatten();
    let started =
        match runner.begin_recovering(dir, &reference, Some(&thread), lost_task.as_deref()) {
            Ok(started) => started,
            Err(refused) => {
                let outcome = match refused {
                    Refused::Claimed(_) if lost_task.is_none() => "skipped",
                    Refused::Claimed(_) => "not_started",
                    Refused::Closed(_) => "closed",
                    Refused::Failed(_) => "not_started",
                };
                let _ = sender.send(Told::Done {
                    issue,
                    outcome: outcome.into(),
                    message: refused.to_string(),
                    thread: None,
                    task: None,
                    commits: Vec::new(),
                });
                return;
            }
        };
    let task = started.record.task.clone();
    let _ = sender.send(Told::Started {
        issue,
        thread: thread.clone(),
        task: task.clone(),
        project: started.record.project.clone(),
        url: started.issue.url.clone(),
    });
    // The flow goes to a process of its own (the `microcoder` engine), so
    // it survives this command, the shell, or the terminal closing; it is
    // worked here only when no engine can take it.
    let driver = local::controller().ok();
    let finishing = std::thread::spawn(move || started.hand_off(driver.as_deref()));
    let ended = |store: &Path| {
        issue_run::load(store, &task).is_none_or(|flow| flow.finished || issue_run::orphaned(&flow))
    };
    let mut follow = Local::new(store.to_path_buf()).follow(&task, Some(&thread), None);
    loop {
        match follow.poll() {
            Ok((lines, state)) => {
                for line in lines {
                    let _ = sender.send(Told::Line {
                        issue,
                        line: Box::new(line),
                    });
                }
                if state != State::Running {
                    break;
                }
            }
            Err(_) if finishing.is_finished() && ended(store) => break,
            Err(_) => {}
        }
        // A flow whose process is gone never ends: stop following it.
        if finishing.is_finished()
            && issue_run::load(store, &task).is_some_and(|flow| issue_run::orphaned(&flow))
        {
            break;
        }
        std::thread::sleep(POLL);
    }
    let flow = match finishing.join() {
        Ok(Handed::Here(flow)) => Some(*flow),
        Ok(Handed::Detached { .. }) => issue_run::load(store, &task).filter(|flow| flow.finished),
        Err(_) => None,
    };
    let (outcome, message, commits) = match flow {
        Some(flow) => (flow.link.outcome, flow.closing, flow.link.commits),
        None => (
            "failed".into(),
            "The flow ended unexpectedly.".into(),
            Vec::new(),
        ),
    };
    let _ = sender.send(Told::Done {
        issue,
        outcome,
        message,
        thread: Some(thread),
        task: Some(task),
        commits,
    });
}

/// One flow's event: an NDJSON line with its issue, or a line on stderr
/// marked with it.
pub(super) fn show(output: &Output, issue: u64, tools: &mut Stream, line: &Line) {
    if output.json() {
        if let Ok(mut value) = serde_json::to_value(line) {
            value["issue"] = json!(issue);
            println!("{value}");
            let _ = std::io::stdout().flush();
        }
        return;
    }
    // Tool calls group as every surface groups them (#10117).
    match tools.push(line.seq, &line.event) {
        Some(lines) => {
            for row in lines {
                eprintln!("#{issue}   {row}");
            }
            return;
        }
        None => {
            for row in tools.flush() {
                eprintln!("#{issue}   {row}");
            }
        }
    }
    if let CoderEvent::Step(step) = &line.event
        && step.kind == coder_events::StepKind::Reply
    {
        return;
    }
    if let Some(text) = coder_events::text(&line.event) {
        for row in text.lines() {
            eprintln!("#{issue} {row}");
        }
    }
}
