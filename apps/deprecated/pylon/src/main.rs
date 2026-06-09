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
#![cfg_attr(
    test,
    allow(
        clippy::expect_used,
        clippy::panic,
        clippy::panic_in_result_fn,
        clippy::unwrap_used
    )
)]

use std::io::IsTerminal;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, anyhow};

fn pylon_tui_args(args: &[String]) -> Result<Option<Vec<String>>> {
    pylon_tui_args_with_terminal(args, std::io::stdout().is_terminal())
}

fn pylon_tui_args_with_terminal(args: &[String], is_terminal: bool) -> Result<Option<Vec<String>>> {
    if args.is_empty() && is_terminal {
        return Ok(Some(Vec::new()));
    }

    let mut index = 0usize;
    let mut tui_args = Vec::new();
    while index < args.len() {
        match args[index].as_str() {
            "--config-path" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| anyhow!("missing value for --config-path"))?;
                tui_args.push(args[index].clone());
                tui_args.push(value.clone());
                index += 2;
            }
            "--help" | "-h" => return Ok(None),
            "tui" => {
                tui_args.extend(args.iter().skip(index + 1).cloned());
                return Ok(Some(tui_args));
            }
            _ => return Ok(None),
        }
    }

    Ok(None)
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

fn main() -> Result<()> {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let args = match pylon::strip_startup_thread_limit_args(raw_args) {
        Ok((args, limit)) => {
            pylon::apply_startup_thread_limit(limit);
            args
        }
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{}", pylon::usage());
            std::process::exit(1);
        }
    };
    if matches!(args.as_slice(), [flag] if flag == "--version" || flag == "-V") {
        println!("pylon {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    let tui_args = match pylon_tui_args(&args) {
        Ok(tui_args) => tui_args,
        Err(error) => {
            eprintln!("{error}");
            eprintln!("{}", pylon::usage());
            std::process::exit(1);
        }
    };

    if let Some(tui_args) = tui_args {
        if let Err(error) = launch_pylon_tui(&tui_args) {
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

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build pylon runtime")?;
    if let Some(output) = runtime.block_on(pylon::run_cli(cli))? {
        println!("{output}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{pylon_tui_args, pylon_tui_args_with_terminal};

    #[test]
    fn bare_interactive_pylon_launches_tui() {
        assert_eq!(
            pylon_tui_args_with_terminal(&[], true).expect("interactive no-arg path"),
            Some(Vec::new())
        );
    }

    #[test]
    fn bare_noninteractive_pylon_stays_on_cli_path() {
        assert_eq!(
            pylon_tui_args_with_terminal(&[], false).expect("noninteractive no-arg path"),
            None
        );
    }

    #[test]
    fn config_only_stays_on_cli_path() {
        assert_eq!(
            pylon_tui_args(&["--config-path".into(), "/tmp/pylon.json".into()])
                .expect("config-only path"),
            None
        );
    }

    #[test]
    fn explicit_tui_launches_tui() {
        assert_eq!(
            pylon_tui_args(&["tui".into()]).expect("tui launch"),
            Some(Vec::new())
        );
        assert_eq!(
            pylon_tui_args(&[
                "--config-path".into(),
                "/tmp/pylon.json".into(),
                "tui".into()
            ])
            .expect("config tui launch"),
            Some(vec!["--config-path".into(), "/tmp/pylon.json".into()])
        );
    }

    #[test]
    fn explicit_help_stays_on_cli_path() {
        assert_eq!(pylon_tui_args(&["--help".into()]).expect("help path"), None);
    }

    #[test]
    fn explicit_subcommands_stay_on_cli_path() {
        assert_eq!(
            pylon_tui_args(&["status".into()]).expect("status path"),
            None
        );
        assert_eq!(
            pylon_tui_args(&[
                "--config-path".into(),
                "/tmp/pylon.json".into(),
                "status".into()
            ])
            .expect("status path with config"),
            None
        );
    }
}
