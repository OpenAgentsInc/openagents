//! Guidance Modules CLI helpers.

use std::path::PathBuf;

use anyhow::Context;
use autopilot_core::guidance::{
    GuidanceGoal, GuidanceGuardrailConfig, GuidanceGuardrailContext, GuidanceInputs,
    GuidanceMode, GuidanceNetwork, GuidancePermissions, GuidanceState, apply_guidance_guardrails,
    ensure_guidance_demo_lm, guidance_demo_model, run_guidance_decision,
};
use clap::{Parser, Subcommand};
use dsrs::LM;
use serde_json::{Value, json};

#[derive(Parser)]
pub struct GuidanceArgs {
    #[command(subcommand)]
    pub command: GuidanceCommand,
}

#[derive(Subcommand)]
pub enum GuidanceCommand {
    /// Run the Guidance Modules demo (local Ollama)
    Demo(GuidanceDemoArgs),
}

#[derive(Parser)]
pub struct GuidanceDemoArgs {
    /// Optional JSON file with GuidanceInputs payload
    #[arg(long)]
    pub inputs: Option<PathBuf>,

    /// Optional JSON file with a FullAuto-style turn summary
    #[arg(long)]
    pub summary: Option<PathBuf>,

    /// Optional goal intent (overrides default)
    #[arg(long)]
    pub goal: Option<String>,

    /// Optional success criteria entries (repeatable)
    #[arg(long)]
    pub success: Vec<String>,

    /// Override Ollama model (defaults to guidance demo model)
    #[arg(long)]
    pub model: Option<String>,

    /// Print JSON output instead of a human summary
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: GuidanceArgs) -> anyhow::Result<()> {
    match args.command {
        GuidanceCommand::Demo(args) => run_demo(args).await,
    }
}

async fn run_demo(args: GuidanceDemoArgs) -> anyhow::Result<()> {
    let inputs = if let Some(path) = args.inputs {
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read inputs file: {path:?}"))?;
        serde_json::from_str::<GuidanceInputs>(&raw)
            .with_context(|| "Failed to parse GuidanceInputs JSON")?
    } else {
        let summary_value = if let Some(path) = args.summary {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read summary file: {path:?}"))?;
            serde_json::from_str::<Value>(&raw)
                .with_context(|| "Failed to parse summary JSON")?
        } else {
            demo_summary()
        };

        let turn_count = summary_value
            .get("turn_count")
            .or_else(|| summary_value.get("turnCount"))
            .and_then(|value| value.as_u64())
            .unwrap_or(1);
        let no_progress_count = summary_value
            .get("no_progress_count")
            .or_else(|| summary_value.get("noProgressCount"))
            .and_then(|value| value.as_u64())
            .unwrap_or(0) as u32;
        let tokens_used = summary_value
            .get("token_usage")
            .or_else(|| summary_value.get("tokenUsage"))
            .and_then(|value| value.as_str())
            .and_then(parse_total_tokens);

        let permissions = GuidancePermissions::new(true, true, GuidanceNetwork::Full);
        let mut state = GuidanceState::new(turn_count, no_progress_count, permissions);
        state.tokens_used = tokens_used;

        let goal_intent = args
            .goal
            .unwrap_or_else(|| "Ship the Guidance Modules demo".to_string());
        let goal = GuidanceGoal::new(goal_intent).with_success(args.success);

        GuidanceInputs {
            goal,
            summary: summary_value,
            state,
        }
    };

    let model = args.model.unwrap_or_else(guidance_demo_model);
    let model = if model.contains(':') {
        model
    } else {
        format!("ollama:{}", model)
    };
    let lm = if model == guidance_demo_model() {
        ensure_guidance_demo_lm()
            .await
            .map_err(|e| anyhow::anyhow!(e))?
    } else {
        LM::builder()
            .model(model)
            .temperature(0.2)
            .max_tokens(512)
            .build()
            .await
            .map_err(|e| anyhow::anyhow!("Failed to build guidance demo LM: {e}"))?
    };

    if GuidanceMode::from_env() == GuidanceMode::Legacy {
        eprintln!("Note: OPENAGENTS_GUIDANCE_MODE=legacy, but CLI demo always runs Guidance Modules.");
    }

    let decision_result = run_guidance_decision(&inputs, &lm)
        .await
        .map_err(|e| anyhow::anyhow!(e))?;

    let guardrail_config = GuidanceGuardrailConfig::default();
    let context = GuidanceGuardrailContext {
        last_turn_status: inputs
            .summary
            .get("last_turn_status")
            .or_else(|| inputs.summary.get("lastTurnStatus"))
            .and_then(|value| value.as_str())
            .unwrap_or("completed")
            .to_string(),
        turn_count: inputs.state.turn_count,
        no_progress_count: inputs.state.no_progress_count,
        tokens_used: inputs.state.tokens_used,
    };
    let decision = apply_guidance_guardrails(&context, &guardrail_config, decision_result.decision);

    if args.json {
        let payload = json!({
            "inputs": inputs,
            "decision": decision,
            "diagnostics": decision_result.diagnostics,
            "guardrails": {
                "context": context,
                "config": {
                    "min_confidence": guardrail_config.min_confidence,
                    "max_turns": guardrail_config.max_turns,
                    "no_progress_limit": guardrail_config.no_progress_limit,
                    "max_tokens": guardrail_config.max_tokens,
                }
            }
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
    } else {
        println!("Guidance Modules demo decision");
        println!("- action: {}", decision.action.as_str());
        println!("- confidence: {:.2}", decision.confidence);
        println!("- reason: {}", decision.reason);
        if let Some(next) = decision.next_input.as_deref() {
            println!("- next_input: {}", next);
        }
        if let Some(guardrail) = decision.guardrail.as_ref() {
            if guardrail.triggered {
                println!("- guardrail: {}", guardrail.rule.clone().unwrap_or_default());
            }
        }
    }

    Ok(())
}

fn parse_total_tokens(token_usage: &str) -> Option<u64> {
    let parsed: Value = serde_json::from_str(token_usage).ok()?;
    parsed
        .get("totalTokens")
        .or_else(|| parsed.get("total_tokens"))
        .and_then(|value| value.as_u64())
}

fn demo_summary() -> Value {
    json!({
        "thread_id": "demo-thread",
        "turn_id": "demo-turn",
        "last_turn_status": "completed",
        "turn_error": "",
        "turn_plan": "[{\"step\":\"Implement guidance demo\",\"status\":\"done\"}]",
        "diff_summary": "{\"files_changed\":2}",
        "token_usage": "{\"totalTokens\":420}",
        "pending_approvals": "0",
        "pending_tool_inputs": "0",
        "recent_actions": "[]",
        "compaction_events": "0",
        "turn_count": 1,
        "no_progress_count": 0
    })
}
