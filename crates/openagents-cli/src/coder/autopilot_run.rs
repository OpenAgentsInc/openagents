//! Headless Autopilot: `openagents coder --autopilot` (#328).
//!
//! The TUI mode (#307–#311) needs a person at the composer. This path is
//! what an agent invokes: take stock of the workspace, recent local
//! sessions, and open issues, then keep iterating until a stop condition.
//! `--dry-run` prints that plan and exits without a model.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crate::cli::{CoderArgs, fail};
use crate::coder::autopilot::{AutopilotState, StopConditions, StopReason};
use crate::coder_dev::DoorSpec;
use crate::runtime::{CoderRuntimeSession, Lane};
use crate::tools::{DelegationGate, HarnessToolRegistry};

/// Run Autopilot until a stop condition, or print the dry-run plan.
pub async fn run(
    coder: CoderArgs,
    api_base: String,
    token: Option<String>,
    repository: Option<String>,
    resumed: Option<crate::resume::Resumption>,
    door: DoorSpec,
) -> Result<(), Box<dyn std::error::Error>> {
    if !coder.autopilot && coder.dry_run {
        fail("--dry-run is an Autopilot flag. Try `openagents coder --autopilot --dry-run`.");
    }
    if coder.offline {
        fail(
            "--autopilot needs a model. --offline is the built-in stand-in. \
             Use `--autopilot --dry-run` to print the plan without calling a model.",
        );
    }
    if coder.delegate {
        fail("--autopilot cannot combine with --delegate");
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
    let snapshot = crate::coder::snapshot::workspace_snapshot(&cwd, None).await;
    let recent = recent_session_lines(&cwd);
    let directive = coder
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty());

    if coder.dry_run {
        print_dry_run(directive, &snapshot, &recent);
        return Ok(());
    }

    let lane_name = coder.lane_name().unwrap_or_else(|reason| fail(&reason));
    let lane = Lane::from_str(&lane_name);
    if !lane.is_local() && token.is_none() {
        fail(
            "Autopilot on a hosted lane needs an OpenAgents token. \
             Run `openagents auth login`, or pass `--lane local`.",
        );
    }

    let tools = HarnessToolRegistry::with_delegation(
        None,
        DelegationGate {
            lane: lane_name.clone(),
            user_token: token.clone(),
            api_base: Some(api_base.clone()),
            max_count: crate::delegate::MAX_DELEGATE_COUNT,
            child: coder.child_options(),
            acp_agents: crate::coder::acp::find_agents().await.unwrap_or_default(),
            acp_spent: Arc::new(AtomicBool::new(false)),
        },
    )
    .allowing_plugin_mounts();

    let mut runtime = CoderRuntimeSession::new(lane.clone(), Some(api_base), token, tools)
        .with_cloud_history(false)
        .use_openresponses(door == DoorSpec::OpenResponses);
    runtime.reasoning = coder
        .reasoning
        .clone()
        .or_else(|| lane.default_reasoning().map(str::to_string));
    runtime.repository = repository;
    if let Some(n) = coder.num_ctx {
        runtime.ollama_num_ctx = Some(n);
    }
    if let Some(resumption) = &resumed {
        if let Err(reason) = crate::resume::apply(&mut runtime, resumption).await {
            fail(&reason);
        }
        println!(
            "{}",
            crate::resume::resumed_line(
                resumption,
                runtime.last_model.as_deref().unwrap_or("an unnamed model")
            )
        );
    }
    runtime.seed_workspace_snapshot(&snapshot);

    let mut state = AutopilotState {
        engaged: true,
        directive: directive.map(str::to_string),
        discipline: Default::default(),
        stops: StopConditions::default(),
        stop_word: None,
    };

    let mut prompt = AutopilotState::opening_prompt(directive, &snapshot, &recent);
    println!("[autopilot] engaged");
    println!("{prompt}");
    println!();

    let mut iteration: u32 = 0;
    let outcome = loop {
        iteration = iteration.saturating_add(1);
        println!("[autopilot] iteration {iteration}");
        let started = std::time::Instant::now();
        let result = runtime
            .execute_turn(&prompt, |chunk| {
                print!("{chunk}");
                let _ = std::io::stdout().flush();
            })
            .await;
        state
            .stops
            .record_elapsed(started.elapsed().as_secs().max(1));

        match result {
            Ok(answer) => {
                if !answer.ends_with('\n') {
                    println!();
                }
                if let Some(model) = &runtime.last_model {
                    println!("Model: {model}");
                }
                if runtime.last_usage.reported() {
                    println!("Usage: {}", runtime.last_usage.line());
                }
            }
            Err(error) => {
                let _ = runtime.finish().await;
                fail(&format!(
                    "Autopilot stopped: the turn failed: {error}. \
                     The loop does not retry a dead hop."
                ));
            }
        }

        // Headless Autopilot has no TUI goal store yet; wall clock is the
        // budget signal until `/goal` is wired onto this path.
        if let Some(reason) = state.stops.should_stop(false) {
            break reason;
        }
        if !state.engaged {
            break StopReason::ReaderDisengage;
        }
        prompt = state.iteration_prompt();
    };

    let revoked = runtime.finish().await;
    println!("{}", outcome.line());
    match revoked {
        Ok(spent) => {
            if let Some(line) = runtime.spend_line(spent) {
                println!("{line}");
            }
        }
        Err(error) => eprintln!("oa: the thread was not ended: {error}"),
    }
    for failure in &runtime.record_failures {
        eprintln!("oa: {failure}");
    }
    Ok(())
}

fn print_dry_run(directive: Option<&str>, snapshot: &str, recent: &str) {
    println!("Autopilot dry-run. No model will be called.");
    if let Some(directive) = directive {
        println!("Directive: {directive}");
    } else {
        println!("Directive: (none — pick from open issues, recent sessions, and this workspace)");
    }
    println!("Stop: 1 hour wall clock, or the goal budget, whichever is shorter.");
    println!();
    println!("{snapshot}");
    if !recent.trim().is_empty() {
        println!();
        println!("Recent local sessions:");
        println!("{recent}");
    }
    println!();
    println!(
        "Would engage Autopilot and keep iterating until budget, blocked, or repeat-fail. \
         Drop --dry-run to run it."
    );
}

fn recent_session_lines(cwd: &Path) -> String {
    let mut rows = crate::session_store::summaries_for(&crate::session_store::default_root(), cwd);
    rows.sort_by_key(|(_, summary)| std::cmp::Reverse(summary.updated_at_ms));
    rows.truncate(5);
    if rows.is_empty() {
        return String::new();
    }
    rows.into_iter()
        .map(|(_, summary)| {
            let checkpoint = summary
                .last_checkpoint
                .as_deref()
                .map(str::trim)
                .filter(|text| !text.is_empty())
                .map(|text| {
                    let first = text.lines().next().unwrap_or(text);
                    format!("  last: {first}")
                })
                .unwrap_or_default();
            format!("{}  {}{checkpoint}", summary.id, summary.lane)
        })
        .collect::<Vec<_>>()
        .join("\n")
}
