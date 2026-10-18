//! `openagents host-upgrade`: rebuild this computer's development app from
//! `main` and swap it in at a quiet moment, with chats running on (#12056).
//! The `upgrade_host` app tool starts it with `--detach --agent <chat>`.

use std::time::Duration;

#[derive(clap::Args)]
pub struct Args {
    /// Show the upgrade waiting in the app and the last upgrades, then exit.
    #[arg(long)]
    status: bool,
    /// Rebuild even when main is what's running.
    #[arg(long)]
    force: bool,
    /// Why (it goes in the log).
    #[arg(long, default_value = "")]
    reason: String,
    /// The agent (chat ID) asking. It needs the `host.upgrade` scope.
    #[arg(long)]
    agent: Option<String>,
    /// Run in the background and print where the log is.
    #[arg(long)]
    detach: bool,
    /// How long to wait, in minutes, for the app to find a quiet moment and
    /// reopen on the new build.
    #[arg(long, default_value_t = 240)]
    wait_minutes: u64,
    #[arg(long, hide = true)]
    id: Option<String>,
}

#[cfg(not(unix))]
pub fn run(_: Args, _: std::path::PathBuf, _: u16) -> anyhow::Result<()> {
    anyhow::bail!("host upgrades build the macOS development app")
}

#[cfg(unix)]
pub fn run(args: Args, data_dir: std::path::PathBuf, ipc_port: u16) -> anyhow::Result<()> {
    use openagents_host_upgrade::{Layout, Request, record, system};

    let exe = std::env::current_exe()?;
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME isn't set"))?;
    let env = |name: &str| std::env::var(name).ok();
    let mut layout =
        Layout::for_app(&exe, data_dir, ipc_port, &env, &home).map_err(anyhow::Error::msg)?;
    // The bundle names its own data folder and port; a terminal's
    // environment doesn't.
    if let Some(dir) = system::bundle_env(&layout.bundle, "OPENAGENTS_DATA_DIR") {
        layout.data_dir = dir.into();
    }
    if let Some(port) =
        system::bundle_env(&layout.bundle, "OPENAGENTS_IPC_PORT").and_then(|p| p.parse().ok())
    {
        layout.ipc_port = port;
    }
    let state_dir = layout.state_dir();

    if args.status {
        print_status(&state_dir);
        return Ok(());
    }

    // An agent needs the host.upgrade scope, whether it uses the tool or
    // runs this in a terminal; the owner at a terminal doesn't.
    let agent = openagents_host_upgrade::grant::requester(args.agent.as_deref());
    let requested_by = agent.clone().unwrap_or_else(|| "owner".into());
    let id = args
        .id
        .clone()
        .unwrap_or_else(|| Request::new_id(openagents_host_upgrade::now_ms()));
    if let Some(agent) = &agent {
        openagents_host_upgrade::grant::check(agent, &args.reason).map_err(anyhow::Error::msg)?;
    }

    if args.detach {
        let log = detach(&exe, &args, agent.as_deref(), &id, &state_dir)?;
        println!(
            "Upgrade {id} started. It builds main, checks it, and restarts the app at a quiet moment.\nProgress: {}\nResult: openagents host-upgrade --status",
            log.display()
        );
        return Ok(());
    }

    let _lock = record::Lock::take(&state_dir, &system::alive).map_err(|why| {
        log_refusal(&state_dir, &id, &requested_by, &args.reason, &why);
        anyhow::anyhow!(why)
    })?;
    let mut host = system::SystemHost::new(layout, Duration::from_secs(args.wait_minutes * 60));
    let request = Request {
        id,
        requested_by,
        reason: args.reason,
        force: args.force,
    };
    let entry = openagents_host_upgrade::upgrade(&mut host, &request);
    println!("{}", entry.summary());
    if let Some(agent) = &agent {
        openagents_host_upgrade::grant::record(agent, &request.reason, &entry);
    }
    match entry.outcome {
        openagents_host_upgrade::Outcome::Installed
        | openagents_host_upgrade::Outcome::UpToDate
        | openagents_host_upgrade::Outcome::Waiting => Ok(()),
        _ => std::process::exit(1),
    }
}

#[cfg(unix)]
fn print_status(state_dir: &std::path::Path) {
    use openagents_host_upgrade::record;
    match record::read_pending(state_dir) {
        Some(pending) => println!(
            "Waiting in the app: {} ({} → {}), {:?}",
            pending.id,
            pending
                .from
                .as_deref()
                .map_or("?", openagents_host_upgrade::short),
            openagents_host_upgrade::short(&pending.to),
            pending.phase
        ),
        None => println!("No upgrade is waiting in the app."),
    }
    let log = record::read_log(state_dir);
    if log.is_empty() {
        println!("No upgrades yet.");
    }
    for entry in log.iter().rev().take(5) {
        println!(
            "{} {} (by {})",
            entry.id,
            entry.summary(),
            entry.requested_by
        );
    }
}

#[cfg(unix)]
fn log_refusal(state_dir: &std::path::Path, id: &str, by: &str, reason: &str, why: &str) {
    let entry = openagents_host_upgrade::LogEntry::new(
        id,
        openagents_host_upgrade::now_ms(),
        by,
        reason,
        openagents_host_upgrade::Outcome::Refused,
        why,
    );
    let _ = openagents_host_upgrade::record::log(state_dir, &entry);
}

/// Start this command again in its own session, so it outlives the agent
/// (and the app) that asked; output goes to a log in the state folder.
#[cfg(unix)]
fn detach(
    exe: &std::path::Path,
    args: &Args,
    agent: Option<&str>,
    id: &str,
    state_dir: &std::path::Path,
) -> anyhow::Result<std::path::PathBuf> {
    use std::os::unix::process::CommandExt as _;
    std::fs::create_dir_all(state_dir)?;
    let log_path = state_dir.join(format!("upgrade-{id}.log"));
    let log = std::fs::File::create(&log_path)?;
    let mut command = std::process::Command::new(exe);
    command
        .arg("host-upgrade")
        .arg("--id")
        .arg(id)
        .arg("--reason")
        .arg(&args.reason)
        .arg("--wait-minutes")
        .arg(args.wait_minutes.to_string())
        .current_dir(state_dir)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    if args.force {
        command.arg("--force");
    }
    // Named, because the detached command no longer runs under the agent.
    if let Some(agent) = agent {
        command.arg("--agent").arg(agent);
    }
    // SAFETY: setsid is async-signal-safe and touches no Rust state.
    unsafe {
        command.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    command.spawn()?;
    Ok(log_path)
}
