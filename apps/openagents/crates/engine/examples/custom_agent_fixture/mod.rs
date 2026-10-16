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
    let result = async {
        for (runtime, model) in [
            ("sdk", Some("fixture")),
            ("codex", None),
            ("claude-code", None),
        ] {
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
                task: "Reply with CUSTOM_AGENT_OK".into(),
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
            let follow = agents.command_request(AssignmentAction::FollowUp {
                text: "Reply with CUSTOM_AGENT_OK again".into(),
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
            f.checks.insert(format!("{runtime}_followup"), true);
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
            &json!({"account":ACCOUNT,"checks":f.checks,"host_restarts":f.restarts,"fixture_chats_removed":true,"child_stopped":true,"pass":result.is_ok()}),
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
    let client = Client::new(Deployment::production())?;
    let session = client.authenticate(&f.credential).await?;
    ensure!(
        session.account().id == ACCOUNT,
        "Dedicated account required"
    );
    let agents = client.agents(&session)?;
    let cancel = coder_account::openagents::CancellationToken::new();
    let mut after = None;
    let mut definitions = vec![];
    loop {
        let page = agents.list(after.as_deref(), 100, &cancel).await?;
        definitions.extend(page.data.into_iter().filter(|a| {
            a.spec.name == "Phone custom agent fixture"
                && a.spec.runtime == "sdk"
                && a.spec.model.as_deref() == Some("fixture")
        }));
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
