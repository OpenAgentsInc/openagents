#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::unwrap_used
)]
#![allow(
    clippy::print_stdout,
    reason = "This binary is a CLI diagnostic harness with intentional console output."
)]
#![allow(
    clippy::print_stderr,
    reason = "This binary is a CLI diagnostic harness and may emit explicit error lines."
)]

#[path = "../openagents_dynamic_tools.rs"]
mod openagents_dynamic_tools;

use std::future::Future;
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use codex_client::{
    AppServerChannels, AppServerClient, AppServerConfig, AppServerNotification, AppServerRequest,
    AppServerRequestId, AppsListParams, AskForApproval, ClientInfo, CollaborationModeListParams,
    CommandExecParams, ConfigReadParams, ExperimentalFeatureListParams,
    ExternalAgentConfigDetectParams, ExternalAgentConfigImportParams,
    FuzzyFileSearchSessionStartParams, FuzzyFileSearchSessionStopParams,
    FuzzyFileSearchSessionUpdateParams, GetAccountParams, HazelnutScope, InitializeCapabilities,
    InitializeParams, ListMcpServerStatusParams, ModelListParams, ProductSurface,
    RemoteSkillSummary, ReviewDelivery, ReviewStartParams, ReviewTarget, SandboxMode,
    SandboxPolicy, SkillMetadata, SkillsConfigWriteParams, SkillsListExtraRootsForCwd,
    SkillsListParams, SkillsListResponse, SkillsRemoteReadParams, SkillsRemoteReadResponse,
    SkillsRemoteWriteParams, ThreadArchiveParams, ThreadBackgroundTerminalsCleanParams,
    ThreadCompactStartParams, ThreadForkParams, ThreadListParams, ThreadLoadedListParams,
    ThreadReadParams, ThreadRealtimeAppendTextParams, ThreadRealtimeStartParams,
    ThreadRealtimeStopParams, ThreadRollbackParams, ThreadSetNameParams, ThreadSnapshot,
    ThreadSortKey, ThreadStartParams, ThreadUnarchiveParams, TurnStartParams, UserInput,
    WindowsSandboxSetupStartParams,
};
use serde_json::Value;

const LEGACY_NOTIFICATION_OPT_OUT_METHODS: &[&str] = &[
    "codex/event/agent_message_content_delta",
    "codex/event/agent_message_delta",
    "codex/event/agent_message",
    "codex/event/agent_reasoning_delta",
    "codex/event/agent_reasoning_content_delta",
    "codex/event/agent_reasoning_raw_content_delta",
    "codex/event/agent_reasoning_section_break",
    "codex/event/agent_reasoning",
    "codex/event/reasoning_content_delta",
    "codex/event/reasoning_raw_content_delta",
    "codex/event/item_started",
    "codex/event/item_completed",
    "codex/event/task_started",
    "codex/event/task_complete",
    "codex/event/task_failed",
    "codex/event/task_error",
    "codex/event/thread_status",
    "codex/event/thread_name_changed",
    "codex/event/turn_diff",
    "codex/event/turn_plan",
    "codex/event/token_count",
    "codex/event/user_message",
];
const LEGACY_OPENAGENTS_TOOL_CAD_INTENT: &str = "openagents.cad.intent";
const LEGACY_OPENAGENTS_TOOL_CAD_ACTION: &str = "openagents.cad.action";
const BLINK_KEYCHAIN_SERVICE: &str = "com.openagents.autopilot.credentials";
const BLINK_KEYCHAIN_ACCOUNT_API_KEY: &str = "BLINK_API_KEY";
const BLINK_KEYCHAIN_ACCOUNT_API_URL: &str = "BLINK_API_URL";

#[derive(Clone, Debug)]
struct BlinkSwapProbeArgs {
    direction: String,
    amount: u64,
    unit: Option<String>,
    execute_live: bool,
    require_execute_success: bool,
    memo: Option<String>,
}

impl Default for BlinkSwapProbeArgs {
    fn default() -> Self {
        Self {
            direction: "btc-to-usd".to_string(),
            amount: 1,
            unit: None,
            execute_live: false,
            require_execute_success: false,
            memo: Some("openagents-codex-live-harness".to_string()),
        }
    }
}

impl BlinkSwapProbeArgs {
    fn normalized_direction(&self) -> Result<&'static str> {
        normalize_blink_direction(self.direction.as_str()).ok_or_else(|| {
            anyhow!(
                "invalid --blink-swap-direction '{}'; expected btc-to-usd or usd-to-btc",
                self.direction
            )
        })
    }

    fn resolved_unit(&self) -> Result<&'static str> {
        match self.unit.as_deref() {
            Some(raw) => normalize_blink_unit(raw).ok_or_else(|| {
                anyhow!(
                    "invalid --blink-swap-unit '{}'; expected sats or cents",
                    raw
                )
            }),
            None => match self.normalized_direction()? {
                "btc-to-usd" => Ok("sats"),
                "usd-to-btc" => Ok("cents"),
                _ => unreachable!(),
            },
        }
    }
}

#[derive(Clone, Debug)]
struct BlinkStableSatsSaProbeArgs {
    rounds: u32,
    convert_btc_sats: u64,
    convert_usd_cents: u64,
    require_success: bool,
    memo_prefix: Option<String>,
}

impl Default for BlinkStableSatsSaProbeArgs {
    fn default() -> Self {
        Self {
            rounds: 1,
            convert_btc_sats: 6_000,
            convert_usd_cents: 50,
            require_success: true,
            memo_prefix: Some("openagents-stablesats-sa-live-harness".to_string()),
        }
    }
}

#[derive(Debug)]
struct HarnessArgs {
    cwd: PathBuf,
    model_override: Option<String>,
    prompt: Option<String>,
    skill_name: Option<String>,
    list_limit: u32,
    drain_ms: u64,
    timeout_ms: u64,
    max_events: usize,
    include_writes: bool,
    include_experimental: bool,
    include_thread_mutations: bool,
    include_openagents_dynamic_tools: bool,
    require_cad_tool_call: bool,
    fail_on_echo: bool,
    blink_swap_probe: Option<BlinkSwapProbeArgs>,
    blink_stablesats_sa_probe: Option<BlinkStableSatsSaProbeArgs>,
}

impl Default for HarnessArgs {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            model_override: None,
            prompt: None,
            skill_name: None,
            list_limit: 20,
            drain_ms: 700,
            timeout_ms: 4_000,
            max_events: 24,
            include_writes: false,
            include_experimental: true,
            include_thread_mutations: true,
            include_openagents_dynamic_tools: true,
            require_cad_tool_call: false,
            fail_on_echo: true,
            blink_swap_probe: None,
            blink_stablesats_sa_probe: None,
        }
    }
}

impl HarnessArgs {
    fn from_env() -> Result<Self> {
        let mut args = Self::default();
        let mut input = std::env::args().skip(1);

        while let Some(flag) = input.next() {
            match flag.as_str() {
                "--cwd" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --cwd"))?;
                    args.cwd = PathBuf::from(value);
                }
                "--model" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --model"))?;
                    if value.trim().is_empty() {
                        bail!("--model cannot be empty");
                    }
                    args.model_override = Some(value);
                }
                "--prompt" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --prompt"))?;
                    if value.trim().is_empty() {
                        args.prompt = None;
                    } else {
                        args.prompt = Some(value);
                    }
                }
                "--skill" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --skill"))?;
                    if value.trim().is_empty() {
                        args.skill_name = None;
                    } else {
                        args.skill_name = Some(value);
                    }
                }
                "--list-limit" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --list-limit"))?;
                    args.list_limit = value
                        .parse::<u32>()
                        .map_err(|error| anyhow!("invalid --list-limit '{}': {error}", value))?;
                    if args.list_limit == 0 {
                        bail!("--list-limit must be greater than 0");
                    }
                }
                "--drain-ms" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --drain-ms"))?;
                    args.drain_ms = value
                        .parse::<u64>()
                        .map_err(|error| anyhow!("invalid --drain-ms '{}': {error}", value))?;
                }
                "--timeout-ms" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --timeout-ms"))?;
                    args.timeout_ms = value
                        .parse::<u64>()
                        .map_err(|error| anyhow!("invalid --timeout-ms '{}': {error}", value))?;
                    if args.timeout_ms == 0 {
                        bail!("--timeout-ms must be greater than 0");
                    }
                }
                "--max-events" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --max-events"))?;
                    args.max_events = value
                        .parse::<usize>()
                        .map_err(|error| anyhow!("invalid --max-events '{}': {error}", value))?;
                }
                "--include-writes" => args.include_writes = true,
                "--skip-experimental" => args.include_experimental = false,
                "--skip-thread-mutations" => args.include_thread_mutations = false,
                "--disable-openagents-dynamic-tools" => {
                    args.include_openagents_dynamic_tools = false
                }
                "--require-cad-tool-call" => args.require_cad_tool_call = true,
                "--allow-echo-replies" => args.fail_on_echo = false,
                "--blink-swap-live" => {
                    let _ = args
                        .blink_swap_probe
                        .get_or_insert_with(BlinkSwapProbeArgs::default);
                }
                "--blink-swap-direction" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --blink-swap-direction"))?;
                    let probe = args
                        .blink_swap_probe
                        .get_or_insert_with(BlinkSwapProbeArgs::default);
                    probe.direction = value;
                }
                "--blink-swap-amount" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --blink-swap-amount"))?;
                    let parsed = value.parse::<u64>().map_err(|error| {
                        anyhow!("invalid --blink-swap-amount '{}': {error}", value)
                    })?;
                    if parsed == 0 {
                        bail!("--blink-swap-amount must be greater than 0");
                    }
                    let probe = args
                        .blink_swap_probe
                        .get_or_insert_with(BlinkSwapProbeArgs::default);
                    probe.amount = parsed;
                }
                "--blink-swap-unit" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --blink-swap-unit"))?;
                    let probe = args
                        .blink_swap_probe
                        .get_or_insert_with(BlinkSwapProbeArgs::default);
                    probe.unit = Some(value);
                }
                "--blink-swap-execute-live" => {
                    let probe = args
                        .blink_swap_probe
                        .get_or_insert_with(BlinkSwapProbeArgs::default);
                    probe.execute_live = true;
                }
                "--blink-swap-require-success" => {
                    let probe = args
                        .blink_swap_probe
                        .get_or_insert_with(BlinkSwapProbeArgs::default);
                    probe.require_execute_success = true;
                }
                "--blink-swap-memo" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --blink-swap-memo"))?;
                    let probe = args
                        .blink_swap_probe
                        .get_or_insert_with(BlinkSwapProbeArgs::default);
                    probe.memo = Some(value);
                }
                "--blink-stablesats-sa-live" => {
                    let _ = args
                        .blink_stablesats_sa_probe
                        .get_or_insert_with(BlinkStableSatsSaProbeArgs::default);
                }
                "--blink-stablesats-sa-rounds" => {
                    let value = input
                        .next()
                        .ok_or_else(|| anyhow!("missing value for --blink-stablesats-sa-rounds"))?;
                    let parsed = value.parse::<u32>().map_err(|error| {
                        anyhow!("invalid --blink-stablesats-sa-rounds '{}': {error}", value)
                    })?;
                    let probe = args
                        .blink_stablesats_sa_probe
                        .get_or_insert_with(BlinkStableSatsSaProbeArgs::default);
                    probe.rounds = parsed;
                }
                "--blink-stablesats-sa-convert-btc-sats" => {
                    let value = input.next().ok_or_else(|| {
                        anyhow!("missing value for --blink-stablesats-sa-convert-btc-sats")
                    })?;
                    let parsed = value.parse::<u64>().map_err(|error| {
                        anyhow!(
                            "invalid --blink-stablesats-sa-convert-btc-sats '{}': {error}",
                            value
                        )
                    })?;
                    let probe = args
                        .blink_stablesats_sa_probe
                        .get_or_insert_with(BlinkStableSatsSaProbeArgs::default);
                    probe.convert_btc_sats = parsed;
                }
                "--blink-stablesats-sa-convert-usd-cents" => {
                    let value = input.next().ok_or_else(|| {
                        anyhow!("missing value for --blink-stablesats-sa-convert-usd-cents")
                    })?;
                    let parsed = value.parse::<u64>().map_err(|error| {
                        anyhow!(
                            "invalid --blink-stablesats-sa-convert-usd-cents '{}': {error}",
                            value
                        )
                    })?;
                    let probe = args
                        .blink_stablesats_sa_probe
                        .get_or_insert_with(BlinkStableSatsSaProbeArgs::default);
                    probe.convert_usd_cents = parsed;
                }
                "--blink-stablesats-sa-require-success" => {
                    let probe = args
                        .blink_stablesats_sa_probe
                        .get_or_insert_with(BlinkStableSatsSaProbeArgs::default);
                    probe.require_success = true;
                }
                "--blink-stablesats-sa-memo-prefix" => {
                    let value = input.next().ok_or_else(|| {
                        anyhow!("missing value for --blink-stablesats-sa-memo-prefix")
                    })?;
                    let probe = args
                        .blink_stablesats_sa_probe
                        .get_or_insert_with(BlinkStableSatsSaProbeArgs::default);
                    probe.memo_prefix = Some(value);
                }
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ => {
                    bail!("unknown argument '{}'\n\n{}", flag, usage_text());
                }
            }
        }

        if args.require_cad_tool_call && !args.include_openagents_dynamic_tools {
            bail!(
                "--require-cad-tool-call requires OpenAgents dynamic tools (omit --disable-openagents-dynamic-tools)"
            );
        }
        if let Some(probe) = args.blink_swap_probe.as_ref() {
            let _ = probe.normalized_direction()?;
            let _ = probe.resolved_unit()?;
            if probe.require_execute_success && !probe.execute_live {
                bail!("--blink-swap-require-success requires --blink-swap-execute-live");
            }
        }
        if let Some(probe) = args.blink_stablesats_sa_probe.as_ref() {
            if probe.rounds == 0 {
                bail!("--blink-stablesats-sa-rounds must be greater than 0");
            }
            if probe.convert_btc_sats == 0 {
                bail!("--blink-stablesats-sa-convert-btc-sats must be greater than 0");
            }
            if probe.convert_usd_cents == 0 {
                bail!("--blink-stablesats-sa-convert-usd-cents must be greater than 0");
            }
        }

        Ok(args)
    }
}

#[derive(Default)]
struct ChannelEventBatch {
    notifications: Vec<String>,
    notification_methods: Vec<String>,
    notification_params: Vec<Value>,
    requests: Vec<String>,
    request_methods: Vec<String>,
    request_params: Vec<Value>,
}

fn main() -> Result<()> {
    let args = HarnessArgs::from_env()?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to create tokio runtime")?;
    runtime.block_on(run(args))
}

async fn run(args: HarnessArgs) -> Result<()> {
    println!("codex live harness");
    println!("cwd={}", args.cwd.display());
    println!(
        "model_override={}",
        args.model_override.as_deref().unwrap_or("<none>")
    );
    println!("list_limit={}", args.list_limit);
    println!(
        "prompt={}",
        args.prompt
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("<none>")
    );
    println!(
        "skill={}",
        args.skill_name
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("<none>")
    );
    println!(
        "include_writes={} include_experimental={} include_thread_mutations={} include_openagents_dynamic_tools={} require_cad_tool_call={} fail_on_echo={}",
        args.include_writes,
        args.include_experimental,
        args.include_thread_mutations,
        args.include_openagents_dynamic_tools,
        args.require_cad_tool_call,
        args.fail_on_echo
    );
    if let Some(probe) = args.blink_swap_probe.as_ref() {
        println!(
            "blink_swap_probe=true direction={} amount={} unit={} execute_live={} require_execute_success={} memo={}",
            probe.normalized_direction()?,
            probe.amount,
            probe.resolved_unit()?,
            probe.execute_live,
            probe.require_execute_success,
            probe.memo.as_deref().unwrap_or("<none>")
        );
    } else {
        println!("blink_swap_probe=false");
    }
    if let Some(probe) = args.blink_stablesats_sa_probe.as_ref() {
        println!(
            "blink_stablesats_sa_probe=true rounds={} convert_btc_sats={} convert_usd_cents={} require_success={} memo_prefix={}",
            probe.rounds,
            probe.convert_btc_sats,
            probe.convert_usd_cents,
            probe.require_success,
            probe.memo_prefix.as_deref().unwrap_or("<none>")
        );
    } else {
        println!("blink_stablesats_sa_probe=false");
    }
    if args.include_openagents_dynamic_tools {
        println!(
            "openagents_dynamic_tools_count={}",
            openagents_dynamic_tools::OPENAGENTS_DYNAMIC_TOOL_NAMES.len()
        );
    }
    println!();

    if let Some(probe) = args.blink_swap_probe.as_ref() {
        run_blink_swap_live_probe(args.cwd.as_path(), probe)?;
        println!();
    }
    if let Some(probe) = args.blink_stablesats_sa_probe.as_ref() {
        run_blink_stablesats_sa_live_probe(args.cwd.as_path(), probe)?;
        println!();
    }

    let (client, mut channels) = AppServerClient::spawn(AppServerConfig {
        cwd: Some(args.cwd.clone()),
        ..Default::default()
    })
    .await
    .context("failed to spawn codex app-server client")?;

    client
        .initialize(InitializeParams {
            client_info: ClientInfo {
                name: "openagents-codex-live-harness".to_string(),
                title: Some("OpenAgents Codex Live Harness".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: true,
                opt_out_notification_methods: Some(
                    LEGACY_NOTIFICATION_OPT_OUT_METHODS
                        .iter()
                        .map(|method| method.to_string())
                        .collect(),
                ),
            }),
        })
        .await
        .context("codex initialize failed")?;

    drain_and_print("post-initialize", &mut channels, &args).await?;

    let _account = run_probe(
        "account/read",
        &mut channels,
        &args,
        client.account_read(GetAccountParams {
            refresh_token: false,
        }),
        |response| {
            format!(
                "requires_openai_auth={} account={}",
                response.requires_openai_auth,
                response
                    .account
                    .as_ref()
                    .map(|value| format!("{value:?}"))
                    .unwrap_or_else(|| "none".to_string())
            )
        },
    )
    .await;

    let _rate_limits = run_probe(
        "account/rateLimits/read",
        &mut channels,
        &args,
        client.account_rate_limits_read(),
        |response| {
            let plan_type = response
                .rate_limits
                .plan_type
                .as_ref()
                .map(|value| format!("{value:?}"))
                .unwrap_or_else(|| "none".to_string());
            format!("plan_type={plan_type}")
        },
    )
    .await;

    let model_list = run_probe(
        "model/list",
        &mut channels,
        &args,
        client.model_list(ModelListParams {
            cursor: None,
            limit: Some(100),
            include_hidden: Some(false),
        }),
        |response| {
            let default = response
                .data
                .iter()
                .find(|entry| entry.is_default)
                .map(|entry| entry.model.clone())
                .unwrap_or_else(|| "none".to_string());
            let sample = response
                .data
                .iter()
                .take(5)
                .map(|entry| entry.model.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "models={} default={} sample=[{}]",
                response.data.len(),
                default,
                sample
            )
        },
    )
    .await;

    let resolved_model = resolve_model(&args, model_list.as_ref());
    println!(
        "resolved_model={}",
        resolved_model.as_deref().unwrap_or("server-default")
    );

    let _collab = run_probe(
        "collaborationMode/list",
        &mut channels,
        &args,
        client.collaboration_mode_list(CollaborationModeListParams::default()),
        |response| format!("count={}", response.data.len()),
    )
    .await;

    let _experimental_features = run_probe(
        "experimentalFeature/list",
        &mut channels,
        &args,
        client.experimental_feature_list(ExperimentalFeatureListParams {
            cursor: None,
            limit: Some(50),
        }),
        |response| {
            format!(
                "count={} next_cursor={}",
                response.data.len(),
                response
                    .next_cursor
                    .clone()
                    .unwrap_or_else(|| "none".to_string())
            )
        },
    )
    .await;

    let _config = run_probe(
        "config/read",
        &mut channels,
        &args,
        client.config_read(ConfigReadParams {
            include_layers: true,
            cwd: Some(args.cwd.display().to_string()),
        }),
        |response| {
            let layer_count = response.layers.as_ref().map_or(0, std::vec::Vec::len);
            format!(
                "keys={} layers={}",
                response.config.as_object().map_or(0, serde_json::Map::len),
                layer_count
            )
        },
    )
    .await;

    let _config_requirements = run_probe(
        "configRequirements/read",
        &mut channels,
        &args,
        client.config_requirements_read(),
        |response| format!("requirements_present={}", response.requirements.is_some()),
    )
    .await;

    let external_detect = run_probe(
        "externalAgentConfig/detect",
        &mut channels,
        &args,
        client.external_agent_config_detect(ExternalAgentConfigDetectParams {
            include_home: true,
            cwds: Some(vec![args.cwd.clone()]),
        }),
        |response| format!("items={}", response.items.len()),
    )
    .await;

    if args.include_writes {
        if let Some(response) = external_detect.as_ref() {
            if !response.items.is_empty() {
                let _ = run_probe(
                    "externalAgentConfig/import",
                    &mut channels,
                    &args,
                    client.external_agent_config_import(ExternalAgentConfigImportParams {
                        migration_items: response.items.clone(),
                    }),
                    |_| "imported migration_items".to_string(),
                )
                .await;
            } else {
                println!("probe externalAgentConfig/import: skipped (no migration items)");
            }
        } else {
            println!("probe externalAgentConfig/import: skipped (detect failed)");
        }
    } else {
        println!("probe externalAgentConfig/import: skipped (--include-writes not set)");
    }

    let _mcp_list = run_probe(
        "mcpServerStatus/list",
        &mut channels,
        &args,
        client.mcp_server_status_list(ListMcpServerStatusParams {
            cursor: None,
            limit: Some(100),
        }),
        |response| {
            let sample = response
                .data
                .iter()
                .take(3)
                .map(|entry| entry.name.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!("servers={} sample=[{}]", response.data.len(), sample)
        },
    )
    .await;

    if args.include_writes {
        let _ = run_probe(
            "mcpServer/reload",
            &mut channels,
            &args,
            client.mcp_server_reload(),
            |_| "reloaded".to_string(),
        )
        .await;
    } else {
        println!("probe mcpServer/reload: skipped (--include-writes not set)");
    }

    let _apps = run_probe(
        "app/list",
        &mut channels,
        &args,
        client.app_list(AppsListParams {
            cursor: None,
            limit: Some(100),
            thread_id: None,
            force_refetch: false,
        }),
        |response| {
            let sample = response
                .data
                .iter()
                .take(3)
                .map(|entry| entry.id.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!("apps={} sample=[{}]", response.data.len(), sample)
        },
    )
    .await;

    let remote_skills = run_probe(
        "skills/remote/list",
        &mut channels,
        &args,
        client.skills_remote_list(SkillsRemoteReadParams {
            hazelnut_scope: HazelnutScope::Example,
            product_surface: ProductSurface::Codex,
            enabled: false,
        }),
        |response| {
            let sample = response
                .data
                .iter()
                .take(3)
                .map(|entry| entry.id.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!("remote_skills={} sample=[{}]", response.data.len(), sample)
        },
    )
    .await;

    let requested_skill_name = args
        .skill_name
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if requested_skill_name.is_some()
        && args
            .prompt
            .as_ref()
            .map(|value| value.trim().is_empty())
            .unwrap_or(true)
    {
        bail!("--skill requires --prompt so turn/start can attach and exercise the skill");
    }

    if args.include_writes {
        if let Some(response) = remote_skills.as_ref() {
            if let Some(skill) = response.data.first() {
                let _ = run_probe(
                    "skills/remote/export",
                    &mut channels,
                    &args,
                    client.skills_remote_export(SkillsRemoteWriteParams {
                        hazelnut_id: skill.id.clone(),
                    }),
                    |result| format!("id={} path={}", result.id, result.path.display()),
                )
                .await;
            } else {
                println!("probe skills/remote/export: skipped (no remote skills)");
            }
        } else {
            println!("probe skills/remote/export: skipped (remote list failed)");
        }
    } else {
        println!("probe skills/remote/export: skipped (--include-writes not set)");
    }

    let extra_skills_root = args.cwd.join("skills");
    let mut skills_list = run_probe(
        "skills/list",
        &mut channels,
        &args,
        client.skills_list(build_skills_list_params(&args, &extra_skills_root)),
        summarize_skills_list,
    )
    .await;

    if args.include_writes {
        if let Some(response) = skills_list.as_ref() {
            let maybe_skill = response
                .data
                .iter()
                .flat_map(|entry| entry.skills.iter())
                .find(|skill| skill.path.starts_with(&args.cwd))
                .map(|skill| (skill.path.clone(), skill.enabled));
            if let Some((path, enabled)) = maybe_skill {
                let _ = run_probe(
                    "skills/config/write",
                    &mut channels,
                    &args,
                    client.skills_config_write(SkillsConfigWriteParams { path, enabled }),
                    |response| format!("effective_enabled={}", response.effective_enabled),
                )
                .await;
            } else {
                println!("probe skills/config/write: skipped (no local skill path found)");
            }
        } else {
            println!("probe skills/config/write: skipped (skills/list failed)");
        }
    } else {
        println!("probe skills/config/write: skipped (--include-writes not set)");
    }

    let mut selected_skill = requested_skill_name
        .as_deref()
        .and_then(|query| find_skill_in_list(skills_list.as_ref(), query));

    if selected_skill.is_none()
        && args.include_writes
        && requested_skill_name.is_some()
        && remote_skills.is_some()
    {
        let query = requested_skill_name.as_deref().unwrap_or_default();
        if let Some(remote_match) = find_remote_skill(remote_skills.as_ref(), query) {
            let exported = run_probe(
                "skills/remote/export(requested)",
                &mut channels,
                &args,
                client.skills_remote_export(SkillsRemoteWriteParams {
                    hazelnut_id: remote_match.id.clone(),
                }),
                |result| format!("id={} path={}", result.id, result.path.display()),
            )
            .await;
            if let Some(exported_skill) = exported.as_ref() {
                println!(
                    "requested skill exported remote_id={} local_path={}",
                    exported_skill.id,
                    exported_skill.path.display()
                );
            }
            skills_list = run_probe(
                "skills/list(reload-after-export)",
                &mut channels,
                &args,
                client.skills_list(build_skills_list_params(&args, &extra_skills_root)),
                summarize_skills_list,
            )
            .await;
            selected_skill = find_skill_in_list(skills_list.as_ref(), query);
        }
    }

    if let Some(query) = requested_skill_name.as_deref() {
        let mut selected = selected_skill
            .ok_or_else(|| {
                anyhow!(
                    "requested skill '{}' not found in skills/list{}",
                    query,
                    if args.include_writes {
                        ""
                    } else {
                        " (rerun with --include-writes to allow remote export)"
                    }
                )
            })
            .context("skill selection failed")?;

        println!(
            "selected_skill name={} path={} enabled={} scope={:?}",
            selected.name,
            selected.path.display(),
            selected.enabled,
            selected.scope
        );

        if !selected.enabled {
            if !args.include_writes {
                bail!(
                    "requested skill '{}' is disabled at {} (rerun with --include-writes to enable via skills/config/write)",
                    selected.name,
                    selected.path.display()
                );
            }
            let enabled_response = run_probe(
                "skills/config/write(enable-requested)",
                &mut channels,
                &args,
                client.skills_config_write(SkillsConfigWriteParams {
                    path: selected.path.clone(),
                    enabled: true,
                }),
                |response| format!("effective_enabled={}", response.effective_enabled),
            )
            .await;
            if enabled_response
                .as_ref()
                .map(|response| response.effective_enabled)
                .unwrap_or(false)
            {
                selected.enabled = true;
            }
        }
        selected_skill = Some(selected);
    }

    println!();
    println!("simulate_click chat.refresh_threads -> thread/list");
    let list = client
        .thread_list(ThreadListParams {
            cursor: None,
            limit: Some(args.list_limit),
            sort_key: Some(ThreadSortKey::UpdatedAt),
            model_providers: None,
            source_kinds: None,
            archived: Some(false),
            cwd: Some(args.cwd.display().to_string()),
            search_term: None,
        })
        .await
        .context("thread/list failed")?;
    println!("thread/list returned {} entries", list.data.len());

    let _loaded_list = run_probe(
        "thread/loaded/list",
        &mut channels,
        &args,
        client.thread_loaded_list(ThreadLoadedListParams {
            cursor: None,
            limit: Some(200),
        }),
        |response| format!("loaded_threads={}", response.data.len()),
    )
    .await;

    let previous_thread_id = list.data.first().map(|thread| thread.id.clone());
    if let Some(thread_id) = previous_thread_id.as_deref() {
        let previous_read = client
            .thread_read(ThreadReadParams {
                thread_id: thread_id.to_string(),
                include_turns: true,
            })
            .await
            .with_context(|| format!("thread/read failed for prior thread {}", thread_id))?;
        println!(
            "thread/read prior id={} turns={} transcript_messages={}",
            thread_id,
            previous_read.thread.turns.len(),
            transcript_message_count(&previous_read.thread)
        );
    } else {
        println!("thread/read prior skipped (no prior thread available)");
    }
    drain_and_print("post-refresh", &mut channels, &args).await?;

    println!();
    println!("simulate_click chat.new_thread -> thread/start");
    let started = client
        .thread_start(ThreadStartParams {
            model: resolved_model.clone(),
            model_provider: None,
            service_tier: Some(None),
            cwd: Some(args.cwd.display().to_string()),
            approval_policy: Some(AskForApproval::Never),
            sandbox: Some(SandboxMode::DangerFullAccess),
            personality: None,
            ephemeral: None,
            dynamic_tools: args
                .include_openagents_dynamic_tools
                .then(openagents_dynamic_tools::openagents_dynamic_tool_specs),
        })
        .await
        .context("thread/start failed")?;
    let new_thread_id = started.thread.id;
    println!("thread/start new_thread_id={new_thread_id}");

    let _thread_apps = run_probe(
        "app/list(thread)",
        &mut channels,
        &args,
        client.app_list(AppsListParams {
            cursor: None,
            limit: Some(100),
            thread_id: Some(new_thread_id.clone()),
            force_refetch: true,
        }),
        |response| {
            let sample = response
                .data
                .iter()
                .take(3)
                .map(|entry| entry.id.clone())
                .collect::<Vec<_>>()
                .join(", ");
            format!("apps={} sample=[{}]", response.data.len(), sample)
        },
    )
    .await;

    match client
        .thread_read(ThreadReadParams {
            thread_id: new_thread_id.clone(),
            include_turns: true,
        })
        .await
    {
        Ok(new_read) => {
            println!(
                "thread/read new id={} turns={} transcript_messages={}",
                new_thread_id,
                new_read.thread.turns.len(),
                transcript_message_count(&new_read.thread)
            );
        }
        Err(error) => {
            let error_text = error.to_string();
            if error_text.contains("not materialized yet")
                && error_text.contains("includeTurns is unavailable")
            {
                println!(
                    "thread/read new id={} unavailable pre-materialization: {}",
                    new_thread_id, error_text
                );
            } else {
                return Err(error).with_context(|| {
                    format!("thread/read failed for new thread {}", new_thread_id)
                });
            }
        }
    }

    if let Some(previous_thread_id) = previous_thread_id.as_deref() {
        let same_thread = previous_thread_id == new_thread_id;
        println!(
            "thread-switch-check previous={} new={} changed={}",
            previous_thread_id, new_thread_id, !same_thread
        );
    }
    drain_and_print("post-new-chat", &mut channels, &args).await?;

    let mut last_turn_id: Option<String> = None;
    if let Some(prompt) = args
        .prompt
        .as_ref()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        println!();
        println!("simulate_click chat.send -> turn/start");
        println!("prompt={prompt}");
        let mut turn_input = vec![UserInput::Text {
            text: prompt.clone(),
            text_elements: Vec::new(),
        }];
        if let Some(skill) = selected_skill.as_ref() {
            println!(
                "turn/start includes skill name={} path={}",
                skill.name,
                skill.path.display()
            );
            turn_input.push(UserInput::Skill {
                name: skill.name.clone(),
                path: skill.path.clone(),
            });
        }
        let turn = client
            .turn_start(TurnStartParams {
                thread_id: new_thread_id.clone(),
                input: turn_input,
                cwd: None,
                approval_policy: Some(AskForApproval::Never),
                sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
                model: resolved_model.clone(),
                service_tier: Some(None),
                effort: None,
                summary: None,
                personality: None,
                output_schema: None,
                collaboration_mode: None,
            })
            .await
            .context("turn/start failed")?;
        println!("turn/start turn_id={}", turn.turn.id);
        let mut post_send_events = collect_until_signal(
            &mut channels,
            Duration::from_millis(args.timeout_ms.saturating_mul(6).max(30_000)),
            Duration::from_millis(args.drain_ms.clamp(400, 2_000)),
            Duration::from_millis(args.timeout_ms),
            is_agent_response_signal,
        )
        .await;
        print_events("post-send", &post_send_events, args.max_events);
        if let Some(error) = find_invalid_tool_name_validation_error(&post_send_events) {
            bail!(
                "post-send events included invalid dynamic tool name validation error: {}",
                error
            );
        }
        if args.require_cad_tool_call && !events_include_openagents_cad_tool_call(&post_send_events)
        {
            let cad_followup = collect_channel_events(
                &mut channels,
                Duration::from_millis(args.drain_ms.clamp(600, 2_500)),
                Duration::from_millis(args.timeout_ms.saturating_mul(3).max(12_000)),
            )
            .await;
            print_events("post-send-cad-followup", &cad_followup, args.max_events);
            if let Some(error) = find_invalid_tool_name_validation_error(&cad_followup) {
                bail!(
                    "post-send CAD followup events included invalid dynamic tool name validation error: {}",
                    error
                );
            }
            merge_event_batches(&mut post_send_events, cad_followup);
        }
        let post_send_has_signal = post_send_events
            .notification_methods
            .iter()
            .any(|method| is_agent_response_signal(method));
        if !post_send_has_signal {
            let methods = if post_send_events.notification_methods.is_empty() {
                "none".to_string()
            } else {
                post_send_events.notification_methods.join(", ")
            };
            bail!(
                "post-send notifications for thread {} did not include any agent response signal; methods_seen=[{}]",
                new_thread_id,
                methods
            );
        }
        if args.require_cad_tool_call && !events_include_openagents_cad_tool_call(&post_send_events)
        {
            bail!(
                "post-send events for thread {} never included item/tool/call for CAD tools; rerun with --max-events and inspect request payloads",
                new_thread_id
            );
        }

        let after_turn = wait_for_materialized_thread_after_turn(
            &client,
            &new_thread_id,
            &prompt,
            Duration::from_millis(args.timeout_ms.saturating_mul(4).max(15_000)),
        )
        .await
        .with_context(|| {
            format!(
                "post-send materialization check failed for thread {}",
                new_thread_id
            )
        })?;
        println!(
            "thread/read after-send id={} turns={} transcript_messages={}",
            new_thread_id,
            after_turn.thread.turns.len(),
            transcript_message_count(&after_turn.thread)
        );
        if !thread_has_agent_role_message(&after_turn.thread) {
            bail!(
                "post-send transcript for thread {} has no assistant message; expected streaming/completion signal",
                new_thread_id
            );
        }
        if let Some(skill) = selected_skill.as_ref()
            && !events_include_skill_payload(&post_send_events, &skill.name)
        {
            bail!(
                "post-send notifications for thread {} did not show attached skill '{}' in turn/start payload",
                new_thread_id,
                skill.name
            );
        }
        if let Some(agent_reply) = latest_agent_message(&after_turn.thread) {
            println!(
                "post-send latest_agent_reply={}",
                summarize_text_for_log(&agent_reply, 96)
            );
            if args.fail_on_echo && agent_reply.trim().eq_ignore_ascii_case(prompt.trim()) {
                let latest_user =
                    latest_user_message(&after_turn.thread).unwrap_or_else(|| "<none>".to_string());
                bail!(
                    "assistant echo detected for model={} thread={} turn={} prompt={} agent={} latest_user={} (use --allow-echo-replies to bypass)",
                    resolved_model.as_deref().unwrap_or("server-default"),
                    new_thread_id,
                    turn.turn.id,
                    summarize_text_for_log(&prompt, 72),
                    summarize_text_for_log(&agent_reply, 72),
                    summarize_text_for_log(&latest_user, 72),
                );
            }
        }
        last_turn_id = after_turn.thread.turns.last().map(|turn| turn.id.clone());
    } else {
        println!("simulate_click chat.send: skipped (--prompt not provided)");
    }

    if args.include_thread_mutations {
        println!();
        println!("thread mutation probes");
        let _ = run_probe(
            "thread/name/set",
            &mut channels,
            &args,
            client.thread_name_set(ThreadSetNameParams {
                thread_id: new_thread_id.clone(),
                name: format!("Harness {}", short_thread_id(&new_thread_id)),
            }),
            |_| "renamed".to_string(),
        )
        .await;
        let _ = run_probe(
            "thread/backgroundTerminals/clean",
            &mut channels,
            &args,
            client.thread_background_terminals_clean(ThreadBackgroundTerminalsCleanParams {
                thread_id: new_thread_id.clone(),
            }),
            |_| "cleaned".to_string(),
        )
        .await;
        let _ = run_probe(
            "thread/compact/start",
            &mut channels,
            &args,
            client.thread_compact_start(ThreadCompactStartParams {
                thread_id: new_thread_id.clone(),
            }),
            |_| "started".to_string(),
        )
        .await;
        let _ = run_probe(
            "thread/fork",
            &mut channels,
            &args,
            client.thread_fork(ThreadForkParams {
                thread_id: new_thread_id.clone(),
                path: None,
                model: resolved_model.clone(),
                model_provider: None,
                service_tier: Some(None),
                cwd: Some(args.cwd.display().to_string()),
                approval_policy: Some(AskForApproval::Never),
                sandbox: Some(SandboxMode::DangerFullAccess),
                config: None,
                base_instructions: None,
                developer_instructions: None,
                persist_extended_history: false,
            }),
            |response| format!("forked_thread_id={}", response.thread.id),
        )
        .await;
        if last_turn_id.is_some() {
            let _ = run_probe(
                "thread/rollback",
                &mut channels,
                &args,
                client.thread_rollback(ThreadRollbackParams {
                    thread_id: new_thread_id.clone(),
                    num_turns: 1,
                }),
                |response| format!("thread_id={}", response.thread.id),
            )
            .await;
        } else {
            println!("probe thread/rollback: skipped (no turns available)");
        }
        let _ = run_probe(
            "thread/archive",
            &mut channels,
            &args,
            client.thread_archive(ThreadArchiveParams {
                thread_id: new_thread_id.clone(),
            }),
            |_| "archived".to_string(),
        )
        .await;
        let _ = run_probe(
            "thread/unarchive",
            &mut channels,
            &args,
            client.thread_unarchive(ThreadUnarchiveParams {
                thread_id: new_thread_id.clone(),
            }),
            |response| format!("thread_id={}", response.thread.id),
        )
        .await;
    } else {
        println!("thread mutation probes: skipped (--skip-thread-mutations set)");
    }

    let _command_exec = run_probe(
        "command/exec",
        &mut channels,
        &args,
        client.command_exec(CommandExecParams {
            command: vec!["/bin/zsh".to_string(), "-lc".to_string(), "pwd".to_string()],
            timeout_ms: Some(5_000),
            cwd: Some(args.cwd.display().to_string()),
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
        }),
        |response| {
            format!(
                "exit_code={} stdout_bytes={} stderr_bytes={}",
                response.exit_code,
                response.stdout.len(),
                response.stderr.len()
            )
        },
    )
    .await;

    let _review = run_probe(
        "review/start",
        &mut channels,
        &args,
        client.review_start(ReviewStartParams {
            thread_id: new_thread_id.clone(),
            target: ReviewTarget::UncommittedChanges,
            delivery: Some(ReviewDelivery::Inline),
        }),
        |response| {
            format!(
                "review_thread_id={} turn_id={}",
                response.review_thread_id, response.turn.id
            )
        },
    )
    .await;

    if args.include_experimental {
        println!();
        println!("experimental probes");
        let _ = run_probe(
            "fuzzyFileSearch/sessionStart",
            &mut channels,
            &args,
            client.fuzzy_file_search_session_start(FuzzyFileSearchSessionStartParams {
                session_id: format!("harness-{}", std::process::id()),
                roots: vec![args.cwd.display().to_string()],
            }),
            |_| "started".to_string(),
        )
        .await;
        let _ = run_probe(
            "fuzzyFileSearch/sessionUpdate",
            &mut channels,
            &args,
            client.fuzzy_file_search_session_update(FuzzyFileSearchSessionUpdateParams {
                session_id: format!("harness-{}", std::process::id()),
                query: "codex".to_string(),
            }),
            |_| "updated".to_string(),
        )
        .await;
        let _ = run_probe(
            "fuzzyFileSearch/sessionStop",
            &mut channels,
            &args,
            client.fuzzy_file_search_session_stop(FuzzyFileSearchSessionStopParams {
                session_id: format!("harness-{}", std::process::id()),
            }),
            |_| "stopped".to_string(),
        )
        .await;
        let _ = run_probe(
            "thread/realtime/start",
            &mut channels,
            &args,
            client.thread_realtime_start(ThreadRealtimeStartParams {
                thread_id: new_thread_id.clone(),
                prompt: "Harness realtime start".to_string(),
                session_id: None,
            }),
            |_| "started".to_string(),
        )
        .await;
        let _ = run_probe(
            "thread/realtime/appendText",
            &mut channels,
            &args,
            client.thread_realtime_append_text(ThreadRealtimeAppendTextParams {
                thread_id: new_thread_id.clone(),
                text: "Harness realtime append".to_string(),
            }),
            |_| "appended".to_string(),
        )
        .await;
        let _ = run_probe(
            "thread/realtime/stop",
            &mut channels,
            &args,
            client.thread_realtime_stop(ThreadRealtimeStopParams {
                thread_id: new_thread_id.clone(),
            }),
            |_| "stopped".to_string(),
        )
        .await;
        let _ = run_probe(
            "windowsSandbox/setupStart",
            &mut channels,
            &args,
            client.windows_sandbox_setup_start(WindowsSandboxSetupStartParams {
                mode: "unelevated".to_string(),
            }),
            |response| format!("started={}", response.started),
        )
        .await;
        let _ = run_probe(
            "experimental/mockedMethod",
            &mut channels,
            &args,
            client.mock_experimental_method(codex_client::MockExperimentalMethodParams {
                value: Some("openagents-harness".to_string()),
            }),
            |response| {
                format!(
                    "echoed={}",
                    response
                        .echoed
                        .clone()
                        .unwrap_or_else(|| "none".to_string())
                )
            },
        )
        .await;
    } else {
        println!("experimental probes: skipped (--skip-experimental set)");
    }

    println!();
    println!("harness complete");
    Ok(())
}

fn resolve_model(
    args: &HarnessArgs,
    model_list: Option<&codex_client::ModelListResponse>,
) -> Option<String> {
    if let Some(model) = args.model_override.as_ref() {
        return Some(model.clone());
    }
    let model_list = model_list?;
    if let Some(default_model) = model_list
        .data
        .iter()
        .find(|entry| entry.is_default)
        .map(|entry| entry.model.trim().to_string())
        .filter(|value| !value.is_empty())
    {
        return Some(default_model);
    }
    model_list
        .data
        .iter()
        .map(|entry| entry.model.trim().to_string())
        .find(|value| !value.is_empty())
}

async fn run_probe<T, Fut, SummaryFn>(
    label: &str,
    channels: &mut AppServerChannels,
    args: &HarnessArgs,
    fut: Fut,
    summary_fn: SummaryFn,
) -> Option<T>
where
    Fut: Future<Output = Result<T>>,
    SummaryFn: FnOnce(&T) -> String,
{
    println!("probe {label}");
    let value = match fut.await {
        Ok(value) => {
            println!("  status=ok {}", summary_fn(&value));
            Some(value)
        }
        Err(error) => {
            println!("  status=error {error}");
            None
        }
    };
    if let Err(error) = drain_and_print(&format!("post-{label}"), channels, args).await {
        println!("  post-{label} drain error: {error}");
        return None;
    }
    value
}

fn short_thread_id(thread_id: &str) -> &str {
    if thread_id.len() > 16 {
        &thread_id[..16]
    } else {
        thread_id
    }
}

fn collect_message_text(value: &Value) -> Option<&str> {
    if let Some(text) = value.get("text").and_then(Value::as_str) {
        return Some(text);
    }
    if let Some(message) = value.get("message").and_then(Value::as_str) {
        return Some(message);
    }
    if let Some(content_items) = value.get("content").and_then(Value::as_array) {
        for item in content_items {
            if let Some(text) = item.get("text").and_then(Value::as_str) {
                return Some(text);
            }
            if let Some(text) = item
                .get("annotations")
                .and_then(Value::as_array)
                .and_then(|annotations| annotations.first())
                .and_then(|annotation| annotation.get("text"))
                .and_then(Value::as_str)
            {
                return Some(text);
            }
        }
    }
    None
}

fn transcript_message_count(thread: &ThreadSnapshot) -> usize {
    thread
        .turns
        .iter()
        .map(|turn| {
            turn.items
                .iter()
                .filter(|item| collect_message_text(item).is_some())
                .count()
        })
        .sum()
}

fn thread_contains_text(thread: &ThreadSnapshot, needle: &str) -> bool {
    if needle.trim().is_empty() {
        return true;
    }
    let needle_lower = needle.to_lowercase();
    thread.turns.iter().any(|turn| {
        turn.items.iter().any(|item| {
            collect_message_text(item)
                .map(|text| text.to_lowercase().contains(&needle_lower))
                .unwrap_or(false)
        })
    })
}

fn transcript_role_counts(thread: &ThreadSnapshot) -> (usize, usize) {
    let mut user_count = 0usize;
    let mut agent_count = 0usize;

    for turn in &thread.turns {
        for item in &turn.items {
            let Some(object) = item.as_object() else {
                continue;
            };

            let kind = object
                .get("type")
                .and_then(Value::as_str)
                .or_else(|| {
                    object
                        .get("payload")
                        .and_then(Value::as_object)
                        .and_then(|payload| payload.get("type"))
                        .and_then(Value::as_str)
                })
                .unwrap_or_default();

            let role = object
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or_default();

            let has_text = collect_message_text(item).is_some();
            if !has_text {
                continue;
            }

            if matches!(kind, "user_message" | "userMessage") || role == "user" {
                user_count = user_count.saturating_add(1);
            } else if matches!(kind, "agent_message" | "agentMessage")
                || matches!(role, "assistant" | "codex")
            {
                agent_count = agent_count.saturating_add(1);
            }
        }
    }

    (user_count, agent_count)
}

fn thread_has_agent_role_message(thread: &ThreadSnapshot) -> bool {
    let (_, agent_count) = transcript_role_counts(thread);
    agent_count > 0
}

fn events_include_skill_payload(events: &ChannelEventBatch, skill_name: &str) -> bool {
    let normalized_name = format!("\"name\":\"{}\"", skill_name.to_ascii_lowercase());
    events.notification_params.iter().any(|params| {
        let normalized = params.to_string().to_ascii_lowercase();
        normalized.contains("\"type\":\"skill\"") && normalized.contains(&normalized_name)
    })
}

fn latest_agent_message(thread: &ThreadSnapshot) -> Option<String> {
    latest_role_message(thread, TranscriptMessageRole::Agent)
}

fn latest_user_message(thread: &ThreadSnapshot) -> Option<String> {
    latest_role_message(thread, TranscriptMessageRole::User)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TranscriptMessageRole {
    User,
    Agent,
}

fn latest_role_message(
    thread: &ThreadSnapshot,
    target_role: TranscriptMessageRole,
) -> Option<String> {
    for turn in thread.turns.iter().rev() {
        for item in turn.items.iter().rev() {
            let Some(text) = collect_message_text(item).map(str::to_string) else {
                continue;
            };

            let Some(object) = item.as_object() else {
                continue;
            };
            let kind = object
                .get("type")
                .and_then(Value::as_str)
                .or_else(|| {
                    object
                        .get("payload")
                        .and_then(Value::as_object)
                        .and_then(|payload| payload.get("type"))
                        .and_then(Value::as_str)
                })
                .unwrap_or_default();
            let role = object
                .get("role")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let message_role = if matches!(kind, "user_message" | "userMessage") || role == "user" {
                TranscriptMessageRole::User
            } else if matches!(kind, "agent_message" | "agentMessage")
                || matches!(role, "assistant" | "codex")
            {
                TranscriptMessageRole::Agent
            } else {
                continue;
            };
            if message_role == target_role {
                return Some(text);
            }
        }
    }
    None
}

fn summarize_text_for_log(value: &str, max_chars: usize) -> String {
    let trimmed = value.trim();
    if trimmed.chars().count() <= max_chars {
        return trimmed.to_string();
    }
    let mut result = trimmed.chars().take(max_chars).collect::<String>();
    result.push_str("...");
    result
}

fn is_openagents_cad_tool_name(tool: &str) -> bool {
    let normalized = tool.trim();
    normalized.eq_ignore_ascii_case(openagents_dynamic_tools::OPENAGENTS_TOOL_CAD_INTENT)
        || normalized.eq_ignore_ascii_case(openagents_dynamic_tools::OPENAGENTS_TOOL_CAD_ACTION)
        || normalized.eq_ignore_ascii_case(LEGACY_OPENAGENTS_TOOL_CAD_INTENT)
        || normalized.eq_ignore_ascii_case(LEGACY_OPENAGENTS_TOOL_CAD_ACTION)
}

fn events_include_openagents_cad_tool_call(events: &ChannelEventBatch) -> bool {
    events
        .request_methods
        .iter()
        .zip(events.request_params.iter())
        .any(|(method, params)| {
            method.eq_ignore_ascii_case("item/tool/call")
                && params
                    .get("tool")
                    .and_then(Value::as_str)
                    .is_some_and(is_openagents_cad_tool_name)
        })
}

fn find_invalid_tool_name_validation_error(events: &ChannelEventBatch) -> Option<String> {
    events
        .notification_methods
        .iter()
        .zip(events.notification_params.iter())
        .find_map(|(method, params)| {
            let serialized =
                serde_json::to_string(params).unwrap_or_else(|_| "<invalid-json>".to_string());
            let has_tool_name_pattern_error = serialized.contains("invalid_request_error")
                && serialized.contains("Invalid 'tools[")
                && serialized.contains("^[a-zA-Z0-9_-]+$");
            has_tool_name_pattern_error.then(|| {
                format!(
                    "method={} payload={}",
                    method,
                    summarize_text_for_log(&serialized, 220)
                )
            })
        })
}

fn is_agent_response_signal(method: &str) -> bool {
    matches!(
        method,
        "item/agentMessage/delta"
            | "item/assistantMessage/delta"
            | "agent_message/delta"
            | "agent_message_delta"
            | "agent_message_content_delta"
            | "item/agentMessage/completed"
            | "item/assistantMessage/completed"
            | "codex/event/agent_message_content_delta"
            | "codex/event/agent_message_delta"
            | "codex/event/agent_message"
            | "agent_message"
            | "codex/event/task_complete"
            | "task_complete"
            | "turn/completed"
    )
}

async fn wait_for_materialized_thread_after_turn(
    client: &AppServerClient,
    thread_id: &str,
    prompt: &str,
    timeout: Duration,
) -> Result<codex_client::ThreadReadResponse> {
    let deadline = Instant::now() + timeout;
    let mut attempts: u32 = 0;
    let mut last_observation = "no thread/read attempts made".to_string();

    while Instant::now() <= deadline {
        attempts = attempts.saturating_add(1);
        match client
            .thread_read(ThreadReadParams {
                thread_id: thread_id.to_string(),
                include_turns: true,
            })
            .await
        {
            Ok(response) => {
                let message_count = transcript_message_count(&response.thread);
                let (user_count, agent_count) = transcript_role_counts(&response.thread);
                let prompt_seen = thread_contains_text(&response.thread, prompt);
                if user_count >= 1 && agent_count >= 1 && prompt_seen {
                    println!(
                        "post-send materialization check passed attempts={} messages={} users={} agents={} prompt_seen={}",
                        attempts, message_count, user_count, agent_count, prompt_seen
                    );
                    return Ok(response);
                }
                last_observation = format!(
                    "thread/read attempts={} turns={} messages={} users={} agents={} prompt_seen={}",
                    attempts,
                    response.thread.turns.len(),
                    message_count,
                    user_count,
                    agent_count,
                    prompt_seen
                );
            }
            Err(error) => {
                last_observation = format!("thread/read attempts={} error={}", attempts, error);
            }
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    bail!(
        "timed out waiting for materialized post-send transcript for thread {}: {}",
        thread_id,
        last_observation
    )
}

async fn drain_and_print(
    label: &str,
    channels: &mut AppServerChannels,
    args: &HarnessArgs,
) -> Result<ChannelEventBatch> {
    let events = collect_channel_events(
        channels,
        Duration::from_millis(args.drain_ms),
        Duration::from_millis(args.timeout_ms),
    )
    .await;
    print_events(label, &events, args.max_events);
    let leaked_opt_out_methods = events
        .notification_methods
        .iter()
        .filter(|method| LEGACY_NOTIFICATION_OPT_OUT_METHODS.contains(&method.as_str()))
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    if !leaked_opt_out_methods.is_empty() {
        bail!(
            "{label} emitted opted-out legacy notifications: {}",
            leaked_opt_out_methods
                .into_iter()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    Ok(events)
}

async fn collect_channel_events(
    channels: &mut AppServerChannels,
    idle_break: Duration,
    max_wait: Duration,
) -> ChannelEventBatch {
    let start = Instant::now();
    let deadline = start + max_wait;
    let mut last_event = start;
    let mut batch = ChannelEventBatch::default();

    loop {
        let now = Instant::now();
        if now >= deadline {
            break;
        }
        if (!batch.notifications.is_empty() || !batch.requests.is_empty())
            && now.duration_since(last_event) >= idle_break
        {
            break;
        }

        let sleep_for = deadline
            .saturating_duration_since(now)
            .min(Duration::from_millis(150));

        tokio::select! {
            maybe = channels.notifications.recv() => {
                let Some(notification) = maybe else {
                    break;
                };
                batch
                    .notification_methods
                    .push(notification.method.clone());
                batch
                    .notification_params
                    .push(notification.params.clone().unwrap_or(Value::Null));
                batch.notifications.push(notification_summary(&notification));
                last_event = Instant::now();
            }
            maybe = channels.requests.recv() => {
                let Some(request) = maybe else {
                    break;
                };
                batch.request_methods.push(request.method.clone());
                batch
                    .request_params
                    .push(request.params.clone().unwrap_or(Value::Null));
                batch.requests.push(request_summary(&request));
                last_event = Instant::now();
            }
            () = tokio::time::sleep(sleep_for) => {}
        }
    }

    batch
}

async fn collect_until_signal<F>(
    channels: &mut AppServerChannels,
    overall_timeout: Duration,
    idle_break: Duration,
    max_wait_per_batch: Duration,
    signal: F,
) -> ChannelEventBatch
where
    F: Fn(&str) -> bool,
{
    let deadline = Instant::now() + overall_timeout;
    let mut combined = ChannelEventBatch::default();
    while Instant::now() < deadline {
        let batch = collect_channel_events(channels, idle_break, max_wait_per_batch).await;
        let saw_signal = batch
            .notification_methods
            .iter()
            .any(|method| signal(method));
        combined.notifications.extend(batch.notifications);
        combined
            .notification_methods
            .extend(batch.notification_methods);
        combined
            .notification_params
            .extend(batch.notification_params);
        combined.requests.extend(batch.requests);
        combined.request_methods.extend(batch.request_methods);
        combined.request_params.extend(batch.request_params);
        if saw_signal {
            break;
        }
    }
    combined
}

fn merge_event_batches(target: &mut ChannelEventBatch, source: ChannelEventBatch) {
    target.notifications.extend(source.notifications);
    target
        .notification_methods
        .extend(source.notification_methods);
    target
        .notification_params
        .extend(source.notification_params);
    target.requests.extend(source.requests);
    target.request_methods.extend(source.request_methods);
    target.request_params.extend(source.request_params);
}

fn notification_summary(notification: &AppServerNotification) -> String {
    format!(
        "notify method={} params={}",
        notification.method,
        compact_json(notification.params.as_ref())
    )
}

fn request_summary(request: &AppServerRequest) -> String {
    format!(
        "request method={} id={} params={}",
        request.method,
        request_id_summary(&request.id),
        compact_json(request.params.as_ref())
    )
}

fn request_id_summary(id: &AppServerRequestId) -> String {
    match id {
        AppServerRequestId::String(value) => value.clone(),
        AppServerRequestId::Integer(value) => value.to_string(),
    }
}

fn compact_json(value: Option<&Value>) -> String {
    let Some(value) = value else {
        return "null".to_string();
    };
    let serialized = serde_json::to_string(value).unwrap_or_else(|_| "<invalid-json>".to_string());
    const LIMIT: usize = 180;
    if serialized.chars().count() <= LIMIT {
        serialized
    } else {
        let mut truncated = serialized.chars().take(LIMIT).collect::<String>();
        truncated.push_str("...");
        truncated
    }
}

fn print_events(label: &str, events: &ChannelEventBatch, max_events: usize) {
    println!(
        "{} events: notifications={} requests={}",
        label,
        events.notifications.len(),
        events.requests.len()
    );
    let mut notification_lines = events.notifications.clone();
    let notification_omitted = notification_lines.len().saturating_sub(max_events);
    notification_lines.truncate(max_events);
    for line in notification_lines {
        println!("  {line}");
    }
    if notification_omitted > 0 {
        println!(
            "  ... {} additional notifications omitted",
            notification_omitted
        );
    }
    let mut request_lines = events.requests.clone();
    let request_omitted = request_lines.len().saturating_sub(max_events);
    request_lines.truncate(max_events);
    for line in request_lines {
        println!("  {line}");
    }
    if request_omitted > 0 {
        println!("  ... {} additional requests omitted", request_omitted);
    }
}

fn build_skills_list_params(
    args: &HarnessArgs,
    extra_skills_root: &std::path::Path,
) -> SkillsListParams {
    SkillsListParams {
        cwds: vec![args.cwd.clone()],
        force_reload: true,
        per_cwd_extra_user_roots: if extra_skills_root.exists() {
            Some(vec![SkillsListExtraRootsForCwd {
                cwd: args.cwd.clone(),
                extra_user_roots: vec![extra_skills_root.to_path_buf()],
            }])
        } else {
            None
        },
    }
}

fn summarize_skills_list(response: &SkillsListResponse) -> String {
    let skill_count = response
        .data
        .iter()
        .map(|entry| entry.skills.len())
        .sum::<usize>();
    let error_count = response
        .data
        .iter()
        .map(|entry| entry.errors.len())
        .sum::<usize>();
    format!("skills={} errors={}", skill_count, error_count)
}

fn find_skill_in_list(response: Option<&SkillsListResponse>, query: &str) -> Option<SkillMetadata> {
    let normalized_query = query.trim().to_ascii_lowercase();
    if normalized_query.is_empty() {
        return None;
    }
    let skills = response?
        .data
        .iter()
        .flat_map(|entry| entry.skills.iter().cloned())
        .collect::<Vec<_>>();

    let exact = skills
        .iter()
        .find(|skill| {
            skill.name.eq_ignore_ascii_case(query)
                || skill_dir_name(skill)
                    .map(|name| name.eq_ignore_ascii_case(query))
                    .unwrap_or(false)
                || skill.path.to_string_lossy().eq_ignore_ascii_case(query)
        })
        .cloned();
    if exact.is_some() {
        return exact;
    }

    skills
        .iter()
        .find(|skill| {
            skill.name.to_ascii_lowercase().contains(&normalized_query)
                || skill_dir_name(skill)
                    .map(|name| name.to_ascii_lowercase().contains(&normalized_query))
                    .unwrap_or(false)
                || skill
                    .path
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .contains(&normalized_query)
        })
        .cloned()
}

fn skill_dir_name(skill: &SkillMetadata) -> Option<&str> {
    skill.path.parent()?.file_name()?.to_str()
}

fn find_remote_skill(
    response: Option<&SkillsRemoteReadResponse>,
    query: &str,
) -> Option<RemoteSkillSummary> {
    let normalized_query = query.trim().to_ascii_lowercase();
    if normalized_query.is_empty() {
        return None;
    }
    let remote = response?;
    remote
        .data
        .iter()
        .find(|skill| {
            skill.id.eq_ignore_ascii_case(query)
                || skill.name.eq_ignore_ascii_case(query)
                || skill.id.to_ascii_lowercase().contains(&normalized_query)
                || skill.name.to_ascii_lowercase().contains(&normalized_query)
                || skill
                    .description
                    .to_ascii_lowercase()
                    .contains(&normalized_query)
        })
        .cloned()
}

fn normalize_blink_direction(raw: &str) -> Option<&'static str> {
    let normalized = raw.trim().to_ascii_lowercase().replace('_', "-");
    match normalized.as_str() {
        "btc-to-usd" | "sell-btc" | "buy-usd" => Some("btc-to-usd"),
        "usd-to-btc" | "sell-usd" | "buy-btc" => Some("usd-to-btc"),
        _ => None,
    }
}

fn normalize_blink_unit(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "sats" | "sat" | "satoshi" => Some("sats"),
        "cents" | "cent" | "usd-cents" => Some("cents"),
        _ => None,
    }
}

fn read_keychain_secret(account: &str) -> Option<String> {
    let output = ProcessCommand::new("security")
        .args([
            "find-generic-password",
            "-s",
            BLINK_KEYCHAIN_SERVICE,
            "-a",
            account,
            "-w",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if value.is_empty() { None } else { Some(value) }
}

fn read_env_or_keychain(account: &str) -> Option<String> {
    std::env::var(account)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| read_keychain_secret(account))
}

fn resolve_blink_env() -> Result<Vec<(String, String)>> {
    resolve_blink_env_for_accounts(
        &[BLINK_KEYCHAIN_ACCOUNT_API_KEY],
        &[BLINK_KEYCHAIN_ACCOUNT_API_URL],
    )
}

fn resolve_blink_env_for_accounts(
    api_key_accounts: &[&str],
    api_url_accounts: &[&str],
) -> Result<Vec<(String, String)>> {
    let api_key = api_key_accounts
        .iter()
        .find_map(|account| read_env_or_keychain(account))
        .ok_or_else(|| {
        anyhow!(
            "Blink API key missing from environment and keychain service '{}' for accounts [{}]",
            BLINK_KEYCHAIN_SERVICE,
            api_key_accounts.join(", ")
        )
    })?;

    let api_url = api_url_accounts
        .iter()
        .find_map(|account| read_env_or_keychain(account));

    let mut env = vec![("BLINK_API_KEY".to_string(), api_key)];
    if let Some(api_url) = api_url {
        env.push(("BLINK_API_URL".to_string(), api_url));
    }
    Ok(env)
}

#[derive(Debug)]
struct ScriptRunOutput {
    success: bool,
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

fn run_node_script(
    script_path: &std::path::Path,
    args: &[String],
    env: &[(String, String)],
) -> Result<ScriptRunOutput> {
    let mut command = ProcessCommand::new("node");
    command.arg(script_path);
    command.args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    let output = command
        .output()
        .with_context(|| format!("failed to execute node script {}", script_path.display()))?;
    Ok(ScriptRunOutput {
        success: output.status.success(),
        exit_code: output.status.code(),
        stdout: String::from_utf8_lossy(&output.stdout).trim().to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
    })
}

fn parse_script_json(output: &ScriptRunOutput, script_label: &str) -> Result<Value> {
    if !output.success {
        bail!(
            "{} failed (exit_code={:?}) stderr='{}' stdout='{}'",
            script_label,
            output.exit_code,
            summarize_text_for_log(output.stderr.as_str(), 240),
            summarize_text_for_log(output.stdout.as_str(), 240),
        );
    }
    if output.stdout.trim().is_empty() {
        bail!("{script_label} returned empty stdout");
    }
    serde_json::from_str::<Value>(&output.stdout).with_context(|| {
        format!(
            "{} returned non-JSON stdout: {}",
            script_label,
            summarize_text_for_log(output.stdout.as_str(), 240)
        )
    })
}

#[derive(Clone, Debug)]
struct BlinkScriptPaths {
    balance: PathBuf,
    create_invoice: PathBuf,
    create_invoice_usd: PathBuf,
    swap_execute: PathBuf,
    swap_quote: PathBuf,
}

impl BlinkScriptPaths {
    fn resolve(cwd: &std::path::Path) -> Result<Self> {
        let scripts_root = cwd.join("skills").join("blink").join("scripts");
        let paths = Self {
            balance: scripts_root.join("balance.js"),
            create_invoice: scripts_root.join("create_invoice.js"),
            create_invoice_usd: scripts_root.join("create_invoice_usd.js"),
            swap_execute: scripts_root.join("swap_execute.js"),
            swap_quote: scripts_root.join("swap_quote.js"),
        };
        for required in [
            paths.balance.as_path(),
            paths.create_invoice.as_path(),
            paths.create_invoice_usd.as_path(),
            paths.swap_execute.as_path(),
            paths.swap_quote.as_path(),
        ] {
            if !required.is_file() {
                bail!("Blink script missing: {}", required.display());
            }
        }
        Ok(paths)
    }
}

#[derive(Clone, Debug)]
struct BlinkWalletRuntime {
    owner: &'static str,
    env: Vec<(String, String)>,
}

#[derive(Clone, Debug)]
struct BlinkWalletBalances {
    owner: &'static str,
    btc_wallet_id: String,
    usd_wallet_id: String,
    btc_balance_sats: u64,
    usd_balance_cents: u64,
}

#[derive(Clone, Debug)]
struct BlinkSwapExecutionResult {
    status: String,
    quote_id: String,
    quote_amount_in: u64,
    quote_amount_out: u64,
    script_btc_delta_sats: i64,
    script_usd_delta_cents: i64,
    transaction_id: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BlinkTransferAsset {
    BtcSats,
    UsdCents,
}

impl BlinkTransferAsset {
    const fn label(self) -> &'static str {
        match self {
            Self::BtcSats => "btc_sats",
            Self::UsdCents => "usd_cents",
        }
    }

    const fn unit_label(self) -> &'static str {
        match self {
            Self::BtcSats => "sats",
            Self::UsdCents => "cents",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StableSatsFundingDeficit {
    owner: &'static str,
    asset: BlinkTransferAsset,
    required: u64,
    available: u64,
    shortfall: u64,
}

#[derive(Clone, Debug)]
struct BlinkFundingInvoice {
    owner: String,
    asset: BlinkTransferAsset,
    amount_requested: u64,
    payment_request: String,
    payment_hash: String,
    invoice_sats: u64,
}

fn parse_u64_like(value: &Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|raw| u64::try_from(raw).ok()))
        .or_else(|| {
            value
                .as_str()
                .and_then(|raw| raw.trim().parse::<u64>().ok())
        })
}

fn parse_i64_like(value: &Value) -> Option<i64> {
    value
        .as_i64()
        .or_else(|| value.as_u64().and_then(|raw| i64::try_from(raw).ok()))
        .or_else(|| {
            value
                .as_str()
                .and_then(|raw| raw.trim().parse::<i64>().ok())
        })
}

fn value_u64_at(value: &Value, pointer: &str, label: &str) -> Result<u64> {
    value
        .pointer(pointer)
        .and_then(parse_u64_like)
        .ok_or_else(|| anyhow!("missing numeric field {label} at {pointer}"))
}

fn value_i64_at(value: &Value, pointer: &str, label: &str) -> Result<i64> {
    value
        .pointer(pointer)
        .and_then(parse_i64_like)
        .ok_or_else(|| anyhow!("missing signed numeric field {label} at {pointer}"))
}

fn value_string_at(value: &Value, pointer: &str, label: &str) -> Result<String> {
    value
        .pointer(pointer)
        .and_then(Value::as_str)
        .map(ToString::to_string)
        .ok_or_else(|| anyhow!("missing string field {label} at {pointer}"))
}

fn parse_wallet_balance_payload(
    owner: &'static str,
    payload: &Value,
) -> Result<BlinkWalletBalances> {
    Ok(BlinkWalletBalances {
        owner,
        btc_wallet_id: value_string_at(payload, "/btcWalletId", "btc_wallet_id")?,
        usd_wallet_id: value_string_at(payload, "/usdWalletId", "usd_wallet_id")?,
        btc_balance_sats: value_u64_at(payload, "/btcBalanceSats", "btc_balance_sats")?,
        usd_balance_cents: value_u64_at(payload, "/usdBalanceCents", "usd_balance_cents")?,
    })
}

fn stable_sats_funding_requirements(probe: &BlinkStableSatsSaProbeArgs) -> (u64, u64) {
    let rounds = u64::from(probe.rounds);
    // Conservative buffers absorb spread and occasional routing costs.
    let operator_btc_required = rounds
        .saturating_mul(probe.convert_btc_sats)
        .saturating_add(rounds.saturating_mul(120));
    let operator_usd_required = rounds
        .saturating_mul(probe.convert_usd_cents)
        .saturating_add(rounds.saturating_mul(20));
    (operator_btc_required, operator_usd_required)
}

fn stable_sats_funding_deficits(
    probe: &BlinkStableSatsSaProbeArgs,
    balances: &std::collections::BTreeMap<String, BlinkWalletBalances>,
) -> Result<Vec<StableSatsFundingDeficit>> {
    let operator = lookup_balances(balances, "operator")?;
    let (operator_btc_required, operator_usd_required) = stable_sats_funding_requirements(probe);

    let mut deficits = Vec::new();
    if operator.btc_balance_sats < operator_btc_required {
        deficits.push(StableSatsFundingDeficit {
            owner: "operator",
            asset: BlinkTransferAsset::BtcSats,
            required: operator_btc_required,
            available: operator.btc_balance_sats,
            shortfall: operator_btc_required.saturating_sub(operator.btc_balance_sats),
        });
    }
    if operator.usd_balance_cents < operator_usd_required {
        deficits.push(StableSatsFundingDeficit {
            owner: "operator",
            asset: BlinkTransferAsset::UsdCents,
            required: operator_usd_required,
            available: operator.usd_balance_cents,
            shortfall: operator_usd_required.saturating_sub(operator.usd_balance_cents),
        });
    }

    Ok(deficits)
}

fn load_wallet_balances(
    owner: &'static str,
    env: &[(String, String)],
    script_paths: &BlinkScriptPaths,
) -> Result<BlinkWalletBalances> {
    let output = run_node_script(script_paths.balance.as_path(), &[], env)?;
    let payload = parse_script_json(&output, "blink balance")?;
    parse_wallet_balance_payload(owner, &payload)
}

fn load_all_wallet_balances(
    wallets: &[BlinkWalletRuntime],
    script_paths: &BlinkScriptPaths,
) -> Result<std::collections::BTreeMap<String, BlinkWalletBalances>> {
    let mut balances = std::collections::BTreeMap::new();
    for wallet in wallets {
        let snapshot = load_wallet_balances(wallet.owner, wallet.env.as_slice(), script_paths)?;
        balances.insert(wallet.owner.to_string(), snapshot);
    }
    Ok(balances)
}

fn signed_delta_u64(after: u64, before: u64) -> i64 {
    let delta = i128::from(after) - i128::from(before);
    delta.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn wallet_deltas(before: &BlinkWalletBalances, after: &BlinkWalletBalances) -> (i64, i64) {
    (
        signed_delta_u64(after.btc_balance_sats, before.btc_balance_sats),
        signed_delta_u64(after.usd_balance_cents, before.usd_balance_cents),
    )
}

fn compute_effective_swap_spread_units(
    direction: &str,
    quote_amount_out: u64,
    observed_btc_delta_sats: i64,
    observed_usd_delta_cents: i64,
) -> Result<i64> {
    let quoted = i64::try_from(quote_amount_out)
        .map_err(|_| anyhow!("quote amount out exceeds i64 range: {}", quote_amount_out))?;
    match normalize_blink_direction(direction) {
        Some("btc-to-usd") => Ok(quoted.saturating_sub(observed_usd_delta_cents)),
        Some("usd-to-btc") => Ok(quoted.saturating_sub(observed_btc_delta_sats)),
        _ => bail!(
            "unsupported swap direction for spread computation: {}",
            direction
        ),
    }
}

fn lookup_balances<'a>(
    balances: &'a std::collections::BTreeMap<String, BlinkWalletBalances>,
    owner: &str,
) -> Result<&'a BlinkWalletBalances> {
    balances
        .get(owner)
        .ok_or_else(|| anyhow!("missing balances for owner '{}'", owner))
}

fn aggregate_wallet_balances(
    balances: &std::collections::BTreeMap<String, BlinkWalletBalances>,
) -> (u64, u64) {
    balances.values().fold((0_u64, 0_u64), |(btc, usd), entry| {
        (
            btc.saturating_add(entry.btc_balance_sats),
            usd.saturating_add(entry.usd_balance_cents),
        )
    })
}

fn balances_map_to_json(
    balances: &std::collections::BTreeMap<String, BlinkWalletBalances>,
) -> serde_json::Value {
    let mut wallet_map = serde_json::Map::new();
    for (owner, entry) in balances {
        wallet_map.insert(
            owner.clone(),
            serde_json::json!({
                "owner": entry.owner,
                "btc_wallet_id": entry.btc_wallet_id,
                "usd_wallet_id": entry.usd_wallet_id,
                "btc_balance_sats": entry.btc_balance_sats,
                "usd_balance_cents": entry.usd_balance_cents,
                "usd_balance_formatted": format!("${}.{:02}", entry.usd_balance_cents / 100, entry.usd_balance_cents % 100),
            }),
        );
    }
    serde_json::Value::Object(wallet_map)
}

fn create_wallet_funding_invoice(
    script_paths: &BlinkScriptPaths,
    wallet_env: &[(String, String)],
    owner: &str,
    asset: BlinkTransferAsset,
    amount: u64,
    memo: Option<&str>,
) -> Result<BlinkFundingInvoice> {
    let invoice_script = match asset {
        BlinkTransferAsset::BtcSats => script_paths.create_invoice.as_path(),
        BlinkTransferAsset::UsdCents => script_paths.create_invoice_usd.as_path(),
    };
    let mut create_args = vec![amount.to_string(), "--no-subscribe".to_string()];
    if let Some(memo) = memo.filter(|raw| !raw.trim().is_empty()) {
        create_args.push(memo.to_string());
    }
    let create_output = run_node_script(invoice_script, create_args.as_slice(), wallet_env)?;
    let create_json = parse_script_json(&create_output, "blink funding invoice")?;
    let create_event = create_json
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    if !create_event.eq_ignore_ascii_case("invoice_created") {
        bail!(
            "blink funding invoice returned unexpected event '{}' for owner={}",
            create_event,
            owner
        );
    }

    Ok(BlinkFundingInvoice {
        owner: owner.to_string(),
        asset,
        amount_requested: amount,
        payment_request: value_string_at(&create_json, "/paymentRequest", "payment_request")?,
        payment_hash: value_string_at(&create_json, "/paymentHash", "payment_hash")?,
        invoice_sats: value_u64_at(&create_json, "/satoshis", "invoice_sats")?,
    })
}

fn run_wallet_swap_execute(
    script_paths: &BlinkScriptPaths,
    wallet_env: &[(String, String)],
    direction: &str,
    amount: u64,
    unit: &str,
    memo: Option<&str>,
) -> Result<BlinkSwapExecutionResult> {
    let mut args = vec![
        direction.to_string(),
        amount.to_string(),
        "--unit".to_string(),
        unit.to_string(),
    ];
    if let Some(memo) = memo.filter(|raw| !raw.trim().is_empty()) {
        args.push("--memo".to_string());
        args.push(memo.to_string());
    }
    let output = run_node_script(
        script_paths.swap_execute.as_path(),
        args.as_slice(),
        wallet_env,
    )?;
    let payload = parse_script_json(&output, "blink swap execute")?;
    let event = payload
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    if !event.eq_ignore_ascii_case("swap_execution") {
        bail!("blink swap execute returned unexpected event '{}'", event);
    }
    Ok(BlinkSwapExecutionResult {
        status: value_string_at(&payload, "/status", "swap_status")?,
        quote_id: value_string_at(&payload, "/quote/quoteId", "swap_quote_id")?,
        quote_amount_in: value_u64_at(&payload, "/quote/amountIn/value", "swap_quote_amount_in")?,
        quote_amount_out: value_u64_at(
            &payload,
            "/quote/amountOut/value",
            "swap_quote_amount_out",
        )?,
        script_btc_delta_sats: value_i64_at(
            &payload,
            "/balanceDelta/btcDeltaSats",
            "swap_btc_delta_sats",
        )?,
        script_usd_delta_cents: value_i64_at(
            &payload,
            "/balanceDelta/usdDeltaCents",
            "swap_usd_delta_cents",
        )?,
        transaction_id: payload
            .pointer("/execution/transactionId")
            .and_then(Value::as_str)
            .map(ToString::to_string),
    })
}

fn run_blink_stablesats_sa_live_probe(
    cwd: &std::path::Path,
    probe: &BlinkStableSatsSaProbeArgs,
) -> Result<()> {
    println!("blink_stablesats_sa_probe start");
    let script_paths = BlinkScriptPaths::resolve(cwd)?;
    let wallets = vec![BlinkWalletRuntime {
        owner: "operator",
        env: resolve_blink_env()?,
    }];
    let memo_prefix = probe
        .memo_prefix
        .as_deref()
        .filter(|raw| !raw.trim().is_empty())
        .unwrap_or("openagents-stablesats-sa-live-harness");

    let mut balances = load_all_wallet_balances(wallets.as_slice(), &script_paths)?;
    let initial_balances = balances.clone();
    let funding_deficits = stable_sats_funding_deficits(probe, &balances)?;
    if !funding_deficits.is_empty() {
        let (operator_btc_required, operator_usd_required) =
            stable_sats_funding_requirements(probe);
        let mut funding_invoices = Vec::new();
        let mut funding_invoice_errors = Vec::new();
        for deficit in funding_deficits.as_slice() {
            let Some(wallet) = wallets.iter().find(|wallet| wallet.owner == deficit.owner) else {
                funding_invoice_errors.push(serde_json::json!({
                    "owner": deficit.owner,
                    "asset": deficit.asset.label(),
                    "error": "wallet runtime entry not found",
                }));
                continue;
            };
            match create_wallet_funding_invoice(
                &script_paths,
                wallet.env.as_slice(),
                deficit.owner,
                deficit.asset,
                deficit.shortfall,
                Some(&format!(
                    "{memo_prefix}:funding:{}:{}",
                    deficit.owner,
                    deficit.asset.label()
                )),
            ) {
                Ok(invoice) => {
                    funding_invoices.push(serde_json::json!({
                        "owner": invoice.owner,
                        "asset": invoice.asset.label(),
                        "amount_requested": invoice.amount_requested,
                        "invoice_sats": invoice.invoice_sats,
                        "payment_hash": invoice.payment_hash,
                        "payment_request": invoice.payment_request,
                    }));
                }
                Err(error) => {
                    funding_invoice_errors.push(serde_json::json!({
                        "owner": deficit.owner,
                        "asset": deficit.asset.label(),
                        "amount_requested": deficit.shortfall,
                        "error": error.to_string(),
                    }));
                }
            }
        }
        let report = serde_json::json!({
            "event": "blink_stablesats_sa_demo",
            "status": "funding_required",
            "rounds": probe.rounds,
            "required_minimums": {
                "operator_btc_sats": operator_btc_required,
                "operator_usd_cents": operator_usd_required,
            },
            "deficits": funding_deficits
                .iter()
                .map(|deficit| serde_json::json!({
                    "owner": deficit.owner,
                    "asset": deficit.asset.label(),
                    "unit": deficit.asset.unit_label(),
                    "required": deficit.required,
                    "available": deficit.available,
                    "shortfall": deficit.shortfall,
                }))
                .collect::<Vec<_>>(),
            "funding_invoices": funding_invoices,
            "funding_invoice_errors": funding_invoice_errors,
        });
        println!(
            "blink_stablesats_sa_probe funding_required={}",
            serde_json::to_string_pretty(&report)
                .context("failed to serialize stablesats sa funding report")?
        );
        bail!(
            "insufficient balances for stablesats sa probe: {} wallet deficit(s)",
            funding_deficits.len()
        );
    }

    let mut swap_events = Vec::new();
    let mut swap_effective_spread_total_sats = 0_i64;
    let mut swap_effective_spread_total_cents = 0_i64;

    for round in 1..=probe.rounds {
        println!("blink_stablesats_sa_probe round={} start", round);

        let operator_before = lookup_balances(&balances, "operator")?.clone();
        let operator_swap = run_wallet_swap_execute(
            &script_paths,
            wallets[0].env.as_slice(),
            "btc-to-usd",
            probe.convert_btc_sats,
            "sats",
            Some(&format!(
                "{memo_prefix}:round-{round:02}:operator-btc-to-usd"
            )),
        )?;
        if probe.require_success && !operator_swap.status.eq_ignore_ascii_case("SUCCESS") {
            bail!(
                "operator btc-to-usd status={} (require_success=true)",
                operator_swap.status
            );
        }
        balances = load_all_wallet_balances(wallets.as_slice(), &script_paths)?;
        let operator_after = lookup_balances(&balances, "operator")?.clone();
        let (observed_btc_delta, observed_usd_delta) =
            wallet_deltas(&operator_before, &operator_after);
        if probe.require_success
            && (observed_btc_delta != operator_swap.script_btc_delta_sats
                || observed_usd_delta != operator_swap.script_usd_delta_cents)
        {
            bail!(
                "operator btc-to-usd post-balance delta mismatch script=({},{}) observed=({},{})",
                operator_swap.script_btc_delta_sats,
                operator_swap.script_usd_delta_cents,
                observed_btc_delta,
                observed_usd_delta
            );
        }
        let effective_spread_units = compute_effective_swap_spread_units(
            "btc-to-usd",
            operator_swap.quote_amount_out,
            observed_btc_delta,
            observed_usd_delta,
        )?;
        swap_effective_spread_total_cents =
            swap_effective_spread_total_cents.saturating_add(effective_spread_units);
        swap_events.push(serde_json::json!({
            "round": round,
            "owner": "operator",
            "direction": "btc-to-usd",
            "amount_requested": probe.convert_btc_sats,
            "unit": "sats",
            "status": operator_swap.status,
            "quote_id": operator_swap.quote_id,
            "quote_amount_in": operator_swap.quote_amount_in,
            "quote_amount_out": operator_swap.quote_amount_out,
            "script_btc_delta_sats": operator_swap.script_btc_delta_sats,
            "script_usd_delta_cents": operator_swap.script_usd_delta_cents,
            "observed_btc_delta_sats": observed_btc_delta,
            "observed_usd_delta_cents": observed_usd_delta,
            "effective_spread_cents": effective_spread_units,
            "transaction_id": operator_swap.transaction_id,
        }));

        let operator_before = lookup_balances(&balances, "operator")?.clone();
        if operator_before.usd_balance_cents < probe.convert_usd_cents {
            bail!(
                "operator USD balance {} cents below round {} requirement {} cents for usd-to-btc step",
                operator_before.usd_balance_cents,
                round,
                probe.convert_usd_cents
            );
        }
        let operator_usd_swap = run_wallet_swap_execute(
            &script_paths,
            wallets[0].env.as_slice(),
            "usd-to-btc",
            probe.convert_usd_cents,
            "cents",
            Some(&format!(
                "{memo_prefix}:round-{round:02}:operator-usd-to-btc"
            )),
        )?;
        if probe.require_success && !operator_usd_swap.status.eq_ignore_ascii_case("SUCCESS") {
            bail!(
                "operator usd-to-btc status={} (require_success=true)",
                operator_usd_swap.status
            );
        }
        balances = load_all_wallet_balances(wallets.as_slice(), &script_paths)?;
        let operator_after = lookup_balances(&balances, "operator")?.clone();
        let (observed_btc_delta, observed_usd_delta) =
            wallet_deltas(&operator_before, &operator_after);
        if probe.require_success
            && (observed_btc_delta != operator_usd_swap.script_btc_delta_sats
                || observed_usd_delta != operator_usd_swap.script_usd_delta_cents)
        {
            bail!(
                "operator usd-to-btc post-balance delta mismatch script=({},{}) observed=({},{})",
                operator_usd_swap.script_btc_delta_sats,
                operator_usd_swap.script_usd_delta_cents,
                observed_btc_delta,
                observed_usd_delta
            );
        }
        let effective_spread_units = compute_effective_swap_spread_units(
            "usd-to-btc",
            operator_usd_swap.quote_amount_out,
            observed_btc_delta,
            observed_usd_delta,
        )?;
        swap_effective_spread_total_sats =
            swap_effective_spread_total_sats.saturating_add(effective_spread_units);
        swap_events.push(serde_json::json!({
            "round": round,
            "owner": "operator",
            "direction": "usd-to-btc",
            "amount_requested": probe.convert_usd_cents,
            "unit": "cents",
            "status": operator_usd_swap.status,
            "quote_id": operator_usd_swap.quote_id,
            "quote_amount_in": operator_usd_swap.quote_amount_in,
            "quote_amount_out": operator_usd_swap.quote_amount_out,
            "script_btc_delta_sats": operator_usd_swap.script_btc_delta_sats,
            "script_usd_delta_cents": operator_usd_swap.script_usd_delta_cents,
            "observed_btc_delta_sats": observed_btc_delta,
            "observed_usd_delta_cents": observed_usd_delta,
            "effective_spread_sats": effective_spread_units,
            "transaction_id": operator_usd_swap.transaction_id,
        }));
    }

    let final_balances = balances;
    let (initial_total_btc_sats, initial_total_usd_cents) =
        aggregate_wallet_balances(&initial_balances);
    let (final_total_btc_sats, final_total_usd_cents) = aggregate_wallet_balances(&final_balances);

    let report = serde_json::json!({
        "event": "blink_stablesats_sa_demo",
        "status": "completed",
        "rounds": probe.rounds,
        "wallet_mode": "single_wallet",
        "configuration": {
            "convert_btc_sats": probe.convert_btc_sats,
            "convert_usd_cents": probe.convert_usd_cents,
            "require_success": probe.require_success,
            "memo_prefix": memo_prefix,
        },
        "balances": {
            "initial": balances_map_to_json(&initial_balances),
            "final": balances_map_to_json(&final_balances),
            "aggregate": {
                "initial_total_btc_sats": initial_total_btc_sats,
                "initial_total_usd_cents": initial_total_usd_cents,
                "final_total_btc_sats": final_total_btc_sats,
                "final_total_usd_cents": final_total_usd_cents,
                "delta_btc_sats": signed_delta_u64(final_total_btc_sats, initial_total_btc_sats),
                "delta_usd_cents": signed_delta_u64(final_total_usd_cents, initial_total_usd_cents),
            },
        },
        "operations": {
            "swaps": swap_events,
        },
        "fees_and_spread": {
            "swap_effective_spread_total_sats": swap_effective_spread_total_sats,
            "swap_effective_spread_total_cents": swap_effective_spread_total_cents,
        },
    });
    println!(
        "blink_stablesats_sa_probe report={}",
        serde_json::to_string_pretty(&report)
            .context("failed to serialize blink stablesats sa report")?
    );
    Ok(())
}

fn run_blink_swap_live_probe(cwd: &std::path::Path, probe: &BlinkSwapProbeArgs) -> Result<()> {
    let direction = probe.normalized_direction()?;
    let unit = probe.resolved_unit()?;
    let scripts = BlinkScriptPaths::resolve(cwd)?;
    let quote_script = scripts.swap_quote.clone();
    let execute_script = scripts.swap_execute.clone();

    let blink_env = resolve_blink_env()?;
    println!(
        "blink_swap_probe quote script={} direction={} amount={} unit={}",
        quote_script.display(),
        direction,
        probe.amount,
        unit
    );
    let quote_args = vec![
        direction.to_string(),
        probe.amount.to_string(),
        "--unit".to_string(),
        unit.to_string(),
        "--ttl-seconds".to_string(),
        "45".to_string(),
    ];
    let quote_output = run_node_script(
        quote_script.as_path(),
        quote_args.as_slice(),
        blink_env.as_slice(),
    )?;
    let quote_json = parse_script_json(&quote_output, "blink swap quote")?;
    let quote_event = quote_json
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    if !quote_event.eq_ignore_ascii_case("swap_quote") {
        bail!(
            "blink swap quote returned unexpected event '{}'",
            quote_event
        );
    }
    let quote_id = quote_json
        .pointer("/quote/quoteId")
        .and_then(Value::as_str)
        .unwrap_or("<missing>");
    let amount_in = quote_json
        .pointer("/quote/amountIn/value")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let amount_out = quote_json
        .pointer("/quote/amountOut/value")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    println!(
        "blink_swap_probe quote_ok quote_id={} amount_in={} amount_out={}",
        quote_id, amount_in, amount_out
    );

    if !probe.execute_live {
        println!("blink_swap_probe execute skipped (--blink-swap-execute-live not set)");
        return Ok(());
    }

    let mut execute_args = vec![
        direction.to_string(),
        probe.amount.to_string(),
        "--unit".to_string(),
        unit.to_string(),
    ];
    if let Some(memo) = probe
        .memo
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        execute_args.push("--memo".to_string());
        execute_args.push(memo.to_string());
    }
    println!(
        "blink_swap_probe execute script={} direction={} amount={} unit={}",
        execute_script.display(),
        direction,
        probe.amount,
        unit
    );
    let execute_output = run_node_script(
        execute_script.as_path(),
        execute_args.as_slice(),
        blink_env.as_slice(),
    )?;

    if execute_output.success {
        let execute_json = parse_script_json(&execute_output, "blink swap execute")?;
        let execute_event = execute_json
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or("<missing>");
        if !execute_event.eq_ignore_ascii_case("swap_execution") {
            bail!(
                "blink swap execute returned unexpected event '{}'",
                execute_event
            );
        }
        let status = execute_json
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("<missing>");
        println!(
            "blink_swap_probe execute_ok status={} tx={}",
            status,
            execute_json
                .pointer("/execution/transactionId")
                .and_then(Value::as_str)
                .unwrap_or("<none>")
        );
        if probe.require_execute_success && !status.eq_ignore_ascii_case("SUCCESS") {
            bail!(
                "blink swap execute status was '{}' but --blink-swap-require-success is set",
                status
            );
        }
        return Ok(());
    }

    let failure_text = if !execute_output.stderr.is_empty() {
        execute_output.stderr.as_str()
    } else {
        execute_output.stdout.as_str()
    };
    println!(
        "blink_swap_probe execute_live_failure exit_code={:?} details={}",
        execute_output.exit_code,
        summarize_text_for_log(failure_text, 240)
    );
    if probe.require_execute_success {
        bail!(
            "blink swap execute failed but --blink-swap-require-success is set: {}",
            summarize_text_for_log(failure_text, 240)
        );
    }
    if !(failure_text.contains("Swap failed") || failure_text.contains("INVALID_INPUT")) {
        bail!(
            "blink swap execute failed with unexpected error: {}",
            summarize_text_for_log(failure_text, 240)
        );
    }
    Ok(())
}

fn usage_text() -> String {
    [
        "Usage:",
        "  cargo run -p autopilot-desktop --bin codex-live-harness -- [options]",
        "",
        "Options:",
        "  --cwd <path>              Workspace cwd to send to app-server",
        "  --model <id>              Explicit model override; default is live model/list default",
        "  --prompt <text>           Optional prompt to send after new thread starts",
        "  --skill <name>            Optional skill to attach in turn/start",
        "  --list-limit <n>          thread/list limit (default: 20)",
        "  --drain-ms <n>            Idle settle period for channel drains (default: 700)",
        "  --timeout-ms <n>          Max wait per channel drain phase (default: 4000)",
        "  --max-events <n>          Max notifications/requests to print per phase (default: 24)",
        "  --include-writes          Include write/mutation probes (imports, exports, reloads)",
        "  --skip-experimental       Skip experimental method probes",
        "  --skip-thread-mutations   Skip thread mutation probes",
        "  --disable-openagents-dynamic-tools  Do not attach OpenAgents dynamic tools on thread/start",
        "  --require-cad-tool-call   Fail unless post-send captures CAD item/tool/call request",
        "  --allow-echo-replies      Do not fail when assistant reply exactly matches prompt",
        "  --blink-swap-live         Run live Blink BTC<->USD swap quote probe (real network)",
        "  --blink-swap-direction <d>  Swap direction: btc-to-usd or usd-to-btc (default: btc-to-usd)",
        "  --blink-swap-amount <n>   Probe swap amount (default: 1)",
        "  --blink-swap-unit <u>     Probe unit: sats or cents (default derives from direction)",
        "  --blink-swap-execute-live Attempt real Blink settlement after quote probe",
        "  --blink-swap-require-success  Fail unless execute status is SUCCESS",
        "  --blink-swap-memo <text>  Optional memo for execute probe",
        "  --blink-stablesats-sa-live  Run real single-wallet StableSats conversion scenario",
        "  --blink-stablesats-sa-rounds <n>  Number of scenario rounds (default: 1)",
        "  --blink-stablesats-sa-convert-btc-sats <n>  BTC->USD convert size per round (default: 6000)",
        "  --blink-stablesats-sa-convert-usd-cents <n>  USD->BTC convert size per round (default: 50)",
        "  --blink-stablesats-sa-require-success  Fail if any swap step is non-SUCCESS (default: true)",
        "  --blink-stablesats-sa-memo-prefix <text>  Memo prefix used for scenario swaps",
        "  --help                    Show this help",
    ]
    .join("\n")
}

fn print_usage() {
    println!("{}", usage_text());
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_client::{SkillScope, SkillsListEntry};

    fn mock_skill(name: &str, path: &str, enabled: bool) -> SkillMetadata {
        SkillMetadata {
            name: name.to_string(),
            description: format!("{name} description"),
            interface: None,
            dependencies: None,
            short_description: None,
            path: PathBuf::from(path),
            scope: SkillScope::Repo,
            enabled,
        }
    }

    #[test]
    fn finds_skill_by_name_and_path_segment() {
        let list = SkillsListResponse {
            data: vec![SkillsListEntry {
                cwd: PathBuf::from("/repo"),
                skills: vec![
                    mock_skill("mezo", "/repo/skills/mezo/SKILL.md", true),
                    mock_skill("blink", "/repo/skills/blink/SKILL.md", false),
                ],
                errors: Vec::new(),
            }],
        };

        let by_name =
            find_skill_in_list(Some(&list), "blink").expect("expected to find blink by name");
        assert_eq!(by_name.name, "blink");

        let by_dir = find_skill_in_list(Some(&list), "skills/blink")
            .expect("expected to find blink by path segment");
        assert_eq!(by_dir.name, "blink");
    }

    #[test]
    fn finds_remote_skill_by_name_and_id() {
        let remote = SkillsRemoteReadResponse {
            data: vec![
                RemoteSkillSummary {
                    id: "hazelnut-1".to_string(),
                    name: "blink".to_string(),
                    description: "Blink payment helper".to_string(),
                },
                RemoteSkillSummary {
                    id: "hazelnut-2".to_string(),
                    name: "other".to_string(),
                    description: "something else".to_string(),
                },
            ],
        };

        let by_name = find_remote_skill(Some(&remote), "blink")
            .expect("expected to find remote skill by name");
        assert_eq!(by_name.id, "hazelnut-1");

        let by_id = find_remote_skill(Some(&remote), "hazelnut-2")
            .expect("expected to find remote skill by id");
        assert_eq!(by_id.name, "other");
    }

    #[test]
    fn normalizes_blink_swap_direction_and_unit_aliases() {
        assert_eq!(normalize_blink_direction("btc_to_usd"), Some("btc-to-usd"));
        assert_eq!(normalize_blink_direction("buy-btc"), Some("usd-to-btc"));
        assert_eq!(normalize_blink_unit("sat"), Some("sats"));
        assert_eq!(normalize_blink_unit("usd-cents"), Some("cents"));
        assert_eq!(normalize_blink_unit("bogus"), None);
    }

    #[test]
    fn parses_wallet_balance_payload_contract() {
        let payload = serde_json::json!({
            "btcWalletId": "btc-wallet-123",
            "usdWalletId": "usd-wallet-abc",
            "btcBalanceSats": "4200",
            "usdBalanceCents": 875
        });
        let parsed = parse_wallet_balance_payload("operator", &payload)
            .expect("wallet payload should parse");
        assert_eq!(parsed.owner, "operator");
        assert_eq!(parsed.btc_wallet_id, "btc-wallet-123");
        assert_eq!(parsed.usd_wallet_id, "usd-wallet-abc");
        assert_eq!(parsed.btc_balance_sats, 4200);
        assert_eq!(parsed.usd_balance_cents, 875);
    }

    fn sample_balances(
        operator_btc_sats: u64,
        operator_usd_cents: u64,
    ) -> std::collections::BTreeMap<String, BlinkWalletBalances> {
        let mut balances = std::collections::BTreeMap::new();
        balances.insert(
            "operator".to_string(),
            BlinkWalletBalances {
                owner: "operator",
                btc_wallet_id: "operator-btc".to_string(),
                usd_wallet_id: "operator-usd".to_string(),
                btc_balance_sats: operator_btc_sats,
                usd_balance_cents: operator_usd_cents,
            },
        );
        balances
    }

    #[test]
    fn stablesats_funding_deficits_report_shortfalls_for_operator_wallet() {
        let probe = BlinkStableSatsSaProbeArgs {
            rounds: 2,
            convert_btc_sats: 6_000,
            convert_usd_cents: 50,
            require_success: true,
            memo_prefix: None,
        };
        let balances = sample_balances(10_000, 80);
        let deficits =
            stable_sats_funding_deficits(&probe, &balances).expect("deficits should compute");
        assert_eq!(deficits.len(), 2);
        assert_eq!(deficits[0].owner, "operator");
        assert_eq!(deficits[0].asset, BlinkTransferAsset::BtcSats);
        assert_eq!(deficits[0].required, 12_240);
        assert_eq!(deficits[0].available, 10_000);
        assert_eq!(deficits[0].shortfall, 2_240);
        assert_eq!(deficits[1].owner, "operator");
        assert_eq!(deficits[1].asset, BlinkTransferAsset::UsdCents);
        assert_eq!(deficits[1].required, 140);
        assert_eq!(deficits[1].available, 80);
        assert_eq!(deficits[1].shortfall, 60);
    }

    #[test]
    fn stablesats_funding_deficits_empty_when_balances_cover_plan() {
        let probe = BlinkStableSatsSaProbeArgs {
            rounds: 1,
            convert_btc_sats: 6_000,
            convert_usd_cents: 50,
            require_success: true,
            memo_prefix: None,
        };
        let balances = sample_balances(6_120, 70);
        let deficits =
            stable_sats_funding_deficits(&probe, &balances).expect("deficits should compute");
        assert!(deficits.is_empty());
    }

    #[test]
    fn computes_effective_swap_spread_by_direction() {
        let btc_to_usd = compute_effective_swap_spread_units("btc-to-usd", 78, -500, 77)
            .expect("btc-to-usd spread should compute");
        assert_eq!(btc_to_usd, 1);

        let usd_to_btc = compute_effective_swap_spread_units("usd-to-btc", 121, 120, -90)
            .expect("usd-to-btc spread should compute");
        assert_eq!(usd_to_btc, 1);
    }
}
