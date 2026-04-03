#![allow(
    clippy::print_stdout,
    reason = "headless runtime intentionally prints operator-facing startup information."
)]

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Result;
use autopilot_desktop::desktop_control::{
    DESKTOP_CONTROL_BIND_ENV, DESKTOP_CONTROL_MANIFEST_ENV, control_manifest_path,
};
use autopilot_desktop::{DesktopAppOptions, run_desktop_app_with_options};
use clap::Parser;
use probe_client::{INTERNAL_DAEMON_SUBCOMMAND, INTERNAL_SERVER_SUBCOMMAND};

#[derive(Parser, Debug)]
#[command(name = "autopilot-headless-forge")]
#[command(about = "No-window Forge desktop-control host")]
struct Cli {
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    #[arg(long)]
    bind: Option<String>,
    #[arg(long, default_value_t = false)]
    enable_codex: bool,
}

#[derive(Debug)]
enum InternalAction {
    RunProbeServer { probe_home: Option<PathBuf> },
    RunProbeDaemon { probe_home: Option<PathBuf> },
}

fn main() -> ExitCode {
    match parse_internal_action(std::env::args().skip(1)) {
        Ok(Some(InternalAction::RunProbeServer { probe_home })) => {
            match probe_server::server::run_stdio_server(probe_home) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        Ok(Some(InternalAction::RunProbeDaemon { probe_home })) => {
            match probe_server::server::run_local_daemon(probe_home, None) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        Ok(None) => match run_headless_forge() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn run_headless_forge() -> Result<()> {
    let cli = Cli::parse();
    if let Some(manifest_path) = cli.manifest_path.as_ref() {
        // SAFETY: this process sets the override before any worker threads start.
        unsafe {
            std::env::set_var(DESKTOP_CONTROL_MANIFEST_ENV, manifest_path);
        }
    }
    if let Some(bind) = cli.bind.as_ref() {
        // SAFETY: this process sets the override before any worker threads start.
        unsafe {
            std::env::set_var(DESKTOP_CONTROL_BIND_ENV, bind);
        }
    }
    let manifest_path = control_manifest_path();
    println!(
        "starting autopilot-headless-forge; manifest will be written to {}",
        manifest_path.display()
    );
    run_desktop_app_with_options(DesktopAppOptions {
        window_visible: false,
        disable_codex: !cli.enable_codex,
    })
}

fn parse_internal_action(
    args: impl IntoIterator<Item = String>,
) -> std::result::Result<Option<InternalAction>, String> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(None);
    };
    let internal_server = first == INTERNAL_SERVER_SUBCOMMAND;
    let internal_daemon = first == INTERNAL_DAEMON_SUBCOMMAND;
    if !internal_server && !internal_daemon {
        return Ok(None);
    }

    let mut probe_home = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--probe-home" => {
                let Some(value) = args.next() else {
                    return Err(String::from("--probe-home requires a path"));
                };
                probe_home = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                return Err(String::from(
                    "usage: autopilot_headless_forge __internal-probe-server|__internal-probe-daemon [--probe-home <path>]",
                ));
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }

    Ok(Some(if internal_daemon {
        InternalAction::RunProbeDaemon { probe_home }
    } else {
        InternalAction::RunProbeServer { probe_home }
    }))
}

#[cfg(test)]
mod tests {
    use super::{InternalAction, parse_internal_action};
    use probe_client::{INTERNAL_DAEMON_SUBCOMMAND, INTERNAL_SERVER_SUBCOMMAND};
    use std::path::PathBuf;

    #[test]
    fn non_internal_args_fall_back_to_clap_path() {
        assert!(matches!(
            parse_internal_action(Vec::<String>::new()),
            Ok(None)
        ));
        assert!(matches!(
            parse_internal_action(vec![
                String::from("--manifest-path"),
                String::from("/tmp/manifest.json")
            ]),
            Ok(None)
        ));
    }

    #[test]
    fn parses_internal_probe_server_subcommand() {
        let result = parse_internal_action(vec![
            INTERNAL_SERVER_SUBCOMMAND.to_string(),
            String::from("--probe-home"),
            String::from("/tmp/probe-home"),
        ]);
        assert!(matches!(
            result,
            Ok(Some(InternalAction::RunProbeServer { probe_home }))
                if probe_home == Some(PathBuf::from("/tmp/probe-home"))
        ));
    }

    #[test]
    fn parses_internal_probe_daemon_subcommand() {
        let result = parse_internal_action(vec![
            INTERNAL_DAEMON_SUBCOMMAND.to_string(),
            String::from("--probe-home"),
            String::from("/tmp/probe-home"),
        ]);
        assert!(matches!(
            result,
            Ok(Some(InternalAction::RunProbeDaemon { probe_home }))
                if probe_home == Some(PathBuf::from("/tmp/probe-home"))
        ));
    }

    #[test]
    fn rejects_unknown_internal_probe_arguments() {
        let error = parse_internal_action(vec![
            INTERNAL_SERVER_SUBCOMMAND.to_string(),
            String::from("--bogus"),
        ])
        .expect_err("unknown args should fail");
        assert!(error.contains("unknown argument"));
    }
}
