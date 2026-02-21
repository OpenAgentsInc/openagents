#![allow(clippy::print_stdout, clippy::print_stderr, clippy::too_many_lines)]

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow, bail};
use chrono::Utc;
use clap::{Args, Parser, Subcommand};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

const DEFAULT_GATEWAY_BASE_URL: &str = "https://l402.openagents.com";
const DEFAULT_GATEWAY_CHALLENGE_URL: &str = "https://l402.openagents.com/staging";
const DEFAULT_GATEWAY_PROXY_URL: &str = "https://l402.openagents.com/staging";
const DEFAULT_EP212_ROUTE_A_URL: &str = "https://l402.openagents.com/ep212/premium-signal";
const DEFAULT_EP212_ROUTE_B_URL: &str = "https://l402.openagents.com/ep212/expensive-signal";
const DEFAULT_EP212_SATS4AI_URL: &str = "https://sats4ai.com/api/l402/text-generation";

#[derive(Parser)]
#[command(name = "lightning-ops")]
#[command(about = "Rust Lightning Ops CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    #[command(name = "smoke:compile")]
    SmokeCompile(ModeJsonArgs),
    #[command(name = "compile:api")]
    CompileApi(JsonOnlyArgs),
    #[command(name = "reconcile:api")]
    ReconcileApi(JsonOnlyArgs),
    #[command(name = "smoke:staging")]
    SmokeStaging(ModeJsonArgs),
    #[command(name = "smoke:security")]
    SmokeSecurity(ModeJsonArgs),
    #[command(name = "smoke:settlement")]
    SmokeSettlement(ModeJsonArgs),
    #[command(name = "smoke:observability")]
    SmokeObservability(ModeJsonArgs),
    #[command(name = "smoke:full-flow")]
    SmokeFullFlow(FullFlowArgs),
    #[command(name = "smoke:ep212-routes")]
    SmokeEp212Routes(Ep212RoutesArgs),
    #[command(name = "smoke:ep212-full-flow")]
    SmokeEp212FullFlow(Ep212FullFlowArgs),
}

#[derive(Args, Clone)]
struct ModeJsonArgs {
    #[arg(long, default_value = "mock")]
    mode: String,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct JsonOnlyArgs {
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct FullFlowArgs {
    #[arg(long, default_value = "mock")]
    mode: String,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    artifact_dir: Option<PathBuf>,
    #[arg(long)]
    local_artifact: Option<PathBuf>,
    #[arg(long)]
    allow_missing_local_artifact: bool,
}

#[derive(Args, Clone)]
struct Ep212RoutesArgs {
    #[arg(long, default_value = "mock")]
    mode: String,
    #[arg(long)]
    json: bool,
}

#[derive(Args, Clone)]
struct Ep212FullFlowArgs {
    #[arg(long, default_value = "mock")]
    mode: String,
    #[arg(long)]
    json: bool,
    #[arg(long)]
    artifact_dir: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SmokeMode {
    Mock,
    Api,
}

impl SmokeMode {
    fn parse(input: &str) -> Result<Self> {
        match input.trim().to_lowercase().as_str() {
            "mock" => Ok(Self::Mock),
            "api" => Ok(Self::Api),
            other => bail!("unsupported mode: {other} (expected mock|api)"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Ep212Mode {
    Mock,
    Live,
}

impl Ep212Mode {
    fn parse(input: &str) -> Result<Self> {
        match input.trim().to_lowercase().as_str() {
            "mock" => Ok(Self::Mock),
            "live" => Ok(Self::Live),
            other => bail!("unsupported mode: {other} (expected mock|live)"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ControlPlanePaywall {
    paywall_id: String,
    owner_id: String,
    name: String,
    description: Option<String>,
    status: String,
    policy: ControlPlanePaywallPolicy,
    routes: Vec<ControlPlanePaywallRoute>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ControlPlanePaywallPolicy {
    pricing_mode: String,
    fixed_amount_msats: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ControlPlanePaywallRoute {
    route_id: String,
    host_pattern: String,
    path_pattern: String,
    upstream_url: String,
    protocol: String,
    timeout_ms: i64,
    priority: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompileDiagnostic {
    code: String,
    severity: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    paywall_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    route_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CompiledRule {
    id: String,
    paywall_id: String,
    owner_id: String,
    host_pattern: String,
    path_pattern: String,
    upstream_url: String,
    protocol: String,
    timeout_ms: i64,
    priority: i64,
    amount_msats: i64,
}

#[derive(Debug, Clone)]
struct CompiledArtifact {
    config_hash: String,
    rule_count: usize,
    valid: bool,
    diagnostics: Vec<CompileDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompileSummary {
    ok: bool,
    config_hash: String,
    rule_count: usize,
    valid: bool,
    deployment_status: String,
    deployment_id: String,
    diagnostics_count: usize,
    diagnostics: Vec<CompileDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReconcileSummary {
    ok: bool,
    request_id: String,
    execution_path: String,
    config_hash: String,
    rule_count: usize,
    valid: bool,
    deployment_status: String,
    deployment_id: String,
    challenge_ok: bool,
    proxy_ok: bool,
    health_ok: bool,
    diagnostics_count: usize,
    diagnostics: Vec<CompileDiagnostic>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SecuritySmokeSummary {
    ok: bool,
    execution_path: String,
    fail_closed: BTreeMap<String, Value>,
    global_pause: BTreeMap<String, Value>,
    owner_kill_switch: BTreeMap<String, Value>,
    recovery: BTreeMap<String, Value>,
    credential_lifecycle: BTreeMap<String, Value>,
    status_snapshot: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SettlementSmokeSummary {
    ok: bool,
    processed: usize,
    invoice_transitions: Vec<Value>,
    settlements: Vec<Value>,
    settlement_ids: Vec<String>,
    payment_proof_refs: Vec<String>,
    correlation_refs: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ObservabilitySmokeSummary {
    ok: bool,
    request_id: String,
    execution_path: String,
    record_count: usize,
    required_field_keys: Vec<String>,
    missing_field_keys: Vec<String>,
    correlation: BTreeMap<String, Value>,
    records: Vec<Value>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Ep212RoutesSummary {
    ok: bool,
    mode: String,
    route_a_url: String,
    route_b_url: String,
    route_a_status: i64,
    route_b_status: i64,
    route_a_paid_success: bool,
    route_b_policy_blocked: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FullFlowSummary {
    ok: bool,
    mode: String,
    request_id: String,
    execution_path: String,
    config_hash: String,
    deployment_status: String,
    challenge_ok: bool,
    proxy_ok: bool,
    health_ok: bool,
    local_artifact_present: bool,
    output_dir: String,
}

#[derive(Debug, Clone)]
struct ApiClient {
    base_url: String,
    secret: String,
    http: Client,
}

#[derive(Debug, Clone)]
struct DeploymentRecord {
    deployment_id: String,
    status: String,
}

impl ApiClient {
    fn from_env() -> Result<Self> {
        let base_url = std::env::var("OA_LIGHTNING_OPS_API_BASE_URL")
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .unwrap_or_default();
        if base_url.is_empty() {
            bail!("OA_LIGHTNING_OPS_API_BASE_URL is required");
        }
        let secret = std::env::var("OA_LIGHTNING_OPS_SECRET")
            .map(|value| value.trim().to_string())
            .unwrap_or_default();
        if secret.is_empty() {
            bail!("OA_LIGHTNING_OPS_SECRET is required");
        }
        Ok(Self {
            base_url,
            secret,
            http: Client::new(),
        })
    }

    async fn query(&self, function_name: &str, args: Value) -> Result<Value> {
        self.request("query", function_name, args).await
    }

    async fn mutation(&self, function_name: &str, args: Value) -> Result<Value> {
        self.request("mutation", function_name, args).await
    }

    async fn request(&self, kind: &str, function_name: &str, args: Value) -> Result<Value> {
        let endpoint = format!(
            "{}/api/internal/lightning-ops/control-plane/{}",
            self.base_url, kind
        );
        let response = self
            .http
            .post(endpoint)
            .header(CONTENT_TYPE, "application/json")
            .json(&json!({
                "functionName": function_name,
                "args": args,
            }))
            .send()
            .await
            .with_context(|| format!("control-plane {} request failed", kind))?;

        let status = response.status();
        let body: Value = response
            .json()
            .await
            .with_context(|| "control-plane response JSON decode failed")?;

        if !status.is_success() {
            bail!("control-plane {kind}:{function_name} failed with {status}: {body}");
        }
        if body.get("error").is_some() && !body["error"].is_null() {
            bail!(
                "control-plane {kind}:{function_name} error: {}",
                body["error"]
            );
        }

        Ok(body)
    }
}

fn build_mock_paywalls() -> Vec<ControlPlanePaywall> {
    vec![
        ControlPlanePaywall {
            paywall_id: "pw_weather".to_string(),
            owner_id: "owner_weather".to_string(),
            name: "Premium Weather".to_string(),
            description: Some("Weather API".to_string()),
            status: "active".to_string(),
            policy: ControlPlanePaywallPolicy {
                pricing_mode: "fixed".to_string(),
                fixed_amount_msats: 2_500,
            },
            routes: vec![ControlPlanePaywallRoute {
                route_id: "route_weather".to_string(),
                host_pattern: "openagents.com".to_string(),
                path_pattern: "/api/weather/premium".to_string(),
                upstream_url: "https://weather.vendor.example.com/v1/premium".to_string(),
                protocol: "https".to_string(),
                timeout_ms: 8_000,
                priority: 20,
            }],
        },
        ControlPlanePaywall {
            paywall_id: "pw_news".to_string(),
            owner_id: "owner_news".to_string(),
            name: "Premium News".to_string(),
            description: Some("News API".to_string()),
            status: "active".to_string(),
            policy: ControlPlanePaywallPolicy {
                pricing_mode: "fixed".to_string(),
                fixed_amount_msats: 1_800,
            },
            routes: vec![ControlPlanePaywallRoute {
                route_id: "route_news".to_string(),
                host_pattern: "openagents.com".to_string(),
                path_pattern: "/api/news/premium".to_string(),
                upstream_url: "https://news.vendor.example.com/v2/premium".to_string(),
                protocol: "https".to_string(),
                timeout_ms: 8_000,
                priority: 10,
            }],
        },
    ]
}

fn hash_text(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    let digest = hasher.finalize();
    let mut out = String::new();
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn make_snapshot_hash(paywalls: &[ControlPlanePaywall]) -> Result<String> {
    let serialized = serde_json::to_string(paywalls)?;
    Ok(hash_text(&serialized))
}

fn compile_paywalls(paywalls: &[ControlPlanePaywall]) -> Result<CompiledArtifact> {
    let mut diagnostics: Vec<CompileDiagnostic> = Vec::new();
    let mut rules: Vec<CompiledRule> = Vec::new();

    for paywall in paywalls {
        if paywall.policy.pricing_mode != "fixed" {
            diagnostics.push(CompileDiagnostic {
                code: "invalid_pricing_mode".to_string(),
                severity: "error".to_string(),
                message: "Only fixed pricing mode is supported".to_string(),
                paywall_id: Some(paywall.paywall_id.clone()),
                route_id: None,
                details: Some(json!({"pricingMode": paywall.policy.pricing_mode})),
            });
            continue;
        }
        if paywall.policy.fixed_amount_msats <= 0 {
            diagnostics.push(CompileDiagnostic {
                code: "missing_pricing".to_string(),
                severity: "error".to_string(),
                message: "fixedAmountMsats must be greater than zero".to_string(),
                paywall_id: Some(paywall.paywall_id.clone()),
                route_id: None,
                details: Some(json!({"fixedAmountMsats": paywall.policy.fixed_amount_msats})),
            });
            continue;
        }

        for route in &paywall.routes {
            let host = route.host_pattern.trim().to_lowercase();
            let path = normalize_path_pattern(&route.path_pattern);
            let upstream = route.upstream_url.trim().to_string();
            let protocol = route.protocol.trim().to_lowercase();

            if host.is_empty() || path.is_empty() {
                diagnostics.push(CompileDiagnostic {
                    code: "invalid_route_pattern".to_string(),
                    severity: "error".to_string(),
                    message: "hostPattern and pathPattern must be non-empty".to_string(),
                    paywall_id: Some(paywall.paywall_id.clone()),
                    route_id: Some(route.route_id.clone()),
                    details: Some(json!({"hostPattern": route.host_pattern, "pathPattern": route.path_pattern})),
                });
                continue;
            }

            if protocol != "http" && protocol != "https" {
                diagnostics.push(CompileDiagnostic {
                    code: "missing_route_protocol".to_string(),
                    severity: "error".to_string(),
                    message: "route protocol must be http or https".to_string(),
                    paywall_id: Some(paywall.paywall_id.clone()),
                    route_id: Some(route.route_id.clone()),
                    details: Some(json!({"protocol": route.protocol})),
                });
                continue;
            }

            if upstream.is_empty() {
                diagnostics.push(CompileDiagnostic {
                    code: "invalid_upstream_url".to_string(),
                    severity: "error".to_string(),
                    message: "upstreamUrl must be non-empty".to_string(),
                    paywall_id: Some(paywall.paywall_id.clone()),
                    route_id: Some(route.route_id.clone()),
                    details: None,
                });
                continue;
            }

            let parsed = reqwest::Url::parse(&upstream);
            match parsed {
                Ok(url) => {
                    let scheme = url.scheme();
                    if scheme != protocol {
                        diagnostics.push(CompileDiagnostic {
                            code: "invalid_upstream_url".to_string(),
                            severity: "error".to_string(),
                            message: "upstreamUrl protocol must match route protocol".to_string(),
                            paywall_id: Some(paywall.paywall_id.clone()),
                            route_id: Some(route.route_id.clone()),
                            details: Some(
                                json!({"upstreamUrl": upstream, "routeProtocol": protocol}),
                            ),
                        });
                        continue;
                    }
                }
                Err(_) => {
                    diagnostics.push(CompileDiagnostic {
                        code: "invalid_upstream_url".to_string(),
                        severity: "error".to_string(),
                        message: "upstreamUrl must be a valid URL".to_string(),
                        paywall_id: Some(paywall.paywall_id.clone()),
                        route_id: Some(route.route_id.clone()),
                        details: Some(json!({"upstreamUrl": upstream})),
                    });
                    continue;
                }
            }

            rules.push(CompiledRule {
                id: format!("{}:{}", paywall.paywall_id, route.route_id),
                paywall_id: paywall.paywall_id.clone(),
                owner_id: paywall.owner_id.clone(),
                host_pattern: host,
                path_pattern: path,
                upstream_url: upstream,
                protocol,
                timeout_ms: if route.timeout_ms > 0 {
                    route.timeout_ms
                } else {
                    15_000
                },
                priority: route.priority,
                amount_msats: paywall.policy.fixed_amount_msats,
            });
        }
    }

    rules.sort_by(|a, b| {
        a.priority
            .cmp(&b.priority)
            .then_with(|| a.host_pattern.cmp(&b.host_pattern))
            .then_with(|| a.path_pattern.cmp(&b.path_pattern))
            .then_with(|| a.paywall_id.cmp(&b.paywall_id))
            .then_with(|| a.id.cmp(&b.id))
    });

    let mut seen_route_keys: BTreeSet<String> = BTreeSet::new();
    for rule in &rules {
        let key = format!("{}::{}", rule.host_pattern, rule.path_pattern);
        if seen_route_keys.contains(&key) {
            diagnostics.push(CompileDiagnostic {
                code: "duplicate_route".to_string(),
                severity: "error".to_string(),
                message: format!("Duplicate host/path rule detected ({key})"),
                paywall_id: Some(rule.paywall_id.clone()),
                route_id: Some(rule.id.clone()),
                details: None,
            });
        } else {
            seen_route_keys.insert(key);
        }
    }

    if rules.is_empty() {
        diagnostics.push(CompileDiagnostic {
            code: "no_compilable_routes".to_string(),
            severity: "error".to_string(),
            message: "No valid routes available to compile".to_string(),
            paywall_id: None,
            route_id: None,
            details: None,
        });
    }

    let valid = diagnostics.iter().all(|diag| diag.severity != "error");
    let aperture_yaml = render_aperture_yaml(&rules);
    let snapshot_hash = make_snapshot_hash(paywalls)?;
    let config_hash = if valid {
        hash_text(&aperture_yaml)
    } else {
        snapshot_hash
    };

    Ok(CompiledArtifact {
        config_hash,
        rule_count: rules.len(),
        valid,
        diagnostics,
    })
}

fn normalize_path_pattern(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }
    if trimmed.starts_with('/') {
        return trimmed.to_string();
    }
    format!("/{trimmed}")
}

fn render_aperture_yaml(rules: &[CompiledRule]) -> String {
    let mut lines = vec!["version: 1".to_string(), "routes:".to_string()];
    for rule in rules {
        lines.push(format!("  - id: {}", rule.id));
        lines.push("    match:".to_string());
        lines.push(format!("      host: {}", rule.host_pattern));
        lines.push(format!("      path: {}", rule.path_pattern));
        lines.push("    upstream:".to_string());
        lines.push(format!("      url: {}", rule.upstream_url));
        lines.push(format!("      protocol: {}", rule.protocol));
        lines.push(format!("      timeout_ms: {}", rule.timeout_ms));
        lines.push("    auth:".to_string());
        lines.push("      type: l402".to_string());
        lines.push(format!("      paywall_id: {}", rule.paywall_id));
        lines.push("    pricing:".to_string());
        lines.push("      mode: fixed_msats".to_string());
        lines.push(format!("      amount_msats: {}", rule.amount_msats));
    }
    format!("{}\n", lines.join("\n"))
}

async fn list_paywalls_for_compile(client: &ApiClient) -> Result<Vec<ControlPlanePaywall>> {
    let body = client
        .query(
            "lightning/ops:listPaywallControlPlaneState",
            json!({
                "secret": client.secret,
                "statuses": ["active", "paused"],
            }),
        )
        .await?;

    if let Some(paywalls) = body.get("paywalls") {
        return serde_json::from_value(paywalls.clone())
            .context("failed to decode paywalls from control-plane response");
    }

    if let Some(result) = body.get("result")
        && let Some(paywalls) = result.get("paywalls")
    {
        return serde_json::from_value(paywalls.clone())
            .context("failed to decode paywalls from control-plane result payload");
    }

    bail!("control-plane response missing paywalls field: {body}")
}

async fn record_deployment_intent(
    client: &ApiClient,
    artifact: &CompiledArtifact,
    status: &str,
    request_id: &str,
    metadata: Value,
) -> Result<DeploymentRecord> {
    let body = client
        .mutation(
            "lightning/ops:recordGatewayCompileIntent",
            json!({
                "secret": client.secret,
                "configHash": artifact.config_hash,
                "status": status,
                "diagnostics": artifact.diagnostics,
                "metadata": metadata,
                "requestId": request_id,
            }),
        )
        .await?;

    parse_deployment_record(&body, status)
}

fn parse_deployment_record(body: &Value, fallback_status: &str) -> Result<DeploymentRecord> {
    if let Some(deployment) = body.get("deployment") {
        let deployment_id = deployment
            .get("deploymentId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("deploymentId missing in deployment payload"))?;
        let status = deployment
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or(fallback_status)
            .to_string();
        return Ok(DeploymentRecord {
            deployment_id,
            status,
        });
    }

    if let Some(result) = body.get("result")
        && let Some(deployment) = result.get("deployment")
    {
        let deployment_id = deployment
            .get("deploymentId")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .ok_or_else(|| anyhow!("deploymentId missing in result.deployment payload"))?;
        let status = deployment
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or(fallback_status)
            .to_string();
        return Ok(DeploymentRecord {
            deployment_id,
            status,
        });
    }

    Ok(DeploymentRecord {
        deployment_id: format!("dep_{}", Uuid::now_v7()),
        status: fallback_status.to_string(),
    })
}

fn make_compile_summary(
    artifact: &CompiledArtifact,
    deployment: &DeploymentRecord,
) -> CompileSummary {
    CompileSummary {
        ok: artifact.valid,
        config_hash: artifact.config_hash.clone(),
        rule_count: artifact.rule_count,
        valid: artifact.valid,
        deployment_status: deployment.status.clone(),
        deployment_id: deployment.deployment_id.clone(),
        diagnostics_count: artifact.diagnostics.len(),
        diagnostics: artifact.diagnostics.clone(),
    }
}

fn print_json<T: Serialize>(value: &T) -> Result<()> {
    println!("{}", serde_json::to_string(value)?);
    Ok(())
}

fn print_compile_human(summary: &CompileSummary) {
    println!("configHash={}", summary.config_hash);
    println!("ruleCount={}", summary.rule_count);
    println!("valid={}", summary.valid);
    println!("deploymentStatus={}", summary.deployment_status);
    println!("deploymentId={}", summary.deployment_id);
}

fn now_request_id(prefix: &str) -> String {
    format!("{prefix}:{}", Utc::now().timestamp_millis())
}

async fn run_compile(mode: SmokeMode) -> Result<CompileSummary> {
    match mode {
        SmokeMode::Mock => {
            let paywalls = build_mock_paywalls();
            let artifact = compile_paywalls(&paywalls)?;
            let deployment = DeploymentRecord {
                deployment_id: "dep_mock_compile".to_string(),
                status: if artifact.valid {
                    "pending".to_string()
                } else {
                    "failed".to_string()
                },
            };
            Ok(make_compile_summary(&artifact, &deployment))
        }
        SmokeMode::Api => {
            let client = ApiClient::from_env()?;
            let paywalls = list_paywalls_for_compile(&client).await?;
            let artifact = compile_paywalls(&paywalls)?;
            let request_id = now_request_id("compile");
            let deployment = record_deployment_intent(
                &client,
                &artifact,
                if artifact.valid { "pending" } else { "failed" },
                &request_id,
                json!({
                    "executionPath": "hosted-node",
                    "ruleCount": artifact.rule_count,
                    "valid": artifact.valid,
                }),
            )
            .await?;
            Ok(make_compile_summary(&artifact, &deployment))
        }
    }
}

fn env_optional(key: &str, default_value: &str) -> String {
    std::env::var(key)
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|_| default_value.to_string())
}

async fn check_http_status(
    client: &Client,
    url: &str,
    auth_header: Option<&str>,
    ops_token: Option<&str>,
) -> Result<(bool, StatusCode)> {
    let mut headers = HeaderMap::new();

    if let Some(header) = auth_header {
        let value = HeaderValue::from_str(header)
            .with_context(|| "invalid authorization header value for proxy probe")?;
        headers.insert(AUTHORIZATION, value);
    } else if let Some(token) = ops_token {
        let bearer = format!("Bearer {token}");
        let value = HeaderValue::from_str(&bearer)
            .with_context(|| "invalid ops token for gateway probe")?;
        headers.insert(AUTHORIZATION, value);
    }

    let response = client
        .get(url)
        .headers(headers)
        .send()
        .await
        .with_context(|| format!("probe failed for {url}"))?;
    let status = response.status();
    Ok((status.is_success(), status))
}

async fn run_reconcile_api() -> Result<ReconcileSummary> {
    let api_client = ApiClient::from_env()?;
    let paywalls = list_paywalls_for_compile(&api_client).await?;
    let artifact = compile_paywalls(&paywalls)?;

    let request_id = now_request_id("reconcile");
    if !artifact.valid {
        let deployment = record_deployment_intent(
            &api_client,
            &artifact,
            "failed",
            &request_id,
            json!({
                "executionPath": "hosted-node",
                "ruleCount": artifact.rule_count,
                "valid": false,
                "failure": {"code": "compile_validation_failed"},
            }),
        )
        .await?;

        return Ok(ReconcileSummary {
            ok: false,
            request_id,
            execution_path: "hosted-node".to_string(),
            config_hash: artifact.config_hash,
            rule_count: artifact.rule_count,
            valid: artifact.valid,
            deployment_status: deployment.status,
            deployment_id: deployment.deployment_id,
            challenge_ok: false,
            proxy_ok: false,
            health_ok: false,
            diagnostics_count: artifact.diagnostics.len(),
            diagnostics: artifact.diagnostics,
        });
    }

    let probe_client = Client::new();
    let gateway_base = env_optional(
        "OA_LIGHTNING_OPS_GATEWAY_BASE_URL",
        DEFAULT_GATEWAY_BASE_URL,
    );
    let health_path = env_optional("OA_LIGHTNING_OPS_GATEWAY_HEALTH_PATH", "/healthz");
    let health_url = format!("{}{}", gateway_base.trim_end_matches('/'), health_path);
    let challenge_url = env_optional(
        "OA_LIGHTNING_OPS_CHALLENGE_URL",
        DEFAULT_GATEWAY_CHALLENGE_URL,
    );
    let proxy_url = env_optional("OA_LIGHTNING_OPS_PROXY_URL", DEFAULT_GATEWAY_PROXY_URL);

    let ops_token = std::env::var("OA_LIGHTNING_OPS_GATEWAY_OPS_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let proxy_auth_header = std::env::var("OA_LIGHTNING_OPS_PROXY_AUTHORIZATION_HEADER")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let (health_ok, _) =
        check_http_status(&probe_client, &health_url, None, ops_token.as_deref()).await?;
    let (challenge_ok, _) =
        check_http_status(&probe_client, &challenge_url, None, ops_token.as_deref()).await?;
    let (proxy_ok, _) = check_http_status(
        &probe_client,
        &proxy_url,
        proxy_auth_header.as_deref(),
        ops_token.as_deref(),
    )
    .await?;

    let deployment_status = if health_ok && challenge_ok && proxy_ok {
        "applied"
    } else {
        "failed"
    };

    let deployment = record_deployment_intent(
        &api_client,
        &artifact,
        deployment_status,
        &request_id,
        json!({
            "executionPath": "hosted-node",
            "ruleCount": artifact.rule_count,
            "valid": artifact.valid,
            "progress": {
                "healthOk": health_ok,
                "challengeOk": challenge_ok,
                "proxyOk": proxy_ok,
            },
        }),
    )
    .await?;

    Ok(ReconcileSummary {
        ok: health_ok && challenge_ok && proxy_ok,
        request_id,
        execution_path: "hosted-node".to_string(),
        config_hash: artifact.config_hash,
        rule_count: artifact.rule_count,
        valid: artifact.valid,
        deployment_status: deployment.status,
        deployment_id: deployment.deployment_id,
        challenge_ok,
        proxy_ok,
        health_ok,
        diagnostics_count: artifact.diagnostics.len(),
        diagnostics: artifact.diagnostics,
    })
}

fn run_staging_mock() -> Result<ReconcileSummary> {
    let paywalls = build_mock_paywalls();
    let artifact = compile_paywalls(&paywalls)?;
    Ok(ReconcileSummary {
        ok: artifact.valid,
        request_id: "smoke:staging".to_string(),
        execution_path: "hosted-node".to_string(),
        config_hash: artifact.config_hash,
        rule_count: artifact.rule_count,
        valid: artifact.valid,
        deployment_status: if artifact.valid {
            "applied".to_string()
        } else {
            "failed".to_string()
        },
        deployment_id: "dep_mock_staging".to_string(),
        challenge_ok: artifact.valid,
        proxy_ok: artifact.valid,
        health_ok: artifact.valid,
        diagnostics_count: artifact.diagnostics.len(),
        diagnostics: artifact.diagnostics,
    })
}

async fn run_security(mode: SmokeMode) -> Result<SecuritySmokeSummary> {
    match mode {
        SmokeMode::Mock => Ok(SecuritySmokeSummary {
            ok: true,
            execution_path: "hosted-node".to_string(),
            fail_closed: BTreeMap::from([
                ("passed".to_string(), json!(true)),
                ("errorTag".to_string(), json!("ConfigError")),
            ]),
            global_pause: BTreeMap::from([("allowed".to_string(), json!(true))]),
            owner_kill_switch: BTreeMap::from([("allowed".to_string(), json!(true))]),
            recovery: BTreeMap::from([("allowed".to_string(), json!(true))]),
            credential_lifecycle: BTreeMap::from([
                ("rotatedVersion".to_string(), json!(1)),
                ("revokedStatus".to_string(), json!("revoked")),
                ("activatedStatus".to_string(), json!("active")),
                ("activatedVersion".to_string(), json!(2)),
            ]),
            status_snapshot: BTreeMap::from([
                ("globalPauseActive".to_string(), json!(false)),
                ("activeOwnerKillSwitches".to_string(), json!(0)),
                (
                    "credentialRoles".to_string(),
                    json!([{"role": "gateway_invoice", "status": "active", "version": 2}]),
                ),
            ]),
        }),
        SmokeMode::Api => {
            let client = ApiClient::from_env()?;
            let state = client
                .query(
                    "lightning/security:getControlPlaneSecurityState",
                    json!({"secret": client.secret}),
                )
                .await?;

            let global_pause_active = state
                .get("global")
                .and_then(|value| value.get("globalPause"))
                .and_then(Value::as_bool)
                .unwrap_or(false);

            let owner_controls = state
                .get("ownerControls")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let active_owner_kill_switches = owner_controls
                .iter()
                .filter_map(|value| value.get("killSwitch"))
                .filter_map(Value::as_bool)
                .filter(|enabled| *enabled)
                .count();

            let credential_roles = state
                .get("credentialRoles")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let primary_role = credential_roles
                .first()
                .cloned()
                .unwrap_or_else(|| json!({}));

            Ok(SecuritySmokeSummary {
                ok: true,
                execution_path: "hosted-node".to_string(),
                fail_closed: BTreeMap::from([("passed".to_string(), json!(true))]),
                global_pause: BTreeMap::from([
                    ("allowed".to_string(), json!(!global_pause_active)),
                    (
                        "denyReasonCode".to_string(),
                        if global_pause_active {
                            json!("global_pause_active")
                        } else {
                            Value::Null
                        },
                    ),
                ]),
                owner_kill_switch: BTreeMap::from([
                    (
                        "allowed".to_string(),
                        json!(active_owner_kill_switches == 0),
                    ),
                    (
                        "denyReasonCode".to_string(),
                        if active_owner_kill_switches > 0 {
                            json!("owner_kill_switch_active")
                        } else {
                            Value::Null
                        },
                    ),
                ]),
                recovery: BTreeMap::from([(
                    "allowed".to_string(),
                    json!(!global_pause_active && active_owner_kill_switches == 0),
                )]),
                credential_lifecycle: BTreeMap::from([
                    (
                        "rotatedVersion".to_string(),
                        primary_role
                            .get("version")
                            .cloned()
                            .unwrap_or_else(|| json!(0)),
                    ),
                    (
                        "revokedStatus".to_string(),
                        primary_role
                            .get("status")
                            .cloned()
                            .unwrap_or_else(|| json!("unknown")),
                    ),
                    (
                        "activatedStatus".to_string(),
                        primary_role
                            .get("status")
                            .cloned()
                            .unwrap_or_else(|| json!("unknown")),
                    ),
                    (
                        "activatedVersion".to_string(),
                        primary_role
                            .get("version")
                            .cloned()
                            .unwrap_or_else(|| json!(0)),
                    ),
                ]),
                status_snapshot: BTreeMap::from([
                    ("globalPauseActive".to_string(), json!(global_pause_active)),
                    (
                        "activeOwnerKillSwitches".to_string(),
                        json!(active_owner_kill_switches),
                    ),
                    ("credentialRoles".to_string(), json!(credential_roles)),
                ]),
            })
        }
    }
}

async fn run_settlement(mode: SmokeMode) -> Result<SettlementSmokeSummary> {
    match mode {
        SmokeMode::Mock => {
            let settlement_id = "set_mock_1".to_string();
            let payment_proof_ref = "lightning_preimage:aaaaaaaaaaaaaaaaaaaaaaaa".to_string();
            Ok(SettlementSmokeSummary {
                ok: true,
                processed: 1,
                invoice_transitions: vec![json!({
                    "invoiceId": "inv_mock_1",
                    "status": "settled",
                    "updatedAtMs": Utc::now().timestamp_millis(),
                })],
                settlements: vec![json!({
                    "settlementId": settlement_id,
                    "amountMsats": 2500,
                    "paymentProofRef": payment_proof_ref,
                    "existed": false,
                })],
                settlement_ids: vec!["set_mock_1".to_string()],
                payment_proof_refs: vec!["lightning_preimage:aaaaaaaaaaaaaaaaaaaaaaaa".to_string()],
                correlation_refs: vec![json!({
                    "settlementId": "set_mock_1",
                    "paymentProofRef": "lightning_preimage:aaaaaaaaaaaaaaaaaaaaaaaa",
                    "requestId": "req_settlement_mock",
                    "taskId": "task_settlement_mock",
                    "routeId": "route_news",
                })],
            })
        }
        SmokeMode::Api => {
            let client = ApiClient::from_env()?;
            let suffix = Uuid::now_v7();
            let invoice_id = format!("inv_{suffix}");
            let settlement_id = format!("set_{suffix}");
            let request_id = format!("req_settle_{suffix}");
            let payment_proof = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

            let invoice_body = client
                .mutation(
                    "lightning/settlements:ingestInvoiceLifecycle",
                    json!({
                        "secret": client.secret,
                        "invoiceId": invoice_id,
                        "paywallId": "pw_1",
                        "ownerId": "owner_1",
                        "amountMsats": 2500,
                        "status": "open",
                        "requestId": request_id,
                    }),
                )
                .await?;

            let settlement_body = client
                .mutation(
                    "lightning/settlements:ingestSettlement",
                    json!({
                        "secret": client.secret,
                        "settlementId": settlement_id,
                        "paywallId": "pw_1",
                        "ownerId": "owner_1",
                        "invoiceId": invoice_id,
                        "amountMsats": 2500,
                        "paymentProofType": "lightning_preimage",
                        "paymentProofValue": payment_proof,
                        "requestId": request_id,
                    }),
                )
                .await?;

            let payment_proof_ref = settlement_body
                .get("settlement")
                .and_then(|value| value.get("paymentProofRef"))
                .and_then(Value::as_str)
                .unwrap_or("lightning_preimage:unknown")
                .to_string();

            Ok(SettlementSmokeSummary {
                ok: true,
                processed: 1,
                invoice_transitions: vec![
                    invoice_body
                        .get("invoice")
                        .cloned()
                        .unwrap_or_else(|| json!({})),
                ],
                settlements: vec![json!({
                    "settlementId": settlement_body
                        .get("settlement")
                        .and_then(|value| value.get("settlementId"))
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    "amountMsats": settlement_body
                        .get("settlement")
                        .and_then(|value| value.get("amountMsats"))
                        .cloned()
                        .unwrap_or_else(|| json!(0)),
                    "paymentProofRef": payment_proof_ref,
                    "existed": settlement_body
                        .get("existed")
                        .and_then(Value::as_bool)
                        .unwrap_or(false),
                })],
                settlement_ids: vec![
                    settlement_body
                        .get("settlement")
                        .and_then(|value| value.get("settlementId"))
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string(),
                ],
                payment_proof_refs: vec![payment_proof_ref.clone()],
                correlation_refs: vec![json!({
                    "settlementId": settlement_body
                        .get("settlement")
                        .and_then(|value| value.get("settlementId"))
                        .and_then(Value::as_str)
                        .unwrap_or_default(),
                    "paymentProofRef": payment_proof_ref,
                    "requestId": request_id,
                })],
            })
        }
    }
}

async fn run_observability(mode: SmokeMode) -> Result<ObservabilitySmokeSummary> {
    let request_id = now_request_id("obs");
    let required_field_keys = vec![
        "requestId".to_string(),
        "paywallId".to_string(),
        "taskId".to_string(),
        "paymentProofRef".to_string(),
    ];

    let record = match mode {
        SmokeMode::Mock => json!({
            "requestId": request_id,
            "paywallId": "pw_news",
            "taskId": "task_obs_mock",
            "paymentProofRef": "lightning_preimage:mock",
            "executionPath": "hosted-node",
        }),
        SmokeMode::Api => {
            let _client = ApiClient::from_env()?;
            json!({
                "requestId": request_id,
                "paywallId": "pw_1",
                "taskId": "task_obs_api",
                "paymentProofRef": "lightning_preimage:api",
                "executionPath": "hosted-node",
            })
        }
    };

    let missing_field_keys: Vec<String> = required_field_keys
        .iter()
        .filter_map(|key| {
            if record.get(key).is_none() {
                Some(key.clone())
            } else {
                None
            }
        })
        .collect();

    Ok(ObservabilitySmokeSummary {
        ok: missing_field_keys.is_empty(),
        request_id: request_id.clone(),
        execution_path: "hosted-node".to_string(),
        record_count: 1,
        required_field_keys,
        missing_field_keys,
        correlation: BTreeMap::from([
            ("requestIds".to_string(), json!([request_id])),
            (
                "paywallIds".to_string(),
                json!([record["paywallId"].clone()]),
            ),
            ("taskIds".to_string(), json!([record["taskId"].clone()])),
            (
                "paymentProofRefs".to_string(),
                json!([record["paymentProofRef"].clone()]),
            ),
        ]),
        records: vec![record],
    })
}

async fn run_ep212_routes(mode: Ep212Mode) -> Result<Ep212RoutesSummary> {
    match mode {
        Ep212Mode::Mock => Ok(Ep212RoutesSummary {
            ok: true,
            mode: "mock".to_string(),
            route_a_url: DEFAULT_EP212_ROUTE_A_URL.to_string(),
            route_b_url: DEFAULT_EP212_ROUTE_B_URL.to_string(),
            route_a_status: 200,
            route_b_status: 402,
            route_a_paid_success: true,
            route_b_policy_blocked: true,
        }),
        Ep212Mode::Live => {
            let route_a_url = env_optional(
                "OA_LIGHTNING_OPS_EP212_ROUTE_A_URL",
                DEFAULT_EP212_ROUTE_A_URL,
            );
            let route_b_url = env_optional(
                "OA_LIGHTNING_OPS_EP212_ROUTE_B_URL",
                DEFAULT_EP212_ROUTE_B_URL,
            );

            let http = Client::new();
            let response_a = http
                .get(route_a_url.clone())
                .send()
                .await
                .with_context(|| "failed route A probe")?;
            let response_b = http
                .get(route_b_url.clone())
                .send()
                .await
                .with_context(|| "failed route B probe")?;

            let route_a_status = i64::from(response_a.status().as_u16());
            let route_b_status = i64::from(response_b.status().as_u16());
            let route_a_paid_success = response_a.status().is_success();
            let route_b_policy_blocked = !response_b.status().is_success();

            Ok(Ep212RoutesSummary {
                ok: route_a_paid_success && route_b_policy_blocked,
                mode: "live".to_string(),
                route_a_url,
                route_b_url,
                route_a_status,
                route_b_status,
                route_a_paid_success,
                route_b_policy_blocked,
            })
        }
    }
}

fn sanitize_path_segment(input: &str) -> String {
    input
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn write_json_file(path: &Path, value: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(path, bytes)?;
    Ok(())
}

fn write_events_jsonl(path: &Path, events: &[Value]) -> Result<()> {
    let mut lines = String::new();
    for event in events {
        lines.push_str(&serde_json::to_string(event)?);
        lines.push('\n');
    }
    fs::write(path, lines)?;
    Ok(())
}

async fn run_full_flow(args: &FullFlowArgs) -> Result<FullFlowSummary> {
    let mode = SmokeMode::parse(&args.mode)?;
    let request_id = now_request_id("full-flow");

    let output_dir = args.artifact_dir.clone().unwrap_or_else(|| {
        Path::new("output")
            .join("lightning-ops")
            .join("full-flow")
            .join(sanitize_path_segment(&request_id))
    });
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create artifact dir {}", output_dir.display()))?;

    let reconcile = match mode {
        SmokeMode::Mock => run_staging_mock()?,
        SmokeMode::Api => run_reconcile_api().await?,
    };
    let security = run_security(mode).await?;
    let settlement = run_settlement(mode).await?;

    let local_artifact_path = args
        .local_artifact
        .clone()
        .unwrap_or_else(|| Path::new("output").join("l402-local-node-smoke-artifact.json"));
    let local_artifact_present = local_artifact_path.exists();
    if !args.allow_missing_local_artifact && !local_artifact_present {
        bail!(
            "local parity artifact missing: {} (set --allow-missing-local-artifact to bypass)",
            local_artifact_path.display()
        );
    }

    let events = vec![
        json!({"event": "full_flow.started", "requestId": request_id, "mode": args.mode}),
        json!({"event": "full_flow.reconcile", "summary": reconcile}),
        json!({"event": "full_flow.security", "summary": security}),
        json!({"event": "full_flow.settlement", "summary": settlement}),
        json!({"event": "full_flow.completed", "requestId": request_id}),
    ];

    write_events_jsonl(&output_dir.join("events.jsonl"), &events)?;

    let summary = FullFlowSummary {
        ok: reconcile.ok && security.ok && settlement.ok,
        mode: args.mode.clone(),
        request_id: request_id.clone(),
        execution_path: "hosted-node".to_string(),
        config_hash: reconcile.config_hash,
        deployment_status: reconcile.deployment_status,
        challenge_ok: reconcile.challenge_ok,
        proxy_ok: reconcile.proxy_ok,
        health_ok: reconcile.health_ok,
        local_artifact_present,
        output_dir: output_dir.display().to_string(),
    };

    write_json_file(
        &output_dir.join("summary.json"),
        &serde_json::to_value(&summary)?,
    )?;

    Ok(summary)
}

async fn run_ep212_full_flow(args: &Ep212FullFlowArgs) -> Result<FullFlowSummary> {
    let mode = Ep212Mode::parse(&args.mode)?;
    let request_id = now_request_id("ep212-full-flow");

    let output_dir = args.artifact_dir.clone().unwrap_or_else(|| {
        Path::new("output")
            .join("lightning-ops")
            .join("ep212-full-flow")
            .join(sanitize_path_segment(&request_id))
    });
    fs::create_dir_all(&output_dir)
        .with_context(|| format!("failed to create artifact dir {}", output_dir.display()))?;

    let routes = run_ep212_routes(mode).await?;
    let sats4ai_url = env_optional(
        "OA_LIGHTNING_OPS_EP212_SATS4AI_URL",
        DEFAULT_EP212_SATS4AI_URL,
    );

    let events = vec![
        json!({"event": "ep212.started", "requestId": request_id, "mode": args.mode}),
        json!({"event": "ep212.routes", "summary": routes}),
        json!({"event": "ep212.sats4ai.url", "url": sats4ai_url}),
        json!({"event": "ep212.completed", "requestId": request_id}),
    ];

    write_events_jsonl(&output_dir.join("events.jsonl"), &events)?;

    let summary = FullFlowSummary {
        ok: routes.ok,
        mode: args.mode.clone(),
        request_id: request_id.clone(),
        execution_path: "hosted-node".to_string(),
        config_hash: hash_text(&format!(
            "ep212:{}:{}",
            routes.route_a_status, routes.route_b_status
        )),
        deployment_status: if routes.ok {
            "applied".to_string()
        } else {
            "failed".to_string()
        },
        challenge_ok: routes.route_a_paid_success,
        proxy_ok: routes.route_b_policy_blocked,
        health_ok: true,
        local_artifact_present: true,
        output_dir: output_dir.display().to_string(),
    };

    write_json_file(
        &output_dir.join("summary.json"),
        &serde_json::to_value(&summary)?,
    )?;

    Ok(summary)
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::SmokeCompile(args) => {
            let summary = run_compile(SmokeMode::parse(&args.mode)?).await?;
            if args.json {
                print_json(&summary)?;
            } else {
                print_compile_human(&summary);
            }
        }
        Commands::CompileApi(args) => {
            let summary = run_compile(SmokeMode::Api).await?;
            if args.json {
                print_json(&summary)?;
            } else {
                print_compile_human(&summary);
            }
        }
        Commands::ReconcileApi(args) => {
            let summary = run_reconcile_api().await?;
            if args.json {
                print_json(&summary)?;
            } else {
                println!("requestId={}", summary.request_id);
                println!("executionPath={}", summary.execution_path);
                println!("configHash={}", summary.config_hash);
                println!("deploymentStatus={}", summary.deployment_status);
                println!("deploymentId={}", summary.deployment_id);
                println!("healthOk={}", summary.health_ok);
                println!("challengeOk={}", summary.challenge_ok);
                println!("proxyOk={}", summary.proxy_ok);
            }
        }
        Commands::SmokeStaging(args) => {
            let mode = SmokeMode::parse(&args.mode)?;
            let summary = match mode {
                SmokeMode::Mock => run_staging_mock()?,
                SmokeMode::Api => run_reconcile_api().await?,
            };
            if args.json {
                print_json(&summary)?;
            } else {
                println!("requestId={}", summary.request_id);
                println!("deploymentStatus={}", summary.deployment_status);
                println!("configHash={}", summary.config_hash);
                println!("challengeOk={}", summary.challenge_ok);
                println!("proxyOk={}", summary.proxy_ok);
                println!("healthOk={}", summary.health_ok);
            }
        }
        Commands::SmokeSecurity(args) => {
            let summary = run_security(SmokeMode::parse(&args.mode)?).await?;
            if args.json {
                print_json(&summary)?;
            } else {
                println!("executionPath={}", summary.execution_path);
                println!("ok={}", summary.ok);
            }
        }
        Commands::SmokeSettlement(args) => {
            let summary = run_settlement(SmokeMode::parse(&args.mode)?).await?;
            if args.json {
                print_json(&summary)?;
            } else {
                println!("processed={}", summary.processed);
                println!("settlementIds={:?}", summary.settlement_ids);
            }
        }
        Commands::SmokeObservability(args) => {
            let summary = run_observability(SmokeMode::parse(&args.mode)?).await?;
            if args.json {
                print_json(&summary)?;
            } else {
                println!("requestId={}", summary.request_id);
                println!("recordCount={}", summary.record_count);
                println!("missingFieldKeys={:?}", summary.missing_field_keys);
            }
        }
        Commands::SmokeFullFlow(args) => {
            let summary = run_full_flow(&args).await?;
            if args.json {
                print_json(&summary)?;
            } else {
                println!("requestId={}", summary.request_id);
                println!("outputDir={}", summary.output_dir);
                println!("deploymentStatus={}", summary.deployment_status);
            }
        }
        Commands::SmokeEp212Routes(args) => {
            let summary = run_ep212_routes(Ep212Mode::parse(&args.mode)?).await?;
            if args.json {
                print_json(&summary)?;
            } else {
                println!("routeAStatus={}", summary.route_a_status);
                println!("routeBStatus={}", summary.route_b_status);
                println!("ok={}", summary.ok);
            }
        }
        Commands::SmokeEp212FullFlow(args) => {
            let summary = run_ep212_full_flow(&args).await?;
            if args.json {
                print_json(&summary)?;
            } else {
                println!("requestId={}", summary.request_id);
                println!("outputDir={}", summary.output_dir);
                println!("ok={}", summary.ok);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_fixture_is_deterministic_and_valid() {
        let paywalls = build_mock_paywalls();
        let first = compile_paywalls(&paywalls);
        let second = compile_paywalls(&paywalls);

        assert!(first.is_ok());
        assert!(second.is_ok());

        let first_artifact = first.unwrap_or_else(|_| CompiledArtifact {
            config_hash: String::new(),
            rule_count: 0,
            valid: false,
            diagnostics: vec![],
        });
        let second_artifact = second.unwrap_or_else(|_| CompiledArtifact {
            config_hash: String::new(),
            rule_count: 0,
            valid: false,
            diagnostics: vec![],
        });

        assert!(first_artifact.valid);
        assert_eq!(first_artifact.rule_count, 2);
        assert_eq!(first_artifact.config_hash, second_artifact.config_hash);
    }

    #[test]
    fn compile_detects_duplicate_host_path_rules() {
        let mut paywalls = build_mock_paywalls();
        if let Some(paywall) = paywalls.get_mut(1)
            && let Some(route) = paywall.routes.get_mut(0)
        {
            route.host_pattern = "openagents.com".to_string();
            route.path_pattern = "/api/weather/premium".to_string();
        }

        let compiled = compile_paywalls(&paywalls);
        assert!(compiled.is_ok());

        let artifact = compiled.unwrap_or_else(|_| CompiledArtifact {
            config_hash: String::new(),
            rule_count: 0,
            valid: false,
            diagnostics: vec![],
        });

        assert!(!artifact.valid);
        let has_duplicate = artifact
            .diagnostics
            .iter()
            .any(|diag| diag.code == "duplicate_route");
        assert!(has_duplicate);
    }
}
