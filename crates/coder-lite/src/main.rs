//! coder-lite binary entry point.
//!
//! ## What this binary is called
//!
//! It is `coder-lite`, and it is the coder TUI. `oa coder`, in the
//! `openagents-cli` crate next door, is a second front end onto the same
//! runtime and still exists; the two share their tools, their composer, their
//! lanes, and their metering, and differ in the frame they draw.
//! [`HELP`] says so, because a `--help` that named only one of them would be
//! telling a reader the other did not exist.
//!
//! It is not `openagents`. That name belongs to the CLI on `PATH` and this
//! binary does not take it.

use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::{sleep, timeout};

use coder_lite::interactive::SessionOptions;

const DEV_API_URL: &str = "http://127.0.0.1:4000";
const DEV_BASE_URL: &str = "http://127.0.0.1:4000/api/v1";

/// Every flag this binary reads. A flag listed here does what it says or the
/// binary refuses to start; there is nothing accepted-and-ignored.
const HELP: &str = "\
coder-lite — the OpenAgents coder, in a terminal.

Usage: coder-lite [options]

Options:
  --dev              Talk to a server on this machine at http://127.0.0.1:4000
                     over the OpenResponses streaming surface. Starts one from
                     ../openagents.com if none is running, and tolerates one
                     that already is.
  --lane <name>      Which model answers. `auto` leaves it to the deployment;
                     `flash` and `pro` are tiers; `local` or `ollama:<model>`
                     answers from this machine; any other name is checked
                     against GET /api/v1/models and refused if it is not
                     served. Defaults to `auto`.
  --reasoning <how>  Recorded on the thread as its reasoning effort. Omit to
                     leave the deployment's own default.
  -h, --help         Print this and exit.
  -V, --version      Print the version and exit.

Environment:
  OPENAGENTS_API_URL    The API origin to use. `--dev` sets it.
  OPENAGENTS_BASE_URL   The /api/v1 base to use. `--dev` sets it.
  OPENAGENTS_API_KEY    The credential to spend. Optional if signed in.
  OPENAGENTS_WEB_REPO   Where `--dev` looks for start_server.sh.
  ACP_REGISTRY          Where the `acp` tool looks for installed agents.

`oa coder`, in the openagents-cli crate, is a second front end onto the same
runtime, tools, composer, lanes, and metering. This binary is not `openagents`
and does not take that name.

Inside the session, `/help` lists the commands and the keys.
";

/// True when the first argument names a command the CLI surface answers.
///
/// The set is read out of clap rather than kept by hand here, so a subcommand
/// added to `openagents-cli` is reachable from this binary the moment it
/// exists, and one removed stops being claimed. A hand-kept list is how the
/// two surfaces would drift.
fn names_a_cli_command(arguments: &[String]) -> bool {
    use clap::CommandFactory;
    let Some(first) = arguments.first() else {
        return false;
    };
    if first.starts_with('-') {
        return false;
    }
    openagents_cli::cli::Cli::command()
        .get_subcommands()
        .any(|sub| sub.get_name() == first || sub.get_all_aliases().any(|alias| alias == first))
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let arguments: Vec<String> = env::args().skip(1).collect();

    // One binary, two surfaces. Bare, it is the coder session; given a command
    // the CLI answers, it is that command. Without this, shipping this binary
    // would drop `issue`, `repo`, `auth` and the rest from the release --
    // including `update`, so an installed build could not replace itself.
    if names_a_cli_command(&arguments) {
        use clap::Parser;
        let cli = openagents_cli::cli::Cli::parse();
        if let Err(error) = openagents_cli::cli::run(cli).await {
            openagents_cli::errors::fail(&openagents_cli::errors::CliError::Internal(
                error.to_string(),
            ));
        }
        return Ok(());
    }
    let options = match parse(&arguments) {
        Ok(Parsed::Run(mut options, dev)) => {
            if dev {
                boot_dev_server().await?;
                // SAFETY: edition 2024 marks `set_var` unsafe because another
                // thread reading the environment concurrently is UB. This runs
                // before the TUI and its tokio tasks start, so no other thread
                // exists yet.
                unsafe {
                    env::set_var("OPENAGENTS_API_URL", DEV_API_URL);
                    env::set_var("OPENAGENTS_BASE_URL", DEV_BASE_URL);
                }
            }
            options.dev = dev;
            options
        }
        Ok(Parsed::Said) => return Ok(()),
        Err(refusal) => {
            eprintln!("coder-lite: {refusal}");
            std::process::exit(2);
        }
    };

    coder_lite::interactive::run_tui(options).await
}

enum Parsed {
    /// Run a session. The flag is whether `--dev` was given.
    Run(SessionOptions, bool),
    /// `--help` or `--version` answered and there is nothing to run.
    Said,
}

/// Read the command line, or say what is wrong with it.
///
/// An unknown flag is refused rather than ignored: a flag that is silently
/// dropped is a flag that lied about being read.
fn parse(arguments: &[String]) -> Result<Parsed, String> {
    let mut options = SessionOptions {
        lane_name: "auto".to_string(),
        reasoning: None,
        dev: false,
    };
    let mut dev = false;
    let mut index = 0;

    while index < arguments.len() {
        let argument = arguments[index].as_str();
        let value = |name: &str| -> Result<String, String> {
            arguments
                .get(index + 1)
                .filter(|next| !next.starts_with('-'))
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match argument {
            "-h" | "--help" => {
                print!("{HELP}");
                return Ok(Parsed::Said);
            }
            "-V" | "--version" => {
                println!("coder-lite {}", openagents_cli::VERSION);
                return Ok(Parsed::Said);
            }
            "--dev" => dev = true,
            "--lane" => {
                options.lane_name = value("--lane")?;
                index += 1;
            }
            "--reasoning" => {
                options.reasoning = Some(value("--reasoning")?);
                index += 1;
            }
            other => {
                return Err(format!(
                    "`{other}` is not a flag this binary reads. `--help` lists the ones it does."
                ))
            }
        }
        index += 1;
    }

    Ok(Parsed::Run(options, dev))
}

async fn boot_dev_server() -> Result<(), Box<dyn std::error::Error>> {
    if is_dev_server_up().await {
        eprintln!("dev server already running at {}", DEV_BASE_URL);
        return Ok(());
    }

    let repo = web_repo()?;
    eprintln!("starting dev server in {}", repo.display());

    let mut command = Command::new("sh");
    command
        .arg("start_server.sh")
        .current_dir(&repo)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // The server outlives this TUI; put it in its own group so the shell's
    // exit and any terminal events do not take it down.
    #[cfg(unix)]
    command.process_group(0);
    let _child = command.spawn()?;

    for _ in 0..300 {
        if is_dev_server_up().await {
            return Ok(());
        }
        sleep(Duration::from_millis(500)).await;
    }

    Err("dev server did not become ready in 150 seconds".into())
}

async fn is_dev_server_up() -> bool {
    timeout(Duration::from_secs(2), async {
        let mut stream = TcpStream::connect("127.0.0.1:4000").await.ok()?;
        let request = "GET /health HTTP/1.1\r\nHost: 127.0.0.1:4000\r\nConnection: close\r\n\r\n";
        stream.write_all(request.as_bytes()).await.ok()?;
        let mut buf = [0u8; 256];
        let n = stream.read(&mut buf).await.ok()?;
        let head = std::str::from_utf8(&buf[..n]).unwrap_or("");
        Some(head.starts_with("HTTP/1.1 200") || head.contains(" 200 "))
    })
    .await
    .unwrap_or(Some(false))
    .unwrap_or(false)
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
