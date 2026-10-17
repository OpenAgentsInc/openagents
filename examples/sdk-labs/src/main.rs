use openagents::{
    CancellationToken, Client, Deployment,
    labs::{self, cad, *},
};
use serde_json::{Value, json};
use std::{
    error::Error,
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
fn save(path: &Path, value: &impl serde::Serialize) -> Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.sync_all()?;
    Ok(())
}
fn option(args: &[String], name: &str) -> Result<String> {
    args.windows(2)
        .find(|pair| pair[0] == name)
        .map(|pair| pair[1].clone())
        .ok_or_else(|| format!("Provide {name}.").into())
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let expected = option(&args, "--expected-account")?;
    let state = PathBuf::from(option(&args, "--state")?);
    if !state.is_dir() {
        return Err("Choose an existing private state folder.".into());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if state.metadata()?.permissions().mode() & 0o077 != 0 {
            return Err("Make the state folder private with chmod 700.".into());
        }
    }
    let token_env = option(&args, "--token-env")?;
    let api = option(&args, "--api").ok();
    let website = option(&args, "--website").ok();
    let deployment = match (api, website) {
        (None, None) => Deployment::production(),
        (Some(api), Some(website)) => Deployment::new(&api, &website)?,
        _ => return Err("Provide both --api and --website.".into()),
    };
    let transport = Client::new(deployment)?;
    let token = std::env::var(token_env)?;
    let session = transport.authenticate(&token).await?;
    drop(token);
    if session.account().id != expected
        || session
            .account()
            .email
            .as_deref()
            .is_some_and(|email| email.eq_ignore_ascii_case("chris@openagents.com"))
    {
        return Err("Use the expected dedicated test account.".into());
    }
    let client = transport.labs(&session)?;
    let cancel = CancellationToken::new();
    let interrupt = cancel.clone();
    let signals = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            interrupt.cancel();
        }
    });
    let id = if let Ok(id) = option(&args, "--cleanup") {
        id
    } else {
        format!("sdk-labs-{}", RequestKey::generate()?.as_str())
    };
    let document = client.document_ref("me", &id)?;
    let mut job = None;
    let result = if args.iter().any(|arg| arg == "--cleanup") {
        Ok(json!({"cleanup_only":true}))
    } else {
        save(
            &state.join("cleanup.json"),
            &json!({"account":expected,"document":id}),
        )?;
        workflow(&client, &document, &state, &cancel, &mut job).await
    };
    // Cleanup uses a fresh token even if Ctrl+C stopped the workflow.
    let cleanup_cancel = CancellationToken::new();
    if let Some(job) = job {
        let _ = client.cancel_job(&job, &cleanup_cancel).await;
    }
    let deleted = match client.delete_document(&document, &cleanup_cancel).await {
        Ok(deleted) => deleted,
        Err(error) if error.status() == Some(404) => false,
        Err(error) => return Err(error.into()),
    };
    match client.document(&document, &cleanup_cancel).await {
        Err(error) if error.status() == Some(404) => {}
        _ => return Err("The test design still exists; run --cleanup with its saved ID.".into()),
    }
    signals.abort();
    let mut evidence = result?;
    if let Some(id) = evidence.get("artifact_id").and_then(Value::as_str) {
        match client.artifact("me", id, &cleanup_cancel).await {
            Err(error) if error.status() == Some(404) => {}
            _ => return Err("The test model file is still readable after cleanup.".into()),
        }
        evidence.as_object_mut().unwrap().remove("artifact_id");
        evidence["artifact_not_found"] = json!(true);
    }
    evidence["cleanup"] = json!({"deleted":deleted,"document_not_found":true});
    client.clear_session();
    save(&state.join("evidence.json"), &evidence)?;
    println!("{}", serde_json::to_string_pretty(&evidence)?);
    Ok(())
}
async fn workflow(
    client: &labs::Client,
    document: &DocumentRef,
    state: &Path,
    cancel: &CancellationToken,
    active_job: &mut Option<JobRef>,
) -> Result<Value> {
    let kinds = client.kinds("me", cancel).await?;
    if !kinds.iter().any(|kind| kind.kind == "cad") {
        return Err("CAD Lab isn't available for this test account.".into());
    }
    let tools = client.tools("me", "cad", cancel).await?;
    if !tools.iter().any(|tool| tool.name == "cad_from_json") {
        return Err("CAD creation isn't available.".into());
    }
    let mut content = cad::new_part(
        document.id(),
        "SDK test bracket",
        cad::DEFAULT_MATERIAL,
        None,
    );
    content["features"] =
        json!([{"id":"body","op":"solid.box","params":{"length":40,"width":40,"height":3}}]);
    let created=client.invoke("cad","cad_from_json",&client.bind(ToolCall{space:"me".into(),chat:None,base:None,arguments:json!({"doc":document.id(),"document":content,"summary":"SDK test creation"})})?,cancel).await?;
    let first: Version = serde_json::from_value(created.result["version"].clone())?;
    if first.document != document.id() || first.number != 1 {
        return Err("The created version didn't match the request.".into());
    }
    content["features"][0]["params"]["length"] = json!(42);
    let change = Change::Intent {
        intent: serde_json::to_value(cad::Intent::Replace {
            content: content.clone(),
        })?,
    };
    if cad::changed_content(&first.content, &change)? != content {
        return Err("The local preview didn't match the change.".into());
    }
    let edited = match client
        .edit(
            document,
            &client.bind(EditDocument {
                space: "me".into(),
                base: 1,
                change,
                summary: "Longer test bracket".into(),
            })?,
            cancel,
        )
        .await?
    {
        EditResult::Created(version) => version,
        _ => return Err("The test edit wasn't saved.".into()),
    };
    content["material"] = json!("Unsupported test material");
    if cad::check(&content, &json!({})).is_ok() {
        return Err("The local check accepted an unsupported material.".into());
    }
    let refusal = match client
        .edit(
            document,
            &client.bind(EditDocument {
                space: "me".into(),
                base: edited.number,
                change: Change::Intent {
                    intent: serde_json::to_value(cad::Intent::Replace { content })?,
                },
                summary: "Unsupported test material".into(),
            })?,
            cancel,
        )
        .await?
    {
        EditResult::Refused(refusal) => refusal.code,
        _ => return Err("The server accepted an unsupported material.".into()),
    };
    if client.document(document, cancel).await?.latest != edited.number {
        return Err("The refused edit changed the saved version.".into());
    }
    let request = client.job_request(CreateJob {
        space: "me".into(),
        version: edited.reference(),
        want: Want::Job {
            job_type: "rebuild".into(),
            params: json!({}),
        },
    })?;
    save(&state.join("job-request.json"), &request)?;
    let restored: JobRequest =
        serde_json::from_slice(&std::fs::read(state.join("job-request.json"))?)?;
    let job = client.create_job(&restored, cancel).await?;
    if client.create_job(&restored, cancel).await?.id != job.id {
        return Err("The saved job request returned a different job.".into());
    }
    let job_ref = client.job_ref("me", &job.id)?;
    *active_job = Some(job_ref.clone());
    let done = client
        .wait(
            &job_ref,
            WaitOptions {
                timeout: Duration::from_secs(180),
                poll_interval: Duration::from_millis(500),
            },
            cancel,
        )
        .await?;
    if done.status != JobStatus::Done {
        return Err("The test rebuild didn't finish successfully.".into());
    }
    let card = client.card(document, edited.number, cancel).await?;
    let artifact = client
        .artifact(
            "me",
            card.primary
                .as_deref()
                .ok_or("The card has no model file.")?,
            cancel,
        )
        .await?;
    let mut download = client.download(&artifact, Some(0..4), cancel).await?;
    let mut bytes = Vec::new();
    while let Some(chunk) = download.next_chunk(cancel).await? {
        bytes.extend(chunk);
    }
    if bytes != b"glTF" {
        return Err("The model file has an unexpected header.".into());
    }
    client
        .add_note(
            document,
            &client.bind(AddNote {
                space: "me".into(),
                version: edited.number,
                locator: Locator::None,
                text: "SDK test note".into(),
            })?,
            cancel,
        )
        .await?;
    let notes = client
        .notes(document, &PageOptions::default(), cancel)
        .await?;
    if notes.data.len() != 1 {
        return Err("The test note wasn't saved.".into());
    }
    let changes = client.events_since(document, 0, 100, cancel).await?;
    if changes.data.is_empty() {
        return Err("The test changes weren't listed.".into());
    }
    Ok(
        json!({"schema":"openagents.sdk.labs-client-run.v1","created_version":first.number,"edited_version":edited.number,"refusal_code":refusal,"job_done":true,"job_request_restored":true,"idempotent_retry_same_job":true,"artifact_id":artifact.metadata().id,"artifact_size":artifact.metadata().size,"artifact_sha256":artifact.metadata().sha256,"signed_url":artifact.signed_url().is_some(),"range_bytes":bytes.len(),"glb_header":true,"notes":notes.data.len(),"events":changes.data.len(),"local_preview_checked":true}),
    )
}
