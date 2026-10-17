//! Sign in with a key and read your account: the first call on
//! openagents.com/docs/sdk, which shows this file.
use openagents_sdk::{Client, Deployment};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // A sign-in key or app session for your account.
    let credential = std::env::var("OPENAGENTS_TOKEN")?;
    let client = Client::new(Deployment::production())?;
    let session = client.authenticate(&credential).await?;
    let view = client.account(&session).await?;
    println!("Signed in as {}", view.account.label);
    for workspace in &view.workspaces {
        println!("Workspace {} ({})", workspace.name, workspace.role);
    }
    Ok(())
}
