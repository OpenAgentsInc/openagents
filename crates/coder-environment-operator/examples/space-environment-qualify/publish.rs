use super::*;
use coder_environment_operator::space::{BuildAccepted, BuildState, TemplateClient, template_name};
use computer_driver::{Artifact, Outcome, Secret};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct Work {
    job: String,
    inputs_digest: String,
    base: Artifact,
    native_recipe_digest: String,
    namespace_issued: bool,
    namespace_ready: bool,
    build_issued: bool,
    accepted: Option<Accepted>,
    rootfs: Option<String>,
    base_requested: bool,
    template: Option<Value>,
}

#[derive(Serialize, Deserialize)]
struct Accepted {
    job_id: String,
    template_name: String,
}

fn save(path: &Path, work: &Work) -> Result<()> {
    use std::io::Write;
    let temp = path.with_extension("writing");
    let mut file = fs::File::create(&temp)?;
    file.write_all(&serde_json::to_vec_pretty(work)?)?;
    file.sync_all()?;
    fs::rename(temp, path)?;
    Ok(())
}

fn done<T>(outcome: Outcome<T>) -> Result<T> {
    match outcome {
        Outcome::Done { value } => Ok(value),
        Outcome::Failed { reason } | Outcome::Unknown { reason } => Err(reason.into()),
    }
}

pub(super) async fn cleanup(state: &Path, args: &[String]) -> Result<()> {
    let job: BuildJob = serde_json::from_slice(&fs::read(
        args.first().ok_or("Choose the owned build job.")?,
    )?)?;
    job.validate()?;
    if job.inputs.environment != ENVIRONMENT {
        return Err("Choose this qualification's build.".into());
    }
    let client = TemplateClient::new(
        &std::env::var("SPACE_API_URL")?,
        Secret::new(std::env::var("SPACE_API_TOKEN")?),
    )?;
    let accepted = done(client.delete_owned(&job).await)?;
    fs::write(
        state.join("publication-cleanup-accepted.json"),
        serde_json::to_vec_pretty(&accepted)?,
    )?;
    if accepted["absent"] != true {
        let operation = accepted["operation_id"]
            .as_str()
            .ok_or("The deletion has no operation identity.")?;
        let deadline = now_ms() + 180_000;
        loop {
            let observed = done(client.operation(operation).await)?;
            if observed["state"] == "succeeded" {
                fs::write(
                    state.join("publication-cleanup-complete.json"),
                    serde_json::to_vec_pretty(&observed)?,
                )?;
                break;
            }
            if now_ms() >= deadline
                || matches!(
                    observed["state"].as_str(),
                    Some("failed" | "canceled" | "expired")
                )
            {
                return Err("The publication deletion did not finish.".into());
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    println!(
        "{}",
        json!({"job":job.id,"template":template_name(&job),"deleted":true})
    );
    Ok(())
}

/// A definite failed native build may be replaced before it publishes any
/// artifact. Unknown submissions never enter this path. Keep its complete
/// frozen request and accepted identity as a separate attempt record.
pub(super) async fn retry_failed(state: &Path, args: &[String]) -> Result<()> {
    let job: BuildJob = serde_json::from_slice(&fs::read(
        args.first().ok_or("Choose the frozen build job.")?,
    )?)?;
    job.validate()?;
    if job.inputs.environment != ENVIRONMENT
        || job.phase != Phase::PublishingSpace
        || now_ms() >= job.deadline_ms
    {
        return Err("Use this qualification's current clean build.".into());
    }
    let path = state.join("publication-work.json");
    let original = fs::read(&path)?;
    let mut work: Work = serde_json::from_slice(&original)?;
    if work.job != job.id
        || work.inputs_digest != job.artifact_inputs_digest()
        || work.rootfs.is_some()
        || work.base_requested
    {
        return Err("This publication cannot be replaced.".into());
    }
    let accepted = work
        .accepted
        .as_ref()
        .ok_or("An unknown submission cannot be repeated.")?;
    let client = TemplateClient::new(
        &std::env::var("SPACE_API_URL")?,
        Secret::new(std::env::var("SPACE_API_TOKEN")?),
    )?;
    let status = done(
        client
            .status(&BuildAccepted {
                job_id: accepted.job_id.clone(),
                template_name: accepted.template_name.clone(),
            })
            .await,
    )?;
    if !matches!(status.state, BuildState::Failed) || status.content_hash.is_some() {
        return Err("Only a definite failed build with no published image can be replaced.".into());
    }
    let template = done(client.template(&job).await)?;
    if template["name"] != template_name(&job)
        || template["description"]
            != format!(
                "OpenAgents environment build {}",
                job.artifact_inputs_digest()
            )
        || template["content_hash"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
        || !template["snapshots"].as_array().is_some_and(Vec::is_empty)
    {
        return Err("This namespace already has an image or its ownership is unknown.".into());
    }
    let history = state.join(format!("failed-native-{}.json", accepted.job_id));
    if history.exists() {
        if fs::read(&history)? != original {
            return Err("The failed attempt record changed.".into());
        }
    } else {
        fs::write(&history, &original)?;
    }
    let native =
        coder_environment_operator::space::recipe(&job, &work.base, install_script().as_bytes())?;
    work.native_recipe_digest = env::digest(&serde_json::to_vec(&native)?);
    work.accepted = None;
    work.build_issued = false;
    save(&path, &work)?;
    println!(
        "{}",
        json!({"job":job.id,"failed_attempt_retained":history.file_name().and_then(|name| name.to_str()),"new_attempt_not_submitted":true})
    );
    Ok(())
}

pub(super) async fn run(state: &Path, args: &[String]) -> Result<()> {
    let job: BuildJob = serde_json::from_slice(&fs::read(
        args.first().ok_or("Choose the frozen build job.")?,
    )?)?;
    let base: Artifact = serde_json::from_slice(&fs::read(
        args.get(1).ok_or("Choose the pinned SPACE base.")?,
    )?)?;
    job.validate()?;
    if job.inputs.environment != ENVIRONMENT
        || job.phase != Phase::PublishingSpace
        || now_ms() >= job.deadline_ms
    {
        return Err("Use this qualification's current clean build.".into());
    }
    let script = install_script();
    let native = coder_environment_operator::space::recipe(&job, &base, script.as_bytes())?;
    let recipe_digest = env::digest(&serde_json::to_vec(&native)?);
    fs::create_dir_all(state)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(state, fs::Permissions::from_mode(0o700))?;
    }
    let path = state.join("publication-work.json");
    let mut work = if path.exists() {
        let existing: Work = serde_json::from_slice(&fs::read(&path)?)?;
        if existing.job != job.id
            || existing.inputs_digest != job.artifact_inputs_digest()
            || existing.base != base
            || existing.native_recipe_digest != recipe_digest
        {
            return Err("This publication belongs to different frozen inputs.".into());
        }
        existing
    } else {
        Work {
            job: job.id.clone(),
            inputs_digest: job.artifact_inputs_digest(),
            base,
            native_recipe_digest: recipe_digest,
            namespace_issued: false,
            namespace_ready: false,
            build_issued: false,
            accepted: None,
            rootfs: None,
            base_requested: false,
            template: None,
        }
    };
    save(&path, &work)?;
    let client = TemplateClient::new(
        &std::env::var("SPACE_API_URL")?,
        Secret::new(std::env::var("SPACE_API_TOKEN")?),
    )?;
    if !work.namespace_ready {
        if work.namespace_issued {
            return Err("The template creation reply is unknown. Inspect and clean this publication before another build.".into());
        }
        work.namespace_issued = true;
        save(&path, &work)?;
        let made = done(client.create_owned(&job).await)?;
        if made["name"].as_str() != Some(template_name(&job).as_str()) {
            return Err("The template creation reply names another publication.".into());
        }
        fs::write(
            state.join("namespace-created.json"),
            serde_json::to_vec_pretty(&made)?,
        )?;
        work.namespace_ready = true;
        save(&path, &work)?;
    }
    if work.accepted.is_none() {
        if work.build_issued {
            return Err("The build reply is unknown. Do not submit it again; clean the owned template first.".into());
        }
        let until = (now_ms() + 90_000).min(job.deadline_ms);
        loop {
            let status = done(client.refresh_builders().await)?;
            if status["refreshed"] == true && status["ready"] == true {
                break;
            }
            if now_ms() >= until {
                return Err("No builder is ready. The build was not submitted.".into());
            }
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        work.build_issued = true;
        save(&path, &work)?;
        let accepted = done(client.submit(&job, &work.base, script.as_bytes()).await)?;
        work.accepted = Some(Accepted {
            job_id: accepted.job_id,
            template_name: accepted.template_name,
        });
        save(&path, &work)?;
    }
    let accepted = work.accepted.as_ref().unwrap();
    let accepted = BuildAccepted {
        job_id: accepted.job_id.clone(),
        template_name: accepted.template_name.clone(),
    };
    while work.rootfs.is_none() {
        if now_ms() >= job.deadline_ms {
            return Err("The publication deadline passed; clean the owned template.".into());
        }
        match client.status(&accepted).await {
            Outcome::Done { value: status } => match status.state {
                BuildState::Succeeded => {
                    let rootfs = status
                        .content_hash
                        .ok_or("The successful build has no rootfs identity.")?;
                    let artifact = Artifact {
                        kind: computer_driver::ArtifactKind::SpaceTemplate,
                        reference: format!("{}@{rootfs}", accepted.template_name),
                        provider_identity: Some(rootfs.clone()),
                        disk_gib: None,
                    };
                    artifact.validate()?;
                    work.rootfs = Some(rootfs);
                    save(&path, &work)?;
                }
                BuildState::Failed | BuildState::PublishFailed | BuildState::Unspecified => {
                    return Err(
                        "The SPACE template build failed; clean its owned publication.".into(),
                    );
                }
                BuildState::Pending | BuildState::Running => {}
            },
            // The accepted build keeps running. Status loss never resubmits it.
            Outcome::Failed { .. } | Outcome::Unknown { .. } => {}
        }
        if work.rootfs.is_none() {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }
    if !work.base_requested {
        done(client.prepare_base(&job).await)?;
        work.base_requested = true;
        save(&path, &work)?;
    }
    loop {
        if now_ms() >= job.deadline_ms {
            return Err("The clean boot image deadline passed.".into());
        }
        let template = done(client.template(&job).await)?;
        if template["content_hash"].as_str() == work.rootfs.as_deref()
            && template["snapshots"].as_array().is_some_and(|rows| {
                rows.iter().any(|row| {
                    row["vcpu"] == 2
                        && row["memory_mib"] == 4096
                        && row["snapshot_hash"].as_str().is_some_and(|s| !s.is_empty())
                })
            })
        {
            fs::write(
                state.join("published-template.json"),
                serde_json::to_vec_pretty(&template)?,
            )?;
            work.template = Some(template);
            save(&path, &work)?;
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    println!(
        "{}",
        json!({"job":job.id,"native_job":accepted.job_id,"template":accepted.template_name,"rootfs":work.rootfs,"clean_base_ready":true})
    );
    Ok(())
}
