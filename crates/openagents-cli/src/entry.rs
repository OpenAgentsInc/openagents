//! Root command dispatch for the OpenAgents CLI.
//!
//! The release installs this binary as `openagents`. A bare invocation opens
//! Coder. Named OpenAgents commands dispatch to the shared CLI runtime.

use std::env;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::process::Command;
use tokio::time::{sleep, timeout};

use crate::coder::interactive::SessionOptions;

#[allow(dead_code)]
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
  inference   Local GGUF load and in-process inference
  psionic     Psionic library harness
  trace       Inspect and export traces
  update      Update this CLI

Run `openagents <command> --help` to view command options.

Coder options:
  --dev              Talk to the local Coder inference door (catalog,
                     threads, grants, proxy). Starts `openagents-coder-api`
                     if none is running. Prefers port 4100; if that is
                     taken, binds 4101. Production origin is unchanged
                     without --dev. Optional: OPENAGENTS_CODER_API_BIN.
  --lane <name>      Which model answers. `flash`, `pro`, `nitro`, and `free`
                     are the switchable lanes, and shift+tab moves between
                     them; each resolves its model from GET /api/v1/models at
                     open. `pro` is Coder Pro (Sol Medium: gpt-5.6-sol,
                     reasoning medium). `nitro` is the Nitro door: an Open
                     Responses server on this machine
                     (OPENAGENTS_NITRO_ORIGIN, default 127.0.0.1:4200).
                     `local` or `ollama:<model>` answers from this machine; any other name is a catalog id, checked against
                     GET /api/v1/models, and refused if it is not served.
                     Defaults to `flash`.
  --reasoning <how>  Recorded on the thread as its reasoning effort. Omit to
                     leave the deployment's own default.
  --prompt <text>    Send one prompt and exit. For headless tests and scripts.
  --continue         Resume the most recent local session for this directory.
  --resume [id]      Resume a local session. Without an id, use this directory's
                     most recent session.
  --cloud-history    Also store transcript events and outcome text on the server.
                     Off by default; local files remain the source of truth.
  --autopilot        Run Autopilot unattended: pick the next unit from open
                     issues, recent sessions, and this workspace, and keep going.
                     Use `openagents coder --autopilot`. `--dry-run` prints the
                     plan without calling a model.
  -h, --help         Print this and exit.
  -V, --version      Print the version and exit.

Environment:
  OPENAGENTS_API_URL    The API origin to use. `--dev` sets it.
  OPENAGENTS_BASE_URL   The /api/v1 base to use. `--dev` sets it.
  OPENAGENTS_API_KEY    The credential to spend. Optional for `--dev`.
  OPENAGENTS_PRO_ORIGIN Production Pro origin. Default https://pro.openagents.com.
  OPENAGENTS_PRO_API_KEY  Bearer for the Pro door in production.
  OPENAGENTS_NITRO_ORIGIN  The Nitro door origin. Default http://127.0.0.1:4200.
  OPENAGENTS_NITRO_API_KEY  Bearer for the Nitro door, when it runs with a key.
  OPENAGENTS_CODER_API_BIN
                      Path to the local `openagents-coder-api` binary for `--dev`.
  ACP_REGISTRY          Where the `delegate` tool looks for installed external
                        agents (cursor, devin, opencode, ...).

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
        if matches!(argument.as_str(), "--lane" | "--reasoning" | "--prompt") {
            index += 2;
            continue;
        }

        if argument == "--resume" {
            index += 1;
            if arguments
                .get(index)
                .is_some_and(|value| !value.starts_with('-'))
            {
                index += 1;
            }
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
            "--dev" | "--continue" | "--cloud-history" | "-h" | "--help" | "-V" | "--version" => {
                index += 1
            }
            "--prompt" => {
                let Some(value) = arguments.get(index + 1) else {
                    return false;
                };
                if value.starts_with('-') {
                    return false;
                }
                index += 2;
            }
            "--resume" => {
                index += 1;
                if arguments
                    .get(index)
                    .is_some_and(|value| !value.starts_with('-'))
                {
                    index += 1;
                }
            }
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
    let _ = tracing_subscriber::fmt::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .try_init();
    crate::diag::initialize_color_from_environment();
    let mut arguments: Vec<String> = env::args().skip(1).collect();
    // `openagents --autopilot` is the agent-facing spelling. The loop lives
    // on `coder`; prepend that command so clap sees it.
    if arguments.iter().any(|argument| argument == "--autopilot")
        && arguments.first().map(String::as_str) != Some("coder")
        && !names_a_cli_command(&arguments)
    {
        let mut forwarded = vec!["coder".to_string()];
        forwarded.append(&mut arguments);
        arguments = forwarded;
    }

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
                // Capture a production credential before the origin switches
                // to loopback, so a signed-in account still has a bearer.
                // `--dev` does not require one: the local API admits a
                // unsigned principal named `local`.
                let existing = env::var("OPENAGENTS_API_KEY")
                    .ok()
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
                    .or_else(|| {
                        crate::auth::resolve_endpoint(None, None)
                            .ok()
                            .and_then(|endpoint| {
                                crate::auth::CredentialStore::for_origin(&endpoint.origin)
                                    .get_token()
                            })
                    });
                let api = crate::coder_dev::ensure_running().await?;
                // SAFETY: edition 2024 marks `set_var` unsafe because another
                // thread reading the environment concurrently is UB. This runs
                // before the TUI and its tokio tasks start, so no other thread
                // exists yet.
                unsafe {
                    env::set_var("OPENAGENTS_API_URL", &api.origin);
                    env::set_var("OPENAGENTS_BASE_URL", api.api_v1());
                    if let Some(token) = existing {
                        env::set_var("OPENAGENTS_API_KEY", token);
                    }
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
        lane_explicit: false,
        reasoning: None,
        dev: false,
        resume: None,
        cloud_history: false,
        prompt: None,
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
            "--continue" => {
                if options.resume.is_some() {
                    return Err("use either `--continue` or `--resume`, not both".to_string());
                }
                options.resume = Some(String::new());
            }
            "--resume" => {
                if options.resume.is_some() {
                    return Err("use either `--continue` or `--resume`, not both".to_string());
                }
                options.resume = Some(
                    arguments
                        .get(index + 1)
                        .filter(|next| !next.starts_with('-'))
                        .cloned()
                        .unwrap_or_default(),
                );
                if arguments
                    .get(index + 1)
                    .is_some_and(|next| !next.starts_with('-'))
                {
                    index += 1;
                }
            }
            "--cloud-history" => options.cloud_history = true,
            "--lane" => {
                options.lane_name = value("--lane")?;
                options.lane_explicit = true;
                index += 1;
            }
            "--reasoning" => {
                options.reasoning = Some(value("--reasoning")?);
                index += 1;
            }
            "--prompt" => {
                options.prompt = Some(value("--prompt")?);
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

#[allow(dead_code)]
async fn boot_dev_server() -> Result<(), Box<dyn std::error::Error>> {
    let repo = web_repo()?;
    match probe_dev_server().await {
        DevServerProbe::Ready => {
            eprintln!("dev server already running at {}", DEV_BASE_URL);
            return Ok(());
        }
        DevServerProbe::Http(status) => {
            return Err(
                dev_server_failure(&repo, &format!("the health check returned {status}")).into(),
            );
        }
        DevServerProbe::Unreachable => {}
    }

    let log_path = repo.join("dev_server.log");
    eprintln!("starting dev server in {}", repo.display());
    eprintln!("waiting for {}", DEV_BASE_URL);
    eprintln!("server log: {}", log_path.display());

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

    let mut unhealthy_responses = 0;
    for attempt in 1..=300 {
        match probe_dev_server().await {
            DevServerProbe::Ready => {
                eprintln!("dev server ready at {}", DEV_BASE_URL);
                return Ok(());
            }
            DevServerProbe::Http(status) => {
                unhealthy_responses += 1;
                eprintln!("dev server health check returned {status}");
                if unhealthy_responses >= 2 {
                    return Err(dev_server_failure(
                        &repo,
                        &format!("the health check returned {status} twice"),
                    )
                    .into());
                }
            }
            DevServerProbe::Unreachable => {
                unhealthy_responses = 0;
                if attempt % 10 == 0 {
                    eprintln!("still waiting for the dev server ({} seconds)", attempt / 2);
                }
            }
        }
        sleep(Duration::from_millis(500)).await;
    }

    Err(dev_server_failure(&repo, "it did not become ready in 150 seconds").into())
}

#[derive(Debug, PartialEq, Eq)]
enum DevServerProbe {
    Ready,
    Unreachable,
    Http(String),
}

async fn probe_dev_server() -> DevServerProbe {
    timeout(Duration::from_secs(2), async {
        let Ok(mut stream) = TcpStream::connect("127.0.0.1:4000").await else {
            return DevServerProbe::Unreachable;
        };
        let request = "GET /health HTTP/1.1\r\nHost: 127.0.0.1:4000\r\nConnection: close\r\n\r\n";
        if stream.write_all(request.as_bytes()).await.is_err() {
            return DevServerProbe::Unreachable;
        }
        let mut buf = [0u8; 256];
        let Ok(n) = stream.read(&mut buf).await else {
            return DevServerProbe::Unreachable;
        };
        let head = std::str::from_utf8(&buf[..n]).unwrap_or("");
        classify_health_response(head)
    })
    .await
    .unwrap_or(DevServerProbe::Unreachable)
}

fn classify_health_response(response: &str) -> DevServerProbe {
    let status = response.lines().next().unwrap_or("").trim();
    if status.split_whitespace().nth(1) == Some("200") {
        DevServerProbe::Ready
    } else if status.starts_with("HTTP/") {
        DevServerProbe::Http(status.to_string())
    } else {
        DevServerProbe::Unreachable
    }
}

fn dev_server_failure(repo: &std::path::Path, reason: &str) -> String {
    let log_path = repo.join("dev_server.log");
    let mut message = format!(
        "dev server is not ready because {reason}.\nserver log: {}",
        log_path.display()
    );

    if let Some(excerpt) = dev_log_excerpt(&log_path) {
        message.push_str("\n\nlatest server error:\n");
        message.push_str(&excerpt);
        if excerpt.contains("PendingMigrationError") {
            message.push_str(&format!(
                "\n\nRun `cd {} && mix ecto.migrate`, then retry.",
                repo.display()
            ));
        }
    }

    message
}

fn dev_log_excerpt(path: &std::path::Path) -> Option<String> {
    const MAX_LOG_BYTES: u64 = 64 * 1024;
    const MAX_ERROR_LINES: usize = 5;

    let mut file = File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(length.saturating_sub(MAX_LOG_BYTES)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let error_start = text.rfind("[error]").or_else(|| text.rfind("** ("))?;
    Some(
        text[error_start..]
            .lines()
            .take(MAX_ERROR_LINES)
            .collect::<Vec<_>>()
            .join("\n"),
    )
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn health_responses_distinguish_ready_unhealthy_and_unreachable() {
        assert_eq!(
            classify_health_response("HTTP/1.1 200 OK\r\ncontent-length: 2\r\n"),
            DevServerProbe::Ready
        );
        assert_eq!(
            classify_health_response("HTTP/1.1 503 Service Unavailable\r\n"),
            DevServerProbe::Http("HTTP/1.1 503 Service Unavailable".to_string())
        );
        assert_eq!(
            classify_health_response("not an HTTP response"),
            DevServerProbe::Unreachable
        );
    }

    #[test]
    fn dev_is_a_coder_flag() {
        let Parsed::Run(_, dev) = parse(&["--dev".into()]).unwrap() else {
            panic!("expected Coder options");
        };
        assert!(dev);
        assert!(has_only_coder_options(&["--dev".into()]));
    }

    #[test]
    fn prompt_is_recorded_on_the_session_options() {
        let Parsed::Run(options, _) = parse(&["--prompt".into(), "hello".into()]).unwrap() else {
            panic!("expected Coder options");
        };
        assert_eq!(options.prompt.as_deref(), Some("hello"));
    }

    #[test]
    fn coder_history_is_local_by_default() {
        let Parsed::Run(options, _) = parse(&[]).unwrap() else {
            panic!("expected Coder options");
        };
        assert_eq!(options.resume, None);
        assert!(!options.cloud_history);
    }

    #[test]
    fn continue_and_resume_select_local_sessions() {
        let Parsed::Run(continued, _) = parse(&["--continue".into()]).unwrap() else {
            panic!("expected Coder options");
        };
        assert_eq!(continued.resume.as_deref(), Some(""));

        let Parsed::Run(resumed, _) = parse(&[
            "--resume".into(),
            "session-7".into(),
            "--cloud-history".into(),
        ])
        .unwrap() else {
            panic!("expected Coder options");
        };
        assert_eq!(resumed.resume.as_deref(), Some("session-7"));
        assert!(resumed.cloud_history);
    }

    #[test]
    fn continue_and_resume_conflict() {
        let error = match parse(&["--continue".into(), "--resume".into()]) {
            Ok(_) => panic!("conflicting session selectors were accepted"),
            Err(error) => error,
        };
        assert!(error.contains("either"), "{error}");
    }

    #[test]
    fn failure_reports_the_log_and_pending_migration_remedy() {
        let directory = tempfile::tempdir().expect("temp directory");
        fs::write(
            directory.path().join("dev_server.log"),
            "[debug] booting\n[error] ** (Phoenix.Ecto.PendingMigrationError) migrate first\n    stack line\n",
        )
        .expect("write log");

        let failure = dev_server_failure(directory.path(), "the health check returned HTTP 503");
        assert!(failure.contains("HTTP 503"), "{failure}");
        assert!(failure.contains("PendingMigrationError"), "{failure}");
        assert!(failure.contains("mix ecto.migrate"), "{failure}");
        assert!(failure.contains("dev_server.log"), "{failure}");
    }
}
