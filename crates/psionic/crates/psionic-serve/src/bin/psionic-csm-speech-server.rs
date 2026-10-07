use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};

use psionic_models::CsmRuntimeBackend;
use psionic_observe::{TokioRuntimeTelemetryConfig, build_main_runtime};
use psionic_serve::{CsmSpeechServer, CsmSpeechServerConfig};
use tokio::net::TcpListener;

fn main() -> ExitCode {
    match run_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "{error}");
            ExitCode::FAILURE
        }
    }
}

fn run_main() -> Result<(), String> {
    let telemetry = TokioRuntimeTelemetryConfig::from_env()
        .map_err(|error| format!("failed to load Tokio telemetry config: {error}"))?;
    let (runtime, _telemetry_guard) = build_main_runtime(&telemetry)
        .map_err(|error| format!("failed to build Tokio runtime: {error}"))?;
    runtime.block_on(run())
}

async fn run() -> Result<(), String> {
    let config = parse_args()?;
    let address = config.socket_addr().map_err(|error| error.to_string())?;
    let listener = TcpListener::bind(address)
        .await
        .map_err(|error| format!("failed to bind {address}: {error}"))?;
    let server = CsmSpeechServer::from_config(config.clone())
        .map_err(|error| format!("failed to initialize CSM speech server: {error}"))?;
    let mut stdout = io::stdout();
    let _ = writeln!(
        stdout,
        "psionic csm speech server listening on http://{} model={} execution_engine={}",
        listener
            .local_addr()
            .map_err(|error| format!("failed to query listener address: {error}"))?,
        config.model_id,
        CsmRuntimeBackend::parse(&config.backend)
            .map(|backend| backend.execution_engine().to_string())
            .unwrap_or_else(|_| "unsupported_backend".to_string()),
    );
    server
        .serve(listener)
        .await
        .map_err(|error| format!("server failed: {error}"))
}

fn parse_args() -> Result<CsmSpeechServerConfig, String> {
    parse_args_from(env::args().skip(1))
}

fn parse_args_from<I, S>(args: I) -> Result<CsmSpeechServerConfig, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut config = CsmSpeechServerConfig::from_env();
    let mut args = args.into_iter().map(Into::into);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--host" => {
                config.host = next_value(&mut args, argument.as_str())?;
            }
            "--port" => {
                config.port = next_value(&mut args, argument.as_str())?
                    .parse()
                    .map_err(|error| format!("invalid --port value: {error}"))?;
            }
            "--model" => {
                config.model_id = next_value(&mut args, argument.as_str())?;
            }
            "-h" | "--help" => {
                return Err(usage());
            }
            other => {
                return Err(format!("unrecognized argument `{other}`\n\n{}", usage()));
            }
        }
    }
    Ok(config)
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for `{flag}`"))
}

fn usage() -> String {
    String::from(
        "usage: psionic-csm-speech-server [--host <ip>] [--port <port>] [--model <model-id>]",
    )
}

#[cfg(test)]
mod tests {
    use super::parse_args_from;

    #[test]
    fn parse_args_accepts_host_port_and_model() {
        let config = parse_args_from([
            "--host",
            "0.0.0.0",
            "--port",
            "9001",
            "--model",
            "sesame/csm-1b",
        ])
        .expect("config");

        assert_eq!(config.host, "0.0.0.0");
        assert_eq!(config.port, 9001);
        assert_eq!(config.model_id, "sesame/csm-1b");
    }

    #[test]
    fn parse_args_rejects_unknown_flags() {
        let error = parse_args_from(["--worker", "nope"]).expect_err("worker flags are forbidden");

        assert!(error.contains("unrecognized argument"));
        assert!(error.contains("--worker"));
    }
}
