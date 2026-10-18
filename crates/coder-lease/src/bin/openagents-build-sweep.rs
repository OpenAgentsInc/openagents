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
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--worker-job") {
        if args.len() != 2 {
            eprintln!("openagents-build-sweep: --worker-job needs one job file");
            return std::process::ExitCode::from(2);
        }
        return match sweep::worker::run_job(std::path::Path::new(&args[1])) {
            Ok(()) => std::process::ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("openagents-build-sweep: worker failed: {error}");
                std::process::ExitCode::FAILURE
            }
        };
    }
    let mut dry_run = false;
    for arg in args {
        match arg.to_str() {
            Some("--dry-run" | "-n") => dry_run = true,
            Some("-h" | "--help") => {
                println!(
                    "usage: openagents-build-sweep [--dry-run]\n\nKeeps 100 GiB free: caps shared Cargo targets at 120 GiB, removes unheld targets below the floor, and flags extra target copies. Log: ~/.openagents/logs/build-sweep.log. Off: touch ~/.openagents/build-sweep.off"
                );
                return std::process::ExitCode::SUCCESS;
            }
            _ => {
                eprintln!(
                    "openagents-build-sweep: unknown argument {}",
                    arg.to_string_lossy()
                );
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
    let report = match sweep::worker::run_isolated(&layout, &policy, dry_run) {
        Ok(report) => report,
        Err(error) => {
            eprintln!("openagents-build-sweep: the sweep couldn't start: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
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
