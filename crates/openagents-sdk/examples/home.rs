//! Read Home without opening transcripts or starting work.
use openagents_sdk::{CancellationToken, Client, Deployment};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let credential = std::env::var("OPENAGENTS_TOKEN")?;
    let client = Client::new(Deployment::production())?;
    let session = client.authenticate(&credential).await?;
    let home = client.home(&session)?;
    let cancel = CancellationToken::new();
    let projects = home.projects(&cancel).await?;
    let work = home.work(None, &cancel).await?;
    let library = home.library(&cancel).await?;
    let layouts = home.directory("", None, false, &cancel).await?;
    println!(
        "{} projects; {} work reports; {} saved dashboards; {} available layouts",
        projects.len(),
        work.data.len(),
        library.dashboards.len(),
        layouts.data.len()
    );
    Ok(())
}
