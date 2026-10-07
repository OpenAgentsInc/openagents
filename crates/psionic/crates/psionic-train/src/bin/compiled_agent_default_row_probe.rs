use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use reqwest::blocking::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::{Value, json};

use psionic_eval::{
    COMPILED_AGENT_DEFAULT_ROW_LIVE_REPORT_PATH, CompiledAgentDefaultRowBenchmarkCaseReport,
    CompiledAgentDefaultRowBenchmarkReport, CompiledAgentDefaultRowUsage,
};

#[derive(Debug)]
struct Args {
    base_url: String,
    model: String,
    report_path: PathBuf,
    psionic_root: PathBuf,
    backend_label: String,
    model_path: String,
    host_label: String,
    psionic_revision: Option<String>,
    request_timeout_seconds: f64,
}

#[derive(Debug)]
struct ProbeCase {
    case_id: &'static str,
    prompt: &'static str,
    system_message: &'static str,
    facts: Option<&'static str>,
    use_structured_route_output: bool,
    expected_exact: Option<&'static str>,
    expected_contains: &'static [&'static str],
}

#[derive(Debug)]
struct ObservedCase {
    latency_ms: f64,
    usage: CompiledAgentDefaultRowUsage,
    final_text: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<ChatChoice>,
    usage: Option<ChatUsage>,
    psionic_structured_value: Option<PsionicStructuredValueEnvelope>,
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatMessage,
}

#[derive(Debug, Deserialize)]
struct ChatMessage {
    content: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
struct ChatUsage {
    prompt_tokens: Option<u32>,
    completion_tokens: Option<u32>,
    total_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct PsionicStructuredValueEnvelope {
    value: Value,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = parse_args()?;
    if let Some(parent) = args.report_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let timeout = Duration::from_secs_f64(args.request_timeout_seconds);
    let client = Client::builder().timeout(timeout).build()?;
    let psionic_revision = match args.psionic_revision {
        Some(revision) => revision,
        None => git_head(args.psionic_root.as_path())?,
    };

    let mut case_reports = Vec::new();
    let mut all_passed = true;
    for case in probe_cases() {
        let observed = run_case(&client, &args.base_url, &args.model, &case)?;
        let passed = if let Some(expected_exact) = case.expected_exact {
            observed.final_text == expected_exact
        } else {
            let lowered = observed.final_text.to_ascii_lowercase();
            case.expected_contains
                .iter()
                .all(|token| lowered.contains(&token.to_ascii_lowercase()))
        };
        all_passed &= passed;
        case_reports.push(CompiledAgentDefaultRowBenchmarkCaseReport {
            case_id: String::from(case.case_id),
            prompt: String::from(case.prompt),
            pass: passed,
            latency_ms: observed.latency_ms,
            observed_text: observed.final_text,
            expected_summary: expected_summary(&case),
            usage: observed.usage,
        });
    }

    let report = CompiledAgentDefaultRowBenchmarkReport {
        schema_version: String::from("psionic.compiled_agent_default_row_benchmark.v1"),
        row_id: String::from("compiled_agent.qwen35_9b_q4km.archlinux.consumer_gpu.v1"),
        backend_label: args.backend_label,
        host_label: args.host_label,
        psionic_revision,
        measured_at: chrono_like_timestamp(),
        model_artifact: model_artifact_name(&args.model_path, &args.model),
        model_path: args.model_path,
        case_reports,
        all_passed,
        detail: String::from(
            "This is the retained live probe for the first compiled-agent default learned row. It measures only narrow route, grounded-answer-from-facts, and refusal behavior.",
        ),
    };

    fs::write(&args.report_path, serde_json::to_vec_pretty(&report)?)?;
    println!(
        "{{\"report_path\":\"{}\",\"all_passed\":{}}}",
        args.report_path.display(),
        report.all_passed
    );
    if !report.all_passed {
        return Err("compiled-agent default-row live probe failed".into());
    }
    Ok(())
}

fn parse_args() -> Result<Args, Box<dyn std::error::Error>> {
    let mut base_url = None;
    let mut model = None;
    let mut report_path = None;
    let mut psionic_root = None;
    let mut backend_label = String::from("psionic");
    let mut model_path = String::new();
    let mut host_label = default_host_label();
    let mut psionic_revision = None;
    let mut request_timeout_seconds = 60.0_f64;

    let mut args = env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--base-url" => base_url = args.next(),
            "--model" => model = args.next(),
            "--report-path" => report_path = args.next().map(PathBuf::from),
            "--psionic-root" => psionic_root = args.next().map(PathBuf::from),
            "--backend-label" => {
                backend_label = args.next().ok_or("missing value for --backend-label")?
            }
            "--model-path" => model_path = args.next().ok_or("missing value for --model-path")?,
            "--host-label" => host_label = args.next().ok_or("missing value for --host-label")?,
            "--psionic-revision" => {
                psionic_revision = Some(args.next().ok_or("missing value for --psionic-revision")?)
            }
            "--request-timeout-seconds" => {
                let value = args
                    .next()
                    .ok_or("missing value for --request-timeout-seconds")?;
                request_timeout_seconds = value.parse()?;
            }
            "--help" | "-h" => {
                print_usage();
                std::process::exit(0);
            }
            other => return Err(format!("unsupported argument `{other}`").into()),
        }
    }

    Ok(Args {
        base_url: base_url.ok_or("missing --base-url")?,
        model: model.ok_or("missing --model")?,
        report_path: report_path
            .unwrap_or_else(|| PathBuf::from(COMPILED_AGENT_DEFAULT_ROW_LIVE_REPORT_PATH)),
        psionic_root: psionic_root.ok_or("missing --psionic-root")?,
        backend_label,
        model_path,
        host_label,
        psionic_revision,
        request_timeout_seconds,
    })
}

fn print_usage() {
    eprintln!(
        "usage: cargo run -q -p psionic-train --bin compiled_agent_default_row_probe -- --base-url <url> --model <model> --report-path <path> --psionic-root <path> [--backend-label <label>] [--model-path <path>] [--host-label <label>] [--psionic-revision <sha>] [--request-timeout-seconds <seconds>]"
    );
}

fn default_host_label() -> String {
    env::var("HOSTNAME")
        .or_else(|_| env::var("HOST"))
        .unwrap_or_else(|_| String::from("unknown-host"))
}

fn git_head(psionic_root: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let output = std::process::Command::new("git")
        .arg("rev-parse")
        .arg("HEAD")
        .current_dir(psionic_root)
        .output()?;
    if !output.status.success() {
        return Err("failed to resolve psionic git revision".into());
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}

fn probe_cases() -> Vec<ProbeCase> {
    vec![
        ProbeCase {
            case_id: "route_provider",
            prompt: "Can I go online right now?",
            system_message: "Return JSON only.",
            facts: None,
            use_structured_route_output: true,
            expected_exact: Some("provider_status"),
            expected_contains: &[],
        },
        ProbeCase {
            case_id: "route_wallet",
            prompt: "How many sats are in the wallet?",
            system_message: "Return JSON only.",
            facts: None,
            use_structured_route_output: true,
            expected_exact: Some("wallet_status"),
            expected_contains: &[],
        },
        ProbeCase {
            case_id: "grounded_provider",
            prompt: "Can I go online right now?",
            system_message: "You are the grounded-answer module for a narrow compiled agent. Use only the supplied facts. Answer in one short sentence. Do not invent tools or extra facts.",
            facts: Some("provider_ready=true blockers=[]"),
            use_structured_route_output: false,
            expected_exact: None,
            expected_contains: &["ready", "online"],
        },
        ProbeCase {
            case_id: "grounded_wallet",
            prompt: "How many sats are in the wallet?",
            system_message: "You are the grounded-answer module for a narrow compiled agent. Use only the supplied facts. Answer in one short sentence. Do not invent tools or extra facts.",
            facts: Some("wallet_balance_sats=1200 recent_earnings_sats=240"),
            use_structured_route_output: false,
            expected_exact: None,
            expected_contains: &["1200", "sats"],
        },
        ProbeCase {
            case_id: "unsupported_refusal",
            prompt: "Write a poem about GPUs.",
            system_message: "You are the grounded-answer module for a narrow compiled agent. Use only the supplied facts. Answer in one short sentence. Do not invent tools or extra facts.",
            facts: Some("scope=\"provider readiness and wallet balance only\""),
            use_structured_route_output: false,
            expected_exact: None,
            expected_contains: &["provider", "wallet"],
        },
    ]
}

fn run_case(
    client: &Client,
    base_url: &str,
    model: &str,
    case: &ProbeCase,
) -> Result<ObservedCase, Box<dyn std::error::Error>> {
    let user_content = match case.facts {
        Some(facts) => format!("Question: {}\nFacts: {}", case.prompt, facts),
        None => String::from(case.prompt),
    };
    let payload = json!({
        "model": model,
        "temperature": 0,
        "seed": 0,
        "messages": [
            {"role": "system", "content": case.system_message},
            {"role": "user", "content": user_content},
        ],
    });
    let payload = if case.use_structured_route_output {
        let mut payload = payload;
        payload["response_format"] = json!({
            "type": "json_schema",
            "json_schema": {
                "name": "route_decision",
                "schema": {
                    "type": "object",
                    "properties": {
                        "route": {
                            "type": "string",
                            "enum": ["provider_status", "wallet_status", "unsupported"]
                        }
                    },
                    "required": ["route"],
                    "additionalProperties": false
                },
                "strict": true
            }
        });
        payload
    } else {
        payload
    };
    let started = Instant::now();
    let response = client
        .post(format!(
            "{}/chat/completions",
            base_url.trim_end_matches('/')
        ))
        .header(AUTHORIZATION, "Bearer dummy")
        .header(CONTENT_TYPE, "application/json")
        .json(&payload)
        .send()?
        .error_for_status()?;
    let body: ChatCompletionResponse = response.json()?;
    let latency_ms = started.elapsed().as_secs_f64() * 1000.0;
    let final_text = if case.use_structured_route_output {
        body.psionic_structured_value
            .as_ref()
            .and_then(|structured| structured.value.get("route"))
            .and_then(Value::as_str)
            .map(String::from)
            .or_else(|| {
                body.choices
                    .first()
                    .and_then(|choice| choice.message.content.as_ref())
                    .map(|text| text.trim().to_string())
            })
            .unwrap_or_default()
    } else {
        body.choices
            .first()
            .and_then(|choice| choice.message.content.as_ref())
            .map(|text| text.trim().to_string())
            .unwrap_or_default()
    };
    let usage = usage_from(body.usage.unwrap_or_default());
    Ok(ObservedCase {
        latency_ms: (latency_ms * 1000.0).round() / 1000.0,
        usage,
        final_text,
    })
}

fn usage_from(usage: ChatUsage) -> CompiledAgentDefaultRowUsage {
    let prompt_tokens = usage.prompt_tokens.unwrap_or(0);
    let completion_tokens = usage.completion_tokens.unwrap_or(0);
    let total_tokens = usage
        .total_tokens
        .unwrap_or(prompt_tokens.saturating_add(completion_tokens));
    CompiledAgentDefaultRowUsage {
        prompt_tokens,
        completion_tokens,
        total_tokens,
    }
}

fn expected_summary(case: &ProbeCase) -> String {
    match case.expected_exact {
        Some(value) => String::from(value),
        None => case.expected_contains.join(", "),
    }
}

fn model_artifact_name(model_path: &str, model: &str) -> String {
    if model_path.is_empty() {
        return String::from(model);
    }
    Path::new(model_path)
        .file_name()
        .and_then(|value| value.to_str())
        .map_or_else(|| String::from(model), String::from)
}

fn chrono_like_timestamp() -> String {
    let output = std::process::Command::new("date")
        .arg("-u")
        .arg("+%Y-%m-%dT%H:%M:%SZ")
        .output();
    match output {
        Ok(result) if result.status.success() => String::from_utf8(result.stdout)
            .map(|value| value.trim().to_string())
            .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z")),
        _ => String::from("1970-01-01T00:00:00Z"),
    }
}
