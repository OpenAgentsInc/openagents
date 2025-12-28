use std::io::{self, Write};
use std::time::Instant;
use tracing::info;

use autopilot::{StartupState, LogStatus, ClaudeModel, StartupPhase};

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| {
                    tracing_subscriber::EnvFilter::new(
                        "autopilot=debug,openagents=debug,claude_agent_sdk=info,info"
                    )
                })
        )
        .with_target(true)
        .init();

    info!("Starting Autopilot CLI");
    println!("\n=== AUTOPILOT CLI MODE ===\n");

    let model = match std::env::var("AUTOPILOT_MODEL").as_deref() {
        Ok("opus") | Ok("Opus") | Ok("OPUS") => {
            println!("Model: Claude Opus");
            ClaudeModel::Opus
        }
        _ => {
            println!("Model: Claude Sonnet");
            ClaudeModel::Sonnet
        }
    };

    let mut startup_state = StartupState::with_model(model);
    let start_time = Instant::now();
    let mut last_line_count = 0;
    let mut last_phase = StartupPhase::CheckingOpenCode;

    loop {
        let elapsed = start_time.elapsed().as_secs_f32();
        
        startup_state.tick(elapsed);

        if startup_state.lines.len() > last_line_count {
            for line in startup_state.lines.iter().skip(last_line_count) {
                let prefix = match line.status {
                    LogStatus::Pending => "\x1b[33m>\x1b[0m",
                    LogStatus::Success => "\x1b[32m✓\x1b[0m",
                    LogStatus::Error => "\x1b[31m✗\x1b[0m",
                    LogStatus::Info => "\x1b[36m·\x1b[0m",
                    LogStatus::Thinking => "\x1b[35m…\x1b[0m",
                };
                
                if line.text.is_empty() {
                    println!();
                } else {
                    println!("{} {}", prefix, line.text);
                }
                io::stdout().flush().ok();
            }
            last_line_count = startup_state.lines.len();
        }

        if startup_state.phase != last_phase {
            info!("Phase transition: {:?} -> {:?}", last_phase, startup_state.phase);
            last_phase = startup_state.phase;
        }

        if startup_state.phase == StartupPhase::Complete {
            println!("\n=== AUTOPILOT COMPLETE ===\n");
            break;
        }

        std::thread::sleep(std::time::Duration::from_millis(100));
    }
}
