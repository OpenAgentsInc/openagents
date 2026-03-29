use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use autopilot_desktop::compiled_agent_slice::{
    CompiledAgentFeedbackSignal, CompiledAgentSliceState, run_compiled_agent_slice,
};
use clap::{Parser, ValueEnum};
use openagents_compiled_agent::ShadowMode;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ShadowModeArg {
    Disabled,
    EvaluateCandidate,
    CandidateAuthority,
}

#[derive(Debug, Parser)]
#[command(name = "autopilot-compiled-agent-harness")]
#[command(about = "Run the first narrow compiled-agent vertical slice and emit a receipt.")]
struct Args {
    #[arg(long)]
    prompt: String,
    #[arg(long, default_value_t = true)]
    provider_ready: bool,
    #[arg(long = "provider-blocker")]
    provider_blockers: Vec<String>,
    #[arg(long, default_value_t = 1_200)]
    wallet_balance_sats: u64,
    #[arg(long, default_value_t = 240)]
    recent_earnings_sats: u64,
    #[arg(long, value_enum, default_value_t = ShadowModeArg::Disabled)]
    shadow_mode: ShadowModeArg,
    #[arg(long, default_value = "psionic_candidate")]
    candidate_label: String,
    #[arg(long)]
    receipt_out: Option<PathBuf>,
    #[arg(long, default_value_t = false)]
    show_trace: bool,
    #[arg(long, default_value_t = false)]
    user_disagreed: bool,
    #[arg(long)]
    correction_text: Option<String>,
    #[arg(long)]
    disagreement_reason_code: Option<String>,
    #[arg(long)]
    operator_note: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let state = CompiledAgentSliceState {
        provider_ready: args.provider_ready,
        provider_blockers: args.provider_blockers,
        wallet_balance_sats: args.wallet_balance_sats,
        recent_earnings_sats: args.recent_earnings_sats,
    };
    let shadow_mode = match args.shadow_mode {
        ShadowModeArg::Disabled => ShadowMode::Disabled,
        ShadowModeArg::EvaluateCandidate => ShadowMode::EvaluateCandidate {
            label: args.candidate_label.clone(),
        },
        ShadowModeArg::CandidateAuthority => ShadowMode::CandidateAuthority {
            label: args.candidate_label.clone(),
        },
    };

    let mut receipt = run_compiled_agent_slice(&args.prompt, &state, shadow_mode);
    if args.user_disagreed
        || args.correction_text.is_some()
        || args.disagreement_reason_code.is_some()
        || args.operator_note.is_some()
    {
        receipt = receipt.with_feedback(CompiledAgentFeedbackSignal {
            disagreed: args.user_disagreed,
            correction_text: args.correction_text.clone(),
            reason_code: args.disagreement_reason_code.clone(),
            operator_note: args.operator_note.clone(),
        });
    }

    println!("== Public Response ==");
    println!("{}", receipt.run.public_response.response);
    println!();
    println!("== Outcome ==");
    println!("{:?}", receipt.run.public_response.kind);
    println!();
    println!("== Runtime Telemetry ==");
    println!(
        "{}",
        serde_json::to_string_pretty(&receipt.telemetry).context("serialize runtime telemetry")?
    );
    if args.show_trace {
        println!();
        println!("== Internal Trace ==");
        println!(
            "{}",
            serde_json::to_string_pretty(&receipt.run.internal_trace)
                .context("serialize internal trace")?
        );
    }

    if let Some(path) = args.receipt_out {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("create receipt directory {}", parent.display()))?;
        }
        let serialized =
            serde_json::to_string_pretty(&receipt).context("serialize compiled agent receipt")?;
        fs::write(&path, serialized)
            .with_context(|| format!("write compiled agent receipt to {}", path.display()))?;
        println!();
        println!("== Receipt ==");
        println!("{}", path.display());
    }

    Ok(())
}
