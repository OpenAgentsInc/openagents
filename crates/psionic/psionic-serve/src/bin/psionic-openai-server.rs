#![cfg_attr(test, allow(clippy::expect_used))]

use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};

use psionic_serve::{OpenAiCompatBackend, OpenAiCompatConfig, OpenAiCompatServer};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            let _ = writeln!(io::stderr(), "{error}");
            ExitCode::FAILURE
        }
    }
}

async fn run() -> Result<(), String> {
    let config = parse_args()?;
    let address = config.socket_addr().map_err(|error| error.to_string())?;
    let listener = TcpListener::bind(address)
        .await
        .map_err(|error| format!("failed to bind {address}: {error}"))?;
    let server = OpenAiCompatServer::from_config(&config)
        .map_err(|error| format!("failed to load models: {error}"))?;
    let mut stdout = io::stdout();
    let _ = writeln!(
        stdout,
        "psionic openai server listening on http://{} models={} backend={} execution_mode={} execution_engine={}",
        listener
            .local_addr()
            .map_err(|error| format!("failed to query listener address: {error}"))?,
        config
            .model_paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join(","),
        server.backend_label(),
        server.execution_mode_label(),
        server.execution_engine_label(),
    );
    server
        .serve(listener)
        .await
        .map_err(|error| format!("server failed: {error}"))
}

fn parse_args() -> Result<OpenAiCompatConfig, String> {
    parse_args_from(env::args().skip(1))
}

fn parse_args_from<I, S>(args: I) -> Result<OpenAiCompatConfig, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut model_paths = Vec::new();
    let mut host = String::from("127.0.0.1");
    let mut port = 8080_u16;
    let mut backend = OpenAiCompatBackend::Cpu;
    let mut reasoning_budget = 0_u8;

    let mut args = args.into_iter().map(Into::into);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-m" | "--model" => {
                model_paths.push(next_value(&mut args, argument.as_str())?);
            }
            "--host" => {
                host = next_value(&mut args, argument.as_str())?;
            }
            "--port" => {
                port = next_value(&mut args, argument.as_str())?
                    .parse()
                    .map_err(|error| format!("invalid --port value: {error}"))?;
            }
            "--backend" => {
                let value = next_value(&mut args, argument.as_str())?;
                if value != "cpu" {
                    return Err(format!(
                        "invalid --backend value `{value}` (expected cpu)\n\n{}",
                        usage()
                    ));
                }
                backend = OpenAiCompatBackend::Cpu;
            }
            "--reasoning-budget" => {
                reasoning_budget = next_value(&mut args, argument.as_str())?
                    .parse()
                    .map_err(|error| format!("invalid --reasoning-budget value: {error}"))?;
            }
            "-h" | "--help" => {
                return Err(usage());
            }
            other => {
                return Err(format!("unrecognized argument `{other}`\n\n{}", usage()));
            }
        }
    }

    let Some(first_model_path) = model_paths.first().cloned() else {
        return Err(format!("missing required `-m` / `--model`\n\n{}", usage()));
    };
    let mut config = OpenAiCompatConfig::new(first_model_path);
    for model_path in model_paths.into_iter().skip(1) {
        config.add_model_path(model_path);
    }
    config.host = host;
    config.port = port;
    config.backend = backend;
    config.reasoning_budget = reasoning_budget;
    Ok(config)
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for `{flag}`"))
}

fn usage() -> String {
    String::from(
        "usage: psionic-openai-server -m <model.gguf> [-m <model.gguf> ...] [--backend cpu] [--host <ip>] [--port <port>] [--reasoning-budget <n>]",
    )
}

#[cfg(test)]
mod tests {
    use super::parse_args_from;

    #[test]
    fn parse_args_accepts_multiple_models() {
        let config =
            parse_args_from(["-m", "/tmp/one.gguf", "-m", "/tmp/two.gguf"]).expect("config");

        assert_eq!(config.model_paths.len(), 2);
        assert_eq!(config.model_paths[0].to_string_lossy(), "/tmp/one.gguf");
        assert_eq!(config.model_paths[1].to_string_lossy(), "/tmp/two.gguf");
    }

    #[test]
    fn parse_args_rejects_non_cpu_backend() {
        let error = parse_args_from(["-m", "/tmp/model.gguf", "--backend", "cuda"])
            .expect_err("generic server should reject non-cpu backend");

        assert!(error.contains("expected cpu"));
    }
}
