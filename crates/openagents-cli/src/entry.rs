//! Root command dispatch for the OpenAgents CLI.
//!
//! The release installs this binary as `openagents`. A bare invocation opens
//! Coder. Named OpenAgents commands dispatch to the shared CLI runtime.

use std::env;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::{sleep, timeout};

use crate::coder::interactive::SessionOptions;

const DEV_API_URL: &str = "http://127.0.0.1:4000";
const DEV_BASE_URL: &str = "http://127.0.0.1:4000/api/v1";

/// Every flag this binary reads. A flag listed here does what it says or the
/// binary refuses to start; there is nothing accepted-and-ignored.
const HELP: &str = "\
OpenAgents CLI

Usage:
  openagents                       Start Coder
  openagents coder [options]       Start Coder
  openagents <command> [options]   Run an OpenAgents command

Run `openagents` with no arguments to start Coder.

Commands:
  forge       Manage forge repositories and deployment targets
  auth        Manage authentication
  issue       Manage issues
  project     Manage projects
  milestone   Manage repository milestones
  repo        Manage repositories
  coder       Start Coder
  delegate    Run coding agents in parallel
  deploy      Manage fleet deployment targets
  provider    Inspect verified-work settlement decisions
  box         Manage sandboxed Box VMs
  computer    Manage this Computer connection
  forum       Manage forum boards and topics
  memory      Manage account memory
  api         Invoke an OpenAgents API route
  plugin      Manage capability plugins
  trace       Inspect and export traces
  update      Update this CLI

Run `openagents <command> --help` to view command options.

Coder options:
  --dev              Talk to a server on this machine at http://127.0.0.1:4000
                     over the OpenResponses streaming surface. Starts one from
                     ../openagents.com if none is running, and tolerates one
                     that already is.
  --lane <name>      Which model answers. `flash` and `free` are the two
                     switchable lanes, and shift+tab moves between them; each
                     resolves its model from GET /api/v1/models at open.
                     `local` or `ollama:<model>` answers from this machine; any
                     other name is read as a catalog id, checked against
                     GET /api/v1/models, and refused if it is not served.
                     Defaults to `flash`.
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

Inside the session, `/help` lists the commands and the keys.
";

/// True when the command position names a shared CLI command.
///
/// The set is read out of clap rather than kept by hand here, so a subcommand
/// added to `openagents-cli` is reachable from this binary the moment it
/// exists, and one removed stops being claimed. A hand-kept list is how the
/// two surfaces would drift.
fn names_a_cli_command(arguments: &[String]) -> bool {
    use clap::CommandFactory;
    let command = crate::cli::Cli::command();
    let mut index = 0;

    while let Some(argument) = arguments.get(index) {
        if matches!(
            argument.as_str(),
            "--api-url" | "--profile" | "--completions"
        ) {
            index += 2;
            continue;
        }

        // These flags belong to Coder, so values such as `trace` or `forge`
        // must not be mistaken for a command name.
        if matches!(argument.as_str(), "--lane" | "--reasoning") {
            index += 2;
            continue;
        }

        if argument.starts_with('-') {
            index += 1;
            continue;
        }

        return command.get_subcommands().any(|sub| {
            sub.get_name() == argument || sub.get_all_aliases().any(|alias| alias == argument)
        });
    }

    false
}

/// Whether every argument belongs to the Coder front door.
///
/// This performs the same shape check as [`parse`] without printing help,
/// because `parse` owns the visible response.
fn has_only_coder_options(arguments: &[String]) -> bool {
    let mut index = 0;

    while let Some(argument) = arguments.get(index) {
        match argument.as_str() {
            "--dev" | "-h" | "--help" | "-V" | "--version" => index += 1,
            "--lane" | "--reasoning" => {
                let Some(value) = arguments.get(index + 1) else {
                    return false;
                };
                if value.starts_with('-') {
                    return false;
                }
                index += 2;
            }
            _ => return false,
        }
    }

    true
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let _ = tracing_subscriber::fmt::try_init();
    crate::diag::initialize_color_from_environment();
    let arguments: Vec<String> = env::args().skip(1).collect();

    if matches!(arguments.as_slice(), [flag] if flag == "-h" || flag == "--help") {
        print!("{HELP}");
        return Ok(());
    }

    if matches!(arguments.as_slice(), [flag] if flag == "-V" || flag == "--version") {
        println!("OpenAgents v{}", crate::VERSION);
        return Ok(());
    }

    // One binary, two surfaces. Bare, it is the Coder session. `coder` with
    // interactive options uses the same front door explicitly. Named commands
    // and advanced Coder automation options use the shared CLI runtime.
    let coder_arguments = arguments
        .first()
        .filter(|argument| argument.as_str() == "coder")
        .map(|_| arguments[1..].to_vec())
        .filter(|arguments| arguments.is_empty() || has_only_coder_options(arguments));

    if let Some(coder_arguments) = coder_arguments {
        return run_coder(&coder_arguments).await;
    }

    if names_a_cli_command(&arguments) {
        use clap::Parser;
        let cli = crate::cli::Cli::parse();
        if let Err(error) = crate::cli::run(cli).await {
            crate::errors::fail(&crate::errors::CliError::Internal(error.to_string()));
        }
        return Ok(());
    }

    if arguments.is_empty() || has_only_coder_options(&arguments) {
        return run_coder(&arguments).await;
    }

    use clap::Parser;
    let cli = crate::cli::Cli::parse();
    if let Err(error) = crate::cli::run(cli).await {
        crate::errors::fail(&crate::errors::CliError::Internal(error.to_string()));
    }
    Ok(())
}

async fn run_coder(arguments: &[String]) -> Result<(), Box<dyn std::error::Error>> {
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
            eprintln!("openagents: {refusal}");
            std::process::exit(2);
        }
    };

    crate::coder::interactive::run_tui(options).await
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
        lane_name: "flash".to_string(),
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
                println!("OpenAgents v{}", crate::VERSION);
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
                ));
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
