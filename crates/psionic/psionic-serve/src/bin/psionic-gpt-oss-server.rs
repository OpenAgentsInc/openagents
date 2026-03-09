use std::{env, process::ExitCode};

use psionic_serve::{GptOssCudaOpenAiCompatServer, GptOssOpenAiCompatConfig};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> ExitCode {
    match run().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
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
    let server = GptOssCudaOpenAiCompatServer::from_config(&config)
        .map_err(|error| format!("failed to load GPT-OSS GGUF: {error}"))?;
    eprintln!(
        "psionic gpt-oss server listening on http://{} with model {}",
        listener
            .local_addr()
            .map_err(|error| format!("failed to query listener address: {error}"))?,
        config.model_path.display(),
    );
    server
        .serve(listener)
        .await
        .map_err(|error| format!("server failed: {error}"))
}

fn parse_args() -> Result<GptOssOpenAiCompatConfig, String> {
    let mut model_path = None;
    let mut host = String::from("127.0.0.1");
    let mut port = 8080_u16;
    let mut context_length = None;
    let mut gpu_layers = None;
    let mut reasoning_budget = 0_u8;
    let mut webui_enabled = false;

    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-m" | "--model" => {
                model_path = Some(next_value(&mut args, argument.as_str())?);
            }
            "--host" => {
                host = next_value(&mut args, argument.as_str())?;
            }
            "--port" => {
                port = next_value(&mut args, argument.as_str())?
                    .parse()
                    .map_err(|error| format!("invalid --port value: {error}"))?;
            }
            "-c" | "--ctx-size" => {
                context_length = Some(
                    next_value(&mut args, argument.as_str())?
                        .parse()
                        .map_err(|error| format!("invalid {argument} value: {error}"))?,
                );
            }
            "-ngl" => {
                gpu_layers = Some(
                    next_value(&mut args, argument.as_str())?
                        .parse()
                        .map_err(|error| format!("invalid -ngl value: {error}"))?,
                );
            }
            "--reasoning-budget" => {
                reasoning_budget = next_value(&mut args, argument.as_str())?
                    .parse()
                    .map_err(|error| format!("invalid --reasoning-budget value: {error}"))?;
            }
            "--no-webui" => {
                webui_enabled = false;
            }
            "--webui" => {
                webui_enabled = true;
            }
            "-h" | "--help" => {
                return Err(usage());
            }
            other => {
                return Err(format!("unrecognized argument `{other}`\n\n{}", usage()));
            }
        }
    }

    let Some(model_path) = model_path else {
        return Err(format!("missing required `-m` / `--model`\n\n{}", usage()));
    };
    Ok(GptOssOpenAiCompatConfig {
        model_path: model_path.into(),
        host,
        port,
        context_length,
        gpu_layers,
        reasoning_budget,
        webui_enabled,
    })
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for `{flag}`"))
}

fn usage() -> String {
    String::from(
        "usage: psionic-gpt-oss-server -m <model.gguf> [--host <ip>] [--port <port>] [-c <ctx>] [-ngl <n>] [--reasoning-budget <n>] [--no-webui]",
    )
}
