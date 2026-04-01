use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match parse_args() {
        Ok(Action::RunDesktop) => match autopilot_desktop::run_desktop_app() {
            Ok(()) => ExitCode::SUCCESS,
            Err(error) => {
                eprintln!("{error}");
                ExitCode::from(1)
            }
        },
        Ok(Action::RunInternalProbeServer { probe_home }) => {
            match probe_server::server::run_stdio_server(probe_home) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        Ok(Action::RunInternalProbeDaemon { probe_home }) => {
            match probe_server::server::run_local_daemon(probe_home, None) {
                Ok(()) => ExitCode::SUCCESS,
                Err(error) => {
                    eprintln!("{error}");
                    ExitCode::from(1)
                }
            }
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

#[derive(Debug)]
enum Action {
    RunDesktop,
    RunInternalProbeServer { probe_home: Option<PathBuf> },
    RunInternalProbeDaemon { probe_home: Option<PathBuf> },
}

fn parse_args() -> std::result::Result<Action, String> {
    parse_args_from(std::env::args().skip(1))
}

fn parse_args_from(args: impl IntoIterator<Item = String>) -> std::result::Result<Action, String> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Action::RunDesktop);
    };
    let internal_server = first == probe_client::INTERNAL_SERVER_SUBCOMMAND;
    let internal_daemon = first == probe_client::INTERNAL_DAEMON_SUBCOMMAND;
    if !internal_server && !internal_daemon {
        return Ok(Action::RunDesktop);
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
                    "usage: autopilot-desktop __internal-probe-server|__internal-probe-daemon [--probe-home <path>]",
                ));
            }
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    Ok(if internal_daemon {
        Action::RunInternalProbeDaemon { probe_home }
    } else {
        Action::RunInternalProbeServer { probe_home }
    })
}

#[cfg(test)]
mod tests {
    use super::{Action, parse_args_from};
    use std::path::PathBuf;

    #[test]
    fn desktop_action_is_default_for_normal_launch() {
        assert!(matches!(
            parse_args_from(Vec::<String>::new()),
            Ok(Action::RunDesktop)
        ));
        assert!(matches!(
            parse_args_from(vec![String::from("--not-an-internal-subcommand")]),
            Ok(Action::RunDesktop)
        ));
    }

    #[test]
    fn parses_internal_probe_server_subcommand() {
        let result = parse_args_from(vec![
            probe_client::INTERNAL_SERVER_SUBCOMMAND.to_string(),
            String::from("--probe-home"),
            String::from("/tmp/probe-home"),
        ]);
        assert!(matches!(
            result,
            Ok(Action::RunInternalProbeServer { probe_home })
                if probe_home == Some(PathBuf::from("/tmp/probe-home"))
        ));
    }

    #[test]
    fn parses_internal_probe_daemon_subcommand() {
        let result = parse_args_from(vec![
            probe_client::INTERNAL_DAEMON_SUBCOMMAND.to_string(),
            String::from("--probe-home"),
            String::from("/tmp/probe-home"),
        ]);
        assert!(matches!(
            result,
            Ok(Action::RunInternalProbeDaemon { probe_home })
                if probe_home == Some(PathBuf::from("/tmp/probe-home"))
        ));
    }

    #[test]
    fn rejects_unknown_internal_probe_arguments() {
        let error = parse_args_from(vec![
            probe_client::INTERNAL_SERVER_SUBCOMMAND.to_string(),
            String::from("--bogus"),
        ])
        .expect_err("unknown args should fail");
        assert!(error.contains("unknown argument"));
    }
}
