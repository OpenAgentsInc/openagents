#![cfg_attr(test, allow(clippy::expect_used))]

use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};

use psionic_serve::{
    GptOssMetalExecutionMode, GptOssOpenAiCompatBackend, GptOssOpenAiCompatConfig,
    GptOssOpenAiCompatServer,
};
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
    let server = GptOssOpenAiCompatServer::from_config(&config)
        .map_err(|error| format!("failed to load GPT-OSS GGUF: {error}"))?;
    let mut stdout = io::stdout();
    let _ = writeln!(
        stdout,
        "psionic gpt-oss server listening on http://{} with model {} backend={} execution_mode={} execution_engine={}",
        listener
            .local_addr()
            .map_err(|error| format!("failed to query listener address: {error}"))?,
        config.model_path.display(),
        server.backend_label(),
        server.execution_mode_label(),
        server.execution_engine_label(),
    );
    server
        .serve(listener)
        .await
        .map_err(|error| format!("server failed: {error}"))
}

fn parse_args() -> Result<GptOssOpenAiCompatConfig, String> {
    parse_args_from(env::args().skip(1))
}

fn parse_args_from<I, S>(args: I) -> Result<GptOssOpenAiCompatConfig, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut model_path = None;
    let mut host = String::from("127.0.0.1");
    let mut port = 8080_u16;
    let mut backend = GptOssOpenAiCompatBackend::Auto;
    let mut context_length = None;
    let mut gpu_layers = None;
    let mut metal_mode = GptOssMetalExecutionMode::Auto;
    let mut reasoning_budget = 0_u8;
    let mut webui_enabled = false;

    let mut args = args.into_iter().map(Into::into);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "-m" | "--model" => {
                model_path = Some(next_value(&mut args, argument.as_str())?);
            }
            "--host" => {
                host = next_value(&mut args, argument.as_str())?;
            }
            "--backend" => {
                backend = match next_value(&mut args, argument.as_str())?.as_str() {
                    "auto" => GptOssOpenAiCompatBackend::Auto,
                    "cpu" => GptOssOpenAiCompatBackend::Cpu,
                    "cuda" => GptOssOpenAiCompatBackend::Cuda,
                    "metal" => GptOssOpenAiCompatBackend::Metal,
                    other => {
                        return Err(format!(
                            "invalid --backend value `{other}` (expected auto, cpu, cuda, or metal)"
                        ));
                    }
                };
            }
            "--metal-mode" => {
                metal_mode = match next_value(&mut args, argument.as_str())?.as_str() {
                    "auto" => GptOssMetalExecutionMode::Auto,
                    "native" => GptOssMetalExecutionMode::Native,
                    "proxy" => GptOssMetalExecutionMode::ProxyLlamaCpp,
                    other => {
                        return Err(format!(
                            "invalid --metal-mode value `{other}` (expected auto, native, or proxy)"
                        ));
                    }
                };
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
    if !matches!(
        backend,
        GptOssOpenAiCompatBackend::Auto | GptOssOpenAiCompatBackend::Metal
    ) && !matches!(metal_mode, GptOssMetalExecutionMode::Auto)
    {
        return Err(format!(
            "`--metal-mode` is only valid with `--backend auto` or `--backend metal`\n\n{}",
            usage()
        ));
    }
    Ok(GptOssOpenAiCompatConfig {
        model_path: model_path.into(),
        host,
        port,
        backend,
        context_length,
        gpu_layers,
        metal_mode,
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
        "usage: psionic-gpt-oss-server -m <model.gguf> [--backend <auto|cpu|cuda|metal>] [--metal-mode <auto|native|proxy>] [--host <ip>] [--port <port>] [-c <ctx>] [-ngl <n>] [--reasoning-budget <n>] [--no-webui]",
    )
}

#[cfg(test)]
mod tests {
    use super::{GptOssMetalExecutionMode, GptOssOpenAiCompatBackend, parse_args_from};

    #[test]
    fn parse_args_accepts_explicit_proxy_metal_mode() {
        let config = parse_args_from([
            "-m",
            "/tmp/model.gguf",
            "--backend",
            "metal",
            "--metal-mode",
            "proxy",
        ])
        .expect("config");

        assert_eq!(config.backend, GptOssOpenAiCompatBackend::Metal);
        assert_eq!(config.metal_mode, GptOssMetalExecutionMode::ProxyLlamaCpp);
    }

    #[test]
    fn parse_args_rejects_metal_mode_for_cpu_backend() {
        let error = parse_args_from([
            "-m",
            "/tmp/model.gguf",
            "--backend",
            "cpu",
            "--metal-mode",
            "native",
        ])
        .expect_err("cpu backend should reject metal mode");

        assert!(error.contains("--metal-mode"));
    }
}
