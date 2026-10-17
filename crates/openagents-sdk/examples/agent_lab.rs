//! Save a finite experiment through the same SDK used by the app.
use openagents_sdk::{CancellationToken, Client, Deployment, agent_lab::*};
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new(Deployment::production())?;
    let credential = std::env::var("OPENAGENTS_TOKEN")?;
    let session = client.authenticate(&credential).await?;
    drop(credential);
    let labs = client.agent_lab(&session)?;
    let cancel = CancellationToken::new();
    let hosts = labs.hosts(&cancel).await?;
    let host = hosts.first().ok_or("Open the updated desktop app first.")?;
    let project = host
        .projects
        .first()
        .ok_or("Choose a project on that computer first.")?;
    let mut experiment = starter();
    experiment.host = host.id.clone();
    experiment.project = project.id.clone();
    let request = labs.bind(SaveDocument {
        request_id: openagents_sdk::agents::request_id()?,
        expected_revision: 0,
        content: Content::Experiment { experiment },
        summary: "Created experiment".into(),
    });
    // Keep this account-bound record in private application storage before sending.
    let path = std::env::var("OPENAGENTS_LAB_REQUEST_FILE")?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    let mut file = options.open(path)?;
    file.write_all(&serde_json::to_vec(&request)?)?;
    file.sync_all()?;
    let document = labs.save(None, &request, &cancel).await?;
    println!("Saved experiment {} version 1", document.id);
    // Starting is a separate, explicit mutation: labs.start(document, saved StartRun request, cancel).
    Ok(())
}
