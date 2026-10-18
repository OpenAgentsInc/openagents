//! Start from the published Foreman, save it as an Agent Lab experiment, run
//! its tests on a computer, print the report, and with `--promote` save it as
//! an agent (#11926, #12048).
//!
//! `--built-in` starts from the copy in this build instead, for a change to
//! the playbook that isn't published yet. `--publish` (with `--promote`)
//! publishes the saved agent for anyone to start from: a new revision of
//! OPENAGENTS_PUBLISHED_ID when it is set, otherwise a new published agent
//! under OPENAGENTS_PUBLISH_REQUEST_ID (or a fresh request ID).
//!
//! `check-in` runs the Foreman loop instead, with SDK calls only (#12052):
//! `foreman check-in --repo owner/name [--rounds N] [--interval SECONDS]`.
//! Each round reads the account's live runs (`runs`), the repository's claims
//! (`issues`) and the landing queue (`land`), drains the attention stream
//! (`attention`), steers runs (`runs.command`), releases claims no live run
//! holds, asks the owner when a run needs a person (`notify_owner`), and
//! appends every decision as a JSON line to OPENAGENTS_FOREMAN_LOG (stdout
//! when unset). OPENAGENTS_FOREMAN_AGENT and OPENAGENTS_FOREMAN_CHAT name the
//! agent definition and the chat the owner's notification opens; without
//! them the loop logs the question instead of sending it. It runs no shell
//! command and no engine RPC, so any harness can run it.
//!
//! OPENAGENTS_TOKEN signs in. OPENAGENTS_LAB_HOST picks the computer by name
//! or ID and OPENAGENTS_LAB_PROJECT the project, defaulting to the first.
//! OPENAGENTS_LAB_REQUEST_DIR is a private, empty folder for the request
//! records the SDK requires before each mutation is sent.
use openagents_sdk::{
    CancellationToken, Client, Deployment, Session,
    agent_lab::*,
    attention::{Attention, AttentionEvent},
    issues::BoardStatus,
    owner_notifications::OwnerNote,
    runs::{CommandOutcome, RunFilter, SessionCommand},
};
use std::time::Duration;

fn keep<T: serde::Serialize>(name: &str, request: &T) -> Result<(), Box<dyn std::error::Error>> {
    use std::io::Write;
    let dir = std::env::var("OPENAGENTS_LAB_REQUEST_DIR")?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(std::path::Path::new(&dir).join(name))?;
    file.write_all(&serde_json::to_vec(request)?)?;
    file.sync_all()?;
    Ok(())
}

/// What the loop does about one attention event.
#[derive(Debug, PartialEq, Eq)]
enum Step {
    /// Queue a message for a run.
    Nudge { run: String, text: String },
    /// A person has to answer or look.
    AskOwner { title: String, body: String },
    /// Nothing for the loop to do.
    Skip,
}

/// Decide from one event and the claims no live run holds. Pure, so it is
/// testable without an account.
fn decide(event: &AttentionEvent, live: &[String]) -> Step {
    let is_live = |run: &str| live.iter().any(|r| r == run);
    match &event.attention {
        Attention::RunStalled { run, .. } if is_live(run) => Step::Nudge {
            run: run.clone(),
            text: "You have been quiet for a while. Say where you are and what is next.".into(),
        },
        Attention::ClaimNoProgress { issue, holder, .. } if is_live(holder) => Step::Nudge {
            run: holder.clone(),
            text: format!(
                "Issue {issue} has seen no progress. Push what you have or say what is blocking you."
            ),
        },
        Attention::RunAwaitingInput { run, question } => Step::AskOwner {
            title: "An agent is waiting for you".into(),
            body: format!("Run {run} asks: {question}"),
        },
        Attention::RunErrored { run, reason } => Step::AskOwner {
            title: "An agent hit an error".into(),
            body: format!("Run {run} stopped: {reason}"),
        },
        Attention::LandingFailed { entry, reason, .. } => Step::AskOwner {
            title: "A change did not land".into(),
            body: format!("Landing {entry} failed: {reason}"),
        },
        _ => Step::Skip,
    }
}

fn decision(log: &mut dyn std::io::Write, kind: &str, detail: serde_json::Value) {
    let line = serde_json::json!({"at": std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64),
        "decision": kind, "detail": detail});
    let _ = writeln!(log, "{line}");
}

async fn check_in(client: &Client, session: &Session) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let flag = |name: &str| {
        args.iter()
            .position(|a| a == name)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let repo = flag("--repo").ok_or("Pass --repo owner/name: the repository to look after.")?;
    let rounds: u32 = flag("--rounds").map_or(Ok(1), |r| r.parse())?;
    let interval = Duration::from_secs(flag("--interval").map_or(Ok(60), |r| r.parse())?);
    let agent = std::env::var("OPENAGENTS_FOREMAN_AGENT").ok();
    let chat = std::env::var("OPENAGENTS_FOREMAN_CHAT").ok();
    let mut log: Box<dyn std::io::Write> = match std::env::var("OPENAGENTS_FOREMAN_LOG") {
        Ok(path) => Box::new(
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)?,
        ),
        Err(_) => Box::new(std::io::stdout()),
    };
    let cancel = CancellationToken::new();
    let (runs, issues, land) = (
        client.runs(session)?,
        client.issues(session)?,
        client.land(session)?,
    );
    let mut cursor = 0u64;
    for round in 0..rounds {
        let live = runs
            .list(
                RunFilter {
                    live: Some(true),
                    issue: None,
                },
                &cancel,
            )
            .await?;
        let live_ids: Vec<String> = live.iter().map(|r| r.run.clone()).collect();
        let claims = issues.claims(&repo, &cancel).await?;
        let queue = land.status(&cancel).await?;
        decision(
            &mut log,
            "look",
            serde_json::json!({"round": round, "live_runs": live.len(), "claims": claims.len(),
                "landing_waiting": queue.waiting.len(), "landing_failed": queue.failed.len()}),
        );

        // A claim with no live run behind it blocks the issue for everyone.
        for claim in claims
            .iter()
            .filter(|c| c.run.as_ref().is_none_or(|r| !live_ids.contains(r)))
        {
            issues
                .release(
                    &repo,
                    claim.issue,
                    Some("Released by the check-in: no live run holds this claim."),
                    &cancel,
                )
                .await?;
            issues
                .board(&repo, claim.issue, BoardStatus::Ready, &cancel)
                .await?;
            decision(
                &mut log,
                "release_claim",
                serde_json::json!({"issue": claim.issue, "holder": claim.holder}),
            );
        }

        // Drain what is waiting; the stream stays open, so stop when it goes quiet.
        let mut stream = client.attention(session, cursor, &cancel).await?;
        while let Ok(Ok(Some(event))) =
            tokio::time::timeout(Duration::from_secs(3), stream.next()).await
        {
            match decide(&event, &live_ids) {
                Step::Nudge { run, text } => {
                    let receipt = runs
                        .command(
                            &run,
                            &format!("foreman-{}", event.seq),
                            SessionCommand::Message { text },
                            &cancel,
                        )
                        .await?;
                    let accepted = receipt.outcome == CommandOutcome::Accepted;
                    decision(
                        &mut log,
                        "nudge",
                        serde_json::json!({"seq": event.seq, "run": run, "accepted": accepted}),
                    );
                }
                Step::AskOwner { title, body } => {
                    let sent = match (&agent, &chat) {
                        (Some(agent), Some(chat)) => {
                            let note = OwnerNote {
                                agent: agent.clone(),
                                chat: chat.clone(),
                                title,
                                body,
                            };
                            client
                                .notify_owner(
                                    session,
                                    &format!("foreman-notify-{}", event.seq),
                                    note,
                                    &cancel,
                                )
                                .await
                                .is_ok()
                        }
                        _ => false,
                    };
                    decision(
                        &mut log,
                        "ask_owner",
                        serde_json::json!({"seq": event.seq, "sent": sent}),
                    );
                }
                Step::Skip => {}
            }
        }
        cursor = stream.cursor();
        if round + 1 < rounds {
            tokio::time::sleep(interval).await;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(attention: Attention) -> AttentionEvent {
        AttentionEvent {
            seq: 1,
            at: 0,
            agent: None,
            attention,
        }
    }

    #[test]
    fn nudges_live_runs_and_asks_the_owner_about_people_problems() {
        let live = vec!["r1".to_string()];
        let stalled = |run: &str| {
            event(Attention::RunStalled {
                run: run.into(),
                since: 0,
            })
        };
        assert!(matches!(decide(&stalled("r1"), &live), Step::Nudge { .. }));
        assert_eq!(decide(&stalled("gone"), &live), Step::Skip);
        let asking = event(Attention::RunAwaitingInput {
            run: "r1".into(),
            question: "Which branch?".into(),
        });
        assert!(matches!(decide(&asking, &live), Step::AskOwner { .. }));
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::args().nth(1).as_deref() == Some("check-in") {
        let client = Client::new(Deployment::production())?;
        let credential = std::env::var("OPENAGENTS_TOKEN")?;
        let session = client.authenticate(&credential).await?;
        drop(credential);
        return check_in(&client, &session).await;
    }
    let promote = std::env::args().any(|a| a == "--promote");
    let publish = std::env::args().any(|a| a == "--publish");
    let built_in = std::env::args().any(|a| a == "--built-in");
    if publish && !promote {
        return Err("Add --promote: only a saved agent can be published.".into());
    }
    let client = Client::new(Deployment::production())?;
    let credential = std::env::var("OPENAGENTS_TOKEN")?;
    let session = client.authenticate(&credential).await?;
    drop(credential);
    let labs = client.agent_lab(&session)?;
    let cancel = CancellationToken::new();

    let wanted = std::env::var("OPENAGENTS_LAB_HOST").ok();
    let hosts = labs.hosts(&cancel).await?;
    let host = hosts
        .iter()
        .find(|h| wanted.as_ref().is_none_or(|w| &h.id == w || &h.name == w))
        .ok_or("Open the desktop app on the computer that should run Foreman.")?;
    if !host.runtimes.iter().any(|r| r == "claude-code") {
        return Err("Sign in to Claude Code on that computer first.".into());
    }
    let wanted = std::env::var("OPENAGENTS_LAB_PROJECT").ok();
    let project = host
        .projects
        .iter()
        .find(|p| wanted.as_ref().is_none_or(|w| &p.id == w || &p.name == w))
        .ok_or("Choose a project on that computer first.")?;

    let template = if built_in {
        println!("Starting from the Foreman in this build");
        ::agent_lab::templates::foreman()
    } else {
        let published = client
            .published_agent(::agent_lab::templates::FOREMAN_ID, None, &cancel)
            .await?;
        println!(
            "Starting from published Foreman revision {}",
            published.revision
        );
        published.template
    };
    let mut experiment = template.clone();
    experiment.host = host.id.clone();
    experiment.project = project.id.clone();
    let save = labs.bind(SaveDocument {
        request_id: openagents_sdk::agents::request_id()?,
        expected_revision: 0,
        content: Content::Experiment { experiment },
        summary: "Started from Foreman".into(),
    });
    keep("save.json", &save)?;
    let document = labs.save(None, &save, &cancel).await?;
    let version = document
        .versions
        .last()
        .ok_or("The saved experiment has no version.")?
        .number;
    println!(
        "Saved experiment {} version {version} on {} / {}",
        document.id, host.name, project.name
    );

    let start = labs.bind(StartRun {
        request_id: openagents_sdk::agents::request_id()?,
        version,
    });
    keep("start.json", &start)?;
    let mut run = labs.start(&document.id, &start, &cancel).await?;
    println!("Started run {} with {} tests", run.id, run.trials.len());
    while !run.trials.iter().all(|t| t.outcome.finished()) {
        tokio::time::sleep(Duration::from_secs(10)).await;
        run = labs.run(&run.id, &cancel).await?;
    }
    for t in &run.trials {
        let case = &run.experiment.suite.cases[t.case];
        println!("\n## {} — {:?}", case.title, t.outcome);
        if let Some(result) = &t.result {
            println!("{result}");
        }
    }
    let counts = run.counts(0);
    println!(
        "\n{} passed, {} failed, {} need review, {} did not finish",
        counts.passed, counts.failed, counts.review, counts.incomplete
    );

    if promote {
        let request = labs.bind(Promote {
            request_id: openagents_sdk::agents::request_id()?,
            version,
        });
        keep("promote.json", &request)?;
        let agent = labs.promote(&document.id, &request, &cancel).await?;
        println!(
            "Saved agent {} revision {}: it is now in the agent picker.",
            agent.spec.name, agent.revision
        );
        if publish {
            let definition = Pin {
                id: agent.id.clone(),
                revision: agent.revision,
            };
            let published = match std::env::var("OPENAGENTS_PUBLISHED_ID") {
                Ok(id) => {
                    let current = client.published_agent(&id, None, &cancel).await?;
                    let request = labs.bind(UpdatePublishedAgent {
                        request_id: openagents_sdk::agents::request_id()?,
                        expected_revision: current.revision,
                        definition,
                        template,
                    });
                    keep("publish.json", &request)?;
                    labs.update_published(&id, &request, &cancel).await?
                }
                Err(_) => {
                    let request_id = match std::env::var("OPENAGENTS_PUBLISH_REQUEST_ID") {
                        Ok(r) => r,
                        Err(_) => openagents_sdk::agents::request_id()?,
                    };
                    let request = labs.bind(PublishAgent {
                        request_id,
                        definition,
                        template,
                    });
                    keep("publish.json", &request)?;
                    labs.publish(&request, &cancel).await?
                }
            };
            println!(
                "Published {} revision {}: anyone can start from it.",
                published.id, published.revision
            );
        }
    }
    Ok(())
}
