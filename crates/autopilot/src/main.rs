//! Autopilot CLI - Run autonomous tasks with Claude and log trajectories

use anyhow::Result;
use clap::Parser;
use claude_agent_sdk::{
    QueryOptions, SdkMessage, SettingSource, query,
    HookCallback, HookCallbackMatcher, HookEvent, HookInput, HookOutput,
    SyncHookOutput, unstable_v2_create_session,
};
use async_trait::async_trait;
use colored::*;
use futures::StreamExt;
use serde_json::json;
use std::io::{self, Write};
use std::path::PathBuf;

/// Print and flush stdout immediately (for piped output)
macro_rules! println_flush {
    ($($arg:tt)*) => {{
        println!($($arg)*);
        let _ = io::stdout().flush();
    }};
}

use autopilot::apm::APMTier;
use autopilot::analyze;
use autopilot::cli::{
    Cli, Commands, IssueCommands, ProjectCommands, SessionCommands,
    DirectiveCommands, MetricsCommands, ApmCommands, AlertCommands,
    BaselineCommands, LogsCommands,
};
use autopilot::lockfile::{
    check_and_handle_stale_lockfile, cleanup_lockfile, cleanup_mcp_json,
    setup_cleanup_handlers, write_lockfile,
    MCP_JSON_PATH,
};
use autopilot::memory::{check_memory, check_and_kill_memory_hogs, format_bytes, min_available_memory_bytes};
use autopilot::replay;
use autopilot::rlog::RlogWriter;
use autopilot::timestamp::{date_dir, filename, generate_slug};
use autopilot::trajectory::{StepType, Trajectory};
use autopilot::{extract_session_id_from_json, extract_session_id_from_rlog};
use autopilot::TrajectoryCollector;
use autopilot::trajectory_publisher::{TrajectoryPublishConfig, TrajectorySessionPublisher};
use autopilot::nip_sa_trajectory::TrajectoryPublisher as NipSaTrajectoryPublisher;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup cleanup handlers for signals and panics
    setup_cleanup_handlers();

    let cli = Cli::parse();

    match cli.command {
        Commands::Run {
            prompt,
            project,
            cwd,
            agent,
            model,
            max_turns,
            max_budget,
            output_dir,
            slug,
            dry_run,
            verbose,
            with_issues,
            issues_db,
            full_auto,
            ui,
            no_apm,
            publish_trajectory,
        } => {
            run_task(
                prompt, project, cwd, agent, model, max_turns, max_budget, output_dir, slug, dry_run, verbose,
                with_issues, issues_db, full_auto, ui, no_apm, publish_trajectory,
            )
            .await
        }
        Commands::Replay { trajectory, mode } => {
            replay_trajectory(trajectory, mode).await
        }
        Commands::Compare { trajectory1, trajectory2 } => {
            compare_trajectories(trajectory1, trajectory2).await
        }
        Commands::Analyze { path, aggregate, json } => {
            analyze_trajectories(path, aggregate, json).await
        }
        Commands::Resume {
            trajectory,
            continue_last,
            cwd,
            prompt,
            max_budget,
            with_issues,
            issues_db,
        } => {
            resume_task(
                trajectory, continue_last, cwd, prompt, max_budget, with_issues, issues_db,
            )
            .await
        }
        Commands::Issue { command } => {
            handle_issue_command(command).await
        }
        Commands::Project { command } => {
            handle_project_command(command).await
        }
        Commands::Session { command } => {
            handle_session_command(command).await
        }
        Commands::Directive { command } => {
            handle_directive_command(command).await
        }
        Commands::Metrics { command } => {
            handle_metrics_command(command).await
        }
        Commands::Apm { command } => {
            handle_apm_command(command).await
        }
        Commands::Benchmark {
            benchmark_id,
            category,
            baseline,
            save_baseline,
            list_baselines,
            compare_commits,
            compare_db1,
            compare_db2,
            threshold,
            db,
            workspace,
        } => {
            handle_benchmark_command(
                benchmark_id,
                category,
                baseline,
                save_baseline,
                list_baselines,
                compare_commits,
                compare_db1,
                compare_db2,
                threshold,
                db,
                workspace,
            )
            .await
        }
        Commands::Logs { command } => handle_logs_command(command).await,
        Commands::Notify {
            title,
            message,
            severity,
            webhook,
            config,
            metadata,
        } => {
            handle_notify_command(title, message, severity, webhook, config, metadata).await
        }
    }
}

/// Map friendly model names to full model IDs
fn resolve_model(model: &str) -> String {
    match model.to_lowercase().as_str() {
        "sonnet" => "claude-sonnet-4-5-20250929".to_string(),
        "opus" => "claude-opus-4-5-20251101".to_string(),
        "haiku" => "claude-haiku-4-20250514".to_string(),
        // If not a friendly name, assume it's a full model ID
        _ => model.to_string(),
    }
}

/// Compaction hook to provide custom instructions
struct CompactionHook;

#[async_trait]
impl HookCallback for CompactionHook {
    async fn call(
        &self,
        input: HookInput,
        _tool_use_id: Option<String>,
    ) -> Result<HookOutput, claude_agent_sdk::Error> {
        if let HookInput::PreCompact(compact_input) = input {
            // Detect appropriate strategy based on session (simple heuristic for now)
            let strategy = if compact_input.base.session_id.contains("auto") {
                autopilot::compaction::CompactionStrategy::Autonomous
            } else {
                autopilot::compaction::CompactionStrategy::Detailed
            };

            let custom_instructions = autopilot::compaction::generate_compaction_prompt(
                strategy,
                compact_input.custom_instructions.as_deref(),
            );

            eprintln!("🔄 Compaction triggered ({})",
                if matches!(compact_input.trigger, claude_agent_sdk::CompactTrigger::Auto) {
                    "auto"
                } else {
                    "manual"
                });
            eprintln!("📝 Using strategy: {}", strategy.as_str());

            return Ok(HookOutput::Sync(SyncHookOutput {
                decision: Some(claude_agent_sdk::HookDecision::Approve),
                system_message: Some(custom_instructions),
                hook_specific_output: None,
                ..Default::default()
            }));
        }

        Ok(HookOutput::Sync(SyncHookOutput {
            decision: Some(claude_agent_sdk::HookDecision::Approve),
            system_message: None,
            hook_specific_output: None,
            ..Default::default()
        }))
    }
}

/// Plan mode hook to enforce restrictions
struct PlanModeHook;

#[async_trait]
impl HookCallback for PlanModeHook {
    async fn call(
        &self,
        input: HookInput,
        _tool_use_id: Option<String>,
    ) -> Result<HookOutput, claude_agent_sdk::Error> {
        match input {
            HookInput::PreToolUse(pre) => {
                // Check if this tool is allowed in plan mode
                if let Err(reason) = autopilot::planmode::is_tool_allowed_in_plan_mode(
                    &pre.tool_name,
                    &pre.tool_input,
                ) {
                    // Block the tool
                    return Ok(SyncHookOutput::block(&reason).into());
                }
                // Allow the tool
                Ok(SyncHookOutput::continue_execution().into())
            }
            _ => Ok(SyncHookOutput::continue_execution().into()),
        }
    }
}

/// Load active directives and format as a summary for the prompt
fn load_directive_summary(cwd: &std::path::Path) -> String {
    use issues::directive;

    let directives_dir = cwd.join(".openagents/directives");
    let directives = match directive::get_active_directives(&directives_dir) {
        Ok(d) => d,
        Err(_) => return String::new(),
    };

    if directives.is_empty() {
        return String::new();
    }

    let mut summary = String::from("\n\nACTIVE DIRECTIVES (high-level goals guiding your work):\n");
    for d in &directives {
        // Extract first line of Goal section if present
        let goal_line = d.body.lines()
            .skip_while(|l| !l.starts_with("## Goal"))
            .nth(2)  // Skip "## Goal" and blank line
            .unwrap_or("")
            .trim();

        summary.push_str(&format!(
            "\n[{}] {} (priority: {})\n  {}\n",
            d.id, d.title, d.priority.as_str(),
            if goal_line.is_empty() { "See directive for details" } else { goal_line }
        ));
    }
    summary.push_str("\nUse directive_get <id> for full details, success criteria, and phases.\n");
    summary.push_str("When creating issues, link them with: issue_create title=\"...\" directive_id=\"<id>\"\n");

    summary
}

/// Full auto mode prompt suffix (without directives - those are added dynamically)
const FULL_AUTO_PROMPT_BASE: &str = r#"

FULL AUTO MODE - CRITICAL AUTONOMOUS LOOP INSTRUCTIONS:

You are in FULLY AUTONOMOUS mode. You MUST follow this exact loop:

LOOP START:
1. Call issue_ready to get the next available issue
2. If issue exists:
   - Check if the issue is linked to a directive (has directive_id)
   - If linked, review that directive with directive_get to understand the bigger picture
   - Implement → test → commit → PUSH → complete
3. IMMEDIATELY call issue_ready again (NO SUMMARIES, NO PAUSES)
4. GOTO LOOP START

IF issue_ready returns "No ready issues available":
- Review the active directives shown above
- Pick the highest priority directive that needs work
- Create 1-3 specific, actionable issues linked to it using:
  issue_create title="..." directive_id="<id>"
- Claim and implement the new issue
- GOTO LOOP START

CRITICAL RULES - VIOLATION MEANS FAILURE:
- NEVER output a "session summary" or "issues completed" message
- NEVER stop to reflect on progress
- NEVER say "I've completed X issues" - this is a STOP SIGNAL, do NOT emit it
- After EVERY issue_complete, your VERY NEXT action MUST be issue_ready
- The ONLY valid reasons to stop: budget exhausted error, max turns error, or system crash
- If you output text without immediately calling issue_ready after completing an issue, YOU HAVE FAILED

IMPORTANT: Always `git push origin main` after each commit.

This is an INFINITE LOOP. You run until the system stops you. DO NOT STOP YOURSELF.
"#;

/// Run Claude agent with the given prompt
async fn run_claude_agent(
    prompt: &str,
    options: QueryOptions,
    collector: &mut TrajectoryCollector,
    verbose: bool,
    ui_port: Option<u16>,
) -> Result<()> {
    let mut stream = query(prompt, options).await?;

    while let Some(msg) = stream.next().await {
        let msg = msg?;

        // Collect trajectory
        collector.process_message(&msg);

        // Stream to desktop UI if enabled
        if let Some(port) = ui_port {
            if let Some(html) = autopilot::ui_renderer::render_sdk_message(&msg) {
                let _ = stream_to_desktop(port, html.into_string()).await;
            }
        }

        // Print progress
        if verbose {
            print_message(&msg);
        } else {
            print_progress(&msg);
        }
    }

    Ok(())
}

/// Full-auto loop: keeps running until budget exhausted
/// If agent stops prematurely, we detect it and force continuation
async fn run_full_auto_loop(
    initial_prompt: &str,
    options: QueryOptions,
    collector: &mut TrajectoryCollector,
    verbose: bool,
    ui_port: Option<u16>,
    cwd: &PathBuf,
    issues_db: Option<&PathBuf>,
) -> Result<()> {
    use issues::{db, issue};

    let mut continuation_count = 0;
    const MAX_CONTINUATIONS: u32 = 1000; // Safety limit
    let mut current_prompt = initial_prompt.to_string();

    loop {
        // Check memory at start of each iteration
        let (available_mem, needs_cleanup, is_critical) = check_memory();

        if needs_cleanup || is_critical {
            println!("\n{} Memory {} ({}) - checking for processes to kill...",
                "MEMORY:".yellow().bold(),
                if is_critical { "critical" } else { "low" },
                format_bytes(available_mem));

            // Try to free memory by killing hogs with retry
            let mut new_avail = check_and_kill_memory_hogs();

            // If still critical after cleanup, wait and retry a few times
            // macOS can take a moment to reclaim memory
            if new_avail < min_available_memory_bytes() {
                for retry in 1..=3 {
                    println!("{} Waiting for memory to be reclaimed (attempt {}/3)...",
                        "MEM:".yellow().bold(), retry);
                    std::thread::sleep(std::time::Duration::from_secs(2));

                    let (new_check, _, still_critical) = check_memory();
                    new_avail = new_check;

                    if !still_critical {
                        println!("{} Memory recovered to {} - continuing",
                            "MEMORY:".green().bold(), format_bytes(new_avail));
                        break;
                    }
                }
            }

            // Final check after all retries
            if new_avail < min_available_memory_bytes() {
                println!("\n{} Still insufficient memory ({}) after cleanup - aborting",
                    "MEMORY:".red().bold(),
                    format_bytes(new_avail));
                anyhow::bail!("Insufficient memory: {} available, {} required",
                    format_bytes(new_avail),
                    format_bytes(min_available_memory_bytes()));
            } else {
                println!("{} Memory recovered to {} - continuing", "MEMORY:".green().bold(), format_bytes(new_avail));
            }
        }

        // Log memory status periodically (every iteration or every 5)
        if continuation_count % 5 == 0 {
            println!("{} Available memory: {}", "MEM:".dimmed(), format_bytes(available_mem));
        }

        // Use query() directly - same approach as run_claude_agent which works
        // For continuations, set continue_session=true to resume conversation
        let mut query_options = options.clone();
        if continuation_count > 0 {
            query_options.continue_session = true;
        }

        let mut stream = query(&current_prompt, query_options).await?;

        // Process messages until stream ends
        let mut budget_exhausted = false;
        let mut max_turns_reached = false;
        let mut message_count = 0;

        while let Some(msg) = stream.next().await {
            message_count += 1;

            // Check memory every 10 messages
            if message_count % 10 == 0 {
                let (avail, needs_cleanup, is_critical) = check_memory();
                if message_count % 100 == 0 {
                    println!("{} Memory: {}", "MEM:".dimmed(), format_bytes(avail));
                }
                if is_critical {
                    println!("\n{} Memory critical ({}) - attempting cleanup", "MEMORY:".yellow().bold(), format_bytes(avail));
                    let mut new_avail = check_and_kill_memory_hogs();

                    // Retry a couple times for memory to be reclaimed
                    if new_avail < min_available_memory_bytes() {
                        for _ in 1..=2 {
                            std::thread::sleep(std::time::Duration::from_secs(2));
                            let (check, _, still_critical) = check_memory();
                            new_avail = check;
                            if !still_critical { break; }
                        }
                    }

                    if new_avail < min_available_memory_bytes() {
                        anyhow::bail!("Memory critical after cleanup: {} available", format_bytes(new_avail));
                    }
                } else if needs_cleanup && message_count % 50 == 0 {
                    // Proactive cleanup when memory is getting low (not critical)
                    println!("{} Memory getting low ({}) - proactive cleanup", "MEM:".yellow(), format_bytes(avail));
                    check_and_kill_memory_hogs();
                }
            }
            let msg = msg?;
            collector.process_message(&msg);

            // Check if this is a result message indicating session end
            if let SdkMessage::Result(ref result) = msg {
                // Check for budget/turns exhaustion based on result type
                match result {
                    claude_agent_sdk::SdkResultMessage::ErrorMaxBudget(_) => {
                        budget_exhausted = true;
                    }
                    claude_agent_sdk::SdkResultMessage::ErrorMaxTurns(_) => {
                        max_turns_reached = true;
                    }
                    claude_agent_sdk::SdkResultMessage::ErrorDuringExecution(e) => {
                        // Check error messages for budget/turn related errors
                        for err in &e.errors {
                            let err_lower = err.to_lowercase();
                            if err_lower.contains("budget") || err_lower.contains("cost") {
                                budget_exhausted = true;
                            }
                            if err_lower.contains("turn") || err_lower.contains("max_turn") {
                                max_turns_reached = true;
                            }
                        }
                    }
                    claude_agent_sdk::SdkResultMessage::Success(_) => {
                        // Success means the agent decided to stop - we may need to continue
                    }
                    _ => {}
                }
            }

            // Stream to UI
            if let Some(port) = ui_port {
                if let Some(html) = autopilot::ui_renderer::render_sdk_message(&msg) {
                    let _ = stream_to_desktop(port, html.into_string()).await;
                }
            }

            // Print progress
            if verbose {
                print_message(&msg);
            } else {
                print_progress(&msg);
            }
        }

        // If budget or turns exhausted, we're done
        if budget_exhausted {
            println!("\n{} Budget exhausted - stopping full-auto loop", "STOP:".red().bold());
            break;
        }
        if max_turns_reached {
            println!("\n{} Max turns reached - stopping full-auto loop", "STOP:".red().bold());
            break;
        }

        // Safety limit
        continuation_count += 1;
        if continuation_count >= MAX_CONTINUATIONS {
            println!("\n{} Max continuations ({}) reached", "STOP:".yellow().bold(), MAX_CONTINUATIONS);
            break;
        }

        // Check if there are more issues to work on
        let default_db = autopilot::default_db_path();
        let db_path = issues_db.unwrap_or(&default_db);

        let has_more_work = if let Ok(conn) = db::init_db(db_path) {
            issue::get_next_ready_issue(&conn, Some("claude"))?.is_some()
        } else {
            false
        };

        if !has_more_work {
            // No more issues - but in full-auto we should create new work
            println!("\n{} No ready issues - sending continuation to create work", "AUTO:".cyan().bold());
        } else {
            println!("\n{} Issues still available - forcing continuation", "AUTO:".cyan().bold());
        }

        // Set up for continuation - include directive summary and FULL_AUTO_PROMPT again
        println!("{} Continuing with new query (attempt {})", "AUTO:".yellow().bold(), continuation_count);

        let directive_summary = load_directive_summary(&cwd);
        current_prompt = if has_more_work {
            format!("{}{}\n\nCONTINUE: You stopped prematurely. There are still issues to work on. Call issue_ready NOW. DO NOT output any text first - immediately call issue_ready.", directive_summary, FULL_AUTO_PROMPT_BASE)
        } else {
            format!("{}{}\n\nCONTINUE: You stopped prematurely. No issues are ready. Review the directives above and create a new issue linked to one. DO NOT output any text first.", directive_summary, FULL_AUTO_PROMPT_BASE)
        };
    }

    Ok(())
}

/// Run Codex agent with the given prompt
async fn run_codex_agent(
    prompt: &str,
    cwd: &PathBuf,
    _max_turns: u32,
    _max_budget: f64,
    _collector: &mut TrajectoryCollector,
    verbose: bool,
) -> Result<()> {
    use codex_agent_sdk::{Codex, SandboxMode, ThreadOptions, TurnOptions};

    let codex = Codex::new();
    let thread_options = ThreadOptions {
        working_directory: Some(cwd.clone()),
        sandbox_mode: Some(SandboxMode::WorkspaceWrite),
        ..Default::default()
    };

    let mut thread = codex.start_thread(thread_options);
    let mut streamed = thread.run_streamed(prompt, TurnOptions::default()).await?;

    let mut turn_items = Vec::new();
    let mut usage = None;

    while let Some(event_result) = streamed.next().await {
        let event = event_result?;

        // Add to trajectory collector
        _collector.process_codex_event(&event);

        // Process events for console output
        match &event {
            codex_agent_sdk::ThreadEvent::ThreadStarted(e) => {
                if verbose {
                    println!("{} Thread started: {}", "Codex:".cyan().bold(), e.thread_id);
                }
            }
            codex_agent_sdk::ThreadEvent::TurnStarted(_) => {
                if verbose {
                    println!("{} Turn started", "Codex:".dimmed());
                }
            }
            codex_agent_sdk::ThreadEvent::ItemStarted(e) => {
                if verbose {
                    println!("{} Item started: {:?}", "Codex:".dimmed(), e.item.details);
                }
            }
            codex_agent_sdk::ThreadEvent::ItemUpdated(_) => {
                // Progress updates
            }
            codex_agent_sdk::ThreadEvent::ItemCompleted(e) => {
                turn_items.push(e.item.clone());

                use codex_agent_sdk::ThreadItemDetails;
                match &e.item.details {
                    ThreadItemDetails::AgentMessage(msg) => {
                        if verbose {
                            println!("{} {}", "Agent:".cyan().bold(), msg.text);
                        }
                    }
                    ThreadItemDetails::CommandExecution(cmd) => {
                        println!("{} Executing: {}", "Command:".yellow().bold(), cmd.command);
                        if verbose && !cmd.aggregated_output.is_empty() {
                            println!("{}", cmd.aggregated_output);
                        }
                    }
                    ThreadItemDetails::FileChange(file) => {
                        println!("{} {} file(s) changed", "File change:".green().bold(), file.changes.len());
                        if verbose {
                            for change in &file.changes {
                                println!("  {}", change.path);
                            }
                        }
                    }
                    ThreadItemDetails::Reasoning(reasoning) => {
                        if verbose {
                            println!("{} {}", "Reasoning:".magenta().dimmed(), reasoning.text);
                        }
                    }
                    _ => {}
                }
            }
            codex_agent_sdk::ThreadEvent::TurnCompleted(e) => {
                usage = Some(e.usage.clone());
                if verbose {
                    println!("{} Turn completed", "Codex:".green().bold());
                    println!("  Input tokens: {}", e.usage.input_tokens);
                    println!("  Output tokens: {}", e.usage.output_tokens);
                }
            }
            codex_agent_sdk::ThreadEvent::TurnFailed(e) => {
                eprintln!("{} Turn failed: {}", "Error:".red().bold(), e.error.message);
                anyhow::bail!("Codex turn failed: {}", e.error.message);
            }
            codex_agent_sdk::ThreadEvent::Error(e) => {
                eprintln!("{} {}", "Error:".red().bold(), e.message);
                anyhow::bail!("Codex error: {}", e.message);
            }
        }
    }

    // Add summary to trajectory collector
    // Note: TrajectoryCollector expects SdkMessage format, but for now we can add a simple result
    // This would need proper adapter in the future for full Codex trajectory support
    if let Some(usage) = usage {
        // Add usage tracking
        // For now, just print - full trajectory integration would need TrajectoryEvent adapter
        println!("{} Total tokens: {}", "Usage:".dimmed(),
            usage.input_tokens + usage.output_tokens);
    }

    Ok(())
}

/// Publish trajectory to Nostr relays
async fn publish_trajectory_to_nostr(
    trajectory: &Trajectory,
    session_id: Option<&String>,
) -> Result<()> {
    use anyhow::Context;
    use bip39::Mnemonic;
    use nostr::TrajectoryVisibility;
    use std::str::FromStr;
    use std::sync::Arc;
    use wallet::core::UnifiedIdentity;
    use wallet::storage::config::WalletConfig;
    use wallet::storage::keychain::SecureKeychain;

    // Load wallet config to get relay URLs
    let config = WalletConfig::load()?;

    if config.nostr.relays.is_empty() {
        anyhow::bail!("No Nostr relays configured in wallet.toml");
    }

    // Try to load identity from keychain
    let identity = if SecureKeychain::has_mnemonic() {
        let mnemonic_str = SecureKeychain::retrieve_mnemonic()?;
        let mnemonic = Mnemonic::from_str(&mnemonic_str)?;
        Arc::new(UnifiedIdentity::from_mnemonic(mnemonic)?)
    } else {
        eprintln!("{} No wallet identity found. Run 'openagents wallet init' first.", "Warning:".yellow());
        anyhow::bail!("No wallet identity found");
    };

    // Create trajectory publish config
    let publish_config = TrajectoryPublishConfig::new(config.nostr.relays.clone());

    // Publish TrajectorySession (kind:38030)
    let tick_id = session_id
        .map(|s| s.clone())
        .unwrap_or_else(|| trajectory.session_id.clone());

    let started_at = trajectory.started_at.timestamp() as u64;

    let session_publisher = TrajectorySessionPublisher::with_identity(publish_config, identity.clone());

    println!("{} Publishing trajectory session to Nostr relays...", "Publishing:".cyan());

    match session_publisher
        .publish_session(&trajectory.session_id, &tick_id, &trajectory.model, started_at)
        .await
    {
        Ok(Some(event_id)) => {
            println!("{} Trajectory session published: {}", "✓".green(), event_id);
        }
        Ok(None) => {
            eprintln!("{} Trajectory session publishing was skipped", "Warning:".yellow());
        }
        Err(e) => {
            eprintln!("{} Failed to publish trajectory session: {}", "Error:".red(), e);
            return Err(e);
        }
    }

    // Publish individual trajectory events (kind:38031)
    let nip_sa_publisher = NipSaTrajectoryPublisher::new(&trajectory.session_id, &tick_id);
    let events = nip_sa_publisher.trajectory_to_events(trajectory);

    // Create session with trajectory hash
    let _session_with_hash = nip_sa_publisher.create_session_with_hash(
        trajectory,
        &events,
        TrajectoryVisibility::Public,
    );

    println!("{} Publishing {} trajectory events...", "Publishing:".cyan(), events.len());

    // Publish each event to relays
    use nostr_client::{PoolConfig, RelayPool};
    let pool_config = PoolConfig::default();
    let pool = RelayPool::new(pool_config);

    // Connect to relays
    for relay_url in &config.nostr.relays {
        pool.add_relay(relay_url)
            .await
            .with_context(|| format!("Failed to add relay: {}", relay_url))?;
    }

    // Wait for connections
    tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

    let mut published_count = 0;

    for (i, trajectory_event) in events.iter().enumerate() {
        // Build Nostr event
        let tags = vec![
            vec!["session_id".to_string(), trajectory_event.session_id.clone()],
            vec!["tick_id".to_string(), trajectory_event.tick_id.clone()],
            vec!["sequence".to_string(), trajectory_event.sequence.to_string()],
        ];

        let content_json = trajectory_event
            .content
            .to_json()
            .with_context(|| format!("Failed to serialize trajectory event {}", i))?;

        let template = nostr::EventTemplate {
            created_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_secs(),
            kind: autopilot::nip_sa_trajectory::TrajectoryPublisher::event_kind(),
            tags,
            content: content_json,
        };

        // Sign event
        let event = identity
            .sign_event(template)
            .with_context(|| format!("Failed to sign trajectory event {}", i))?;

        // Publish to relays
        match pool.publish(&event).await {
            Ok(results) => {
                let success_count = results.iter().filter(|r| r.accepted).count();
                if success_count > 0 {
                    published_count += 1;
                }
            }
            Err(e) => {
                eprintln!("{} Failed to publish event {}: {}", "Warning:".yellow(), i, e);
            }
        }
    }

    // Disconnect from pool
    let _ = pool.disconnect_all().await;

    println!(
        "{} Published {}/{} trajectory events to Nostr relays",
        "✓".green(),
        published_count,
        events.len()
    );

    Ok(())
}

async fn run_task(
    prompt: String,
    project: Option<String>,
    cwd: Option<PathBuf>,
    agent: String,
    model: String,
    max_turns: u32,
    max_budget: f64,
    output_dir: Option<PathBuf>,
    slug: Option<String>,
    dry_run: bool,
    verbose: bool,
    with_issues: bool,
    issues_db: Option<PathBuf>,
    full_auto: bool,
    ui: bool,
    no_apm: bool,
    publish_trajectory: bool,
) -> Result<()> {
    // Load project if specified
    let (cwd, issues_db, project_id) = if let Some(project_name) = project {
        use issues::{db, project};

        let default_db = autopilot::default_db_path();
        let conn = db::init_db(&default_db)?;

        match project::get_project_by_name(&conn, &project_name)? {
            Some(proj) => {
                println_flush!("{} Loading project '{}'", "Project:".cyan().bold(), proj.name);
                println_flush!("{} {}", "Path:".dimmed(), proj.path);
                (
                    PathBuf::from(&proj.path),
                    Some(PathBuf::from(&proj.path).join("autopilot.db")),
                    Some(proj.id)
                )
            }
            None => {
                eprintln!("{} Project '{}' not found", "Error:".red(), project_name);
                eprintln!("Run `cargo autopilot project list` to see available projects");
                std::process::exit(1);
            }
        }
    } else {
        (cwd.unwrap_or_else(|| {
            std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
        }), issues_db, None)
    };

    // Create session record if we have a project
    let session_id = if let Some(ref proj_id) = project_id {
        use issues::{db, session};

        let default_db = autopilot::default_db_path();
        let db_path = issues_db.as_ref().unwrap_or(&default_db);
        let conn = db::init_db(db_path)?;

        let pid = std::process::id() as i32;
        let session = session::create_session(&conn, proj_id, &prompt, &model, Some(pid))?;

        println_flush!("{} Session ID: {}", "Session:".dimmed(), &session.id[..8]);
        Some(session.id)
    } else {
        None
    };

    // Check for stale lockfile and handle crash recovery
    check_and_handle_stale_lockfile(&cwd).await?;

    // Launch desktop UI if requested
    let _ui_port: Option<u16> = if ui {
        println_flush!("{} Launching desktop UI...", "UI:".cyan().bold());

        // Spawn desktop app as subprocess
        let mut child = std::process::Command::new("cargo")
            .args(["run", "--release", "-p", "desktop"])
            .current_dir(&cwd)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        // Wait for server to start by reading stdout for the port
        use std::io::{BufRead, BufReader};
        let port = if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);

            let mut port = None;
            for line in reader.lines().take(20).flatten() {
                // Look for "DESKTOP_PORT=PORT"
                if let Some(rest) = line.strip_prefix("DESKTOP_PORT=") {
                    if let Ok(p) = rest.trim().parse::<u16>() {
                        port = Some(p);
                        break;
                    }
                }
            }
            port
        } else {
            eprintln!("Warning: Failed to get stdout from desktop process");
            None
        };

        if let Some(p) = port {
            println_flush!("{} Desktop running at http://127.0.0.1:{}/autopilot", "UI:".cyan().bold(), p);
            // Open browser
            let _ = std::process::Command::new("open")
                .arg(format!("http://127.0.0.1:{}/autopilot", p))
                .spawn();
            Some(p)
        } else {
            eprintln!("{} Failed to detect desktop UI port", "Warning:".yellow());
            // Kill the child process
            let _ = child.kill();
            None
        }
    } else {
        None
    };

    // Resolve friendly model names to full model IDs
    let model = resolve_model(&model);

    // Get git info
    let repo_sha = get_git_sha(&cwd).unwrap_or_else(|_| "unknown".to_string());
    let branch = get_git_branch(&cwd).ok();

    // Generate slug
    let slug = slug.unwrap_or_else(|| generate_slug(&prompt));

    // Setup output directory
    let output_dir = output_dir.unwrap_or_else(|| PathBuf::from("docs/logs").join(date_dir()));

    // Enhance prompt for full-auto mode
    let prompt = if full_auto {
        let directive_summary = load_directive_summary(&cwd);
        format!("{}{}{}", prompt, directive_summary, FULL_AUTO_PROMPT_BASE)
    } else {
        prompt
    };

    println_flush!("{} {}", "Running:".cyan().bold(), prompt.lines().next().unwrap_or(&prompt));
    println_flush!("{} {}", "Model:".dimmed(), model);
    println_flush!("{} {}", "CWD:".dimmed(), cwd.display());
    if full_auto {
        println_flush!("{} {}", "Mode:".magenta().bold(), "FULL AUTO");
    }
    println_flush!();

    // Create trajectory collector
    let mut collector = TrajectoryCollector::new(
        prompt.clone(),
        model.clone(),
        cwd.display().to_string(),
        repo_sha,
        branch,
    );

    // Enable streaming rlog output (unless in dry-run mode)
    let rlog_path = if !dry_run {
        std::fs::create_dir_all(&output_dir)?;
        let rlog_path = output_dir.join(filename(&slug, "rlog"));
        if let Err(e) = collector.enable_streaming(&rlog_path) {
            eprintln!("Warning: Failed to enable rlog streaming: {}", e);
            None
        } else {
            println!("{} {} {}", "Streaming to:".dimmed(), rlog_path.display(), "(tail -f to watch)".dimmed());
            Some(rlog_path)
        }
    } else {
        None
    };

    // Enable JSONL streaming for full data capture (alongside rlog)
    if !dry_run {
        let jsonl_path = output_dir.join(filename(&slug, "jsonl"));
        if let Err(e) = collector.enable_jsonl_streaming(&jsonl_path) {
            eprintln!("Warning: Failed to enable JSONL streaming: {}", e);
        } else {
            println!("{} {} {}", "Full data:".dimmed(), jsonl_path.display(), "(APM source)".dimmed());
        }
    }

    // Enable JSON stdout for GUI consumption in full-auto mode
    if full_auto {
        collector.enable_json_stdout();
    }

    // Enable APM tracking (unless disabled)
    if !no_apm && !dry_run {
        let default_db = autopilot::default_db_path();
        let db_path = issues_db.as_ref().unwrap_or(&default_db);
        match collector.enable_apm_tracking(db_path, autopilot::apm::APMSource::Autopilot) {
            Ok(apm_session_id) => {
                println!("{} {} {}", "APM:".dimmed(), &apm_session_id[..apm_session_id.len().min(20)], "(tracking enabled)".dimmed());
            }
            Err(e) => {
                eprintln!("Warning: Failed to enable APM tracking: {}", e);
            }
        }
    }

    // Setup query options with hooks
    let plan_mode_hook = std::sync::Arc::new(PlanModeHook);
    let plan_hook_matcher = HookCallbackMatcher::new().hook(plan_mode_hook);

    let compaction_hook = std::sync::Arc::new(CompactionHook);
    let compact_hook_matcher = HookCallbackMatcher::new().hook(compaction_hook);

    let mut hooks = std::collections::HashMap::new();
    hooks.insert(HookEvent::PreToolUse, vec![plan_hook_matcher]);
    hooks.insert(HookEvent::PreCompact, vec![compact_hook_matcher]);

    let mut options = QueryOptions::new()
        .model(&model)
        .max_turns(max_turns)
        .cwd(&cwd)
        .setting_sources(vec![SettingSource::Project, SettingSource::User])
        .hooks(hooks)
        .dangerously_skip_permissions(true);

    // Only set budget constraint if explicitly specified (> 0)
    if max_budget > 0.0 {
        options = options.max_budget_usd(max_budget);
    }

    // Write .mcp.json file for issue tracking MCP server if requested
    if with_issues {
        let mcp_json_path = cwd.join(".mcp.json");
        let default_issues_db = autopilot::default_db_path();
        let db_path = issues_db
            .as_ref()
            .unwrap_or(&default_issues_db)
            .display()
            .to_string();

        println!("{} {}", "Issues DB:".dimmed(), db_path);

        // Build MCP server configuration
        let mcp_config = json!({
            "mcpServers": {
                "issues": {
                    "command": "cargo",
                    "args": ["run", "--release", "-p", "issues-mcp"],
                    "env": {
                        "ISSUES_DB": db_path
                    }
                }
            }
        });

        // Write .mcp.json file
        let json = serde_json::to_string_pretty(&mcp_config)
            .expect("Failed to serialize MCP config to JSON");
        std::fs::write(&mcp_json_path, json)?;
        println!("{} {}", "MCP config:".dimmed(), mcp_json_path.display());

        // Store path for cleanup on panic/signal
        MCP_JSON_PATH.set(mcp_json_path).ok();
    }

    // Write lockfile to track this run (for crash recovery)
    // Note: session_id will be written to the collector later and available in the rlog
    // For now we write basic info, issue_number would need to be passed as a parameter
    if let Err(e) = write_lockfile(None, None, rlog_path.clone()) {
        eprintln!("Warning: Failed to write lockfile: {}", e);
    }

    // Dispatch to appropriate agent
    // In full-auto mode, we loop and force continuation if the agent stops prematurely
    if full_auto && agent == "claude" {
        run_full_auto_loop(
            &prompt,
            options,
            &mut collector,
            verbose,
            _ui_port,
            &cwd,
            issues_db.as_ref(),
        )
        .await?;
    } else {
        match agent.as_str() {
            "claude" => {
                run_claude_agent(
                    &prompt,
                    options,
                    &mut collector,
                    verbose,
                    _ui_port,
                )
                .await?;
            }
            "codex" => {
                run_codex_agent(
                    &prompt,
                    &cwd,
                    max_turns,
                    max_budget,
                    &mut collector,
                    verbose,
                )
                .await?;
            }
            _ => {
                anyhow::bail!("Unknown agent: {}. Use 'claude' or 'codex'", agent);
            }
        }
    }

    let trajectory = collector.finish();

    println!();
    println!("{}", "=".repeat(60).dimmed());
    print_summary(&trajectory);

    // Extract and store metrics
    store_trajectory_metrics(&trajectory);

    // Save outputs
    if !dry_run {
        std::fs::create_dir_all(&output_dir)?;

        // Write .rlog
        let mut rlog_writer = RlogWriter::new();
        let rlog_content = rlog_writer.write(&trajectory);
        let rlog_path = output_dir.join(filename(&slug, "rlog"));
        std::fs::write(&rlog_path, &rlog_content)?;
        println!("{} {}", "Saved:".green(), rlog_path.display());

        // Write .json
        let json_content = trajectory.to_json();
        let json_path = output_dir.join(filename(&slug, "json"));
        std::fs::write(&json_path, &json_content)?;
        println!("{} {}", "Saved:".green(), json_path.display());

        // Print resume hints if session failed or was interrupted
        if let Some(ref result) = trajectory.result {
            let is_budget_error = result.errors.iter().any(|e| e.contains("budget") || e.contains("Budget"));
            let is_max_turns = result.errors.iter().any(|e| e.contains("max_turns") || e.contains("turns"));

            if !result.success && (is_budget_error || is_max_turns || !result.errors.is_empty()) {
                println!();
                println!("{}", "=".repeat(60).yellow());
                println!("{} Session interrupted", "⚠".yellow().bold());

                if is_budget_error {
                    println!("  Reason: Budget exhausted");
                } else if is_max_turns {
                    println!("  Reason: Max turns reached");
                } else if !result.errors.is_empty() {
                    println!("  Reason: {}", result.errors[0]);
                }

                println!();
                println!("{} To resume this session:", "→".cyan());
                println!("  {}", format!("autopilot resume {}", json_path.display()).cyan());
                println!("  or");
                println!("  {}", "autopilot resume --continue-last".cyan());
                println!("{}", "=".repeat(60).yellow());
            }
        }

        // Update session with trajectory path if we have a session
        if let Some(ref sess_id) = session_id {
            use issues::{db, session};
            let default_db = autopilot::default_db_path();
            let db_path = issues_db.as_ref().unwrap_or(&default_db);
            if let Ok(conn) = db::init_db(db_path) {
                let _ = session::update_session_trajectory(&conn, sess_id, &json_path.display().to_string());
            }
        }
    }

    // Update session status on completion
    if let Some(ref sess_id) = session_id {
        use issues::{db, session, SessionStatus};
        let default_db = autopilot::default_db_path();
        let db_path = issues_db.as_ref().unwrap_or(&default_db);
        if let Ok(conn) = db::init_db(db_path) {
            let status = if trajectory.result.as_ref().map(|r| r.success).unwrap_or(false) {
                SessionStatus::Completed
            } else {
                SessionStatus::Failed
            };
            let _ = session::update_session_status(&conn, sess_id, status);
            let issues_completed = trajectory.result.as_ref().map(|r| r.issues_completed as i32).unwrap_or(0);
            let _ = session::update_session_metrics(&conn, sess_id, trajectory.usage.cost_usd, issues_completed);
        }
    }

    // Publish trajectory to Nostr relays if enabled
    if publish_trajectory {
        if let Err(e) = publish_trajectory_to_nostr(&trajectory, session_id.as_ref()).await {
            eprintln!("{} Failed to publish trajectory: {}", "Warning:".yellow(), e);
        }
    }

    // Cleanup .mcp.json and lockfile on normal exit
    cleanup_mcp_json();
    cleanup_lockfile();

    Ok(())
}

fn print_message(msg: &SdkMessage) {
    match msg {
        SdkMessage::Assistant(a) => {
            // Parse content blocks
            if let Some(content) = a.message.get("content").and_then(|c| c.as_array()) {
                for block in content {
                    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match block_type {
                        "thinking" => {
                            let text = block
                                .get("thinking")
                                .and_then(|t| t.as_str())
                                .unwrap_or("");
                            println!(
                                "{} {}",
                                "THINK".yellow(),
                                truncate(text, 100)
                            );
                        }
                        "text" => {
                            let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            println!("{} {}", "ASST".green(), truncate(text, 100));
                        }
                        "tool_use" => {
                            let tool = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            println!("{} {}", "TOOL".blue(), tool);
                        }
                        _ => {}
                    }
                }
            }
        }
        SdkMessage::User(u) => {
            if let Some(content) = u.message.get("content") {
                match content {
                    serde_json::Value::String(s) => {
                        println!("{} {}", "USER".cyan(), truncate(s, 100));
                    }
                    serde_json::Value::Array(arr) => {
                        for block in arr {
                            if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                                let tool_id = block
                                    .get("tool_use_id")
                                    .and_then(|i| i.as_str())
                                    .unwrap_or("");
                                let is_error = block
                                    .get("is_error")
                                    .and_then(|e| e.as_bool())
                                    .unwrap_or(false);
                                let status = if is_error { "ERROR" } else { "OK" };
                                println!(
                                    "{} {} [{}]",
                                    "RSLT".magenta(),
                                    &tool_id[..tool_id.len().min(8)],
                                    status
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        SdkMessage::System(s) => println!("{} {:?}", "SYS ".yellow(), s),
        SdkMessage::Result(r) => println!("{} {:?}", "DONE".cyan().bold(), r),
        SdkMessage::ToolProgress(p) => {
            println!(
                "{} {} ({:.1}s)",
                "PROG".dimmed(),
                p.tool_name,
                p.elapsed_time_seconds
            );
        }
        _ => {}
    }
}

fn print_progress(msg: &SdkMessage) {
    match msg {
        SdkMessage::ToolProgress(p) => {
            println!(
                "  {} ({:.1}s)",
                "working...".yellow().dimmed(),
                p.elapsed_time_seconds
            );
        }
        SdkMessage::Result(_) => {
            println!("{}", "@end".green().bold());
        }
        SdkMessage::Assistant(a) => {
            if let Some(content) = a.message.get("content").and_then(|c| c.as_array()) {
                for block in content {
                    let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    match block_type {
                        "text" => {
                            // Show what the agent is saying (recorder-style)
                            let text = block.get("text").and_then(|t| t.as_str()).unwrap_or("");
                            let first_line = text.lines().next().unwrap_or("");
                            let truncated = if first_line.len() > 100 {
                                format!("{}...", &first_line[..97])
                            } else {
                                first_line.to_string()
                            };
                            if !truncated.is_empty() {
                                println!("{} {}", "a:".green(), truncated.dimmed());
                            }
                        }
                        "thinking" => {
                            // Show thinking (recorder-style)
                            let text = block.get("thinking").and_then(|t| t.as_str()).unwrap_or("");
                            let first_line = text.lines().next().unwrap_or("");
                            let truncated = if first_line.len() > 80 {
                                format!("{}...", &first_line[..77])
                            } else {
                                first_line.to_string()
                            };
                            if !truncated.is_empty() {
                                println!("{} {}", "th:".yellow(), truncated.dimmed());
                            }
                        }
                        "tool_use" => {
                            let tool = block.get("name").and_then(|n| n.as_str()).unwrap_or("");
                            let tool_id = block.get("id").and_then(|i| i.as_str()).unwrap_or("");
                            let input = block.get("input");

                            // Format tool args (same as rlog)
                            let args = match tool {
                                "Bash" => input
                                    .and_then(|i| i.get("command"))
                                    .and_then(|c| c.as_str())
                                    .map(|c| {
                                        let truncated = if c.len() > 50 { format!("{}...", &c[..47]) } else { c.to_string() };
                                        format!("cmd=\"{}\"", truncated)
                                    })
                                    .unwrap_or_default(),
                                "Read" | "Write" | "Edit" => input
                                    .and_then(|i| i.get("file_path"))
                                    .and_then(|p| p.as_str())
                                    .map(|p| format!("file_path={}", p))
                                    .unwrap_or_default(),
                                "Glob" => input
                                    .and_then(|i| i.get("pattern"))
                                    .and_then(|p| p.as_str())
                                    .map(|p| format!("pattern=\"{}\"", p))
                                    .unwrap_or_default(),
                                "Grep" => input
                                    .and_then(|i| i.get("pattern"))
                                    .and_then(|p| p.as_str())
                                    .map(|p| {
                                        let truncated = if p.len() > 30 { format!("{}...", &p[..27]) } else { p.to_string() };
                                        format!("pattern=\"{}\"", truncated)
                                    })
                                    .unwrap_or_default(),
                                "Task" => input
                                    .and_then(|i| i.get("description"))
                                    .and_then(|d| d.as_str())
                                    .map(|d| {
                                        let truncated = if d.len() > 40 { format!("{}...", &d[..37]) } else { d.to_string() };
                                        format!("desc=\"{}\"", truncated)
                                    })
                                    .unwrap_or_default(),
                                _ => String::new(),
                            };

                            // Get short tool ID (last 8 chars)
                            let id_short = if tool_id.len() > 8 {
                                &tool_id[tool_id.len() - 8..]
                            } else {
                                tool_id
                            };

                            // Print in recorder style: t!:ToolName id=xxx args → [running]
                            let args_str = if args.is_empty() {
                                String::new()
                            } else {
                                format!(" {}", args)
                            };
                            println!(
                                "{} {} {}{} {}",
                                "t!:".blue().bold(),
                                tool.cyan(),
                                format!("id={}", id_short).dimmed(),
                                args_str.dimmed(),
                                "→ [running]".yellow()
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        SdkMessage::User(u) => {
            // Show tool results in recorder style
            if let Some(content) = u.message.get("content") {
                if let serde_json::Value::Array(arr) = content {
                    for block in arr {
                        if block.get("type").and_then(|t| t.as_str()) == Some("tool_result") {
                            let tool_id = block
                                .get("tool_use_id")
                                .and_then(|i| i.as_str())
                                .unwrap_or("");
                            let is_error = block
                                .get("is_error")
                                .and_then(|e| e.as_bool())
                                .unwrap_or(false);

                            // Get short tool ID (last 8 chars)
                            let id_short = if tool_id.len() > 8 {
                                &tool_id[tool_id.len() - 8..]
                            } else {
                                tool_id
                            };

                            // Get output content
                            let output = block
                                .get("content")
                                .and_then(|c| {
                                    if let Some(s) = c.as_str() {
                                        Some(s.to_string())
                                    } else if let Some(arr) = c.as_array() {
                                        arr.first()
                                            .and_then(|b| b.get("text"))
                                            .and_then(|t| t.as_str())
                                            .map(|s| s.to_string())
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or_default();

                            let first_line = output.lines().next().unwrap_or("");
                            let truncated = if first_line.len() > 60 {
                                format!("{}...", &first_line[..57])
                            } else {
                                first_line.to_string()
                            };

                            // Print in recorder style: o: id=xxx → [ok]/[error] output
                            let status = if is_error {
                                "[error]".red()
                            } else {
                                "[ok]".green()
                            };

                            let output_str = if truncated.is_empty() {
                                String::new()
                            } else {
                                format!(" {}", truncated.dimmed())
                            };

                            println!(
                                "{} {} {} {}{}",
                                "o:".magenta(),
                                format!("id={}", id_short).dimmed(),
                                "→".dimmed(),
                                status,
                                output_str
                            );
                        }
                    }
                }
            }
        }
        _ => {}
    }
}

fn print_summary(traj: &Trajectory) {
    use autopilot::apm::APMTier;

    println!("{}", "Summary".cyan().bold());
    println!("  Session:  {}", traj.session_id);
    println!(
        "  Tokens:   {} in / {} out",
        traj.usage.input_tokens, traj.usage.output_tokens
    );
    println!("  Cached:   {}", traj.usage.cache_read_tokens);
    println!("  Cost:     ${:.4}", traj.usage.cost_usd);

    if let Some(ref result) = traj.result {
        println!("  Duration: {}ms", result.duration_ms);
        println!("  Turns:    {}", result.num_turns);
        println!(
            "  Success:  {}",
            if result.success {
                "yes".green()
            } else {
                "no".red()
            }
        );

        // Display APM if available
        if let Some(apm) = result.apm {
            let tier = APMTier::from_apm(apm);
            let colored_apm = match tier {
                APMTier::Elite => format!("{:.1}", apm).yellow().bold(),
                APMTier::HighPerformance => format!("{:.1}", apm).green().bold(),
                APMTier::Productive => format!("{:.1}", apm).green(),
                APMTier::Active => format!("{:.1}", apm).blue(),
                APMTier::Baseline => format!("{:.1}", apm).dimmed(),
            };
            println!("  APM:      {} ({})", colored_apm, tier.name().dimmed());
        }
    }

    // Count steps by type
    let mut tool_calls = 0;
    let mut thinking = 0;
    let mut assistant = 0;
    for step in &traj.steps {
        match &step.step_type {
            StepType::ToolCall { .. } => tool_calls += 1,
            StepType::Thinking { .. } => thinking += 1,
            StepType::Assistant { .. } => assistant += 1,
            _ => {}
        }
    }
    println!("  Steps:    {} total", traj.steps.len());
    println!(
        "            {} tool calls, {} thinking, {} responses",
        tool_calls, thinking, assistant
    );
}

/// Store trajectory metrics in the metrics database
fn store_trajectory_metrics(trajectory: &Trajectory) {
    use autopilot::metrics::{extract_metrics_from_trajectory, MetricsDb, default_db_path};

    match extract_metrics_from_trajectory(trajectory) {
        Ok((session_metrics, tool_call_metrics)) => {
            match MetricsDb::open(default_db_path()) {
                Ok(db) => {
                    // Store session metrics
                    if let Err(e) = db.store_session(&session_metrics) {
                        eprintln!("Warning: Failed to store session metrics: {}", e);
                        return;
                    }

                    // Store tool call metrics
                    let mut stored = 0;
                    let mut errors = 0;
                    for tool_call in &tool_call_metrics {
                        match db.store_tool_call(tool_call) {
                            Ok(_) => stored += 1,
                            Err(e) => {
                                eprintln!("Warning: Failed to store tool call: {}", e);
                                errors += 1;
                            }
                        }
                    }

                    println!(
                        "{}",
                        format!(
                            "✓ Stored metrics: {} tool calls ({} errors)",
                            stored, errors
                        )
                        .green()
                    );

                    // Detect and report anomalies (PostRun hook behavior)
                    match db.detect_anomalies(&session_metrics) {
                        Ok(anomalies) => {
                            if !anomalies.is_empty() {
                                println!("\n{}", "⚠ Anomalies Detected:".yellow().bold());
                                for anomaly in &anomalies {
                                    let severity_str = match anomaly.severity {
                                        autopilot::metrics::AnomalySeverity::Critical => "CRITICAL".red().bold(),
                                        autopilot::metrics::AnomalySeverity::Error => "ERROR".red(),
                                        autopilot::metrics::AnomalySeverity::Warning => "WARNING".yellow(),
                                    };
                                    println!(
                                        "  [{}] {}: expected {:.3}, got {:.3}",
                                        severity_str,
                                        anomaly.dimension,
                                        anomaly.expected_value,
                                        anomaly.actual_value
                                    );
                                }

                                // Store anomalies in database
                                for anomaly in &anomalies {
                                    if let Err(e) = db.store_anomaly(anomaly) {
                                        eprintln!("Warning: Failed to store anomaly: {}", e);
                                    }
                                }

                                // Auto-create issues from patterns (if threshold met)
                                let workdir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                                if let Err(e) = auto_create_issues_from_patterns(&db, &workdir) {
                                    eprintln!("Warning: Failed to auto-create issues from patterns: {}", e);
                                }
                            }
                        }
                        Err(e) => {
                            eprintln!("Warning: Failed to detect anomalies: {}", e);
                        }
                    }

                    // Evaluate configured alerts (PostRun hook - Phase 6)
                    if let Err(e) = evaluate_and_notify_alerts(&db, &session_metrics) {
                        eprintln!("Warning: Failed to evaluate alerts: {}", e);
                    }
                }
                Err(e) => {
                    eprintln!("Warning: Failed to open metrics database: {}", e);
                }
            }
        }
        Err(e) => {
            eprintln!("Warning: Failed to extract metrics from trajectory: {}", e);
        }
    }
}

/// Auto-create issues from detected patterns (PostRun hook)
fn auto_create_issues_from_patterns(metrics_db: &autopilot::metrics::MetricsDb, workdir: &PathBuf) -> Result<()> {
    use autopilot::auto_issues::{create_issues, detect_all_patterns, generate_issues};

    // Detect all patterns (both anomaly and tool error patterns)
    let patterns = detect_all_patterns(metrics_db)?;

    if patterns.is_empty() {
        return Ok(());
    }

    // Generate issues from patterns
    let improvement_issues = generate_issues(patterns);

    if improvement_issues.is_empty() {
        return Ok(());
    }

    // Find issues database
    let issues_db_path = workdir.join("autopilot.db");
    if !issues_db_path.exists() {
        // No issues database yet, skip auto-creation
        return Ok(());
    }

    // Create issues
    let issue_numbers = create_issues(&issues_db_path, &improvement_issues, metrics_db)?;

    if !issue_numbers.is_empty() {
        println!(
            "\n{} Auto-created {} improvement issues from detected patterns:",
            "🤖".cyan().bold(),
            issue_numbers.len()
        );
        for (issue, number) in improvement_issues.iter().zip(issue_numbers.iter()) {
            println!("  #{}: {} [{}]", number, issue.title, issue.priority);
        }
        println!();
    }

    Ok(())
}

/// Evaluate alert rules and send notifications for metric thresholds (PostRun hook - Phase 6)
fn evaluate_and_notify_alerts(
    _metrics_db: &autopilot::metrics::MetricsDb,
    session_metrics: &autopilot::metrics::SessionMetrics,
) -> Result<()> {
    use autopilot::alerts::{
        init_alerts_schema, add_default_alerts, evaluate_alerts,
        log_alert_to_stdout, log_alert_to_file,
    };
    use rusqlite::Connection;

    // Open metrics database connection for alerts
    let conn = Connection::open(autopilot::metrics::default_db_path())?;

    // Initialize alert schema and add defaults if needed
    init_alerts_schema(&conn)?;
    add_default_alerts(&conn)?;

    let session_id = &session_metrics.id;
    let mut all_alerts = Vec::new();

    // Calculate derived metrics
    let tool_error_rate = if session_metrics.tool_calls > 0 {
        session_metrics.tool_errors as f64 / session_metrics.tool_calls as f64
    } else {
        0.0
    };

    let task_completion_rate = if session_metrics.issues_claimed > 0 {
        session_metrics.issues_completed as f64 / session_metrics.issues_claimed as f64
    } else {
        1.0 // Default to 100% if no issues claimed (not applicable)
    };

    let tokens_per_task = if session_metrics.issues_completed > 0 {
        (session_metrics.tokens_in + session_metrics.tokens_out) as f64 / session_metrics.issues_completed as f64
    } else {
        0.0
    };

    // Evaluate key metrics that have alert rules
    let metrics_to_check = vec![
        ("tool_error_rate", tool_error_rate),
        ("task_completion_rate", task_completion_rate),
        ("tokens_per_task", tokens_per_task),
    ];

    for (metric_name, value) in metrics_to_check {
        match evaluate_alerts(&conn, session_id, metric_name, value) {
            Ok(mut alerts) => all_alerts.append(&mut alerts),
            Err(e) => {
                eprintln!("Warning: Failed to evaluate alerts for {}: {}", metric_name, e);
            }
        }
    }

    // Send notifications if alerts fired
    if !all_alerts.is_empty() {
        println!("\n{}", "🚨 Alerts Triggered:".red().bold());

        for alert in &all_alerts {
            // Log to stdout (always)
            log_alert_to_stdout(alert);

            // Log to file
            let log_path = std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join("alerts.log");
            if let Err(e) = log_alert_to_file(alert, &log_path) {
                eprintln!("Warning: Failed to write alert to file: {}", e);
            }
        }

        println!(); // Add spacing after alerts
    }

    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");
    if first_line.chars().count() <= max {
        first_line.to_string()
    } else {
        format!(
            "{}...",
            first_line.chars().take(max - 3).collect::<String>()
        )
    }
}

fn get_git_sha(cwd: &PathBuf) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(cwd)
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn get_git_branch(cwd: &PathBuf) -> Result<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(cwd)
        .output()?;
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Stream HTML fragment to desktop app /events endpoint
async fn stream_to_desktop(port: u16, html: String) -> Result<()> {
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/events", port);

    let _ = client
        .post(&url)
        .header("Content-Type", "text/html")
        .body(html)
        .send()
        .await;

    Ok(())
}

/// Resume a previous autopilot session
async fn resume_task(
    trajectory: Option<PathBuf>,
    continue_last: bool,
    cwd: Option<PathBuf>,
    prompt: Option<String>,
    max_budget: f64,
    with_issues: bool,
    issues_db: Option<PathBuf>,
) -> Result<()> {
    let cwd = cwd.unwrap_or_else(|| {
        std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
    });

    // Track original trajectory path for appending logs
    let original_trajectory_path = trajectory.clone();

    // Get session_id from trajectory file or use --continue
    let session_id = if continue_last {
        println!("{} Continuing most recent session...", "Resume:".cyan().bold());
        None
    } else {
        let path = trajectory.ok_or_else(|| anyhow::anyhow!("trajectory path required for resume"))?;
        println!("{} Loading session from {:?}", "Resume:".cyan().bold(), path);

        let id = if path.extension().and_then(|e| e.to_str()) == Some("json") {
            extract_session_id_from_json(&path)?
        } else {
            // Try rlog, fall back to error
            extract_session_id_from_rlog(&path)?.ok_or_else(|| {
                anyhow::anyhow!(
                    "No session_id in rlog header. Use --continue-last to resume most recent session."
                )
            })?
        };

        println!("{} session_id={}", "Resume:".dimmed(), &id[..id.len().min(8)]);
        Some(id)
    };

    // Build QueryOptions with resume
    let mut options = QueryOptions::new()
        .cwd(&cwd)
        .setting_sources(vec![SettingSource::Project, SettingSource::User])
        .dangerously_skip_permissions(true);

    // Only set budget constraint if explicitly specified (> 0)
    if max_budget > 0.0 {
        options = options.max_budget_usd(max_budget);
    }

    if let Some(ref id) = session_id {
        options.resume = Some(id.clone());
    } else {
        options.continue_session = true;
    }

    // Setup MCP for issue tracking if requested
    if with_issues {
        let mcp_json_path = cwd.join(".mcp.json");
        let db_path = issues_db
            .unwrap_or_else(|| autopilot::default_db_path())
            .display()
            .to_string();

        let mcp_config = json!({
            "mcpServers": {
                "issues": {
                    "command": "cargo",
                    "args": ["run", "--release", "-p", "issues-mcp"],
                    "env": {
                        "ISSUES_DB": db_path
                    }
                }
            }
        });

        let json = serde_json::to_string_pretty(&mcp_config)
            .expect("Failed to serialize MCP config to JSON");
        std::fs::write(&mcp_json_path, json)?;
        MCP_JSON_PATH.set(mcp_json_path).ok();
    }

    // Determine output paths - append to original files if resuming from a file
    let (rlog_path, json_path, jsonl_path) = if let Some(ref orig_path) = original_trajectory_path {
        // Use same directory and derive paths from original
        let parent = orig_path.parent().unwrap_or(std::path::Path::new("."));
        let stem = orig_path.file_stem().and_then(|s| s.to_str()).unwrap_or("resumed");
        (
            parent.join(format!("{}.rlog", stem)),
            parent.join(format!("{}.json", stem)),
            parent.join(format!("{}.jsonl", stem)),
        )
    } else {
        // Create new files in standard location for --continue-last
        let output_dir = PathBuf::from("docs/logs").join(date_dir());
        std::fs::create_dir_all(&output_dir)?;
        let slug = format!("resumed-{}", chrono::Utc::now().format("%H%M"));
        (
            output_dir.join(filename(&slug, "rlog")),
            output_dir.join(filename(&slug, "json")),
            output_dir.join(filename(&slug, "jsonl")),
        )
    };

    // Get git info
    let repo_sha = get_git_sha(&cwd).unwrap_or_else(|_| "unknown".to_string());
    let branch = get_git_branch(&cwd).ok();

    // Create trajectory collector for the resumed session
    let resume_prompt = prompt.clone().unwrap_or_else(|| "Continue from where you left off.".to_string());
    let mut collector = TrajectoryCollector::new(
        format!("[RESUMED] {}", resume_prompt),
        "resumed".to_string(), // model not known in resume
        cwd.display().to_string(),
        repo_sha,
        branch,
    );

    // Set session_id if we have it
    if let Some(ref id) = session_id {
        collector.set_session_id(id.clone());
    }

    // Enable streaming rlog output
    if let Err(e) = collector.enable_streaming(&rlog_path) {
        eprintln!("Warning: Failed to enable rlog streaming: {}", e);
    } else {
        println!("{} {} {}", "Streaming to:".dimmed(), rlog_path.display(), "(tail -f to watch)".dimmed());
    }

    // Enable JSONL streaming for full data capture
    if let Err(e) = collector.enable_jsonl_streaming(&jsonl_path) {
        eprintln!("Warning: Failed to enable JSONL streaming: {}", e);
    } else {
        println!("{} {} {}", "Full data:".dimmed(), jsonl_path.display(), "(APM source)".dimmed());
    }

    // Create session
    let mut session = unstable_v2_create_session(options).await?;

    // Send prompt if provided
    if let Some(p) = prompt {
        println!("{} Sending: {}", "Resume:".cyan().bold(), p);
        session.send(&p).await?;
    } else {
        // Send a continue message
        session.send("Continue from where you left off.").await?;
    }

    println!("{} Resumed, streaming...", "Resume:".green().bold());
    println!();

    // Process messages - collect trajectory AND print progress
    while let Some(msg) = session.receive().next().await {
        let msg = msg?;
        collector.process_message(&msg);
        print_progress(&msg);
    }

    let trajectory = collector.finish();

    println!();
    println!("{}", "=".repeat(60).dimmed());
    print_summary(&trajectory);

    // Extract and store metrics
    store_trajectory_metrics(&trajectory);

    // Save outputs
    // Write .rlog
    let mut rlog_writer = RlogWriter::new();
    let rlog_content = rlog_writer.write(&trajectory);
    std::fs::write(&rlog_path, &rlog_content)?;
    println!("{} {}", "Saved:".green(), rlog_path.display());

    // Write .json
    let json_content = trajectory.to_json();
    std::fs::write(&json_path, &json_content)?;
    println!("{} {}", "Saved:".green(), json_path.display());

    // Cleanup
    cleanup_mcp_json();
    cleanup_lockfile();

    println!();
    println!("{}", "Session ended.".green());

    Ok(())
}

async fn replay_trajectory(trajectory_path: PathBuf, mode: String) -> Result<()> {
    // Load trajectory
    let trajectory = replay::load_trajectory(&trajectory_path)?;

    // Run appropriate viewer based on mode
    match mode.as_str() {
        "interactive" | "i" => replay::interactive_replay(&trajectory)?,
        "list" | "l" => replay::list_steps(&trajectory)?,
        "summary" | "s" => replay::summary_view(&trajectory)?,
        _ => {
            eprintln!("Unknown mode: {}. Use interactive, list, or summary.", mode);
            std::process::exit(1);
        }
    }

    Ok(())
}

async fn compare_trajectories(trajectory1: PathBuf, trajectory2: PathBuf) -> Result<()> {
    replay::compare_trajectories(&trajectory1, &trajectory2)?;
    Ok(())
}

async fn analyze_trajectories(path: PathBuf, aggregate: bool, json_output: bool) -> Result<()> {
    if aggregate || path.is_dir() {
        // Aggregate mode: analyze all JSON files in directory
        let dir = if path.is_dir() {
            path
        } else {
            path.parent()
                .expect("Path should have a parent directory")
                .to_path_buf()
        };

        let mut analyses = Vec::new();

        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().map(|e| e == "json").unwrap_or(false) {
                match analyze::load_trajectory(&path) {
                    Ok(trajectory) => {
                        analyses.push(analyze::analyze_trajectory(&trajectory));
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to load {}: {}", path.display(), e);
                    }
                }
            }
        }

        if analyses.is_empty() {
            println!("No trajectory files found in {}", dir.display());
            return Ok(());
        }

        let aggregate_analysis = analyze::aggregate_analyses(&analyses);

        if json_output {
            println!("{}", serde_json::to_string_pretty(&aggregate_analysis)?);
        } else {
            analyze::print_aggregate(&aggregate_analysis);
        }
    } else {
        // Single file mode
        let trajectory = analyze::load_trajectory(&path)?;
        let analysis = analyze::analyze_trajectory(&trajectory);

        if json_output {
            println!("{}", serde_json::to_string_pretty(&analysis)?);
        } else {
            analyze::print_analysis(&analysis);
        }
    }

    Ok(())
}

async fn handle_session_command(command: SessionCommands) -> Result<()> {
    use issues::{db, project, session};

    let default_db = autopilot::default_db_path();

    match command {
        SessionCommands::List { project: proj_name, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            // Get project_id if project name is provided
            let project_id = if let Some(ref name) = proj_name {
                match project::get_project_by_name(&conn, name)? {
                    Some(p) => Some(p.id),
                    None => {
                        eprintln!("{} Project '{}' not found", "Error:".red(), name);
                        std::process::exit(1);
                    }
                }
            } else {
                None
            };

            let sessions = session::list_sessions(&conn, project_id.as_deref())?;

            if sessions.is_empty() {
                println!("No sessions found");
            } else {
                println!("{:<10} {:<10} {:<40} {:<10} {:<8}", "ID", "Status", "Prompt", "Budget", "Issues");
                println!("{}", "-".repeat(85));
                for s in sessions {
                    let id_short = if s.id.len() > 8 { &s.id[..8] } else { &s.id };
                    let prompt_short = if s.prompt.len() > 38 {
                        format!("{}...", &s.prompt[..35])
                    } else {
                        s.prompt.clone()
                    };
                    println!(
                        "{:<10} {:<10} {:<40} ${:<9.2} {}",
                        id_short,
                        s.status.as_str(),
                        prompt_short,
                        s.budget_spent,
                        s.issues_completed
                    );
                }
            }
        }
        SessionCommands::Show { id, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            // Try to find session by ID or prefix
            let sessions = session::list_sessions(&conn, None)?;
            let matching: Vec<_> = sessions.iter().filter(|s| s.id.starts_with(&id)).collect();

            match matching.len() {
                0 => {
                    eprintln!("{} Session '{}' not found", "Error:".red(), id);
                    std::process::exit(1);
                }
                1 => {
                    let s = matching[0];
                    println!("{} Session {}", "→".cyan(), &s.id[..8]);
                    println!("  Status:     {}", s.status.as_str());
                    println!("  Prompt:     {}", s.prompt);
                    println!("  Model:      {}", s.model);
                    if let Some(pid) = s.pid {
                        println!("  PID:        {}", pid);
                    }
                    println!("  Started:    {}", s.started_at);
                    if let Some(ref ended) = s.ended_at {
                        println!("  Ended:      {}", ended);
                    }
                    println!("  Budget:     ${:.4}", s.budget_spent);
                    println!("  Issues:     {}", s.issues_completed);
                    if let Some(ref path) = s.trajectory_path {
                        println!("  Trajectory: {}", path);
                    }
                }
                _ => {
                    eprintln!("{} Multiple sessions match '{}'. Please be more specific:", "Error:".yellow(), id);
                    for s in matching {
                        eprintln!("  {}", &s.id[..16]);
                    }
                    std::process::exit(1);
                }
            }
        }
    }

    Ok(())
}

async fn handle_project_command(command: ProjectCommands) -> Result<()> {
    use issues::{db, project, session};

    let default_db = autopilot::default_db_path();

    match command {
        ProjectCommands::Add {
            name,
            path,
            description,
            model,
            budget,
            db,
        } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            // Validate path exists
            if !path.exists() {
                eprintln!("{} Path does not exist: {}", "Error:".red(), path.display());
                std::process::exit(1);
            }

            let created = project::create_project(
                &conn,
                &name,
                &path.display().to_string(),
                description.as_deref(),
                model.as_deref(),
                budget,
            )?;

            println!(
                "{} Created project '{}'",
                "✓".green(),
                created.name
            );
            println!("  Path:   {}", created.path);
            if let Some(ref desc) = created.description {
                println!("  Desc:   {}", desc);
            }
            if let Some(ref m) = created.default_model {
                println!("  Model:  {}", m);
            }
            if let Some(b) = created.default_budget {
                println!("  Budget: ${}", b);
            }
        }
        ProjectCommands::List { db } => {
            use issues::{issue, Status};

            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let projects = project::list_projects(&conn)?;

            if projects.is_empty() {
                println!("No projects found");
                println!("\nCreate a project with:");
                println!("  cargo autopilot project add <name> --path <directory>");
            } else {
                println!("{:<20} {:<40} {:<12}", "Name", "Path", "Sessions");
                println!("{}", "-".repeat(75));

                let mut total_sessions = 0;

                for p in projects {
                    // Count sessions for this project
                    let sessions = session::list_sessions(&conn, Some(&p.id))?;
                    let session_count = sessions.len();
                    total_sessions += session_count;

                    println!(
                        "{:<20} {:<40} {:<12}",
                        p.name,
                        p.path,
                        session_count
                    );
                }

                // Count total issues (not per-project since issues don't have project_id)
                let all_issues = issue::list_issues(&conn, None)?;
                let total_open = all_issues.iter().filter(|i| i.status == Status::Open).count();
                let total_completed = all_issues.iter().filter(|i| i.status == Status::Done).count();

                // Print totals
                println!("{}", "-".repeat(75));
                println!(
                    "{:<20} {:<40} {:<12}",
                    "TOTAL",
                    format!("({} open, {} completed issues)", total_open, total_completed),
                    total_sessions
                );
            }
        }
        ProjectCommands::Remove { name, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            if let Some(p) = project::get_project_by_name(&conn, &name)? {
                if project::delete_project(&conn, &p.id)? {
                    println!("{} Removed project '{}'", "✓".green(), name);
                    println!("  Note: Project files remain at {}", p.path);
                } else {
                    eprintln!("{} Could not remove project '{}'", "✗".red(), name);
                }
            } else {
                eprintln!("{} Project '{}' not found", "✗".red(), name);
            }
        }
    }

    Ok(())
}

async fn handle_issue_command(command: IssueCommands) -> Result<()> {
    use issues::{db, issue, IssueType, Priority, Status};

    let default_db = autopilot::default_db_path();

    match command {
        IssueCommands::List { status, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let status_filter = status.as_deref().map(|s| match s {
                "open" => Status::Open,
                "in_progress" => Status::InProgress,
                "done" => Status::Done,
                _ => Status::Open,
            });

            let issues = issue::list_issues(&conn, status_filter)?;

            if issues.is_empty() {
                println!("No issues found");
            } else {
                println!("{:<6} {:<10} {:<8} {:<8} {:<50}", "Number", "Status", "Priority", "Agent", "Title");
                println!("{}", "-".repeat(90));
                for i in issues {
                    let status_str = i.status.as_str();
                    let blocked = if i.is_blocked { " [BLOCKED]" } else { "" };
                    println!(
                        "{:<6} {:<10} {:<8} {:<8} {}{}",
                        i.number,
                        status_str,
                        i.priority.as_str(),
                        i.agent,
                        i.title,
                        blocked
                    );
                }
            }
        }
        IssueCommands::ListAuto { status, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let status_filter = status.as_deref().map(|s| match s {
                "open" => Status::Open,
                "in_progress" => Status::InProgress,
                "done" => Status::Done,
                _ => Status::Open,
            });

            let issues = issue::list_auto_created_issues(&conn, status_filter)?;

            if issues.is_empty() {
                println!("No auto-created issues found");
            } else {
                println!("Auto-created issues (from anomaly detection):");
                println!();
                println!("{:<6} {:<10} {:<8} {:<8} {:<50}", "Number", "Status", "Priority", "Agent", "Title");
                println!("{}", "-".repeat(90));
                for i in issues {
                    let status_str = i.status.as_str();
                    let blocked = if i.is_blocked { " [BLOCKED]" } else { "" };
                    println!(
                        "{:<6} {:<10} {:<8} {:<8} {}{}",
                        i.number,
                        status_str,
                        i.priority.as_str(),
                        i.agent,
                        i.title,
                        blocked
                    );
                }
            }
        }
        IssueCommands::Create {
            title,
            description,
            priority,
            issue_type,
            agent,
            directive,
            db,
        } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let priority = Priority::from_str(&priority);
            let issue_type = IssueType::from_str(&issue_type);

            let created = issue::create_issue(&conn, &title, description.as_deref(), priority, issue_type, Some(&agent), directive.as_deref(), None)?;

            println!(
                "{} Created issue #{}: {} (agent: {})",
                "✓".green(),
                created.number,
                created.title,
                created.agent
            );
        }
        IssueCommands::Claim { number, run_id, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let run_id = run_id.unwrap_or_else(|| {
                format!("manual-{}", chrono::Utc::now().timestamp())
            });

            if let Some(i) = issue::get_issue_by_number(&conn, number)? {
                if issue::claim_issue(&conn, &i.id, &run_id)? {
                    println!("{} Claimed issue #{}: {}", "✓".green(), number, i.title);
                } else {
                    println!(
                        "{} Could not claim issue #{} (already claimed or blocked)",
                        "✗".red(),
                        number
                    );
                }
            } else {
                println!("{} Issue #{} not found", "✗".red(), number);
            }
        }
        IssueCommands::Complete { number, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            if let Some(i) = issue::get_issue_by_number(&conn, number)? {
                if issue::complete_issue(&conn, &i.id)? {
                    println!("{} Completed issue #{}: {}", "✓".green(), number, i.title);
                } else {
                    println!("{} Could not complete issue #{}", "✗".red(), number);
                }
            } else {
                println!("{} Issue #{} not found", "✗".red(), number);
            }
        }
        IssueCommands::Block { number, reason, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            if let Some(i) = issue::get_issue_by_number(&conn, number)? {
                if issue::block_issue(&conn, &i.id, &reason)? {
                    println!("{} Blocked issue #{}: {}", "✓".green(), number, reason);
                } else {
                    println!("{} Could not block issue #{}", "✗".red(), number);
                }
            } else {
                println!("{} Issue #{} not found", "✗".red(), number);
            }
        }
        IssueCommands::Release { stale_minutes, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let released = issue::release_stale_issues(&conn, stale_minutes)?;
            if released > 0 {
                println!("{} Released {} stale in_progress issue(s) claimed more than {} minutes ago",
                    "✓".green(), released, stale_minutes);
            } else {
                println!("No stale in_progress issues found");
            }
        }
        IssueCommands::Ready { agent, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            match issue::get_next_ready_issue(&conn, agent.as_deref())? {
                Some(i) => {
                    println!("{} Next ready issue:", "→".cyan());
                    println!("  Number:   #{}", i.number);
                    println!("  Title:    {}", i.title);
                    println!("  Priority: {}", i.priority.as_str());
                    println!("  Type:     {}", i.issue_type.as_str());
                    println!("  Agent:    {}", i.agent);
                    if let Some(ref desc) = i.description {
                        println!("  Description:");
                        for line in desc.lines() {
                            println!("    {}", line);
                        }
                    }
                }
                None => {
                    println!("No ready issues available");
                }
            }
        }
        IssueCommands::Export { output, include_completed, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            // Get all issues (filtering will be done during export)
            let issues = issue::list_issues(&conn, None)?;

            // Filter out completed issues if not requested
            let issues_to_export: Vec<_> = if include_completed {
                issues
            } else {
                issues.into_iter().filter(|i| i.status != Status::Done).collect()
            };

            // Serialize to JSON
            let json = serde_json::to_string_pretty(&issues_to_export)?;

            // Determine output path
            let output_path = output.unwrap_or_else(|| {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                cwd.join(".openagents").join("issues.json")
            });

            // Ensure parent directory exists
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            // Write JSON file
            std::fs::write(&output_path, json)?;

            println!("{} Exported {} issues to {}",
                "✓".green(),
                issues_to_export.len(),
                output_path.display()
            );
        }
        IssueCommands::Import { input, force, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            // Determine input path
            let input_path = input.unwrap_or_else(|| {
                let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
                cwd.join(".openagents").join("issues.json")
            });

            // Check if file exists
            if !input_path.exists() {
                eprintln!("{} File not found: {}", "✗".red(), input_path.display());
                std::process::exit(1);
            }

            // Read and parse JSON
            let json = std::fs::read_to_string(&input_path)?;
            let imported_issues: Vec<issue::Issue> = serde_json::from_str(&json)?;

            let mut imported = 0;
            let mut skipped = 0;
            let mut updated = 0;

            for imported_issue in imported_issues {
                // Check if issue with same UUID already exists
                if let Some(_existing) = issue::get_issue_by_id(&conn, &imported_issue.id)? {
                    if force {
                        // Update existing issue
                        issue::update_issue(
                            &conn,
                            &imported_issue.id,
                            Some(&imported_issue.title),
                            imported_issue.description.as_deref(),
                            Some(imported_issue.priority),
                            Some(imported_issue.issue_type),
                        )?;
                        updated += 1;
                    } else {
                        // Skip - UUID already exists
                        skipped += 1;
                    }
                } else {
                    // Insert new issue - need to preserve all fields including number
                    // We need to use raw SQL since create_issue() generates new UUIDs and numbers
                    let now = chrono::Utc::now().to_rfc3339();
                    let sql = r#"
                        INSERT INTO issues (
                            id, number, title, description, status, priority, issue_type, agent,
                            is_blocked, blocked_reason, claimed_by, claimed_at,
                            created_at, updated_at, completed_at, directive_id, project_id
                        )
                        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17)
                    "#;

                    // Build params vec to work with execute() method on Connection
                    conn.execute(
                        sql,
                        &[
                            &imported_issue.id as &str,
                            &imported_issue.number.to_string() as &str,
                            &imported_issue.title as &str,
                            &imported_issue.description.as_deref().unwrap_or("") as &str,
                            imported_issue.status.as_str(),
                            imported_issue.priority.as_str(),
                            imported_issue.issue_type.as_str(),
                            &imported_issue.agent as &str,
                            &if imported_issue.is_blocked { "1" } else { "0" },
                            &imported_issue.blocked_reason.as_deref().unwrap_or("") as &str,
                            &imported_issue.claimed_by.as_deref().unwrap_or("") as &str,
                            &imported_issue.claimed_at.map(|dt| dt.to_rfc3339()).unwrap_or_default() as &str,
                            &imported_issue.created_at.to_rfc3339() as &str,
                            &now as &str,
                            &imported_issue.completed_at.map(|dt| dt.to_rfc3339()).unwrap_or_default() as &str,
                            &imported_issue.directive_id.as_deref().unwrap_or("") as &str,
                            &imported_issue.project_id.as_deref().unwrap_or("") as &str,
                        ],
                    ).map_err(|e| anyhow::anyhow!("Failed to insert issue: {}", e))?;
                    imported += 1;

                    // Update issue counter if needed
                    let current_counter: i32 = conn.query_row(
                        "SELECT next_number FROM issue_counter WHERE id = 1",
                        [],
                        |row| row.get(0),
                    )?;
                    if imported_issue.number >= current_counter {
                        conn.execute(
                            "UPDATE issue_counter SET next_number = ? WHERE id = 1",
                            [imported_issue.number + 1],
                        )?;
                    }
                }
            }

            println!("{} Import complete:", "✓".green());
            println!("  Imported: {}", imported);
            if updated > 0 {
                println!("  Updated:  {}", updated);
            }
            if skipped > 0 {
                println!("  Skipped:  {} (use --force to update)", skipped);
            }
        }
    }

    Ok(())
}

async fn handle_directive_command(command: DirectiveCommands) -> Result<()> {
    use issues::{db, directive, DirectiveStatus};

    let default_db = autopilot::default_db_path();
    let directives_dir = std::env::current_dir()?.join(".openagents/directives");

    match command {
        DirectiveCommands::List { status, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            let all_directives = directive::load_directives(&directives_dir)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            // Filter by status if specified
            let directives: Vec<_> = if let Some(ref status_str) = status {
                let filter_status = DirectiveStatus::from_str(status_str);
                all_directives.into_iter().filter(|d| d.status == filter_status).collect()
            } else {
                all_directives
            };

            if directives.is_empty() {
                if directives_dir.exists() {
                    println!("No directives found");
                } else {
                    println!("No directives directory found. Create {} to get started.", directives_dir.display());
                }
            } else {
                println!("{:<12} {:<8} {:<10} {:<40} {}", "ID", "Status", "Progress", "Title", "Priority");
                println!("{}", "-".repeat(85));
                for d in directives {
                    let progress = directive::calculate_progress(&conn, &d.id)?;
                    let progress_str = if progress.total_issues > 0 {
                        format!("{}/{} ({}%)", progress.completed_issues, progress.total_issues, progress.percentage())
                    } else {
                        "0/0".to_string()
                    };
                    let title_short = if d.title.len() > 38 {
                        format!("{}...", &d.title[..35])
                    } else {
                        d.title.clone()
                    };
                    println!(
                        "{:<12} {:<8} {:<10} {:<40} {}",
                        d.id,
                        d.status.as_str(),
                        progress_str,
                        title_short,
                        d.priority.as_str()
                    );
                }
            }
        }
        DirectiveCommands::Show { id, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = db::init_db(&db_path)?;

            match directive::get_directive_by_id(&directives_dir, &id)
                .map_err(|e| anyhow::anyhow!("{}", e))?
            {
                Some(d) => {
                    let progress = directive::calculate_progress(&conn, &d.id)?;
                    let linked_issues = directive::list_issues_by_directive(&conn, &d.id)?;

                    println!("{} Directive {}", "→".cyan(), d.id);
                    println!("  Title:    {}", d.title);
                    println!("  Status:   {}", d.status.as_str());
                    println!("  Priority: {}", d.priority.as_str());
                    println!("  Created:  {}", d.created);
                    println!("  Updated:  {}", d.updated);
                    println!();
                    println!("{} Progress: {}/{} issues ({}%)", "→".cyan(), progress.completed_issues, progress.total_issues, progress.percentage());
                    if progress.in_progress_issues > 0 {
                        println!("  In progress: {}", progress.in_progress_issues);
                    }
                    if progress.blocked_issues > 0 {
                        println!("  Blocked: {}", progress.blocked_issues);
                    }
                    println!();
                    println!("{} Body:", "→".cyan());
                    for line in d.body.lines() {
                        println!("  {}", line);
                    }
                    if !linked_issues.is_empty() {
                        println!();
                        println!("{} Linked Issues:", "→".cyan());
                        for i in linked_issues {
                            let status_icon = match i.status.as_str() {
                                "done" => "✓".green(),
                                "in_progress" => "●".yellow(),
                                _ => "○".white(),
                            };
                            println!("  {} #{} - {} [{}]", status_icon, i.number, i.title, i.status.as_str());
                        }
                    }
                }
                None => {
                    eprintln!("{} Directive '{}' not found", "Error:".red(), id);
                    std::process::exit(1);
                }
            }
        }
        DirectiveCommands::Create { id, title, priority } => {
            let priority = match priority.as_str() {
                "urgent" => directive::DirectivePriority::Urgent,
                "high" => directive::DirectivePriority::High,
                "low" => directive::DirectivePriority::Low,
                _ => directive::DirectivePriority::Medium,
            };

            let body = "## Goal\n\nDescribe the goal here.\n\n## Success Criteria\n\n- [ ] Criterion 1\n- [ ] Criterion 2";

            let d = issues::Directive::create(&directives_dir, &id, &title, priority, body)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("{} Created directive '{}'", "✓".green(), d.id);
            println!("  Title: {}", d.title);
            println!("  File:  {:?}", d.file_path);
            println!();
            println!("Edit the file to add your goal and success criteria.");
        }
        DirectiveCommands::Pause { id } => {
            let mut d = directive::get_directive_by_id(&directives_dir, &id)
                .map_err(|e| anyhow::anyhow!("{}", e))?
                .ok_or_else(|| anyhow::anyhow!("Directive '{}' not found", id))?;

            d.set_status(DirectiveStatus::Paused)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("{} Paused directive '{}'", "⏸".yellow(), d.id);
        }
        DirectiveCommands::Complete { id } => {
            let mut d = directive::get_directive_by_id(&directives_dir, &id)
                .map_err(|e| anyhow::anyhow!("{}", e))?
                .ok_or_else(|| anyhow::anyhow!("Directive '{}' not found", id))?;

            d.set_status(DirectiveStatus::Completed)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("{} Completed directive '{}'", "✓".green(), d.id);
        }
        DirectiveCommands::Resume { id } => {
            let mut d = directive::get_directive_by_id(&directives_dir, &id)
                .map_err(|e| anyhow::anyhow!("{}", e))?
                .ok_or_else(|| anyhow::anyhow!("Directive '{}' not found", id))?;

            d.set_status(DirectiveStatus::Active)
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            println!("{} Resumed directive '{}' (now active)", "▶".green(), d.id);
        }
    }

    Ok(())
}

/// Handle metrics commands
async fn handle_metrics_command(command: MetricsCommands) -> Result<()> {
    use autopilot::metrics::{extract_metrics_from_json_file, MetricsDb, default_db_path};

    match command {
        MetricsCommands::Import { log_dir, db } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            println!("{} Opening metrics database: {:?}", "📊".cyan(), db_path);

            // Find all .json files in the directory
            let json_files: Vec<_> = std::fs::read_dir(&log_dir)?
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    entry.path().extension().and_then(|s| s.to_str()) == Some("json")
                })
                .map(|entry| entry.path())
                .collect();

            if json_files.is_empty() {
                println!("{} No JSON trajectory files found in {:?}", "⚠".yellow(), log_dir);
                return Ok(());
            }

            println!("{} Found {} trajectory files", "🔍".cyan(), json_files.len());
            println!();

            let mut imported = 0;
            let mut skipped = 0;
            let mut errors = 0;

            for (i, json_file) in json_files.iter().enumerate() {
                let filename = json_file.file_name().unwrap().to_string_lossy();
                print!("[{}/{}] Importing {}... ", i + 1, json_files.len(), filename);

                match extract_metrics_from_json_file(&json_file) {
                    Ok((session_metrics, tool_call_metrics)) => {
                        // Check if session already exists
                        if metrics_db.get_session(&session_metrics.id)?.is_some() {
                            println!("{}", "SKIPPED (already exists)".yellow());
                            skipped += 1;
                            continue;
                        }

                        // Store session metrics
                        metrics_db.store_session(&session_metrics)?;

                        // Store tool call metrics
                        for tool_call in &tool_call_metrics {
                            metrics_db.store_tool_call(tool_call)?;
                        }

                        println!(
                            "{} ({} tools, {} errors)",
                            "✓".green(),
                            tool_call_metrics.len(),
                            session_metrics.tool_errors
                        );
                        imported += 1;
                    }
                    Err(e) => {
                        println!("{} {}", "✗".red(), e);
                        errors += 1;
                    }
                }
            }

            println!();
            println!("{}", "=".repeat(60));
            println!("{} Import complete:", "📊".cyan().bold());
            println!("  Imported: {}", imported.to_string().green());
            println!("  Skipped:  {}", skipped.to_string().yellow());
            println!("  Errors:   {}", errors.to_string().red());
            println!("{}", "=".repeat(60));
        }
        MetricsCommands::Backfill { logs_root, db } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            println!("{} Opening metrics database: {:?}", "📊".cyan(), db_path);
            println!("{} Scanning logs directory: {:?}", "🔍".cyan(), logs_root);
            println!();

            // Find all date directories (YYYYMMDD format)
            let date_dirs: Vec<_> = std::fs::read_dir(&logs_root)?
                .filter_map(|entry| entry.ok())
                .filter(|entry| {
                    let path = entry.path();
                    if !path.is_dir() {
                        return false;
                    }
                    // Check if directory name matches YYYYMMDD pattern
                    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                    name.len() == 8 && name.chars().all(|c| c.is_ascii_digit())
                })
                .map(|entry| entry.path())
                .collect();

            if date_dirs.is_empty() {
                println!("{} No date directories found in {:?}", "⚠".yellow(), logs_root);
                return Ok(());
            }

            println!("{} Found {} date directories", "🔍".cyan(), date_dirs.len());
            println!();

            let mut total_imported = 0;
            let mut total_skipped = 0;
            let mut total_errors = 0;

            for (dir_idx, date_dir) in date_dirs.iter().enumerate() {
                let dir_name = date_dir.file_name().unwrap().to_string_lossy();
                println!("[{}/{}] Processing directory: {}", dir_idx + 1, date_dirs.len(), dir_name);

                // Find all .json files in this directory
                let json_files: Vec<_> = match std::fs::read_dir(date_dir) {
                    Ok(entries) => entries
                        .filter_map(|entry| entry.ok())
                        .filter(|entry| {
                            entry.path().extension().and_then(|s| s.to_str()) == Some("json")
                        })
                        .map(|entry| entry.path())
                        .collect(),
                    Err(e) => {
                        println!("  {} Error reading directory: {}", "✗".red(), e);
                        total_errors += 1;
                        continue;
                    }
                };

                if json_files.is_empty() {
                    println!("  {} No JSON files found", "⚠".yellow());
                    continue;
                }

                println!("  {} Found {} trajectory files", "🔍".cyan(), json_files.len());

                let mut dir_imported = 0;
                let mut dir_skipped = 0;
                let mut dir_errors = 0;

                for (i, json_file) in json_files.iter().enumerate() {
                    let filename = json_file.file_name().unwrap().to_string_lossy();
                    print!("  [{}/{}] {}... ", i + 1, json_files.len(), filename);

                    match extract_metrics_from_json_file(&json_file) {
                        Ok((session_metrics, tool_call_metrics)) => {
                            // Check if session already exists
                            if metrics_db.get_session(&session_metrics.id)?.is_some() {
                                println!("{}", "SKIPPED".yellow());
                                dir_skipped += 1;
                                continue;
                            }

                            // Store session metrics
                            if let Err(e) = metrics_db.store_session(&session_metrics) {
                                println!("{} {}", "✗".red(), e);
                                dir_errors += 1;
                                continue;
                            }

                            // Store tool call metrics
                            for tool_call in &tool_call_metrics {
                                if let Err(e) = metrics_db.store_tool_call(tool_call) {
                                    println!("{} Error storing tool call: {}", "✗".red(), e);
                                    dir_errors += 1;
                                    continue;
                                }
                            }

                            println!(
                                "{} ({} tools, {} errors)",
                                "✓".green(),
                                tool_call_metrics.len(),
                                session_metrics.tool_errors
                            );
                            dir_imported += 1;
                        }
                        Err(e) => {
                            println!("{} {}", "✗".red(), e);
                            dir_errors += 1;
                        }
                    }
                }

                println!(
                    "  {} Imported: {}, Skipped: {}, Errors: {}",
                    "📊".cyan(),
                    dir_imported.to_string().green(),
                    dir_skipped.to_string().yellow(),
                    dir_errors.to_string().red()
                );
                println!();

                total_imported += dir_imported;
                total_skipped += dir_skipped;
                total_errors += dir_errors;
            }

            println!("{}", "=".repeat(60));
            println!("{} Backfill complete:", "📊".cyan().bold());
            println!("  Directories processed: {}", date_dirs.len().to_string().cyan());
            println!("  Total imported:        {}", total_imported.to_string().green());
            println!("  Total skipped:         {}", total_skipped.to_string().yellow());
            println!("  Total errors:          {}", total_errors.to_string().red());
            println!("{}", "=".repeat(60));
        }
        MetricsCommands::Show { session_id, db } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            let session = metrics_db
                .get_session(&session_id)?
                .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", session_id))?;

            let tool_calls = metrics_db.get_tool_calls(&session_id)?;

            println!("{}", "=".repeat(60));
            println!("{} Session: {}", "📊".cyan().bold(), session.id);
            println!("{}", "=".repeat(60));
            println!();
            println!("{} Model:      {}", "🤖", session.model);
            println!("{} Timestamp:  {}", "📅", session.timestamp.format("%Y-%m-%d %H:%M:%S UTC"));
            println!("{} Duration:   {:.1}s", "⏱", session.duration_seconds);
            println!("{} Status:     {:?}", "📍", session.final_status);
            println!();
            println!("{} Tokens:", "💰".cyan().bold());
            println!("  Input:   {}", format_number(session.tokens_in));
            println!("  Output:  {}", format_number(session.tokens_out));
            println!("  Cached:  {}", format_number(session.tokens_cached));
            println!("  Cost:    ${:.4}", session.cost_usd);
            println!();
            println!("{} Tasks:", "📋".cyan().bold());
            println!("  Claimed:   {}", session.issues_claimed);
            println!("  Completed: {}", session.issues_completed);
            println!();
            println!("{} Tool Calls:", "🔧".cyan().bold());
            println!("  Total:  {}", session.tool_calls);
            println!("  Errors: {} ({:.1}%)",
                session.tool_errors,
                if session.tool_calls > 0 {
                    (session.tool_errors as f64 / session.tool_calls as f64) * 100.0
                } else {
                    0.0
                }
            );
            println!();
            println!("{} Performance:", "⚡".cyan().bold());
            println!("  Messages: {}", session.messages);
            if let Some(apm) = session.apm {
                let tier = APMTier::from_apm(apm);
                let colored_apm = match tier {
                    APMTier::Elite => format!("{:.2}", apm).yellow().bold(),
                    APMTier::HighPerformance => format!("{:.2}", apm).green().bold(),
                    APMTier::Productive => format!("{:.2}", apm).green(),
                    APMTier::Active => format!("{:.2}", apm).blue(),
                    APMTier::Baseline => format!("{:.2}", apm).dimmed(),
                };
                println!("  APM:      {} ({})", colored_apm, tier.name().dimmed());
            } else {
                println!("  APM:      {}", "Not calculated".dimmed());
            }
            println!();

            if !tool_calls.is_empty() {
                println!("{} Tool Call Breakdown:", "🔧".cyan().bold());
                for (i, tc) in tool_calls.iter().take(10).enumerate() {
                    let status = if tc.success {
                        "✓".green()
                    } else {
                        format!("✗ ({})", tc.error_type.as_deref().unwrap_or("unknown")).red()
                    };
                    println!(
                        "  {:2}. {} {:20} {}ms",
                        i + 1,
                        status,
                        tc.tool_name,
                        tc.duration_ms
                    );
                }
                if tool_calls.len() > 10 {
                    println!("  ... and {} more", tool_calls.len() - 10);
                }
            }

            println!();

            // Show anomalies if any detected
            let anomalies = metrics_db.get_anomalies(&session_id)?;
            if !anomalies.is_empty() {
                println!("{} Anomalies Detected:", "⚠".yellow().bold());
                for anomaly in &anomalies {
                    let severity_str = match anomaly.severity {
                        autopilot::metrics::AnomalySeverity::Critical => "CRITICAL".red().bold(),
                        autopilot::metrics::AnomalySeverity::Error => "ERROR".red(),
                        autopilot::metrics::AnomalySeverity::Warning => "WARNING".yellow(),
                    };
                    println!(
                        "  [{}] {}: expected {:.3}, got {:.3}",
                        severity_str,
                        anomaly.dimension,
                        anomaly.expected_value,
                        anomaly.actual_value
                    );
                }
                println!();
            }

            // Show comparison to baselines
            println!("{} Comparison to Baselines:", "📈".cyan().bold());

            // Tool error rate
            if session.tool_calls > 0 {
                let error_rate = (session.tool_errors as f64) / (session.tool_calls as f64);
                if let Ok(Some(baseline)) = metrics_db.get_baseline("tool_error_rate") {
                    let deviation = ((error_rate - baseline.mean) / baseline.stddev).abs();
                    let status = if deviation > 2.0 {
                        format!("({:.1}σ above baseline)", deviation).red()
                    } else if error_rate > baseline.mean {
                        format!("({:.1}σ above baseline)", deviation).yellow()
                    } else {
                        format!("({:.1}σ below baseline)", deviation).green()
                    };
                    println!(
                        "  Tool error rate:   {:.1}% vs {:.1}% baseline {}",
                        error_rate * 100.0,
                        baseline.mean * 100.0,
                        status
                    );
                }
            }

            // Tokens per issue
            if session.issues_completed > 0 {
                let total_tokens = session.tokens_in + session.tokens_out;
                let tokens_per_issue = (total_tokens as f64) / (session.issues_completed as f64);
                if let Ok(Some(baseline)) = metrics_db.get_baseline("tokens_per_issue") {
                    let deviation = ((tokens_per_issue - baseline.mean) / baseline.stddev).abs();
                    let status = if deviation > 2.0 {
                        format!("({:.1}σ from baseline)", deviation).yellow()
                    } else {
                        format!("({:.1}σ from baseline)", deviation).dimmed()
                    };
                    println!(
                        "  Tokens per issue:  {} vs {} baseline {}",
                        format_number(tokens_per_issue as i64),
                        format_number(baseline.mean as i64),
                        status
                    );
                }
            }

            // Cost per issue
            if session.issues_completed > 0 {
                let cost_per_issue = session.cost_usd / (session.issues_completed as f64);
                if let Ok(Some(baseline)) = metrics_db.get_baseline("cost_per_issue") {
                    let deviation = ((cost_per_issue - baseline.mean) / baseline.stddev).abs();
                    let status = if deviation > 2.0 {
                        format!("({:.1}σ from baseline)", deviation).yellow()
                    } else {
                        format!("({:.1}σ from baseline)", deviation).dimmed()
                    };
                    println!(
                        "  Cost per issue:    ${:.4} vs ${:.4} baseline {}",
                        cost_per_issue,
                        baseline.mean,
                        status
                    );
                }
            }

            println!();
            println!("{} Prompt:", "📝".cyan().bold());
            let prompt_preview = if session.prompt.len() > 200 {
                format!("{}...", &session.prompt[..200])
            } else {
                session.prompt
            };
            println!("{}", prompt_preview);
            println!("{}", "=".repeat(60));
        }
        MetricsCommands::Stats { session_id, db } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            // Get session (either specified or most recent)
            let session = if let Some(sid) = session_id {
                metrics_db
                    .get_session(&sid)?
                    .ok_or_else(|| anyhow::anyhow!("Session '{}' not found", sid))?
            } else {
                // Get most recent session
                let sessions = metrics_db.get_all_sessions()?;
                sessions
                    .into_iter()
                    .max_by_key(|s| s.timestamp)
                    .ok_or_else(|| anyhow::anyhow!("No sessions found in database"))?
            };

            // Concise one-line format
            let error_rate = if session.tool_calls > 0 {
                (session.tool_errors as f64 / session.tool_calls as f64) * 100.0
            } else {
                0.0
            };

            println!(
                "{} {} | {}s | {} → {} issues | {}k tokens | ${:.3} | {}/{} tools ({:.1}% err) | {:?}",
                "📊".cyan(),
                session.id,
                session.duration_seconds,
                session.issues_claimed,
                session.issues_completed,
                (session.tokens_in + session.tokens_out) / 1000,
                session.cost_usd,
                session.tool_calls - session.tool_errors,
                session.tool_calls,
                error_rate,
                session.final_status
            );

            // Show comparison to baseline
            if session.tool_calls > 0 {
                if let Ok(Some(baseline)) = metrics_db.get_baseline("tool_error_rate") {
                    let actual_error_rate = session.tool_errors as f64 / session.tool_calls as f64;
                    let deviation = ((actual_error_rate - baseline.mean) / baseline.stddev).abs();
                    if deviation > 2.0 {
                        println!("  ⚠️  Error rate {:.1}σ above baseline", deviation);
                    } else if actual_error_rate < baseline.mean - baseline.stddev {
                        println!("  ✨ Error rate {:.1}σ below baseline", deviation);
                    }
                }
            }
        }
        MetricsCommands::List { status, limit, db } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            let sessions = metrics_db.get_all_sessions()?;

            let filtered: Vec<_> = if let Some(status_filter) = status {
                sessions
                    .into_iter()
                    .filter(|s| format!("{:?}", s.final_status).to_lowercase() == status_filter.to_lowercase())
                    .take(limit)
                    .collect()
            } else {
                sessions.into_iter().take(limit).collect()
            };

            if filtered.is_empty() {
                println!("{} No sessions found", "⚠".yellow());
                return Ok(());
            }

            println!("{}", "=".repeat(100));
            println!("{} Sessions (showing {} of total)", "📊".cyan().bold(), filtered.len());
            println!("{}", "=".repeat(100));
            println!(
                "{:20} {:8} {:12} {:>8} {:>8} {:>6} {:>6}  {}",
                "TIMESTAMP", "MODEL", "STATUS", "TOKENS", "COST", "TOOLS", "ERRS", "PROMPT"
            );
            println!("{}", "-".repeat(100));

            for session in &filtered {
                let prompt_preview = if session.prompt.len() > 30 {
                    format!("{}...", &session.prompt[..27])
                } else {
                    session.prompt.clone()
                };

                let status_str = format!("{:?}", session.final_status);
                let status_colored = match session.final_status {
                    autopilot::metrics::SessionStatus::Completed => status_str.green(),
                    autopilot::metrics::SessionStatus::Crashed => status_str.red(),
                    autopilot::metrics::SessionStatus::BudgetExhausted => status_str.yellow(),
                    autopilot::metrics::SessionStatus::MaxTurns => status_str.yellow(),
                    autopilot::metrics::SessionStatus::Running => status_str.cyan(),
                };

                println!(
                    "{:20} {:8} {:12} {:>8} ${:>7.4} {:>6} {:>6}  {}",
                    session.timestamp.format("%Y-%m-%d %H:%M:%S"),
                    session.model,
                    status_colored,
                    format_number(session.tokens_in + session.tokens_out),
                    session.cost_usd,
                    session.tool_calls,
                    session.tool_errors,
                    prompt_preview
                );
            }

            println!("{}", "=".repeat(100));
        }
        MetricsCommands::Analyze { period, compare, db, errors, anomalies } => {
            use autopilot::analyze::{
                calculate_aggregate_stats_from_sessions, detect_regressions,
                get_sessions_in_period, get_slowest_tools, get_top_error_tools,
            };

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            // Handle compare mode if specified
            if let Some(compare_str) = compare {
                return handle_compare_analysis(&metrics_db, &compare_str);
            }

            // Parse period
            let time_period = parse_time_period(&period)?;

            // Get sessions
            let mut sessions = get_sessions_in_period(&metrics_db, time_period)?;

            if sessions.is_empty() {
                println!("{} No sessions found in {}", "⚠".yellow(), time_period.name());
                return Ok(());
            }

            // Filter for high error rate sessions if --errors flag is set
            if errors {
                sessions.retain(|s| {
                    if s.tool_calls > 0 {
                        let error_rate = s.tool_errors as f64 / s.tool_calls as f64;
                        error_rate > 0.10 // >10% error rate
                    } else {
                        false
                    }
                });

                if sessions.is_empty() {
                    println!("{} No high error rate sessions found in {}", "✓".green(), time_period.name());
                    println!("  All sessions have tool error rate ≤10%");
                    return Ok(());
                }
            }

            // Calculate aggregate stats
            let stats = calculate_aggregate_stats_from_sessions(&sessions);

            // Detect regressions
            let regressions = detect_regressions(&metrics_db, time_period)?;

            // Get top error tools
            let top_errors = get_top_error_tools(&metrics_db, time_period, 5)?;

            // Get slowest tools
            let slowest_tools = get_slowest_tools(&metrics_db, time_period, 5)?;

            // Print report
            println!("{}", "=".repeat(80));
            let title = if errors {
                format!("High Error Sessions (>10%): {}", time_period.name())
            } else {
                format!("Metrics Analysis: {}", time_period.name())
            };
            println!(
                "{} {}",
                "📊".cyan().bold(),
                title
            );
            println!("{}", "=".repeat(80));
            println!();
            println!("{} Overview:", "📈".cyan().bold());
            println!("  Sessions:       {}", sessions.len());
            println!();

            // Print aggregate statistics
            println!("{} Aggregate Statistics:", "📊".cyan().bold());
            let metrics_order = vec![
                "tool_error_rate",
                "completion_rate",
                "tokens_per_issue",
                "cost_per_issue",
                "duration_per_issue",
                "session_duration",
            ];

            for metric_name in metrics_order {
                if let Some(stat) = stats.get(metric_name) {
                    let formatted = format_metric_value(metric_name, stat.mean);
                    println!(
                        "  {:20} mean={} p50={} p90={}",
                        metric_name,
                        formatted,
                        format_metric_value(metric_name, stat.median),
                        format_metric_value(metric_name, stat.p90)
                    );
                }
            }
            println!();

            // Print regressions
            if !regressions.is_empty() {
                println!("{} Regressions Detected:", "⚠".red().bold());
                for reg in &regressions {
                    use autopilot::analyze::RegressionSeverity;
                    let severity_text = match reg.severity {
                        RegressionSeverity::Critical => "CRITICAL".red().bold(),
                        RegressionSeverity::Error => "ERROR".red(),
                        RegressionSeverity::Warning => "WARNING".yellow(),
                    };
                    println!(
                        "  {} {:20} {:.1}% worse, {:.1}σ (baseline: {}, current: {})",
                        severity_text,
                        reg.dimension,
                        reg.percent_worse,
                        reg.deviation_sigma,
                        format_metric_value(&reg.dimension, reg.baseline_value),
                        format_metric_value(&reg.dimension, reg.current_value)
                    );
                }
                println!();

                // Store regressions as anomalies for tracking and issue creation
                use autopilot::analyze::store_regressions_as_anomalies;
                let session_id = format!("aggregate-{}", time_period.name());
                match store_regressions_as_anomalies(&metrics_db, &regressions, &session_id) {
                    Ok(count) => {
                        println!("{} Stored {} regressions as anomalies", "💾".dimmed(), count);
                        println!();
                    }
                    Err(e) => {
                        eprintln!("{} Failed to store anomalies: {}", "⚠".yellow(), e);
                        println!();
                    }
                }
            } else {
                println!("{} No regressions detected", "✓".green().bold());
                println!();
            }

            // Print top error tools
            if !top_errors.is_empty() {
                println!("{} Top Error Tools:", "🔧".cyan().bold());
                for (tool, count) in &top_errors {
                    println!("  {:30} {} errors", tool, count);
                }
                println!();
            }

            // Print slowest tools
            if !slowest_tools.is_empty() {
                println!("{} Slowest Tools (avg duration):", "⏱".cyan().bold());
                for (tool, avg_ms, count) in &slowest_tools {
                    println!("  {:30} {:.0}ms avg (n={})", tool, avg_ms, count);
                }
                println!();
            }

            // If --errors flag, display detailed session info and store anomalies
            if errors {
                use autopilot::metrics::{Anomaly, AnomalySeverity};

                println!("{} High Error Rate Sessions (flagged for review):", "🚨".red().bold());
                println!();

                for session in &sessions {
                    let error_rate = if session.tool_calls > 0 {
                        session.tool_errors as f64 / session.tool_calls as f64
                    } else {
                        0.0
                    };

                    let severity = if error_rate > 0.25 {
                        AnomalySeverity::Critical
                    } else if error_rate > 0.15 {
                        AnomalySeverity::Error
                    } else {
                        AnomalySeverity::Warning
                    };

                    let severity_text = match severity {
                        AnomalySeverity::Critical => "CRITICAL".red().bold(),
                        AnomalySeverity::Error => "ERROR".red(),
                        AnomalySeverity::Warning => "WARNING".yellow(),
                    };

                    println!("  {} Session: {} ({})", severity_text, &session.id[..12.min(session.id.len())], session.timestamp.format("%Y-%m-%d %H:%M"));
                    println!("    Error Rate:   {:.1}% ({}/{})", error_rate * 100.0, session.tool_errors, session.tool_calls);
                    println!("    Prompt:       {}", truncate_string(&session.prompt, 60));
                    println!("    Model:        {}", session.model);
                    println!("    Issues:       {}/{} completed", session.issues_completed, session.issues_claimed);
                    println!();

                    // Store in anomalies table
                    let anomaly = Anomaly {
                        session_id: session.id.clone(),
                        dimension: "tool_error_rate".to_string(),
                        expected_value: 0.05, // Expected <5% error rate
                        actual_value: error_rate,
                        severity,
                        investigated: false,
                        issue_number: None,
                    };

                    if let Err(e) = metrics_db.store_anomaly(&anomaly) {
                        eprintln!("Warning: Failed to store anomaly for session {}: {}", session.id, e);
                    }
                }

                println!("{} {} high error sessions flagged and stored in anomalies table", "✓".green(), sessions.len());
                println!("  Run `cargo autopilot metrics create-issues` to create improvement tasks");
                println!();
            }

            // If --anomalies flag, display all detected anomalies from the database
            if anomalies {
                use autopilot::metrics::AnomalySeverity;

                // Get all anomalies for sessions in this period
                let mut all_anomalies = Vec::new();
                for session in &sessions {
                    if let Ok(session_anomalies) = metrics_db.get_anomalies(&session.id) {
                        all_anomalies.extend(session_anomalies);
                    }
                }

                if !all_anomalies.is_empty() {
                    println!("{} Detected Anomalies (>2σ from baseline):", "⚠".yellow().bold());
                    println!();

                    // Group by severity
                    let mut critical = Vec::new();
                    let mut errors = Vec::new();
                    let mut warnings = Vec::new();

                    for anomaly in &all_anomalies {
                        match anomaly.severity {
                            AnomalySeverity::Critical => critical.push(anomaly),
                            AnomalySeverity::Error => errors.push(anomaly),
                            AnomalySeverity::Warning => warnings.push(anomaly),
                        }
                    }

                    if !critical.is_empty() {
                        println!("{} {} Critical Anomalies:", "🔴".red().bold(), critical.len());
                        for anomaly in critical {
                            println!("  Session: {} ({})", &anomaly.session_id[..12.min(anomaly.session_id.len())], anomaly.dimension);
                            println!("    Expected: {:.4}", anomaly.expected_value);
                            println!("    Actual:   {:.4}", anomaly.actual_value);
                            if let Some(issue) = anomaly.issue_number {
                                println!("    Issue:    #{}", issue);
                            }
                            println!();
                        }
                    }

                    if !errors.is_empty() {
                        println!("{} {} Error Anomalies:", "🟠".yellow(), errors.len());
                        for anomaly in errors {
                            println!("  Session: {} ({})", &anomaly.session_id[..12.min(anomaly.session_id.len())], anomaly.dimension);
                            println!("    Expected: {:.4}, Actual: {:.4}", anomaly.expected_value, anomaly.actual_value);
                            if let Some(issue) = anomaly.issue_number {
                                println!("    Issue: #{}", issue);
                            }
                        }
                        println!();
                    }

                    if !warnings.is_empty() {
                        println!("{} {} Warning Anomalies (showing first 5):", "🟡".yellow(), warnings.len());
                        for anomaly in warnings.iter().take(5) {
                            println!("  {} ({}): {:.4} vs {:.4} expected",
                                &anomaly.session_id[..12.min(anomaly.session_id.len())],
                                anomaly.dimension,
                                anomaly.actual_value,
                                anomaly.expected_value
                            );
                        }
                        if warnings.len() > 5 {
                            println!("  ... and {} more warnings", warnings.len() - 5);
                        }
                        println!();
                    }

                    println!("{} Total anomalies detected: {}", "📊".cyan(), all_anomalies.len());
                    println!("  Run `cargo autopilot metrics create-issues` to auto-create improvement tasks");
                    println!();
                } else {
                    println!("{} No anomalies detected in period", "✓".green().bold());
                    println!("  All metrics within 2 standard deviations of baseline");
                    println!();
                }
            }

            println!("{}", "=".repeat(80));
        }
        MetricsCommands::Trends { recent, baseline, db } => {
            use autopilot::analyze::{detect_trends, TrendDirection};

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            // Parse periods
            let recent_period = parse_time_period(&recent)?;
            let baseline_parsed = parse_time_period(&baseline)?;
            let baseline_period = Some(baseline_parsed);

            // Detect trends
            let trends = detect_trends(&metrics_db, recent_period, baseline_period)?;

            if trends.is_empty() {
                println!("{} No trend data available", "⚠".yellow());
                return Ok(());
            }

            // Print trends report
            println!("{}", "=".repeat(80));
            println!(
                "{} Trend Analysis: {} vs {}",
                "📈".cyan().bold(),
                recent_period.name(),
                baseline_parsed.name()
            );
            println!("{}", "=".repeat(80));
            println!();

            for trend in &trends {
                let direction_icon = match trend.direction {
                    TrendDirection::Improving => "↑".green(),
                    TrendDirection::Stable => "→".yellow(),
                    TrendDirection::Degrading => "↓".red(),
                };

                let direction_str = match trend.direction {
                    TrendDirection::Improving => "IMPROVING".green().bold(),
                    TrendDirection::Stable => "STABLE".yellow(),
                    TrendDirection::Degrading => "DEGRADING".red().bold(),
                };

                println!("{} {} {}", direction_icon, trend.dimension, direction_str);
                println!(
                    "    Recent:   {} (n={})",
                    format_metric_value(&trend.dimension, trend.recent.mean),
                    trend.recent.count
                );
                if let Some(ref base) = trend.baseline {
                    println!(
                        "    Baseline: {} (n={})",
                        format_metric_value(&trend.dimension, base.mean),
                        base.count
                    );
                    if trend.percent_change.abs() > 0.1 {
                        let change_str = format!("{:+.1}%", trend.percent_change);
                        let change_colored = if trend.direction == TrendDirection::Improving {
                            change_str.green()
                        } else if trend.direction == TrendDirection::Degrading {
                            change_str.red()
                        } else {
                            change_str.normal()
                        };
                        println!("    Change:   {}", change_colored);
                    }
                }
                println!();
            }

            println!("{}", "=".repeat(80));
        }
        MetricsCommands::Dashboard {
            metrics_db,
            port,
        } => {
            use autopilot::dashboard::start_dashboard;

            let db_path = metrics_db
                .unwrap_or_else(default_db_path)
                .to_string_lossy()
                .to_string();

            println!("{}", "=".repeat(80));
            println!("{} Starting Autopilot Metrics Dashboard", "📊".cyan().bold());
            println!("{}", "=".repeat(80));
            println!();
            println!("  Database: {}", db_path);
            println!("  URL: http://127.0.0.1:{}", port);
            println!();
            println!("  Press Ctrl+C to stop");
            println!();

            start_dashboard(&db_path, port).await?;
        }
        MetricsCommands::Report {
            metrics_db,
            output,
        } => {
            use autopilot::weekly_report::generate_weekly_report;

            let db_path = metrics_db.unwrap_or_else(default_db_path);
            let db = MetricsDb::open(&db_path)?;

            println!("{}", "=".repeat(80));
            println!("{} Generating Weekly Trend Report", "📊".cyan().bold());
            println!("{}", "=".repeat(80));
            println!();

            let report_path = generate_weekly_report(&db, output.as_deref())?;

            println!("{} Report generated successfully", "✓".green().bold());
            println!();
            println!("  Location: {}", report_path.display());
            println!();

            // Print summary of what's in the report
            let content = std::fs::read_to_string(&report_path)?;
            if content.contains("improving") {
                let improving_count = content.match_indices("✅").count();
                if improving_count > 0 {
                    println!("  {} {} metrics improving", "📈".green(), improving_count);
                }
            }
            if content.contains("regressions detected") {
                println!("  {} Regressions detected - review recommended", "⚠️".yellow());
            }
            println!();
        }
        MetricsCommands::Learn {
            metrics_db,
            sessions,
            limit,
            format,
        } => {
            use autopilot::learning::LearningPipeline;

            let db_path = metrics_db.unwrap_or_else(default_db_path);
            let db = MetricsDb::open(&db_path)?;

            println!("{}", "=".repeat(80));
            println!("{} Autopilot Learning Pipeline", "🧠".cyan().bold());
            println!("{}", "=".repeat(80));
            println!();

            // Get session IDs to analyze
            let session_ids: Vec<String> = if sessions.is_empty() {
                println!("{} Analyzing last {} sessions...", "📊".cyan(), limit);
                let recent = db.get_recent_sessions(limit)?;
                recent.into_iter().map(|s| s.id).collect()
            } else {
                println!("{} Analyzing {} specific sessions...", "📊".cyan(), sessions.len());
                sessions
            };

            if session_ids.is_empty() {
                println!("{} No sessions found to analyze", "⚠️".yellow());
                return Ok(());
            }

            // Run the learning pipeline
            let pipeline = LearningPipeline::new(&db);
            let report = pipeline.run(&session_ids)?;

            // Output results
            match format.as_str() {
                "json" => {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                _ => {
                    println!();
                    println!("{} {} improvements detected", "✨".green(), report.improvements.len());
                    println!();

                    for improvement in &report.improvements {
                        println!("{} {:?} (severity: {}/10)", "⚠️".yellow(), improvement.improvement_type, improvement.severity);
                        println!("  Description: {}", improvement.description);
                        println!("  Proposed fix: {}", improvement.proposed_fix);
                        println!("  Evidence: {} items", improvement.evidence.len());
                        println!();
                    }

                    if !report.prompt_updates.is_empty() {
                        println!("{} {} prompt updates proposed", "📝".cyan(), report.prompt_updates.len());
                        for update in &report.prompt_updates {
                            println!("  {}: {}", update.file_path, update.section);
                            println!("    {}", update.rationale);
                        }
                        println!();
                    }

                    if !report.hook_updates.is_empty() {
                        println!("{} {} hook updates proposed", "🪝".cyan(), report.hook_updates.len());
                        for update in &report.hook_updates {
                            println!("  {}", update.hook_name);
                            println!("    {}", update.rationale);
                        }
                        println!();
                    }

                    if !report.issues_created.is_empty() {
                        println!("{} {} issues created", "📋".cyan(), report.issues_created.len());
                        println!();
                    }
                }
            }
        }
        MetricsCommands::Export { db, period, format, output } => {
            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            // Get sessions based on period
            let sessions = if period == "all" {
                metrics_db.get_all_sessions()?
            } else {
                use autopilot::analyze::{get_sessions_in_period, TimePeriod};
                let time_period = match period.as_str() {
                    "7d" => TimePeriod::Last7Days,
                    "30d" => TimePeriod::Last30Days,
                    "last-week" => TimePeriod::LastWeek,
                    "this-week" => TimePeriod::ThisWeek,
                    _ => TimePeriod::Last7Days,
                };
                get_sessions_in_period(&metrics_db, time_period)?
            };

            if sessions.is_empty() {
                eprintln!("No sessions found for period: {}", period);
                return Ok(());
            }

            // Build export data
            let export_data = if format == "csv" {
                // CSV format: header + rows
                let mut csv = String::new();
                csv.push_str("session_id,timestamp,model,duration_s,tokens_in,tokens_out,tokens_cached,cost_usd,issues_claimed,issues_completed,tool_calls,tool_errors,final_status\n");

                for session in &sessions {
                    csv.push_str(&format!(
                        "{},{},{},{},{},{},{},{},{},{},{},{},{:?}\n",
                        session.id,
                        session.timestamp.to_rfc3339(),
                        session.model,
                        session.duration_seconds,
                        session.tokens_in,
                        session.tokens_out,
                        session.tokens_cached,
                        session.cost_usd,
                        session.issues_claimed,
                        session.issues_completed,
                        session.tool_calls,
                        session.tool_errors,
                        session.final_status
                    ));
                }
                csv
            } else {
                // JSON format
                serde_json::to_string_pretty(&sessions)?
            };

            // Output to file or stdout
            if let Some(output_path) = output {
                std::fs::write(&output_path, export_data)?;
                eprintln!("Exported {} sessions to {}", sessions.len(), output_path.display());
            } else {
                println!("{}", export_data);
            }
        }
        MetricsCommands::BackfillApm { db } => {
            use autopilot::metrics::backfill_apm_for_sessions;
            use colored::Colorize;

            let db_path = db.unwrap_or_else(default_db_path);

            println!("{} Backfilling APM data for existing sessions...", "📊".cyan());
            println!("{} Database: {:?}", "📂".dimmed(), db_path);
            println!();

            match backfill_apm_for_sessions(&db_path) {
                Ok(count) => {
                    println!("{} Updated APM for {} sessions", "✅".green(), count);
                    if count == 0 {
                        println!("{}", "All sessions already have APM calculated".dimmed());
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to backfill APM: {}", "❌".red(), e);
                    std::process::exit(1);
                }
            }
        }

        MetricsCommands::BackfillFromLogs { logs_dir, db } => {
            use autopilot::metrics::backfill_metrics_from_logs;
            use colored::Colorize;

            let db_path = db.unwrap_or_else(default_db_path);

            println!("{} Backfilling metrics from trajectory logs...", "📊".cyan());
            println!("{} Logs directory: {:?}", "📂".dimmed(), logs_dir);
            println!("{} Database: {:?}", "💾".dimmed(), db_path);
            println!();

            match backfill_metrics_from_logs(&logs_dir, &db_path) {
                Ok((files_processed, records_created, errors)) => {
                    println!("{} Backfill complete!", "✅".green());
                    println!("  {} Files processed: {}", "📄".dimmed(), files_processed);
                    println!("  {} Records created: {}", "📝".dimmed(), records_created);
                    if errors > 0 {
                        println!("  {} Errors encountered: {}", "⚠".yellow(), errors);
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to backfill metrics: {}", "❌".red(), e);
                    std::process::exit(1);
                }
            }
        }

        MetricsCommands::UpdateBaselines { db, min_samples } => {
            use colored::Colorize;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            println!("{} Updating baseline metrics...", "📊".cyan());
            println!("{} Database: {:?}", "📂".dimmed(), db_path);
            println!("{} Minimum samples: {}", "🔢".dimmed(), min_samples);
            println!();

            match metrics_db.update_baselines(min_samples) {
                Ok(updated) => {
                    if updated.is_empty() {
                        println!("{} No baselines updated - not enough samples (min: {})", "⚠".yellow(), min_samples);
                    } else {
                        println!("{} Updated {} baseline metrics:", "✅".green(), updated.len());
                        for dimension in &updated {
                            if let Ok(Some(baseline)) = metrics_db.get_baseline(dimension) {
                                println!("  {} {:20} mean={:.4}, σ={:.4}, n={}",
                                    "•".dimmed(),
                                    dimension,
                                    baseline.mean,
                                    baseline.stddev,
                                    baseline.sample_count
                                );
                            }
                        }
                        println!();
                        println!("{} Baselines are now available for regression detection", "ℹ".cyan());
                        println!("{} Run {} to analyze metrics against baselines",
                            "→".dimmed(),
                            "cargo autopilot metrics analyze".bold()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to update baselines: {}", "❌".red(), e);
                    std::process::exit(1);
                }
            }
        }

        MetricsCommands::CreateIssues {
            metrics_db,
            issues_db,
            dry_run,
        } => {
            use autopilot::auto_issues::{create_issues, detect_all_patterns, generate_issues, Pattern};

            let metrics_db_path = metrics_db.unwrap_or_else(default_db_path);
            let metrics = MetricsDb::open(&metrics_db_path)?;

            let issues_db_path = issues_db.unwrap_or_else(|| {
                std::env::current_dir()
                    .unwrap_or_else(|_| PathBuf::from("."))
                    .join("autopilot.db")
            });

            println!("{}", "=".repeat(80));
            println!(
                "{} Automated Issue Creation from Pattern Detection",
                "🤖".cyan().bold()
            );
            println!("{}", "=".repeat(80));
            println!();

            // Detect all patterns (both anomaly and tool error patterns)
            println!("{} Detecting patterns...", "🔍".cyan());
            let patterns = detect_all_patterns(&metrics)?;

            if patterns.is_empty() {
                println!("{} No patterns detected", "✓".green());
                println!("  All metrics appear normal, or issues have already been created.");
                println!();
                return Ok(());
            }

            // Count pattern types
            let anomaly_count = patterns.iter().filter(|p| matches!(p, Pattern::Anomaly(_))).count();
            let tool_error_count = patterns.iter().filter(|p| matches!(p, Pattern::ToolError(_))).count();

            println!("{} Found {} patterns:", "📊".cyan(), patterns.len());
            if anomaly_count > 0 {
                println!("  - {} anomaly patterns", anomaly_count);
            }
            if tool_error_count > 0 {
                println!("  - {} tool error patterns", tool_error_count);
            }
            println!();

            // Generate issues
            let improvement_issues = generate_issues(patterns);

            println!("{} Proposed Issues:", "📝".cyan().bold());
            for (i, issue) in improvement_issues.iter().enumerate() {
                let priority_colored = match issue.priority.as_str() {
                    "urgent" => issue.priority.red().bold(),
                    "high" => issue.priority.yellow(),
                    _ => issue.priority.normal(),
                };
                println!(
                    "\n{}. {} [{}]",
                    i + 1,
                    issue.title.bold(),
                    priority_colored
                );
                // Show pattern-specific details
                match &issue.pattern {
                    Pattern::Anomaly(p) => {
                        println!("   Type: Anomaly pattern");
                        println!("   Dimension: {} ({} sessions, {:?} severity)",
                            p.dimension,
                            p.occurrence_count,
                            p.severity
                        );
                    }
                    Pattern::ToolError(p) => {
                        println!("   Type: Tool error pattern");
                        println!("   Tool: {} ({:.1}% error rate, {} failures)",
                            p.tool_name,
                            p.error_rate * 100.0,
                            p.failed_calls
                        );
                    }
                }
            }
            println!();

            if dry_run {
                println!("{} Dry run mode - no issues created", "ℹ".cyan());
                println!();
                return Ok(());
            }

            // Create issues
            println!("{} Creating issues...", "🚀".cyan());
            let issue_numbers = create_issues(&issues_db_path, &improvement_issues, &metrics)?;

            println!();
            println!("{}", "=".repeat(80));
            println!(
                "{} Created {} improvement issues linked to d-004",
                "✓".green().bold(),
                issue_numbers.len()
            );
            println!();
            println!("Issue numbers: {}", issue_numbers.iter()
                .map(|n| format!("#{}", n))
                .collect::<Vec<_>>()
                .join(", "));
            println!();
            println!("View issues: cargo autopilot issue list");
            println!("{}", "=".repeat(80));
        }

        MetricsCommands::Alerts(cmd) => {
            use autopilot::alerts;
            use autopilot::metrics::MetricsDb;

            let db_path = match &cmd {
                AlertCommands::List { db, .. } |
                AlertCommands::Add { db, .. } |
                AlertCommands::Remove { db, .. } |
                AlertCommands::History { db, .. } => db.clone().unwrap_or_else(default_db_path),
            };

            let metrics_db = MetricsDb::open(&db_path)?;
            let conn = metrics_db.connection();

            match cmd {
                AlertCommands::List { .. } => {
                    let rules = alerts::list_alert_rules(conn)?;

                    if rules.is_empty() {
                        println!("{} No alert rules configured", "ℹ".cyan());
                        println!("\nAdd a rule with: cargo autopilot metrics alerts add \\");
                        println!("    --metric tool_error_rate \\");
                        println!("    --alert-type threshold \\");
                        println!("    --severity critical \\");
                        println!("    --threshold 0.10 \\");
                        println!("    --description \"High tool error rate\"");
                        return Ok(());
                    }

                    println!("{} Alert Rules", "📋".cyan().bold());
                    println!();
                    for rule in rules {
                        let enabled = if rule.enabled { "✓".green() } else { "✗".red() };
                        let severity_colored = match rule.severity {
                            alerts::AlertSeverity::Warning => "warning".yellow(),
                            alerts::AlertSeverity::Error => "error".red(),
                            alerts::AlertSeverity::Critical => "critical".red().bold(),
                        };
                        println!("{} Rule #{}: {} [{}]", enabled, rule.id, rule.metric_name, severity_colored);
                        println!("   Type: {:?}, Threshold: {:.2}", rule.alert_type, rule.threshold);
                        println!("   Description: {}", rule.description);
                        println!();
                    }
                }

                AlertCommands::Add { metric, alert_type, severity, threshold, description, .. } => {
                    let alert_type = alerts::AlertType::from_str(&alert_type)
                        .ok_or_else(|| anyhow::anyhow!("Invalid alert type. Use: threshold, regression, or rate_of_change"))?;
                    let severity = alerts::AlertSeverity::from_str(&severity)
                        .ok_or_else(|| anyhow::anyhow!("Invalid severity. Use: warning, error, or critical"))?;

                    let rule_id = alerts::add_alert_rule(conn, &metric, alert_type, severity, threshold, &description)?;

                    println!("{} Created alert rule #{}", "✓".green(), rule_id);
                    println!("  Metric: {}", metric);
                    println!("  Type: {:?}", alert_type);
                    println!("  Severity: {:?}", severity);
                    println!("  Threshold: {:.2}", threshold);
                }

                AlertCommands::Remove { rule_id, .. } => {
                    alerts::remove_alert_rule(conn, rule_id)?;
                    println!("{} Removed alert rule #{}", "✓".green(), rule_id);
                }

                AlertCommands::History { session, metric, limit, .. } => {
                    let alerts = alerts::get_alert_history(
                        conn,
                        session.as_deref(),
                        metric.as_deref(),
                        Some(limit),
                    )?;

                    if alerts.is_empty() {
                        println!("{} No alerts fired", "✓".green());
                        return Ok(());
                    }

                    println!("{} Alert History ({} alerts)", "📜".cyan().bold(), alerts.len());
                    println!();
                    for alert in alerts {
                        let severity_colored = match alert.severity {
                            alerts::AlertSeverity::Warning => "WARNING".yellow(),
                            alerts::AlertSeverity::Error => "ERROR".red(),
                            alerts::AlertSeverity::Critical => "CRITICAL".red().bold(),
                        };
                        println!("[{}] {}", severity_colored, alert.fired_at.format("%Y-%m-%d %H:%M:%S"));
                        println!("  {}", alert.message);
                        println!("  Session: {}", alert.session_id);
                        println!();
                    }
                }
            }
        }

        MetricsCommands::IssueMetrics { issue_number, db, format } => {
            use autopilot::metrics::MetricsDb;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            match metrics_db.get_issue_metrics(issue_number)? {
                Some(metrics) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&metrics)?);
                    } else {
                        println!("{} Issue #{} Metrics", "📊".cyan().bold(), issue_number);
                        println!();
                        println!("Sessions:        {}", metrics.sessions_count);
                        println!("Duration:        {:.1}s total, {:.1}s avg", metrics.total_duration_seconds, metrics.avg_duration_seconds);
                        println!("Tokens:          {} total, {:.0} avg", format_number(metrics.total_tokens), metrics.avg_tokens);
                        println!("Cost:            ${:.4} total, ${:.4} avg", metrics.total_cost_usd, metrics.avg_cost_usd);
                        println!("Tool Calls:      {}", metrics.tool_calls);
                        println!("Tool Errors:     {} ({:.1}% error rate)", metrics.tool_errors, metrics.error_rate);
                    }
                }
                None => {
                    println!("{} No metrics found for issue #{}", "ℹ".yellow(), issue_number);
                }
            }
        }

        MetricsCommands::DirectiveMetrics { directive_id, db, format } => {
            use autopilot::metrics::MetricsDb;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            match metrics_db.get_directive_metrics(&directive_id)? {
                Some(metrics) => {
                    if format == "json" {
                        println!("{}", serde_json::to_string_pretty(&metrics)?);
                    } else {
                        println!("{} Directive {} Metrics", "📊".cyan().bold(), directive_id);
                        println!();
                        println!("Sessions:          {}", metrics.sessions_count);
                        println!("Issues Completed:  {}", metrics.issues_completed);
                        println!("Duration:          {:.1}s total, {:.1}s avg", metrics.total_duration_seconds, metrics.avg_duration_seconds);
                        println!("Tokens:            {} total, {:.0} avg", format_number(metrics.total_tokens), metrics.avg_tokens);
                        println!("Cost:              ${:.4} total, ${:.4} avg", metrics.total_cost_usd, metrics.avg_cost_usd);
                        println!("Tool Calls:        {}", metrics.tool_calls);
                        println!("Tool Errors:       {} ({:.1}% error rate)", metrics.tool_errors, metrics.error_rate);
                    }
                }
                None => {
                    println!("{} No metrics found for directive {}", "ℹ".yellow(), directive_id);
                }
            }
        }

        MetricsCommands::ByIssue { db, format } => {
            use autopilot::metrics::MetricsDb;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            let all_metrics = metrics_db.get_all_issue_metrics()?;

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&all_metrics)?);
            } else {
                if all_metrics.is_empty() {
                    println!("{} No issue metrics available", "ℹ".yellow());
                    return Ok(());
                }

                println!("{} Metrics by Issue", "📊".cyan().bold());
                println!();
                println!("{:<8} {:>10} {:>12} {:>12} {:>10} {:>10}",
                    "Issue", "Sessions", "Duration", "Tokens", "Cost", "Error %");
                println!("{}", "─".repeat(80));

                for metric in all_metrics {
                    println!("{:<8} {:>10} {:>12.1}s {:>12} ${:>9.4} {:>9.1}%",
                        format!("#{}", metric.issue_number),
                        metric.sessions_count,
                        metric.avg_duration_seconds,
                        format_number(metric.avg_tokens as i64),
                        metric.avg_cost_usd,
                        metric.error_rate
                    );
                }
            }
        }

        MetricsCommands::ByDirective { db, format } => {
            use autopilot::metrics::MetricsDb;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            let all_metrics = metrics_db.get_all_directive_metrics()?;

            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&all_metrics)?);
            } else {
                if all_metrics.is_empty() {
                    println!("{} No directive metrics available", "ℹ".yellow());
                    return Ok(());
                }

                println!("{} Metrics by Directive", "📊".cyan().bold());
                println!();
                println!("{:<12} {:>10} {:>12} {:>12} {:>12} {:>10} {:>10}",
                    "Directive", "Sessions", "Issues", "Duration", "Tokens", "Cost", "Error %");
                println!("{}", "─".repeat(95));

                for metric in all_metrics {
                    println!("{:<12} {:>10} {:>12} {:>12.1}s {:>12} ${:>9.4} {:>9.1}%",
                        metric.directive_id,
                        metric.sessions_count,
                        metric.issues_completed,
                        metric.avg_duration_seconds,
                        format_number(metric.avg_tokens as i64),
                        metric.avg_cost_usd,
                        metric.error_rate
                    );
                }
            }
        }

        MetricsCommands::Velocity {
            period,
            db,
            limit,
            celebrate_threshold,
            progress_threshold,
            warning_threshold,
        } => {
            use autopilot::analyze::calculate_velocity;
            use autopilot::metrics::MetricsDb;

            let db_path = db.unwrap_or_else(default_db_path);
            let metrics_db = MetricsDb::open(&db_path)?;

            // Parse period
            let time_period = parse_time_period(&period)?;

            // Calculate current velocity
            let velocity = calculate_velocity(&metrics_db, time_period)?;

            // Store the snapshot
            metrics_db.store_velocity_snapshot(&velocity)?;

            // Get historical snapshots
            let snapshots = metrics_db.get_velocity_snapshots(limit)?;

            // Print report
            println!("{}", "=".repeat(80));
            println!("{} Improvement Velocity", "🚀".cyan().bold());
            println!("{}", "=".repeat(80));
            println!();

            // Current velocity
            println!("{} Current Period: {}", "📊".cyan(), velocity.period);
            println!("  Velocity Score:    {:.2} (-1.0 to 1.0)", velocity.velocity_score);
            println!("  Issues Completed:  {}", velocity.issues_completed.to_string().cyan());
            println!("  Improving Metrics: {}", velocity.improving_metrics.to_string().green());
            println!("  Stable Metrics:    {}", velocity.stable_metrics);
            println!("  Degrading Metrics: {}", velocity.degrading_metrics.to_string().red());
            println!();

            // Celebrate improvements!
            if velocity.velocity_score > celebrate_threshold {
                println!("{} {} Great work! Autopilot is significantly improving!", "🎉".cyan().bold(), "CELEBRATION:".green().bold());
                println!("  {} metrics are improving, showing strong upward momentum!", velocity.improving_metrics);
                println!();
            } else if velocity.velocity_score > progress_threshold {
                println!("{} {} Autopilot is getting better!", "✨".cyan(), "Progress:".green().bold());
                println!("  Positive improvements detected across key metrics.");
                println!();
            } else if velocity.velocity_score < warning_threshold {
                println!("{} {} Attention needed - metrics are degrading.", "⚠️".yellow().bold(), "Warning:".yellow().bold());
                println!("  Consider investigating recent changes and running diagnostics.");
                println!();
            }

            // Key metrics
            if !velocity.key_metrics.is_empty() {
                println!("{} Key Metrics:", "🔑".cyan().bold());
                for metric in &velocity.key_metrics {
                    let direction_icon = match metric.direction.as_str() {
                        "improving" => "📈".green(),
                        "degrading" => "📉".red(),
                        _ => "➡️".yellow(),
                    };
                    println!(
                        "  {:<25} {} {:>7.1}%",
                        metric.dimension,
                        direction_icon,
                        metric.percent_change
                    );
                }
                println!();
            }

            // Historical trend
            if snapshots.len() > 1 {
                println!("{} Historical Velocity:", "📈".cyan().bold());
                for snapshot in &snapshots {
                    let score_color = if snapshot.velocity_score > 0.3 {
                        snapshot.velocity_score.to_string().green()
                    } else if snapshot.velocity_score < -0.3 {
                        snapshot.velocity_score.to_string().red()
                    } else {
                        snapshot.velocity_score.to_string().yellow()
                    };

                    println!(
                        "  {} | {:>8} | Score: {}",
                        snapshot.timestamp.format("%Y-%m-%d %H:%M"),
                        snapshot.period,
                        score_color
                    );
                }
                println!();
            }

            println!("{}", "=".repeat(80));
        }
    }

    Ok(())
}

/// Format large numbers with commas
fn format_number(n: i64) -> String {
    n.to_string()
        .as_bytes()
        .rchunks(3)
        .rev()
        .map(std::str::from_utf8)
        .collect::<Result<Vec<&str>, _>>()
        .unwrap()
        .join(",")
}

/// Parse time period string to TimePeriod enum
fn parse_time_period(period_str: &str) -> Result<autopilot::analyze::TimePeriod> {
    use autopilot::analyze::TimePeriod;

    match period_str {
        "7d" => Ok(TimePeriod::Last7Days),
        "30d" => Ok(TimePeriod::Last30Days),
        "last-week" => Ok(TimePeriod::LastWeek),
        "this-week" => Ok(TimePeriod::ThisWeek),
        _ => Err(anyhow::anyhow!(
            "Invalid period: {}. Valid options: 7d, 30d, last-week, this-week",
            period_str
        )),
    }
}

/// Format metric value for display
fn format_metric_value(metric_name: &str, value: f64) -> String {
    match metric_name {
        "tool_error_rate" | "completion_rate" => format!("{:.1}%", value * 100.0),
        "cost_per_issue" => format!("${:.4}", value),
        "duration_per_issue" => format!("{:.1}s", value),
        "session_duration" => format!("{:.1}s", value),
        "tokens_per_issue" => format!("{:.0}", value),
        _ => format!("{:.2}", value),
    }
}

fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}...", &s[..max_len.saturating_sub(3)])
    }
}

/// Handle compare analysis between two date ranges
fn handle_compare_analysis(metrics_db: &autopilot::metrics::MetricsDb, compare_str: &str) -> Result<()> {
    use anyhow::Context;
    use autopilot::analyze::{calculate_aggregate_stats_from_sessions, get_sessions_between_dates};
    use chrono::NaiveDate;

    // Parse date range (format: YYYY-MM-DD..YYYY-MM-DD)
    let parts: Vec<&str> = compare_str.split("..").collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid compare format. Expected: YYYY-MM-DD..YYYY-MM-DD");
    }

    let start_date = NaiveDate::parse_from_str(parts[0], "%Y-%m-%d")
        .with_context(|| format!("Failed to parse start date: {}", parts[0]))?;
    let end_date = NaiveDate::parse_from_str(parts[1], "%Y-%m-%d")
        .with_context(|| format!("Failed to parse end date: {}", parts[1]))?;

    // Convert to DateTime<Utc>
    let start = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
    let end = end_date.and_hms_opt(23, 59, 59).unwrap().and_utc();

    // Get sessions in the date range
    let sessions = get_sessions_between_dates(metrics_db, start, end)?;

    if sessions.is_empty() {
        println!("{} No sessions found between {} and {}",
            "⚠".yellow(), parts[0], parts[1]);
        return Ok(());
    }

    // Calculate aggregate stats
    let stats = calculate_aggregate_stats_from_sessions(&sessions);

    // Print report
    println!("{}", "=".repeat(80));
    println!("{} Metrics Comparison: {} to {}",
        "📊".cyan().bold(), parts[0], parts[1]);
    println!("{}", "=".repeat(80));
    println!();
    println!("{} Overview:", "📈".cyan().bold());
    println!("  Sessions:       {}", sessions.len());
    println!("  Date Range:     {} to {}", parts[0], parts[1]);
    println!();

    // Print aggregate statistics
    println!("{} Aggregate Statistics:", "📊".cyan().bold());
    let metrics_order = vec![
        "tool_error_rate",
        "completion_rate",
        "tokens_per_issue",
        "cost_per_issue",
        "duration_per_issue",
        "session_duration",
    ];

    for metric_name in metrics_order {
        if let Some(stat) = stats.get(metric_name) {
            let formatted = format_metric_value(metric_name, stat.mean);
            println!(
                "  {:20} mean={} p50={} p90={}",
                metric_name,
                formatted,
                format_metric_value(metric_name, stat.median),
                format_metric_value(metric_name, stat.p90)
            );
        }
    }
    println!();
    println!("{}", "=".repeat(80));

    Ok(())
}

/// Compare two benchmark databases for regression detection
async fn handle_benchmark_comparison(
    db1_path: &PathBuf,
    db2_path: &PathBuf,
    threshold: f64,
) -> Result<()> {
    use autopilot::benchmark::BenchmarkDatabase;

    println!("\n## Benchmark Comparison\n");

    let db1 = BenchmarkDatabase::open(db1_path)?;
    let db2 = BenchmarkDatabase::open(db2_path)?;

    // Get all benchmark IDs from both databases
    let results1 = db1.get_all_latest_results()?;
    let results2 = db2.get_all_latest_results()?;

    if results1.is_empty() || results2.is_empty() {
        println!("⚠ No benchmark data found in one or both databases.");
        return Ok(());
    }

    let by_id1: std::collections::HashMap<_, _> = results1.iter()
        .map(|r| (r.benchmark_id.clone(), r))
        .collect();
    let by_id2: std::collections::HashMap<_, _> = results2.iter()
        .map(|r| (r.benchmark_id.clone(), r))
        .collect();

    println!("| Benchmark | Metric | Baseline | Current | Change | Status |");
    println!("|-----------|--------|----------|---------|--------|--------|");

    let mut has_regression = false;
    let mut regressed_benchmarks = Vec::new();

    for (id, r2) in &by_id2 {
        if let Some(r1) = by_id1.get(id) {
            // Compare duration
            let duration_pct = ((r2.metrics.duration_ms as f64 - r1.metrics.duration_ms as f64)
                / r1.metrics.duration_ms as f64).abs();
            let duration_regressed = r2.metrics.duration_ms > r1.metrics.duration_ms
                && duration_pct > threshold;

            // Compare tokens
            let tokens_in_pct = ((r2.metrics.tokens_in as f64 - r1.metrics.tokens_in as f64)
                / r1.metrics.tokens_in as f64).abs();
            let tokens_in_regressed = r2.metrics.tokens_in > r1.metrics.tokens_in
                && tokens_in_pct > threshold;

            let tokens_out_pct = ((r2.metrics.tokens_out as f64 - r1.metrics.tokens_out as f64)
                / r1.metrics.tokens_out as f64).abs();
            let tokens_out_regressed = r2.metrics.tokens_out > r1.metrics.tokens_out
                && tokens_out_pct > threshold;

            // Compare cost
            let cost_pct = ((r2.metrics.cost_usd - r1.metrics.cost_usd)
                / r1.metrics.cost_usd).abs();
            let cost_regressed = r2.metrics.cost_usd > r1.metrics.cost_usd
                && cost_pct > threshold;

            // Compare errors
            let errors_increased = r2.metrics.tool_errors > r1.metrics.tool_errors;

            // Duration row
            let duration_change = ((r2.metrics.duration_ms as f64 - r1.metrics.duration_ms as f64)
                / r1.metrics.duration_ms as f64) * 100.0;
            let duration_status = if duration_regressed { "❌ FAIL" } else { "✅ PASS" };
            println!("| {} | Duration | {}ms | {}ms | {:+.1}% | {} |",
                id, r1.metrics.duration_ms, r2.metrics.duration_ms, duration_change, duration_status);

            // Tokens in row
            let tokens_in_change = ((r2.metrics.tokens_in as f64 - r1.metrics.tokens_in as f64)
                / r1.metrics.tokens_in as f64) * 100.0;
            let tokens_in_status = if tokens_in_regressed { "❌ FAIL" } else { "✅ PASS" };
            println!("| {} | Tokens In | {} | {} | {:+.1}% | {} |",
                id, r1.metrics.tokens_in, r2.metrics.tokens_in, tokens_in_change, tokens_in_status);

            // Tokens out row
            let tokens_out_change = ((r2.metrics.tokens_out as f64 - r1.metrics.tokens_out as f64)
                / r1.metrics.tokens_out as f64) * 100.0;
            let tokens_out_status = if tokens_out_regressed { "❌ FAIL" } else { "✅ PASS" };
            println!("| {} | Tokens Out | {} | {} | {:+.1}% | {} |",
                id, r1.metrics.tokens_out, r2.metrics.tokens_out, tokens_out_change, tokens_out_status);

            // Cost row
            let cost_change = ((r2.metrics.cost_usd - r1.metrics.cost_usd)
                / r1.metrics.cost_usd) * 100.0;
            let cost_status = if cost_regressed { "❌ FAIL" } else { "✅ PASS" };
            println!("| {} | Cost | ${:.4} | ${:.4} | {:+.1}% | {} |",
                id, r1.metrics.cost_usd, r2.metrics.cost_usd, cost_change, cost_status);

            // Errors row
            let errors_status = if errors_increased { "❌ FAIL" } else { "✅ PASS" };
            println!("| {} | Tool Errors | {} | {} | - | {} |",
                id, r1.metrics.tool_errors, r2.metrics.tool_errors, errors_status);

            if duration_regressed || tokens_in_regressed || tokens_out_regressed || cost_regressed || errors_increased {
                has_regression = true;
                let mut regression_details = Vec::new();
                if duration_regressed {
                    regression_details.push(format!("duration +{:.1}%", duration_change));
                }
                if tokens_in_regressed {
                    regression_details.push(format!("tokens_in +{:.1}%", tokens_in_change));
                }
                if tokens_out_regressed {
                    regression_details.push(format!("tokens_out +{:.1}%", tokens_out_change));
                }
                if cost_regressed {
                    regression_details.push(format!("cost +{:.1}%", cost_change));
                }
                if errors_increased {
                    regression_details.push(format!("errors +{}", r2.metrics.tool_errors - r1.metrics.tool_errors));
                }
                regressed_benchmarks.push(format!("{}: {}", id, regression_details.join(", ")));
            }
        }
    }

    println!();

    if has_regression {
        println!("❌ **Benchmark regression detected!** One or more metrics regressed by more than {:.0}%", threshold * 100.0);
        println!();
        println!("**Regressed benchmarks:**");
        for regression in &regressed_benchmarks {
            println!("- {}", regression);
        }
        println!();
        std::process::exit(1);
    } else {
        println!("✅ **All benchmarks passed!** No regressions detected.");
        println!();
    }

    Ok(())
}

/// Handle benchmark command
async fn handle_benchmark_command(
    benchmark_id: Option<String>,
    category: Option<String>,
    baseline: Option<String>,
    save_baseline: Option<String>,
    list_baselines: bool,
    compare_commits: Option<String>,
    compare_db1: Option<PathBuf>,
    compare_db2: Option<PathBuf>,
    threshold: f64,
    db: Option<PathBuf>,
    workspace: Option<PathBuf>,
) -> Result<()> {
    use autopilot::benchmark::{
        BenchmarkRunner, BenchmarkDatabase,
        B001SimpleFileEdit, B002MultiFileEdit, B003StructRename,
        B004SimpleCommit, B005BranchWorkflow,
        B006IssueWorkflow, B007MultiStepRefactor, B008TestDrivenFix,
        B009DocumentationGeneration, B010DependencyUpdate,
        B011ErrorRecovery, B012ContextGathering, B013CrossFileConsistency,
        B014PerformanceOptimization, B015SecurityFix,
    };

    let db_path = db.unwrap_or_else(|| PathBuf::from("autopilot-benchmarks.db"));
    let workspace_path = workspace.unwrap_or_else(|| PathBuf::from("benchmark-workspace"));
    let version = save_baseline.clone().unwrap_or_else(|| "current".to_string());

    let mut runner = BenchmarkRunner::new(workspace_path.clone(), db_path.clone(), version.clone())?;

    // All available tasks
    let all_tasks: Vec<(&str, Box<dyn autopilot::benchmark::BenchmarkTask>)> = vec![
        ("file-ops", Box::new(B001SimpleFileEdit)),
        ("file-ops", Box::new(B002MultiFileEdit)),
        ("file-ops", Box::new(B003StructRename)),
        ("git", Box::new(B004SimpleCommit)),
        ("git", Box::new(B005BranchWorkflow)),
        ("autopilot", Box::new(B006IssueWorkflow)),
        ("file-ops", Box::new(B007MultiStepRefactor)),
        ("testing", Box::new(B008TestDrivenFix)),
        ("docs", Box::new(B009DocumentationGeneration)),
        ("tooling", Box::new(B010DependencyUpdate)),
        ("resilience", Box::new(B011ErrorRecovery)),
        ("exploration", Box::new(B012ContextGathering)),
        ("refactor", Box::new(B013CrossFileConsistency)),
        ("optimization", Box::new(B014PerformanceOptimization)),
        ("security", Box::new(B015SecurityFix)),
    ];

    // Handle list baselines
    if list_baselines {
        println!("\n{}", "═".repeat(80));
        println!("{} Benchmark Baselines", "📊".cyan().bold());
        println!("{}", "═".repeat(80));
        println!();

        // Query all distinct versions with baselines
        let conn = rusqlite::Connection::open(&db_path)?;
        let mut stmt = conn.prepare(
            "SELECT DISTINCT version FROM benchmark_baselines ORDER BY version"
        )?;
        let versions: Vec<String> = stmt
            .query_map([], |row| row.get(0))?
            .collect::<std::result::Result<_, _>>()?;

        if versions.is_empty() {
            println!("  No baselines found. Run benchmarks with --save-baseline to create one.");
        } else {
            for version in versions {
                // Get sample count and last updated for this version
                let (count, updated): (i64, String) = conn.query_row(
                    "SELECT COUNT(*), MAX(updated_at) FROM benchmark_baselines WHERE version = ?1",
                    [&version],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )?;
                println!("  {} - {} benchmarks (updated: {})",
                    version.cyan().bold(), count, updated);
            }
        }
        println!();
        return Ok(());
    }

    // Handle database comparison for regression detection
    if let (Some(db1), Some(db2)) = (compare_db1, compare_db2) {
        return handle_benchmark_comparison(&db1, &db2, threshold).await;
    }

    // Handle git commit comparison
    if let Some(commits) = compare_commits {
        let parts: Vec<&str> = commits.split("..").collect();
        if parts.len() != 2 {
            anyhow::bail!("Invalid compare format. Expected: commit1..commit2");
        }

        println!("{}", "=".repeat(80));
        println!("{} Comparing Git Commits", "🔍".cyan().bold());
        println!("{}", "=".repeat(80));
        println!();
        println!("  Commit 1: {}", parts[0]);
        println!("  Commit 2: {}", parts[1]);
        println!();

        // Get current branch
        let current_branch = std::process::Command::new("git")
            .args(&["rev-parse", "--abbrev-ref", "HEAD"])
            .output()?;
        let current_branch = String::from_utf8_lossy(&current_branch.stdout).trim().to_string();

        // Get results for commit 1
        let commit1_results = {
            std::process::Command::new("git")
                .args(&["checkout", parts[0]])
                .output()?;
            let db = BenchmarkDatabase::open(&db_path)?;
            let results = db.get_baseline(parts[0])?;
            results
        };

        // Get results for commit 2
        let commit2_results = {
            std::process::Command::new("git")
                .args(&["checkout", parts[1]])
                .output()?;
            let db = BenchmarkDatabase::open(&db_path)?;
            let results = db.get_baseline(parts[1])?;
            results
        };

        // Restore original branch
        std::process::Command::new("git")
            .args(&["checkout", &current_branch])
            .output()?;

        if commit1_results.is_empty() || commit2_results.is_empty() {
            println!("⚠ No benchmark data found for one or both commits.");
            println!("  Run benchmarks first with --save-baseline");
            return Ok(());
        }

        // Compare results
        println!("{} Comparison Results:", "📊".cyan());
        println!("{:<20} {:>15} {:>15} {:>12}", "Benchmark", parts[0], parts[1], "Change");
        println!("{}", "-".repeat(80));

        let by_id1: std::collections::HashMap<_, _> = commit1_results.iter()
            .map(|r| (r.benchmark_id.clone(), r))
            .collect();
        let by_id2: std::collections::HashMap<_, _> = commit2_results.iter()
            .map(|r| (r.benchmark_id.clone(), r))
            .collect();

        for (id, r2) in &by_id2 {
            if let Some(r1) = by_id1.get(id) {
                let duration_change = ((r2.metrics.duration_ms as f64 - r1.metrics.duration_ms as f64)
                    / r1.metrics.duration_ms as f64) * 100.0;
                let change_str = if duration_change > 0.0 {
                    format!("+{:.1}%", duration_change).red()
                } else {
                    format!("{:.1}%", duration_change).green()
                };
                println!("{:<20} {:>12}ms {:>12}ms {:>12}",
                    id,
                    r1.metrics.duration_ms,
                    r2.metrics.duration_ms,
                    change_str
                );
            }
        }

        println!();
        return Ok(());
    }

    // Determine which tasks to run
    let tasks_to_run: Vec<&Box<dyn autopilot::benchmark::BenchmarkTask>> = if let Some(id) = &benchmark_id {
        all_tasks.iter().filter(|(_, t)| t.id() == id).map(|(_, t)| t).collect()
    } else if let Some(cat) = &category {
        all_tasks.iter().filter(|(c, _)| *c == cat).map(|(_, t)| t).collect()
    } else {
        all_tasks.iter().map(|(_, t)| t).collect()
    };

    if tasks_to_run.is_empty() {
        println!("No benchmarks match the specified criteria");
        return Ok(());
    }

    println!("{}", "=".repeat(80));
    println!("{} Autopilot Benchmark Suite", "🏁".cyan().bold());
    println!("{}", "=".repeat(80));
    println!();
    println!("  Version: {}", version);
    println!("  Database: {}", db_path.display());
    println!("  Workspace: {}", workspace_path.display());
    println!("  Tasks: {}", tasks_to_run.len());
    println!();

    // Run benchmarks
    let mut results = Vec::new();
    for task in tasks_to_run {
        match runner.run_benchmark(task.as_ref()).await {
            Ok(result) => {
                let status = if result.success {
                    "PASS".green().bold()
                } else {
                    "FAIL".red().bold()
                };
                println!("  {} {} - {}", status, result.benchmark_id, task.name());
                results.push(result);
            }
            Err(e) => {
                println!("  {} {} - Error: {}", "ERR".yellow().bold(), task.id(), e);
            }
        }
    }

    println!();
    println!("{}", "=".repeat(80));
    println!("{} Results", "📊".cyan().bold());
    println!("{}", "=".repeat(80));
    println!();

    let passed = results.iter().filter(|r| r.success).count();
    let failed = results.len() - passed;

    println!("  Total: {}", results.len());
    println!("  Passed: {}", passed.to_string().green().bold());
    if failed > 0 {
        println!("  Failed: {}", failed.to_string().red().bold());
    }
    println!();

    // Save baseline if requested
    if save_baseline.is_some() {
        let mut db = BenchmarkDatabase::open(&db_path)?;
        println!("{}", "─".repeat(80));
        println!("{} Saving Baseline", "💾".cyan().bold());
        println!("{}", "─".repeat(80));
        println!();
        println!("  Version: {}", version.cyan().bold());
        db.update_baseline(&version)?;
        println!("  ✓ Baseline metrics computed and stored");
        println!();
    }

    // Compare to baseline if requested
    if let Some(baseline_ver) = baseline {
        let report = runner.compare_to_baseline(&results, &baseline_ver)?;
        report.print();
    }

    Ok(())
}

/// Handle APM commands
async fn handle_apm_command(command: ApmCommands) -> Result<()> {
    use autopilot::apm::{APMSource, APMTier};
    use autopilot::apm_storage::{get_latest_snapshot, get_sessions_by_source, get_session_stats, init_apm_tables};
    use autopilot::default_db_path;
    use colored::Colorize;
    use rusqlite::Connection;

    let default_db = default_db_path();

    match command {
        ApmCommands::Stats { source, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = Connection::open(&db_path)?;
            init_apm_tables(&conn)?;

            let source_filter = source.as_deref().map(|s| match s {
                "autopilot" => APMSource::Autopilot,
                "claude_code" | "claude" => APMSource::ClaudeCode,
                _ => {
                    eprintln!("Invalid source: {}. Valid values: autopilot, claude_code", s);
                    std::process::exit(1);
                }
            });

            println!("{}", "APM Statistics".cyan().bold());
            println!("{}", "─".repeat(70).dimmed());
            println!();

            let sources = if let Some(s) = source_filter {
                vec![s]
            } else {
                vec![APMSource::Autopilot, APMSource::ClaudeCode]
            };

            for src in sources {
                let latest = get_latest_snapshot(&conn, src, autopilot::apm::APMWindow::Lifetime)?;

                if let Some(snap) = latest {
                    let tier = APMTier::from_apm(snap.apm);
                    println!(
                        "{:<15} {:>8.1} APM  ({}) - {} messages, {} tool calls",
                        format!("{:?}", src).green().bold(),
                        snap.apm,
                        tier.name().yellow(),
                        snap.messages,
                        snap.tool_calls
                    );
                } else {
                    println!("{:<15} {}", format!("{:?}", src).dimmed(), "No data".dimmed());
                }
            }

            println!();
            Ok(())
        }
        ApmCommands::Sessions { source, limit, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = Connection::open(&db_path)?;
            init_apm_tables(&conn)?;

            let src = source.as_deref().map(|s| match s {
                "autopilot" => APMSource::Autopilot,
                "claude_code" | "claude" => APMSource::ClaudeCode,
                _ => {
                    eprintln!("Invalid source: {}. Valid values: autopilot, claude_code", s);
                    std::process::exit(1);
                }
            }).unwrap_or(APMSource::Autopilot);

            let sessions = get_sessions_by_source(&conn, src)?;

            println!("{}", "APM Sessions".cyan().bold());
            println!("{}", "─".repeat(70).dimmed());
            println!();

            for (id, start_time, end_time) in sessions.iter().take(limit) {
                let status = if end_time.is_some() { "✓" } else { "•" };
                println!("{} {:<20} {}", status.green(), &id[..id.len().min(20)], start_time.format("%Y-%m-%d %H:%M:%S"));
            }

            println!();
            println!("Showing {} of {} sessions", sessions.len().min(limit), sessions.len());
            Ok(())
        }
        ApmCommands::Show { session_id, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = Connection::open(&db_path)?;
            init_apm_tables(&conn)?;

            let (messages, tool_calls) = get_session_stats(&conn, &session_id)?;

            println!("{}", format!("APM Session: {}", session_id).cyan().bold());
            println!("{}", "─".repeat(70).dimmed());
            println!();
            println!("{:<20} {}", "Messages:", messages);
            println!("{:<20} {}", "Tool Calls:", tool_calls);
            println!("{:<20} {}", "Total Actions:", messages + tool_calls);
            println!();

            Ok(())
        }
        ApmCommands::Export { output, source, db } => {
            let db_path = db.unwrap_or(default_db);
            let conn = Connection::open(&db_path)?;
            init_apm_tables(&conn)?;

            let src = source.as_deref().map(|s| match s {
                "autopilot" => APMSource::Autopilot,
                "claude_code" | "claude" => APMSource::ClaudeCode,
                _ => {
                    eprintln!("Invalid source: {}. Valid values: autopilot, claude_code", s);
                    std::process::exit(1);
                }
            }).unwrap_or(APMSource::Autopilot);

            let sessions = get_sessions_by_source(&conn, src)?;

            let export_data = serde_json::json!({
                "source": src.as_str(),
                "exported_at": chrono::Utc::now().to_rfc3339(),
                "sessions": sessions.iter().map(|(id, start, end)| {
                    serde_json::json!({
                        "id": id,
                        "start_time": start.to_rfc3339(),
                        "end_time": end.as_ref().map(|t| t.to_rfc3339()),
                    })
                }).collect::<Vec<_>>()
            });

            std::fs::write(&output, serde_json::to_string_pretty(&export_data)?)?;
            println!("{} Exported {} sessions to {}", "✓".green(), sessions.len(), output.display());

            Ok(())
        }
        ApmCommands::Watch { interval, source, db } => {
            let db_path = db.unwrap_or(default_db);

            let src = source.as_deref().map(|s| match s {
                "autopilot" => APMSource::Autopilot,
                "claude_code" | "claude" => APMSource::ClaudeCode,
                _ => {
                    eprintln!("Invalid source: {}. Valid values: autopilot, claude_code", s);
                    std::process::exit(1);
                }
            }).unwrap_or(APMSource::Autopilot);

            println!("{}", "Press Ctrl+C to stop...".dimmed());
            println!();

            loop {
                // Clear screen (ANSI escape code)
                print!("\x1B[2J\x1B[1;1H");

                let conn = Connection::open(&db_path)?;
                init_apm_tables(&conn)?;

                // Get latest session
                let sessions = get_sessions_by_source(&conn, src)?;

                if let Some((session_id, start_time, end_time)) = sessions.first() {
                    let (messages, tool_calls) = get_session_stats(&conn, session_id)?;
                    let total_actions = messages + tool_calls;

                    // Calculate duration
                    let now = chrono::Utc::now();
                    let duration_secs = if let Some(end) = end_time {
                        (end.timestamp() - start_time.timestamp()) as f64
                    } else {
                        (now.timestamp() - start_time.timestamp()) as f64
                    };
                    let duration_mins = duration_secs / 60.0;

                    // Calculate current APM
                    let apm = if duration_mins > 0.0 {
                        total_actions as f64 / duration_mins
                    } else {
                        0.0
                    };

                    let tier = APMTier::from_apm(apm);
                    let status = if end_time.is_some() { "Complete" } else { "Running" };

                    println!("{}", "═".repeat(70).cyan());
                    println!("{:^70}", format!("APM Dashboard - {:?}", src).bold());
                    println!("{}", "═".repeat(70).cyan());
                    println!();

                    println!("{:<20} {}", "Session:", &session_id[..session_id.len().min(40)]);
                    println!("{:<20} {}", "Status:", if end_time.is_some() { status.green() } else { status.yellow() });
                    println!("{:<20} {:.1} minutes", "Duration:", duration_mins);
                    println!();

                    println!("{:<20} {}", "Messages:", messages.to_string().cyan());
                    println!("{:<20} {}", "Tool Calls:", tool_calls.to_string().cyan());
                    println!("{:<20} {}", "Total Actions:", total_actions.to_string().cyan().bold());
                    println!();

                    println!("{:<20} {:.1} APM", "Current APM:", apm);
                    println!("{:<20} {}", "Tier:", tier.name().yellow().bold());
                    println!();

                    // Show tier thresholds
                    println!("{}", "Tier Thresholds:".dimmed());
                    println!("  {} 0-5   {} 5-10   {} 10-15   {} 15-20   {} 20+",
                        "Baseline".dimmed(),
                        "Active".green(),
                        "Productive".cyan(),
                        "Elite".yellow(),
                        "Superhuman".magenta().bold()
                    );
                } else {
                    println!("{}", "No active sessions found".dimmed());
                }

                println!();
                println!("{}", format!("Updated: {} | Refresh: {}s", chrono::Local::now().format("%H:%M:%S"), interval).dimmed());

                tokio::time::sleep(tokio::time::Duration::from_secs(interval)).await;
            }
        }
        ApmCommands::Baseline { command } => {
            use autopilot::apm::APMBaseline;
            use autopilot::metrics::{delete_apm_baseline, get_apm_baseline, list_apm_baselines, store_apm_baseline};

            let default_db = default_db_path();

            match command {
                BaselineCommands::List { db } => {
                    let db_path = db.unwrap_or(default_db);
                    let baselines = list_apm_baselines(&db_path)?;

                    if baselines.is_empty() {
                        println!("No APM baselines found.");
                        println!();
                        println!("Create a baseline with:");
                        println!("  cargo autopilot apm baseline set <id> <name> --source autopilot --median 19.0");
                        return Ok(());
                    }

                    println!("{}", "═".repeat(80).cyan());
                    println!("{:^80}", "APM Baselines".bold());
                    println!("{}", "═".repeat(80).cyan());
                    println!();

                    for baseline in baselines {
                        println!("{} {}", "ID:".bold(), baseline.id);
                        println!("  Name:        {}", baseline.name);
                        println!("  Source:      {:?}", baseline.source);
                        println!("  Median APM:  {:.1}", baseline.median_apm);
                        println!("  Range:       {:.1} - {:.1}", baseline.min_apm, baseline.max_apm);
                        println!("  Samples:     {}", baseline.sample_size);
                        println!();
                    }
                }
                BaselineCommands::Set { id, name, source, median, min, max, db } => {
                    let db_path = db.unwrap_or(default_db);

                    let source_enum = match source.as_str() {
                        "autopilot" => APMSource::Autopilot,
                        "claude_code" | "claude" => APMSource::ClaudeCode,
                        "combined" => APMSource::Combined,
                        _ => {
                            eprintln!("Invalid source: {}. Valid values: autopilot, claude_code, combined", source);
                            std::process::exit(1);
                        }
                    };

                    let baseline = if let (Some(min_val), Some(max_val)) = (min, max) {
                        APMBaseline::with_thresholds(id, name, source_enum, median, min_val, max_val)
                    } else {
                        APMBaseline::new(id, name, source_enum, median)
                    };

                    store_apm_baseline(&db_path, &baseline)?;

                    println!("{}", "✓ Baseline saved".green().bold());
                    println!();
                    println!("{} {}", "ID:".bold(), baseline.id);
                    println!("  Name:        {}", baseline.name);
                    println!("  Source:      {:?}", baseline.source);
                    println!("  Median APM:  {:.1}", baseline.median_apm);
                    println!("  Min APM:     {:.1} ({})", baseline.min_apm, "warning threshold".yellow());
                    println!("  Max APM:     {:.1} ({})", baseline.max_apm, "excellent threshold".green());
                }
                BaselineCommands::Show { id, db } => {
                    let db_path = db.unwrap_or(default_db);
                    let baseline = get_apm_baseline(&db_path, &id)?;

                    match baseline {
                        Some(b) => {
                            println!("{}", "═".repeat(60).cyan());
                            println!("{:^60}", format!("Baseline: {}", b.name).bold());
                            println!("{}", "═".repeat(60).cyan());
                            println!();
                            println!("{:<20} {}", "ID:", b.id);
                            println!("{:<20} {:?}", "Source:", b.source);
                            println!("{:<20} {:.1} APM", "Median:", b.median_apm);
                            println!("{:<20} {:.1} APM ({})", "Minimum:", b.min_apm, "warning".yellow());
                            println!("{:<20} {:.1} APM ({})", "Maximum:", b.max_apm, "excellent".green());
                            println!("{:<20} {}", "Samples:", b.sample_size);
                            println!("{:<20} {}", "Created:", b.created_at.format("%Y-%m-%d %H:%M:%S"));
                            println!("{:<20} {}", "Updated:", b.updated_at.format("%Y-%m-%d %H:%M:%S"));
                            println!();
                        }
                        None => {
                            println!("{}", format!("Baseline '{}' not found", id).red());
                        }
                    }
                }
                BaselineCommands::Delete { id, db } => {
                    let db_path = db.unwrap_or(default_db);
                    delete_apm_baseline(&db_path, &id)?;
                    println!("{}", format!("✓ Baseline '{}' deleted", id).green().bold());
                }
                BaselineCommands::Check { baseline_id, apm, db } => {
                    use autopilot::apm_storage::get_sessions_by_source;
                    use rusqlite::Connection;

                    let db_path = db.unwrap_or(default_db);
                    let baseline = get_apm_baseline(&db_path, &baseline_id)?;

                    match baseline {
                        Some(b) => {
                            let current_apm = if let Some(apm_val) = apm {
                                apm_val
                            } else {
                                // Get latest session APM
                                let conn = Connection::open(&db_path)?;
                                let sessions = get_sessions_by_source(&conn, b.source)?;
                                if let Some((session_id, start, end)) = sessions.first() {
                                    use autopilot::apm_storage::get_session_stats;
                                    let (messages, tool_calls) = get_session_stats(&conn, session_id)?;
                                    let duration_secs = if let Some(end_time) = end {
                                        (end_time.timestamp() - start.timestamp()) as f64
                                    } else {
                                        (chrono::Utc::now().timestamp() - start.timestamp()) as f64
                                    };
                                    let duration_mins = duration_secs / 60.0;
                                    if duration_mins > 0.0 {
                                        (messages + tool_calls) as f64 / duration_mins
                                    } else {
                                        0.0
                                    }
                                } else {
                                    println!("{}", "No sessions found to check".yellow());
                                    return Ok(());
                                }
                            };

                            let status = b.status(current_apm);
                            let deviation = b.deviation_pct(current_apm);

                            println!("{}", "═".repeat(60).cyan());
                            println!("{:^60}", format!("Baseline Check: {}", b.name).bold());
                            println!("{}", "═".repeat(60).cyan());
                            println!();
                            println!("{:<20} {:.1} APM", "Current APM:", current_apm);
                            println!("{:<20} {:.1} APM", "Baseline Median:", b.median_apm);
                            println!("{:<20} {:.1}%", "Deviation:", deviation.abs());
                            println!();
                            println!("{:<20} {} {}", "Status:", status.emoji(), match status {
                                autopilot::apm::BaselineStatus::BelowBaseline => "Below Baseline (Performance Regression)".red().bold(),
                                autopilot::apm::BaselineStatus::Normal => "Within Expected Range".green(),
                                autopilot::apm::BaselineStatus::AboveBaseline => "Above Baseline (Excellent Performance)".yellow().bold(),
                            });
                            println!();

                            if deviation < -20.0 {
                                println!("{}", "⚠️  WARNING: APM is more than 20% below baseline!".red().bold());
                                println!("   This may indicate a performance regression.");
                                println!();
                            } else if deviation > 50.0 {
                                println!("{}", "⭐ EXCELLENT: APM is significantly above baseline!".yellow().bold());
                                println!();
                            }
                        }
                        None => {
                            println!("{}", format!("Baseline '{}' not found", baseline_id).red());
                        }
                    }
                }
            }
            Ok(())
        }
        ApmCommands::RegenerateSnapshots { db } => {
            use autopilot::apm_storage::{init_apm_tables, regenerate_all_snapshots};
            use rusqlite::Connection;

            let db_path = db.unwrap_or(default_db);
            let conn = Connection::open(&db_path)?;
            init_apm_tables(&conn)?;

            println!("{}", "Regenerating APM snapshots...".cyan().bold());
            println!();

            let count = regenerate_all_snapshots(&conn)?;

            println!("{}", "✓ Snapshot regeneration complete".green().bold());
            println!();
            println!("{:<20} {}", "Snapshots created:", count);
            println!();

            Ok(())
        }
        ApmCommands::Best { metric, project, db } => {
            use autopilot::metrics::MetricsDb;
            use std::iter;

            let db_path = db.unwrap_or(default_db);
            let metrics_db = MetricsDb::open(&db_path)?;

            println!("{}", "Personal Best Records".cyan().bold());
            println!("{}", iter::repeat("─").take(60).collect::<String>());
            println!();

            if let Some(metric_name) = metric {
                // Show specific metric
                if let Some(best) = metrics_db.get_personal_best(&metric_name, project.as_deref())? {
                    display_personal_best(&best);
                } else {
                    println!("{}", format!("No personal best found for {}", metric_name).yellow());
                }
            } else {
                // Show all personal bests
                let bests = metrics_db.get_all_personal_bests()?;

                if bests.is_empty() {
                    println!("{}", "No personal bests recorded yet.".yellow());
                    println!("{}", "Run autopilot sessions to start tracking your bests!".dimmed());
                } else {
                    for best in bests {
                        display_personal_best(&best);
                        println!();
                    }
                }
            }

            println!();
            Ok(())
        }
    }
}

fn display_personal_best(best: &autopilot::metrics::PersonalBest) {
    use colored::Colorize;

    println!("{:<20} {}", "Metric:".bold(), best.metric);
    println!("{:<20} {:.2}", "Best Value:".bold(), best.value);

    if let Some(ref session_id) = best.session_id {
        println!("{:<20} {}", "Session:".bold(), session_id.dimmed());
    }

    if let Some(ref project) = best.project {
        println!("{:<20} {}", "Project:".bold(), project);
    }

    println!("{:<20} {}", "Achieved:".bold(), best.timestamp.format("%Y-%m-%d %H:%M:%S").to_string().dimmed());

    if let Some(ref context) = best.context {
        println!("{:<20} {}", "Context:".bold(), context.dimmed());
    }
}

/// Handle logs commands
async fn handle_logs_command(command: LogsCommands) -> Result<()> {
    use autopilot::logs::{self, LogsConfig};

    match command {
        LogsCommands::Stats { logs_dir } => {
            let config = LogsConfig {
                logs_dir: logs_dir.unwrap_or_else(|| PathBuf::from("docs/logs")),
                ..Default::default()
            };

            let stats = logs::calculate_log_size(&config)?;
            logs::print_stats(&stats);

            Ok(())
        }
        LogsCommands::Archive { days, logs_dir, dry_run } => {
            let config = LogsConfig {
                logs_dir: logs_dir.unwrap_or_else(|| PathBuf::from("docs/logs")),
                archive_after_days: days,
                ..Default::default()
            };

            if dry_run {
                println!("{} Running in dry-run mode (no changes will be made)\n", "ℹ️ ".cyan());
            }

            println!("{} Archiving logs older than {} days...\n", "📦".cyan(), days);

            let archived = logs::archive_logs(&config, dry_run)?;

            println!("\n{} Archived {} files", "✓".green(), archived.len());

            Ok(())
        }
        LogsCommands::Cleanup { days, logs_dir, db, dry_run } => {
            let config = LogsConfig {
                logs_dir: logs_dir.unwrap_or_else(|| PathBuf::from("docs/logs")),
                delete_after_days: days,
                db_path: db.or_else(|| Some(PathBuf::from("autopilot.db"))),
                ..Default::default()
            };

            if dry_run {
                println!("{} Running in dry-run mode (no changes will be made)\n", "ℹ️ ".cyan());
            }

            println!("{} Cleaning up archived logs older than {} days...\n", "🗑️ ".cyan(), days);

            let deleted = logs::cleanup_logs(&config, dry_run)?;

            println!("\n{} Deleted {} files", "✓".green(), deleted.len());

            Ok(())
        }
    }
}

/// Handle notify command
async fn handle_notify_command(
    title: String,
    message: String,
    severity: String,
    webhook: Vec<String>,
    config: Option<PathBuf>,
    metadata: Vec<String>,
) -> Result<()> {
    use autopilot::notifications::{Notification, NotificationConfig, NotificationManager};
    use std::collections::HashMap;

    // Parse metadata from key=value pairs
    let mut metadata_map = HashMap::new();
    for meta in metadata {
        if let Some((key, value)) = meta.split_once('=') {
            metadata_map.insert(key.to_string(), value.to_string());
        } else {
            eprintln!("Warning: Invalid metadata format '{}', expected key=value", meta);
        }
    }

    // Create notification
    let notification = Notification {
        title,
        message,
        severity,
        timestamp: chrono::Utc::now().to_rfc3339(),
        metadata: metadata_map,
    };

    // Load config if specified, otherwise use webhooks from CLI
    if let Some(config_path) = config {
        let config_content = std::fs::read_to_string(&config_path)?;
        let config: NotificationConfig = toml::from_str(&config_content)?;
        let manager = NotificationManager::new(config);
        manager.send(&notification).await?;
    } else if !webhook.is_empty() {
        // Use CLI-provided webhooks directly
        for url in webhook {
            notification.send_webhook(&url).await?;
        }
    } else {
        // Try default config location
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let default_config = PathBuf::from(home).join(".openagents").join("notifications.toml");

        if default_config.exists() {
            let config_content = std::fs::read_to_string(&default_config)?;
            let config: NotificationConfig = toml::from_str(&config_content)?;
            let manager = NotificationManager::new(config);
            manager.send(&notification).await?;
        } else {
            anyhow::bail!("No webhook URLs provided and no config file found. Use --webhook or create ~/.openagents/notifications.toml");
        }
    }

    println!("{} Notification sent successfully", "✓".green());
    Ok(())
}
