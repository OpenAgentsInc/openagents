//! Create an SDK-runner definition without linking the app's host or UI.
//! OPENAGENTS_TOKEN is an app session or sign-in key; never pass it as an argument.
use openagents_sdk::{
    CancellationToken, Client, Deployment,
    agents::{Access, AgentSpec},
};
#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let credential =
        std::env::var("OPENAGENTS_TOKEN").map_err(|_| "Set OPENAGENTS_TOKEN privately.")?;
    let client = Client::new(Deployment::production())?;
    let session = client.authenticate(&credential).await?;
    drop(credential);
    let agents = client.agents(&session)?;
    let spec = AgentSpec {
        name: "My Rust agent".into(),
        description: "My installed Rust runner".into(),
        instructions: "Handle the assigned task.".into(),
        runtime: "sdk".into(),
        model: Some("reviewer".into()),
        access: Access::HostDefault,
        fallbacks: vec![],
    };
    let request = agents.create_request(spec)?;
    // Persist this bound record privately before sending in an application that retries.
    let definition = agents.create(&request, &CancellationToken::new()).await?;
    println!(
        "Saved agent {} revision {}",
        definition.id, definition.revision
    );
    Ok(())
}
