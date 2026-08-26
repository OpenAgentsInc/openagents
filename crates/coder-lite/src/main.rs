//! coder-lite binary entry point.

use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::{sleep, timeout};

const DEV_BASE_URL: &str = "http://localhost:4000/api/v1";
const DEV_API_KEY: &str = "fake";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dev = env::args().any(|a| a == "--dev");

    if dev {
        boot_dev_server().await?;
        if env::var("OPENAGENTS_BASE_URL").is_err() {
            env::set_var("OPENAGENTS_BASE_URL", DEV_BASE_URL);
        }
        if env::var("OPENAGENTS_API_KEY").is_err() {
            env::set_var("OPENAGENTS_API_KEY", DEV_API_KEY);
        }
    }

    coder_lite::interactive::run_tui().await
}

async fn boot_dev_server() -> Result<(), Box<dyn std::error::Error>> {
    if is_dev_server_up().await {
        eprintln!("dev server already running at {}", DEV_BASE_URL);
        return Ok(());
    }

    let repo = web_repo()?;
    eprintln!("starting dev server in {}", repo.display());

    Command::new("sh")
        .arg("start_server.sh")
        .current_dir(&repo)
        .spawn()?;

    for _ in 0..60 {
        if is_dev_server_up().await {
            return Ok(());
        }
        sleep(Duration::from_secs(1)).await;
    }

    Err("dev server did not become ready in 60 seconds".into())
}

async fn is_dev_server_up() -> bool {
    timeout(Duration::from_secs(2), TcpStream::connect("127.0.0.1:4000"))
        .await
        .is_ok()
}

fn web_repo() -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Ok(dir) = env::var("OPENAGENTS_WEB_REPO") {
        return Ok(PathBuf::from(dir));
    }
    let here = env::current_dir()?;
    let candidate = here.join("../openagents.com");
    let canonical = candidate.canonicalize()?;
    if !canonical.is_dir() {
        return Err(format!("{} is not a directory", canonical.display()).into());
    }
    if !canonical.join("start_server.sh").is_file() {
        return Err(format!("{} has no start_server.sh", canonical.display()).into());
    }
    Ok(canonical)
}
