//! `openagents-build-sweep`: one run of the build sweep
//! (`coder_lease::agent_builds::sweep`, #12023). launchd starts it every
//! five minutes (`scripts/dev/install-build-sweep.sh`).
//!
//! Usage: `openagents-build-sweep [--dry-run]`
//!
//! `--dry-run` prints what it would remove and removes nothing. A real run
//! appends its lines to `~/.openagents/logs/build-sweep.log` and writes
//! `build-sweep.last.json` beside it. `touch ~/.openagents/build-sweep.off`
//! turns real runs off.

use coder_lease::agent_builds::sweep;

fn main() -> std::process::ExitCode {
    let mut dry_run = false;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--dry-run" | "-n" => dry_run = true,
            "-h" | "--help" => {
                println!(
                    "usage: openagents-build-sweep [--dry-run]\n\nKeeps 100 GiB free: caps shared Cargo targets at 120 GiB, removes unheld targets below the floor, and flags extra target copies. Log: ~/.openagents/logs/build-sweep.log. Off: touch ~/.openagents/build-sweep.off"
                );
                return std::process::ExitCode::SUCCESS;
            }
            other => {
                eprintln!("openagents-build-sweep: unknown argument {other}");
                return std::process::ExitCode::from(2);
            }
        }
    }
    let Some(home) =
        user_home::home_in(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
    else {
        eprintln!("openagents-build-sweep: no home folder is known");
        return std::process::ExitCode::FAILURE;
    };
    let leases = coder_lease::root_from_env().unwrap_or_else(|_| home.join(".openagents/leases"));
    let layout = sweep::Layout::at(&home, &leases);
    if layout.off.exists() && !dry_run {
        return std::process::ExitCode::SUCCESS;
    }
    let policy = sweep::Policy::from(&|name| std::env::var(name).ok());
    let report = sweep::sweep(&layout, &policy, &sweep::System, dry_run);
    if dry_run {
        for line in report.lines() {
            println!("{line}");
        }
    } else if let Err(error) = sweep::record(&layout, &report) {
        eprintln!("openagents-build-sweep: the log can't be written: {error}");
        return std::process::ExitCode::FAILURE;
    }
    std::process::ExitCode::SUCCESS
}
