#![cfg_attr(test, allow(clippy::expect_used))]

use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process::ExitCode,
};

use psionic_observe::{TokioRuntimeTelemetryConfig, build_main_runtime};
use psionic_serve::{OpenAiCompatBackend, OpenAiCompatConfig, OpenAiCompatServer, clef};
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
    let (decision, rest) = split_decision_args(env::args().skip(1))?;
    let decision_only = !rest.iter().any(|arg| arg == "-m" || arg == "--model");
    if decision_only && decision.model_paths.is_empty() {
        return Err(format!("missing required `-m` / `--model`\n\n{}", usage()));
    }
    let mut args = rest;
    if decision_only {
        args.extend([String::from("-m"), String::from(DECISION_ONLY_PLACEHOLDER)]);
    }
    let mut config = parse_args_from(args)?;
    if decision_only {
        config.model_paths.clear();
    }
    // A Clef GGUF passed with `-m` is a decision model.
    let mut decision_paths = decision.model_paths.clone();
    config.model_paths.retain(|path| {
        if clef::is_clef_gguf(path) {
            decision_paths.push(path.clone());
            false
        } else {
            true
        }
    });
    let lanes = decision_paths
        .iter()
        .map(|path| {
            let head = decision.head.clone().map_or(
                clef::ClefHeadSource::Embedded,
                clef::ClefHeadSource::Safetensors,
            );
            let mut lane = clef::ClefDecisionLane::load(path, head, decision.limits)?;
            if !decision.calibrations.is_empty() {
                let maps = decision
                    .calibrations
                    .iter()
                    .map(|path| clef::calibration::ClefCalibration::load(path))
                    .collect::<Result<Vec<_>, _>>()?;
                lane = lane.with_calibrations(maps)?;
            }
            if let Some(dir) = &decision.export_rows {
                lane = lane.with_row_export(dir)?;
            }
            Ok::<_, String>(lane)
        })
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("failed to load decision model: {error}"))?;
    let backends = lanes
        .iter()
        .map(|lane| lane.backend())
        .collect::<Vec<_>>()
        .join(",");
    let lanes = clef::ClefLanes::new(lanes);
    let address = config.socket_addr().map_err(|error| error.to_string())?;
    let listener = TcpListener::bind(address)
        .await
        .map_err(|error| format!("failed to bind {address}: {error}"))?;
    if config.model_paths.is_empty() {
        let _ = writeln!(
            io::stdout(),
            "psionic openai server listening on http://{} decision_models={} backend={} execution_mode=native route=/v1/systemone",
            listener
                .local_addr()
                .map_err(|error| format!("failed to query listener address: {error}"))?,
            decision_paths
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(","),
            backends,
        );
        return clef::serve(listener, clef::decision_router(lanes))
            .await
            .map_err(|error| format!("server failed: {error}"));
    }
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
    if lanes.is_empty() {
        return server
            .serve(listener)
            .await
            .map_err(|error| format!("server failed: {error}"));
    }
    clef::serve(
        listener,
        server.router().merge(clef::systemone_router(lanes)),
    )
    .await
    .map_err(|error| format!("server failed: {error}"))
}

const DECISION_ONLY_PLACEHOLDER: &str = "<decision-only>";

/// Decision-model flags, taken out before the generic flags are read.
#[derive(Clone, Debug, Default)]
struct DecisionArgs {
    model_paths: Vec<PathBuf>,
    head: Option<PathBuf>,
    limits: clef::ClefLimits,
    calibrations: Vec<PathBuf>,
    export_rows: Option<PathBuf>,
}

fn split_decision_args<I, S>(args: I) -> Result<(DecisionArgs, Vec<String>), String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut decision = DecisionArgs::default();
    let mut rest = Vec::new();
    let mut args = args.into_iter().map(Into::into);
    let number = |flag: &str, value: String| -> Result<usize, String> {
        value
            .parse::<usize>()
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| format!("invalid {flag} value `{value}`"))
    };
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--decision-model" => decision
                .model_paths
                .push(next_value(&mut args, argument.as_str())?.into()),
            "--clef-head" => decision.head = Some(next_value(&mut args, argument.as_str())?.into()),
            "--decision-calibration" => decision
                .calibrations
                .push(next_value(&mut args, argument.as_str())?.into()),
            "--decision-export-rows" => {
                decision.export_rows = Some(next_value(&mut args, argument.as_str())?.into());
            }
            "--decision-max-tokens" => {
                decision.limits.max_tokens =
                    number(&argument, next_value(&mut args, argument.as_str())?)?;
            }
            "--decision-device" => {
                decision.limits.device = next_value(&mut args, argument.as_str())?.parse()?;
            }
            "--decision-accumulate" => {
                decision.limits.accumulate_f16 = match next_value(&mut args, argument.as_str())?.as_str() {
                    "f16" => true,
                    "f32" => false,
                    other => return Err(format!("--decision-accumulate takes f16 or f32, not `{other}`")),
                };
            }
            "--decision-chunk" => {
                decision.limits.prefill_chunk =
                    number(&argument, next_value(&mut args, argument.as_str())?)?;
            }
            "--decision-max-questions" => {
                decision.limits.max_questions =
                    number(&argument, next_value(&mut args, argument.as_str())?)?;
            }
            "--decision-max-options" => {
                let value = number(&argument, next_value(&mut args, argument.as_str())?)?;
                if value < 2 {
                    return Err(String::from("--decision-max-options must be at least 2"));
                }
                decision.limits.max_options = value;
            }
            _ => rest.push(argument),
        }
    }
    Ok((decision, rest))
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
        "usage: psionic-openai-server -m <model-artifact> [-m <model-artifact> ...] [--backend cpu|cuda|metal] [--qwen38-vision-model-dir <official-model-dir>] [--host <ip>] [--port <port>] [--reasoning-budget <n>] [--mesh-coordination enabled|disabled] [--decision-model <clef-or-qwen35-gguf>] [--clef-head <joint_head dir or .safetensors>] [--decision-max-tokens <n>] [--decision-max-questions <n>] [--decision-max-options <n>] [--decision-chunk <n>] [--decision-device auto|cpu|cuda] [--decision-accumulate f16|f32] [--decision-calibration <map.json> ...] [--decision-export-rows <dir>]\n\nA Clef GGUF (general.architecture = clef) given with -m is served as a decision model at POST /v1/systemone; with only decision models, -m may be omitted.",
    )
}

#[cfg(test)]
mod tests {
    use super::parse_args_from;
    use psionic_serve::OpenAiCompatBackend;

    #[test]
    fn decision_flags_are_taken_out_before_the_generic_flags() {
        let (decision, rest) = super::split_decision_args([
            "--decision-model",
            "/tmp/clef.gguf",
            "--port",
            "9000",
            "--clef-head",
            "/tmp/head",
            "--decision-max-tokens",
            "32768",
            "--decision-chunk",
            "64",
        ])
        .expect("decision flags");
        assert_eq!(decision.model_paths.len(), 1);
        assert_eq!(decision.limits.max_tokens, 32768);
        assert_eq!(decision.limits.prefill_chunk, 64);
        assert_eq!(decision.limits.max_options, 255);
        assert!(decision.head.is_some());
        assert_eq!(rest, ["--port", "9000"]);
        assert!(super::split_decision_args(["--decision-max-options", "1"]).is_err());
    }

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
