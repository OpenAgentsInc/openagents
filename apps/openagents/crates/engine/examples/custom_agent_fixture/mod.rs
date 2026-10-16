//! Dedicated-account custom-agent qualification; the parent owns host isolation.
use super::*;
use coder_account::openagents::agents::{Access, AgentSpec, assignments::*};
pub(super) async fn run(f: &mut Fixture) -> Result<()> {
    let client = Client::new(Deployment::production())?;
    let session = client.authenticate(&f.credential).await?;
    ensure!(
        session.account().id == ACCOUNT,
        "Dedicated account required"
    );
    let agents = client.agents(&session)?;
    let cancel = coder_account::openagents::CancellationToken::new();
    let folder = f.root.join("project");
    std::fs::create_dir_all(&folder)?;
    f.ipc.as_ref().unwrap().call(methods::MUTATE,json!({"op":"createSpace","spaceId":f.space,"deviceId":f.device,"path":folder,"name":"Custom agent fixture","gitDetected":false})).await?;
    let mut assignments = Vec::new();
    let mut definitions = Vec::new();
    let mut evidence = Vec::new();
    let result = async {
        for (runtime, model) in [
            ("sdk", Some("fixture")),
            ("codex", None),
            ("claude-code", None),
        ] {
            if runtime == "claude-code"
                && std::env::var_os("OA_CUSTOM_AGENT_FIXTURE_SKIP_CLAUDE").is_some()
            {
                println!("OWNER CHECK: Claude Code needs an isolated provider credential.");
                continue;
            }
            let spec = AgentSpec {
                name: format!("Fixture {runtime}"),
                description: "Dedicated test only".into(),
                instructions: "Respond with CUSTOM_AGENT_OK. Do not run commands or use tools."
                    .into(),
                runtime: runtime.into(),
                model: model.map(str::to_owned),
                access: Access::HostDefault,
            };
            let create = agents.create_request(spec)?;
            let definition = agents.create(&create, &cancel).await?;
            definitions.push(definition.clone());
            ensure!(
                agents.create(&create, &cancel).await? == definition,
                "Definition retry changed identity"
            );
            let request = agents.assignment_request(AssignmentSpec {
                agent: definition.id.clone(),
                agent_revision: definition.revision,
                host: f.device.clone(),
                project: Some(f.space.clone()),
                task: "Please answer.".into(),
            })?;
            let assignment = agents.assign(&request, &cancel).await?;
            assignments.push(assignment.clone());
            ensure!(
                agents.assign(&request, &cancel).await?.id == assignment.id,
                "Assignment retry changed identity"
            );
            let completed = wait(&agents, &assignment.id, &cancel).await?;
            ensure!(
                completed.state == AssignmentState::Completed,
                "{runtime} did not complete: {:?}",
                completed.state
            );
            ensure!(
                completed
                    .result
                    .as_deref()
                    .unwrap_or_default()
                    .contains(if runtime == "sdk" {
                        "received"
                    } else {
                        "CUSTOM_AGENT_OK"
                    }),
                "{runtime} result did not reflect custom instructions"
            );
            f.checks
                .insert(format!("{runtime}_assignment_result"), true);
            if runtime == "codex" {
                let mut next = definition.spec.clone();
                next.instructions = "Respond with NEW_AGENT_REVISION.".into();
                let updated = agents.update(&definition.id, &agents.update_request(definition.revision, next)?, &cancel).await?;
                *definitions.last_mut().unwrap() = updated;
            }
            let follow = agents.command_request(AssignmentAction::FollowUp {
                text: "Answer again.".into(),
            })?;
            let queued = agents.command(&assignment.id, &follow, &cancel).await?;
            ensure!(
                agents
                    .command(&assignment.id, &follow, &cancel)
                    .await?
                    .revision
                    == queued.revision,
                "Follow-up duplicated"
            );
            let completed = wait(&agents, &assignment.id, &cancel).await?;
            ensure!(
                completed.state == AssignmentState::Completed,
                "{runtime} follow-up failed: {:?}",
                completed.state
            );
            if runtime == "codex" {
                ensure!(completed.result.as_deref().is_some_and(|r| r.contains("CUSTOM_AGENT_OK") && !r.contains("NEW_AGENT_REVISION")), "The assigned revision changed after editing the definition");
                f.checks.insert("codex_pinned_revision".into(), true);
            }
            evidence.push(json!({"runtime":runtime,"agent":definition.id,"assigned_revision":1,"assignment":assignment.id,"chat":assignment.chat}));
            f.checks.insert(format!("{runtime}_followup"), true);
            if runtime == "sdk" {
                let request = agents.command_request(AssignmentAction::FollowUp {
                    text: "Wait for cancellation".into(),
                })?;
                agents.command(&assignment.id, &request, &cancel).await?;
                let marker = folder.join("sdk-runner-cancellation-started");
                tokio::time::timeout(Duration::from_secs(30), async {
                    while !marker.exists() {
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                })
                .await
                .context("The external runner did not start its long task")?;
                let pid = std::fs::read_to_string(&marker)?;
                agents
                    .command(
                        &assignment.id,
                        &agents.command_request(AssignmentAction::Stop)?,
                        &cancel,
                    )
                    .await?;
                ensure!(
                    wait(&agents, &assignment.id, &cancel).await?.state == AssignmentState::Stopped,
                    "The SDK runner did not stop"
                );
                ensure!(
                    !Command::new("/bin/kill")
                        .args(["-0", pid.trim()])
                        .stdout(Stdio::null())
                        .stderr(Stdio::null())
                        .status()?
                        .success(),
                    "The SDK runner child was left running"
                );
                f.checks.insert("sdk_cancel_reaped_child".into(), true);
            }
            println!("PASS: {runtime} custom instructions, assignment retry, result, follow-up");
        }
        // A queued assignment survives the host being offline without duplicate execution.
        f.stop().await?;
        let def = &definitions[0];
        let request = agents.assignment_request(AssignmentSpec {
            agent: def.id.clone(),
            agent_revision: def.revision,
            host: f.device.clone(),
            project: Some(f.space.clone()),
            task: "Restart recovery fixture".into(),
        })?;
        let a = agents.assign(&request, &cancel).await?;
        assignments.push(a.clone());
        ensure!(
            a.state == AssignmentState::Queued,
            "Offline assignment unexpectedly ran"
        );
        f.start().await?;
        f.restarts += 1;
        ensure!(
            wait(&agents, &a.id, &cancel).await?.state == AssignmentState::Completed,
            "Queued assignment lost on restart"
        );
        f.checks.insert("offline_restart".into(), true);
        Ok::<_, anyhow::Error>(())
    }
    .await;
    // Tombstone every fixture chat, archive assignments, and delete only our definitions.
    for a in &assignments {
        let observed = agents.assignment(&a.id, &cancel).await?;
        if matches!(
            observed.state,
            AssignmentState::Queued | AssignmentState::Running
        ) {
            agents
                .command(
                    &a.id,
                    &agents.command_request(AssignmentAction::Stop)?,
                    &cancel,
                )
                .await?;
            let _ = wait(&agents, &a.id, &cancel).await;
        }
        let _ = agents
            .command(
                &a.id,
                &agents.command_request(AssignmentAction::Archive)?,
                &cancel,
            )
            .await;
        let ms = chrono::Utc::now().timestamp_millis() + 10000;
        let ops=["chats","sessions"].map(|kind|json!({"kind":kind,"id":a.chat,"op":"delete","hlc":format!("{ms:013}-000000-custom-agent-fixture")}));
        reqwest::Client::new()
            .post(format!("{}/registry/{}/push", f.edge, f.account))
            .bearer_auth(&f.credential)
            .json(&json!({"batch":uuid::Uuid::new_v4().to_string(),"ops":ops}))
            .send()
            .await?
            .error_for_status()?;
    }
    for d in definitions {
        let _ = agents
            .delete(&d.id, &agents.delete_request(d.revision)?, &cancel)
            .await;
    }
    f.cleanup().await?;
    std::fs::write(
        f.root.join("result.json"),
        serde_json::to_vec_pretty(
            &json!({"account":ACCOUNT,"checks":f.checks,"host_restarts":f.restarts,"assignments":evidence,"claude_live":f.checks.contains_key("claude-code_assignment_result"),"fixture_chats_removed":true,"child_stopped":true,"pass":result.is_ok()}),
        )?,
    )?;
    result
}
async fn wait(
    agents: &coder_account::openagents::agents::Agents<'_>,
    id: &str,
    cancel: &coder_account::openagents::CancellationToken,
) -> Result<Assignment> {
    tokio::time::timeout(Duration::from_secs(150), async {
        loop {
            let a = agents.assignment(id, cancel).await?;
            if !matches!(a.state, AssignmentState::Queued | AssignmentState::Running) {
                return Ok(a);
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    })
    .await
    .context("Assignment timed out; inspect the isolated host log")?
}

pub(super) async fn cleanup_phone(f: &mut Fixture) -> Result<()> {
    if std::env::var_os("OA_AGENT_LAB_UI").is_some() {
        return cleanup_lab(f).await;
    }
    let client = Client::new(Deployment::production())?;
    let session = client.authenticate(&f.credential).await?;
    ensure!(
        session.account().id == ACCOUNT,
        "Dedicated account required"
    );
    let agents = client.agents(&session)?;
    let cancel = coder_account::openagents::CancellationToken::new();
    let baseline: Vec<String> =
        serde_json::from_slice(&std::fs::read(f.root.join("agent-baseline.json"))?)?;
    let mut after = None;
    let mut definitions = vec![];
    loop {
        let page = agents.list(after.as_deref(), 100, &cancel).await?;
        definitions.extend(page.data.into_iter().filter(|a| !baseline.contains(&a.id)));
        after = page.next;
        if after.is_none() {
            break;
        }
    }
    f.checks
        .insert("phone_saved_and_relaunched".into(), !definitions.is_empty());
    f.checks.insert(
        "phone_edited".into(),
        definitions.iter().any(|a| a.revision >= 2),
    );
    let mut after = None;
    let mut assignments = vec![];
    loop {
        let page = agents
            .assignments(Some(&f.device), after.as_deref(), &cancel)
            .await?;
        assignments.extend(
            page.data
                .into_iter()
                .filter(|a| definitions.iter().any(|d| d.id == a.spec.agent)),
        );
        after = page.next;
        if after.is_none() {
            break;
        }
    }
    f.checks.insert(
        "phone_assignment_result".into(),
        assignments.iter().any(|a| {
            a.state == AssignmentState::Completed
                && a.result
                    .as_deref()
                    .is_some_and(|r| r.contains("received: Phone custom agent task"))
        }),
    );
    for a in assignments {
        if matches!(a.state, AssignmentState::Queued | AssignmentState::Running) {
            agents
                .command(
                    &a.id,
                    &agents.command_request(AssignmentAction::Stop)?,
                    &cancel,
                )
                .await?;
            let _ = wait(&agents, &a.id, &cancel).await;
        }
        agents
            .command(
                &a.id,
                &agents.command_request(AssignmentAction::Archive)?,
                &cancel,
            )
            .await?;
        let ms = chrono::Utc::now().timestamp_millis() + 10000;
        let ops=["chats","sessions"].map(|kind|json!({"kind":kind,"id":a.chat,"op":"delete","hlc":format!("{ms:013}-000000-custom-agent-fixture")}));
        reqwest::Client::new()
            .post(format!("{}/registry/{}/push", f.edge, f.account))
            .bearer_auth(&f.credential)
            .json(&json!({"batch":uuid::Uuid::new_v4().to_string(),"ops":ops}))
            .send()
            .await?
            .error_for_status()?;
    }
    for d in definitions {
        agents
            .delete(&d.id, &agents.delete_request(d.revision)?, &cancel)
            .await?;
    }
    Ok(())
}

/// Manual real-engine qualification accepts credentials only from a sibling scratch seed.
pub(super) fn install_provider_credentials(f: &Fixture, home: &std::path::Path) -> Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let Some(seed) = std::env::var_os("OA_CUSTOM_AGENT_FIXTURE_PROVIDER_HOME") else {
        return Ok(());
    };
    ensure!(
        std::env::var_os("OA_CUSTOM_AGENT_FIXTURE_RUNNER").is_some(),
        "Use the dedicated agent fixture"
    );
    let seed = PathBuf::from(seed).canonicalize()?;
    ensure!(
        seed.starts_with(
            f.root
                .parent()
                .context("Missing scratch parent")?
                .canonicalize()?
        ),
        "Use a sibling scratch credential seed"
    );
    for relative in [
        ".codex/auth.json",
        ".claude/.credentials.json",
        ".claude.json",
    ] {
        let source = seed.join(relative);
        if !source.is_file() {
            continue;
        }
        let destination = home.join(relative);
        let folder = destination.parent().unwrap();
        std::fs::create_dir_all(folder)?;
        std::fs::set_permissions(folder, std::fs::Permissions::from_mode(0o700))?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(destination)?;
        file.write_all(&std::fs::read(source)?)?;
    }
    Ok(())
}

pub(super) fn provider_environment(
    f: &Fixture,
    home: &std::path::Path,
) -> Result<Vec<(String, std::ffi::OsString)>> {
    let Some(seed) = std::env::var_os("OA_CUSTOM_AGENT_FIXTURE_PROVIDER_HOME") else {
        return Ok(vec![]);
    };
    let seed = PathBuf::from(seed).canonicalize()?;
    ensure!(
        seed.starts_with(
            f.root
                .parent()
                .context("Missing scratch parent")?
                .canonicalize()?
        ),
        "Use a sibling scratch seed"
    );
    let mut env = vec![
        (
            "CLAUDE_CONFIG_DIR".into(),
            home.join(".claude").into_os_string(),
        ),
        ("CODEX_HOME".into(), home.join(".codex").into_os_string()),
    ];
    for (name, file) in [
        ("CLAUDE_CODE_EXECUTABLE", "claude-executable"),
        ("CODEX_EXECUTABLE", "codex-executable"),
        ("CLAUDE_CODE_OAUTH_TOKEN", "claude-token"),
    ] {
        let path = seed.join(file);
        if path.is_file() {
            let value = std::fs::read_to_string(path)?;
            env.push((name.into(), value.trim().into()));
        }
    }
    Ok(env)
}

pub(super) async fn capture_phone_baseline(f: &Fixture) -> Result<()> {
    let client = Client::new(Deployment::production())?;
    let session = client.authenticate(&f.credential).await?;
    ensure!(
        session.account().id == ACCOUNT,
        "Dedicated account required"
    );
    let agents = client.agents(&session)?;
    let cancel = coder_account::openagents::CancellationToken::new();
    let mut ids = vec![];
    let mut after = None;
    loop {
        let page = agents.list(after.as_deref(), 100, &cancel).await?;
        ids.extend(page.data.into_iter().map(|a| a.id));
        after = page.next;
        if after.is_none() {
            break;
        }
    }
    std::fs::write(
        f.root.join("agent-baseline.json"),
        serde_json::to_vec(&ids)?,
    )?;
    Ok(())
}

pub(super) fn project_name(f: &Fixture) -> String {
    if std::env::var_os("OA_CUSTOM_AGENT_UI").is_some() {
        format!("Phone agent {}", &f.nonce[..8])
    } else {
        "Mobile fixture".into()
    }
}

async fn cleanup_lab(f: &mut Fixture) -> Result<()> {
    use coder_account::openagents::labs::*;
    let client = Client::new(Deployment::production())?;
    let session = client.authenticate(&f.credential).await?;
    ensure!(
        session.account().id == ACCOUNT,
        "Dedicated account required"
    );
    let labs = client.labs(&session)?;
    let agents = client.agents(&session)?;
    let cancel = coder_account::openagents::CancellationToken::new();
    let mut runs = vec![];
    let mut after = None;
    loop {
        let p = labs.runs(None, after.as_deref(), &cancel).await?;
        runs.extend(p.data.into_iter().filter(|r| r.experiment.host == f.device));
        after = p.next;
        if after.is_none() {
            break;
        }
    }
    f.checks.insert("phone_lab_saved".into(), !runs.is_empty());
    f.checks.insert(
        "phone_lab_passed".into(),
        runs.iter().any(|r| r.counts(0).passed == 1 && r.complete()),
    );
    for mut r in runs {
        if !r.complete() {
            let request = labs.bind(RunChange {
                request_id: coder_account::openagents::agents::request_id()?,
                expected_revision: r.revision,
                action: RunAction::Stop,
            });
            let _ = labs.change(&r.id, &request, &cancel).await?;
            for _ in 0..40 {
                tokio::time::sleep(Duration::from_millis(500)).await;
                r = labs.run(&r.id, &cancel).await?;
                if r.complete() {
                    break;
                }
            }
        }
        ensure!(r.complete(), "The fixture trial did not stop");
        let request = labs.bind(RunChange {
            request_id: coder_account::openagents::agents::request_id()?,
            expected_revision: r.revision,
            action: RunAction::Archive,
        });
        labs.change(&r.id, &request, &cancel).await?;
        for _ in 0..40 {
            tokio::time::sleep(Duration::from_millis(500)).await;
            if labs.run(&r.id, &cancel).await?.host_settled {
                break;
            }
        }
        let request = labs.bind(DeleteDocument {
            request_id: coder_account::openagents::agents::request_id()?,
            expected_revision: r.version.version,
        });
        labs.delete_document(&r.version.document, &request, &cancel)
            .await?;
        for trial in &r.trials {
            if let Some(chat) = &trial.chat {
                let ms = chrono::Utc::now().timestamp_millis() + 10000;
                let ops=["chats","sessions"].map(|kind|json!({"kind":kind,"id":chat,"op":"delete","hlc":format!("{ms:013}-000000-agent-lab-fixture")}));
                reqwest::Client::new()
                    .post(format!("{}/registry/{}/push", f.edge, f.account))
                    .bearer_auth(&f.credential)
                    .json(&json!({"batch":uuid::Uuid::new_v4().to_string(),"ops":ops}))
                    .send()
                    .await?
                    .error_for_status()?;
            }
        }
    }
    let baseline: Vec<String> =
        serde_json::from_slice(&std::fs::read(f.root.join("agent-baseline.json"))?)?;
    let mut after = None;
    let mut definitions = vec![];
    loop {
        let p = agents.list(after.as_deref(), 100, &cancel).await?;
        definitions.extend(p.data.into_iter().filter(|a| !baseline.contains(&a.id)));
        after = p.next;
        if after.is_none() {
            break;
        }
    }
    f.checks.insert(
        "phone_lab_promoted".into(),
        definitions.iter().any(|d| {
            d.spec.name.contains("phone fixture") && d.spec.instructions.contains("Reply briefly.")
        }),
    );
    for d in definitions {
        agents
            .delete(&d.id, &agents.delete_request(d.revision)?, &cancel)
            .await?;
    }
    Ok(())
}
