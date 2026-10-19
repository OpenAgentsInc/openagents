//! Dedicated-account Computers qualification. Pass a private checkpoint directory;
//! set OPENAGENTS_TEST_TOKEN explicitly. Reruns reuse saved request identities.
use openagents_sdk::{
    CancellationToken, Client, Deployment,
    computers::{
        CommandOptions, CommandOutcome, ComputerRef, Computers, CreateOptions, CreateRequest,
        WaitFor,
    },
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    error::Error,
    fs,
    path::Path,
    time::{Duration, Instant},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn save(path: &Path, value: &impl Serialize) -> Result<()> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
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
fn retained<T: Serialize + DeserializeOwned>(
    path: &Path,
    make: impl FnOnce() -> Result<T>,
) -> Result<T> {
    if path.exists() {
        Ok(serde_json::from_slice(&fs::read(path)?)?)
    } else {
        let value = make()?;
        save(path, &value)?;
        Ok(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, serde::Deserialize)]
struct Selection {
    api: String,
    placement: Option<String>,
}

fn timing(root: &Path, name: &str, elapsed: Duration) -> Result<()> {
    use std::io::Write;
    let path = root.join("timings.json");
    let mut values: BTreeMap<String, u64> = if path.exists() {
        serde_json::from_slice(&fs::read(&path)?)?
    } else {
        BTreeMap::new()
    };
    values
        .entry(name.into())
        .or_insert(u64::try_from(elapsed.as_millis())?);
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(path)?
        .write_all(&serde_json::to_vec_pretty(&values)?)?;
    Ok(())
}

async fn execute(
    computers: &Computers<'_>,
    reference: &ComputerRef,
    root: &Path,
    name: &str,
    text: String,
) -> Result<Vec<u8>> {
    let request = retained(&root.join(format!("command-{name}.json")), || {
        Ok(computers.command_request(
            reference,
            CommandOptions {
                command: text,
                timeout_seconds: Some(60),
                ..Default::default()
            },
        )?)
    })?;
    let cancel = CancellationToken::new();
    let command = computers.execute(&request, &cancel).await?;
    let command = computers
        .wait_command(command.reference(), Duration::from_secs(180), &cancel)
        .await?;
    if command.outcome() != Some(CommandOutcome::Succeeded) {
        return Err("The test command did not succeed.".into());
    }
    let mut reader = computers.output(command.reference())?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(30);
    let output = async {
        let mut bytes = Vec::new();
        while let Some(page) = reader.next_page(computers, &cancel).await? {
            if page.stdout_gap || page.stderr_gap || !page.stderr.is_empty() {
                return Err("The test output is incomplete.".into());
            }
            bytes.extend_from_slice(&page.stdout);
            if bytes.len() > 65536 {
                return Err("The test output is too large.".into());
            }
        }
        Ok(bytes)
    };
    tokio::time::timeout_at(deadline, output).await?
}

async fn workflow(
    computers: &Computers<'_>,
    reference: &ComputerRef,
    root: &Path,
    started: Instant,
) -> Result<()> {
    timing(root, "create_response_ms", started.elapsed())?;
    let ready = Instant::now();
    let cancel = CancellationToken::new();
    computers
        .wait(reference, WaitFor::Awake, Duration::from_secs(480), &cancel)
        .await?;
    timing(root, "readiness_ms", ready.elapsed())?;
    let upload = Instant::now();
    let bytes = b"OpenAgents SDK binary fixture\0\xff\x80\n";
    let intent = retained(&root.join("write.json"), || {
        Ok(computers.write_intent(reference, "sdk-fixture.bin", bytes)?)
    })?;
    // An interrupted upload is recovered by reading, never by an automatic PUT retry.
    if !root.join("write-started.json").exists() {
        save(&root.join("write-started.json"), &json!({"started":true}))?;
        computers.write_file(&intent, bytes, &cancel).await?;
    } else if computers.reconcile_write(&intent, &cancel).await?.is_none() {
        return Err("Check the earlier upload before writing again.".into());
    }
    timing(root, "upload_or_reconcile_ms", upload.elapsed())?;
    let first_command = Instant::now();
    let expected = format!("{:x}", Sha256::digest(bytes));
    let output = execute(
        computers,
        reference,
        root,
        "checksum",
        "sha256sum sdk-fixture.bin".into(),
    )
    .await?;
    if output.split(|b| *b == b' ').next() != Some(expected.as_bytes()) {
        return Err("The test file checksum differs.".into());
    }
    timing(root, "first_command_ms", first_command.elapsed())?;
    timing(root, "first_useful_output_ms", started.elapsed())?;
    let stop = Instant::now();
    if !root.join("stop-started.json").exists() {
        save(&root.join("stop-started.json"), &json!({"started":true}))?;
        computers.stop(reference, &cancel).await?;
    }
    computers
        .wait(
            reference,
            WaitFor::Stopped,
            Duration::from_secs(480),
            &cancel,
        )
        .await?;
    timing(root, "stop_ms", stop.elapsed())?;
    let wake = Instant::now();
    if !root.join("wake-started.json").exists() {
        save(&root.join("wake-started.json"), &json!({"started":true}))?;
        computers.wake(reference, &cancel).await?;
    }
    computers
        .wait(reference, WaitFor::Awake, Duration::from_secs(480), &cancel)
        .await?;
    timing(root, "wake_ms", wake.elapsed())?;
    let read = Instant::now();
    if computers
        .read_file(reference, "sdk-fixture.bin", &cancel)
        .await?
        .bytes
        != bytes
    {
        return Err("The saved file differs after waking.".into());
    }
    timing(root, "read_after_wake_ms", read.elapsed())?;
    let command_after_wake = Instant::now();
    execute(
        computers,
        reference,
        root,
        "remove",
        "rm -- sdk-fixture.bin".into(),
    )
    .await?;
    timing(root, "command_after_wake_ms", command_after_wake.elapsed())?;
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or("Pass a private checkpoint directory.")?;
    let root = Path::new(&root);
    fs::create_dir_all(root)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if fs::metadata(root)?.mode() & 0o077 != 0 {
            fs::set_permissions(root, fs::Permissions::from_mode(0o700))?;
        }
    }
    let origin = std::env::var("OPENAGENTS_TEST_API")
        .unwrap_or_else(|_| "https://api.openagents.com/v1".into());
    // Staging also accepts the dedicated SPACE qualification account (#11816).
    let (website, expected_accounts): (&str, &[&str]) = match origin.as_str() {
        "https://api.openagents.com/v1" => ("https://openagents.com", &["acct_83d16a35955ef908"]),
        "https://staging.openagents.com/v1" => (
            "https://staging.openagents.com",
            &["acct_dc7a799879686fc5", "acct_4570d993d9897bdd"],
        ),
        _ => return Err("Use the production or staging test deployment.".into()),
    };
    let placement = match std::env::var("OPENAGENTS_TEST_PLACEMENT").ok().as_deref() {
        None => None,
        Some("our-cloud") => Some("our-cloud".to_owned()),
        Some("fast-sandbox") => Some("fast-sandbox".to_owned()),
        Some(_) => return Err("Use our-cloud or fast-sandbox for the test placement.".into()),
    };
    let selection = Selection {
        api: origin.clone(),
        placement: placement.clone(),
    };
    if !root.join("selection.json").exists()
        && root.join("create.json").exists()
        && placement.is_some()
    {
        return Err("Use a new checkpoint directory for an explicit placement.".into());
    }
    let saved = retained(&root.join("selection.json"), || Ok(selection.clone()))?;
    if saved != selection {
        return Err("The saved checkpoint uses different test settings.".into());
    }
    let client = Client::with_test_run(
        Deployment::new(&origin, website)?,
        Duration::from_secs(75),
        true,
    )?;
    let mut credential =
        std::env::var("OPENAGENTS_TEST_TOKEN").map_err(|_| "Set the dedicated test credential.")?;
    let session = client.authenticate(&credential).await;
    // Session owns the authenticated credential; don't keep another copy here.
    credential.clear();
    let session = session?;
    if !expected_accounts.contains(&session.account().id.as_str()) {
        return Err("This example requires the dedicated test account.".into());
    }
    let computers = client
        .computers(&session)?
        .with_operation_timeout(Duration::from_secs(90))?;
    let resumed_checkpoint = root.join("create.json").exists();
    let request: CreateRequest = retained(&root.join("create.json"), || {
        Ok(computers.create_request(CreateOptions {
            size: Some("small".into()),
            placement: placement.clone(),
            ..Default::default()
        })?)
    })?;
    let cancel = CancellationToken::new();
    let started = Instant::now();
    let created = computers.create(&request, &cancel).await;
    let (reference, result) = match created {
        Ok(computer) => {
            let reference = computer.reference().clone();
            let result = if computer.data().state == "deleted" {
                if root.join("workflow-passed.json").exists() {
                    Ok(())
                } else {
                    Err("The earlier computer is deleted; its workflow did not complete.".into())
                }
            } else {
                let mut result = workflow(&computers, &reference, root, started).await;
                if result.is_ok() && !root.join("workflow-passed.json").exists() {
                    result = save(&root.join("workflow-passed.json"), &json!({"passed": true}));
                }
                result
            };
            (Some(reference), result)
        }
        Err(error) => {
            // Resolve a possible accepted create with its original saved key.
            let reference = if matches!(error, openagents_sdk::computers::Error::Uncertain { .. }) {
                computers
                    .create(&request, &cancel)
                    .await
                    .ok()
                    .map(|row| row.reference().clone())
            } else {
                None
            };
            (reference, Err(Box::new(error) as Box<dyn Error>))
        }
    };
    let cleanup_started = Instant::now();
    let cleanup = async {
        let reference = reference
            .as_ref()
            .ok_or("The original computer could not be resolved. Keep this checkpoint.")?;
        if computers.get(reference, &cancel).await?.data().state != "deleted" {
            if !root.join("delete-started.json").exists() {
                save(&root.join("delete-started.json"), &json!({"started":true}))?;
                // An uncertain delete is observed below; never replay the old intent.
                let _ = computers.delete(reference, &cancel).await;
            }
            computers
                .wait(
                    reference,
                    WaitFor::Deleted,
                    Duration::from_secs(480),
                    &cancel,
                )
                .await?;
        }
        Ok::<_, Box<dyn Error>>(())
    }
    .await;
    let cleanup_timing = timing(root, "cleanup_ms", cleanup_started.elapsed());
    let timings = fs::read(root.join("timings.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok());
    let evidence = json!({"schema":"openagents.sdk.computers.workflow.v1","api":origin,"dedicated_test_account":true,
        "placement":placement,"resumed_checkpoint":resumed_checkpoint,
        "latency_sample_eligible":!resumed_checkpoint && result.is_ok() && cleanup.is_ok() && cleanup_timing.is_ok(),"timings_ms":timings,
        "workflow_passed":result.is_ok(),"cleanup_passed":cleanup.is_ok(),"computer":reference.as_ref().map(ComputerRef::id),
        "request_key":request.key().as_str(),"elapsed_ms":started.elapsed().as_millis(),
        "contains_credentials":false,"contains_file_or_command_output":false});
    // Scrubbed evidence is replaceable; private request files remain create-once.
    fs::write(
        root.join("evidence.json"),
        serde_json::to_vec_pretty(&evidence)?,
    )?;
    println!("{evidence}");
    cleanup?;
    cleanup_timing?;
    result
}
