#![cfg_attr(test, allow(clippy::expect_used))]

use std::{
    env,
    io::{self, Write},
    process::ExitCode,
};

use psionic_observe::{TokioRuntimeTelemetryConfig, build_main_runtime};
use psionic_serve::{OpenAiCompatBackend, OpenAiCompatConfig, OpenAiCompatServer};
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
    let mut qwen38_vision_model_dir = None;
    let mut reasoning_budget = 0_u8;
    let mut mesh_coordination_enabled = true;

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
                backend = match value.as_str() {
                    "cpu" => OpenAiCompatBackend::Cpu,
                    "cuda" => OpenAiCompatBackend::Cuda,
                    "metal" => OpenAiCompatBackend::Metal,
                    _ => {
                        return Err(format!(
                            "invalid --backend value `{value}` (expected cpu, cuda, or metal)\n\n{}",
                            usage()
                        ));
                    }
                };
            }
            "--qwen38-vision-model-dir" => {
                qwen38_vision_model_dir = Some(next_value(&mut args, argument.as_str())?.into());
            }
            "--reasoning-budget" => {
                reasoning_budget = next_value(&mut args, argument.as_str())?
                    .parse()
                    .map_err(|error| format!("invalid --reasoning-budget value: {error}"))?;
            }
            "--mesh-coordination" => {
                let value = next_value(&mut args, argument.as_str())?;
                mesh_coordination_enabled = match value.as_str() {
                    "enabled" => true,
                    "disabled" => false,
                    _ => {
                        return Err(format!(
                            "invalid --mesh-coordination value `{value}` (expected enabled or disabled)\n\n{}",
                            usage()
                        ));
                    }
                };
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
    config.qwen38_vision_model_dir = qwen38_vision_model_dir;
    config.reasoning_budget = reasoning_budget;
    config.mesh_coordination_enabled = mesh_coordination_enabled;
    Ok(config)
}

fn next_value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next()
        .ok_or_else(|| format!("missing value for `{flag}`"))
}

fn usage() -> String {
    String::from(
        "usage: psionic-openai-server -m <model-artifact> [-m <model-artifact> ...] [--backend cpu|cuda|metal] [--qwen38-vision-model-dir <official-model-dir>] [--host <ip>] [--port <port>] [--reasoning-budget <n>] [--mesh-coordination enabled|disabled]",
    )
}

#[cfg(test)]
mod tests {
    use super::parse_args_from;
    use psionic_serve::OpenAiCompatBackend;

    #[test]
    fn parse_args_accepts_multiple_models() {
        let config =
            parse_args_from(["-m", "/tmp/one.gguf", "-m", "/tmp/two.gguf"]).expect("config");

        assert_eq!(config.model_paths.len(), 2);
        assert_eq!(config.model_paths[0].to_string_lossy(), "/tmp/one.gguf");
        assert_eq!(config.model_paths[1].to_string_lossy(), "/tmp/two.gguf");
    }

    #[test]
    fn parse_args_accepts_cuda_backend() {
        let config = parse_args_from(["-m", "/tmp/model.gguf", "--backend", "cuda"])
            .expect("cuda backend should parse");

        assert!(matches!(config.backend, OpenAiCompatBackend::Cuda));
    }

    #[test]
    fn parse_args_accepts_qwen38_vision_model_dir() {
        let config = parse_args_from([
            "-m",
            "/tmp/model.gguf",
            "--qwen38-vision-model-dir",
            "/tmp/Qwen3.8-27B",
        ])
        .expect("Qwen3.8 vision model directory should parse");

        assert_eq!(
            config
                .qwen38_vision_model_dir
                .as_deref()
                .map(|path| path.to_string_lossy().into_owned()),
            Some(String::from("/tmp/Qwen3.8-27B"))
        );
    }

    #[test]
    fn parse_args_accepts_metal_backend() {
        let config = parse_args_from(["-m", "/tmp/model.gguf", "--backend", "metal"])
            .expect("metal backend should parse");

        assert!(matches!(config.backend, OpenAiCompatBackend::Metal));
    }

    #[test]
    fn parse_args_rejects_unknown_backend() {
        let error = parse_args_from(["-m", "/tmp/model.gguf", "--backend", "bogus"])
            .expect_err("generic server should reject unknown backend");

        assert!(error.contains("expected cpu, cuda, or metal"));
    }

    #[test]
    fn parse_args_can_disable_mesh_coordination() {
        let config = parse_args_from(["-m", "/tmp/model.gguf", "--mesh-coordination", "disabled"])
            .expect("mesh coordination flag should parse");

        assert!(!config.mesh_coordination_enabled);
    }
}
