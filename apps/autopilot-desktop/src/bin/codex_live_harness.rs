use std::future::Future;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, anyhow, bail};
use codex_client::{
    AppServerChannels, AppServerClient, AppServerConfig, AppServerNotification, AppServerRequest,
    AppServerRequestId, AppsListParams, ClientInfo, CollaborationModeListParams, CommandExecParams,
    ConfigReadParams, ExperimentalFeatureListParams, ExternalAgentConfigDetectParams,
    ExternalAgentConfigImportParams, FuzzyFileSearchSessionStartParams,
    FuzzyFileSearchSessionStopParams, FuzzyFileSearchSessionUpdateParams, GetAccountParams,
    HazelnutScope, InitializeCapabilities, InitializeParams, ListMcpServerStatusParams,
    ModelListParams, ProductSurface, ReviewDelivery, ReviewStartParams, ReviewTarget,
    SkillsListExtraRootsForCwd, SkillsListParams, SkillsRemoteReadParams, SkillsRemoteWriteParams,
    ThreadArchiveParams, ThreadBackgroundTerminalsCleanParams, ThreadCompactStartParams,
    ThreadForkParams, ThreadListParams, ThreadLoadedListParams, ThreadReadParams,
    ThreadRealtimeAppendTextParams, ThreadRealtimeStartParams, ThreadRealtimeStopParams,
    ThreadRollbackParams, ThreadSetNameParams, ThreadSnapshot, ThreadSortKey, ThreadStartParams,
    ThreadUnarchiveParams, TurnStartParams, WindowsSandboxSetupStartParams,
};
use serde_json::Value;

#[derive(Debug)]
struct HarnessArgs {
    cwd: PathBuf,
    model_override: Option<String>,
    prompt: Option<String>,
    list_limit: u32,
    drain_ms: u64,
    timeout_ms: u64,
    max_events: usize,
    include_writes: bool,
    include_experimental: bool,
    include_thread_mutations: bool,
}

impl Default for HarnessArgs {
    fn default() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            model_override: None,
            prompt: None,
            list_limit: 20,
            drain_ms: 700,
            timeout_ms: 4_000,
            max_events: 24,
            include_writes: false,
            include_experimental: true,
            include_thread_mutations: true,
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
                "--help" | "-h" => {
                    print_usage();
                    std::process::exit(0);
                }
                _ => {
                    bail!("unknown argument '{}'\n\n{}", flag, usage_text());
                }
            }
        }

        Ok(args)
    }
}

#[derive(Default)]
struct ChannelEventBatch {
    notifications: Vec<String>,
    notification_methods: Vec<String>,
    requests: Vec<String>,
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
        "include_writes={} include_experimental={} include_thread_mutations={}",
        args.include_writes, args.include_experimental, args.include_thread_mutations
    );
    println!();

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
                opt_out_notification_methods: None,
            }),
        })
        .await
        .context("codex initialize failed")?;

    drain_and_print("post-initialize", &mut channels, &args).await;

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
    let skills_list = run_probe(
        "skills/list",
        &mut channels,
        &args,
        client.skills_list(SkillsListParams {
            cwds: vec![args.cwd.clone()],
            force_reload: true,
            per_cwd_extra_user_roots: if extra_skills_root.exists() {
                Some(vec![SkillsListExtraRootsForCwd {
                    cwd: args.cwd.clone(),
                    extra_user_roots: vec![extra_skills_root.clone()],
                }])
            } else {
                None
            },
        }),
        |response| {
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
        },
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
                    client.skills_config_write(codex_client::SkillsConfigWriteParams {
                        path,
                        enabled,
                    }),
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
    drain_and_print("post-refresh", &mut channels, &args).await;

    println!();
    println!("simulate_click chat.new_thread -> thread/start");
    let started = client
        .thread_start(ThreadStartParams {
            model: resolved_model.clone(),
            model_provider: None,
            cwd: Some(args.cwd.display().to_string()),
            approval_policy: None,
            sandbox: None,
        })
        .await
        .context("thread/start failed")?;
    let new_thread_id = started.thread.id;
    println!("thread/start new_thread_id={new_thread_id}");

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
    drain_and_print("post-new-chat", &mut channels, &args).await;

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
        let turn = client
            .turn_start(TurnStartParams {
                thread_id: new_thread_id.clone(),
                input: vec![codex_client::UserInput::Text {
                    text: prompt.clone(),
                    text_elements: Vec::new(),
                }],
                cwd: None,
                approval_policy: None,
                sandbox_policy: None,
                model: resolved_model.clone(),
                effort: None,
                summary: None,
                personality: None,
                output_schema: None,
                collaboration_mode: None,
            })
            .await
            .context("turn/start failed")?;
        println!("turn/start turn_id={}", turn.turn.id);
        let post_send_events = collect_channel_events(
            &mut channels,
            Duration::from_millis(args.drain_ms),
            Duration::from_millis(args.timeout_ms),
        )
        .await;
        if !post_send_events
            .notification_methods
            .iter()
            .any(|method| is_agent_response_signal(method))
        {
            bail!(
                "post-send notifications for thread {} did not include any agent response signal; expected streaming or completion events",
                new_thread_id
            );
        }
        print_events("post-send", post_send_events, args.max_events);

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
                cwd: Some(args.cwd.display().to_string()),
                approval_policy: None,
                sandbox: None,
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
            sandbox_policy: None,
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
    let Some(model_list) = model_list else {
        return None;
    };
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
    drain_and_print(&format!("post-{label}"), channels, args).await;
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

fn is_agent_response_signal(method: &str) -> bool {
    matches!(
        method,
        "item/agentMessage/delta"
            | "item/assistantMessage/delta"
            | "agent_message/delta"
            | "item/agentMessage/completed"
            | "item/assistantMessage/completed"
            | "codex/event/agent_message_content_delta"
            | "codex/event/agent_message_delta"
            | "codex/event/agent_message"
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

async fn drain_and_print(label: &str, channels: &mut AppServerChannels, args: &HarnessArgs) {
    let events = collect_channel_events(
        channels,
        Duration::from_millis(args.drain_ms),
        Duration::from_millis(args.timeout_ms),
    )
    .await;
    print_events(label, events, args.max_events);
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
                batch.notifications.push(notification_summary(&notification));
                last_event = Instant::now();
            }
            maybe = channels.requests.recv() => {
                let Some(request) = maybe else {
                    break;
                };
                batch.requests.push(request_summary(&request));
                last_event = Instant::now();
            }
            _ = tokio::time::sleep(sleep_for) => {}
        }
    }

    batch
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

fn print_events(label: &str, events: ChannelEventBatch, max_events: usize) {
    println!(
        "{} events: notifications={} requests={}",
        label,
        events.notifications.len(),
        events.requests.len()
    );
    let mut notification_lines = events.notifications;
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
    let mut request_lines = events.requests;
    let request_omitted = request_lines.len().saturating_sub(max_events);
    request_lines.truncate(max_events);
    for line in request_lines {
        println!("  {line}");
    }
    if request_omitted > 0 {
        println!("  ... {} additional requests omitted", request_omitted);
    }
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
        "  --list-limit <n>          thread/list limit (default: 20)",
        "  --drain-ms <n>            Idle settle period for channel drains (default: 700)",
        "  --timeout-ms <n>          Max wait per channel drain phase (default: 4000)",
        "  --max-events <n>          Max notifications/requests to print per phase (default: 24)",
        "  --include-writes          Include write/mutation probes (imports, exports, reloads)",
        "  --skip-experimental       Skip experimental method probes",
        "  --skip-thread-mutations   Skip thread mutation probes",
        "  --help                    Show this help",
    ]
    .join("\n")
}

fn print_usage() {
    println!("{}", usage_text());
}
