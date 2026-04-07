#![allow(
    clippy::exit,
    reason = "CLI help flow exits immediately after printing usage."
)]
#![allow(
    clippy::print_stdout,
    reason = "CLI utility intentionally writes operational output to stdout."
)]
#![allow(
    clippy::print_stderr,
    reason = "CLI utility intentionally writes usage/errors to stderr."
)]

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, anyhow};

fn should_launch_tui(args: &[String]) -> Result<bool> {
    if args.is_empty() {
        return Ok(true);
    }

    let mut index = 0usize;
    while index < args.len() {
        match args[index].as_str() {
            "--config-path" => {
                index += 1;
                args.get(index)
                    .ok_or_else(|| anyhow!("missing value for --config-path"))?;
                index += 1;
            }
            "--help" | "-h" => return Ok(false),
            _ => return Ok(false),
        }
    }

    Ok(true)
}

fn resolve_pylon_tui_path() -> PathBuf {
    let executable_name = if cfg!(windows) {
        "pylon-tui.exe"
    } else {
        "pylon-tui"
    };
    std::env::current_exe()
        .ok()
        .and_then(|current| current.parent().map(|parent| parent.join(executable_name)))
        .filter(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from(executable_name))
}

fn launch_pylon_tui(args: &[String]) -> Result<()> {
    let pylon_tui = resolve_pylon_tui_path();
    let status = Command::new(&pylon_tui)
        .args(args)
        .status()
        .with_context(|| format!("failed to launch {}", pylon_tui.display()))?;

    match status.code() {
        Some(0) => Ok(()),
        Some(code) => std::process::exit(code),
        None => std::process::exit(1),
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let launch_tui = match should_launch_tui(&args) {
        Ok(launch_tui) => launch_tui,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{}", pylon::usage());
            std::process::exit(1);
        }
    };

    if launch_tui {
        if let Err(error) = launch_pylon_tui(&args) {
            eprintln!("{error}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let cli = match pylon::parse_args(args) {
        Ok(cli) => cli,
        Err(error) if error.to_string() == pylon::usage() => {
            println!("{}", pylon::usage());
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{}", pylon::usage());
            std::process::exit(1);
        }
    };

    if let Some(output) = pylon::run_cli(cli).await? {
        println!("{output}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::should_launch_tui;

    #[test]
    fn bare_pylon_launches_tui() {
        assert!(should_launch_tui(&[]).expect("no-arg tui launch"));
    }

    #[test]
    fn config_only_launches_tui() {
        assert!(
            should_launch_tui(&["--config-path".into(), "/tmp/pylon.json".into()])
                .expect("config-only tui launch")
        );
    }

    #[test]
    fn explicit_help_stays_on_cli_path() {
        assert!(!should_launch_tui(&["--help".into()]).expect("help path"));
    }

    #[test]
    fn explicit_subcommands_stay_on_cli_path() {
        assert!(!should_launch_tui(&["status".into()]).expect("status path"));
        assert!(
            !should_launch_tui(&["--config-path".into(), "/tmp/pylon.json".into(), "status".into()])
                .expect("status path with config")
        );
    }
}
