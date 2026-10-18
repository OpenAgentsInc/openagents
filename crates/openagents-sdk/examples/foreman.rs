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
//! OPENAGENTS_TOKEN signs in. OPENAGENTS_LAB_HOST picks the computer by name
//! or ID and OPENAGENTS_LAB_PROJECT the project, defaulting to the first.
//! OPENAGENTS_LAB_REQUEST_DIR is a private, empty folder for the request
//! records the SDK requires before each mutation is sent.
use openagents_sdk::{CancellationToken, Client, Deployment, agent_lab::*};
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

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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
