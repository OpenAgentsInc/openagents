//! Arrow-style text rendering for boot sequence.

use super::events::{BootEvent, StageDetails};
use std::io::{self, Write};

/// Symbols for visual output.
pub const SUCCESS: &str = "\u{2713}"; // ✓
pub const FAILURE: &str = "\u{2717}"; // ✗
pub const PENDING: &str = "\u{25CB}"; // ○
pub const ARROW: &str = "\u{2192}"; // →

/// Colors for terminal output (ANSI codes).
pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const GREEN: &str = "\x1b[32m";
    pub const RED: &str = "\x1b[31m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const CYAN: &str = "\x1b[36m";
    pub const DIM: &str = "\x1b[2m";
    pub const BOLD: &str = "\x1b[1m";
}

/// Render a boot event to the terminal.
pub fn render_event(event: &BootEvent) {
    match event {
        BootEvent::BootStarted { total_stages } => {
            println!();
            println!(
                "{}{}=== OpenAgents Boot ==={}",
                colors::BOLD,
                colors::CYAN,
                colors::RESET
            );
            println!("{}Stages: {}{}", colors::DIM, total_stages, colors::RESET);
            println!();
        }

        BootEvent::StageStarted {
            stage,
            description: _,
        } => {
            print!(
                "{}{} {}{} {}... ",
                colors::YELLOW,
                PENDING,
                colors::BOLD,
                stage.name(),
                colors::RESET
            );
            let _ = io::stdout().flush();
        }

        BootEvent::StageProgress { stage: _, message } => {
            println!("  {}{}  {}{}", colors::DIM, ARROW, message, colors::RESET);
        }

        BootEvent::StageCompleted {
            stage,
            duration,
            details,
        } => {
            // Clear the "..." line and print completed
            print!("\r");
            println!(
                "{}{} {}{} {}({:.0}ms){}",
                colors::GREEN,
                SUCCESS,
                colors::BOLD,
                stage.name(),
                colors::DIM,
                duration.as_millis(),
                colors::RESET
            );
            render_stage_details(details);
        }

        BootEvent::StageFailed {
            stage,
            duration,
            error,
        } => {
            print!("\r");
            println!(
                "{}{} {}{} {}({:.0}ms){}",
                colors::RED,
                FAILURE,
                colors::BOLD,
                stage.name(),
                colors::DIM,
                duration.as_millis(),
                colors::RESET
            );
            println!("  {}Error: {}{}", colors::RED, error, colors::RESET);
        }

        BootEvent::StageSkipped { stage, reason } => {
            println!(
                "  {}{} {} - {}{}",
                colors::DIM,
                PENDING,
                stage.name(),
                reason,
                colors::RESET
            );
        }

        BootEvent::BootCompleted {
            manifest: _,
            total_duration,
            summary,
        } => {
            println!();
            println!(
                "{}{}=== Boot Complete ==={} {}({:.0}ms){}",
                colors::BOLD,
                colors::GREEN,
                colors::RESET,
                colors::DIM,
                total_duration.as_millis(),
                colors::RESET
            );

            if let Some(summary_text) = summary {
                println!();
                for line in summary_text.lines() {
                    println!("{}{}{}", colors::CYAN, line, colors::RESET);
                }
            }
            println!();
        }

        BootEvent::BootFailed { error } => {
            println!();
            println!(
                "{}{}=== Boot Failed ==={}",
                colors::BOLD,
                colors::RED,
                colors::RESET
            );
            println!("{}{}{}", colors::RED, error, colors::RESET);
            println!();
        }
    }
}

/// Render stage-specific details with indentation.
fn render_stage_details(details: &StageDetails) {
    match details {
        StageDetails::Hardware(hw) => {
            println!(
                "  {}CPU: {} ({} cores){}",
                colors::DIM,
                hw.cpu_model,
                hw.cpu_cores,
                colors::RESET
            );
            println!("  {}RAM: {:.1} GB{}", colors::DIM, hw.ram_gb, colors::RESET);
            if hw.apple_silicon {
                println!(
                    "  {}GPU: Apple Silicon (Metal){}",
                    colors::DIM,
                    colors::RESET
                );
            } else if hw.gpu_count > 0 {
                println!(
                    "  {}GPU: {} device(s){}",
                    colors::DIM,
                    hw.gpu_count,
                    colors::RESET
                );
            }
        }

        StageDetails::Compute(comp) => {
            if comp.backends.is_empty() {
                println!(
                    "  {}No local backends detected{}",
                    colors::DIM,
                    colors::RESET
                );
            } else {
                for backend in &comp.backends {
                    let status = if backend.ready {
                        format!("{}{}{}", colors::GREEN, SUCCESS, colors::RESET)
                    } else {
                        format!("{}{}{}", colors::RED, FAILURE, colors::RESET)
                    };
                    println!(
                        "  {} {} {}({} models){}",
                        status,
                        backend.name,
                        colors::DIM,
                        backend.model_count,
                        colors::RESET
                    );
                }
            }
            if comp.has_local_llm {
                println!(
                    "  {}LLM: Ready for local inference{}",
                    colors::DIM,
                    colors::RESET
                );
            }
        }

        StageDetails::Network(net) => {
            let inet_status = if net.has_internet {
                format!("{}{}{}", colors::GREEN, SUCCESS, colors::RESET)
            } else {
                format!("{}{}{}", colors::RED, FAILURE, colors::RESET)
            };
            println!("  {} Internet connectivity", inet_status);
            println!(
                "  {}Relays: {}/{} connected{}",
                colors::DIM,
                net.relays_connected,
                net.relays_total,
                colors::RESET
            );
        }

        StageDetails::Identity(id) => {
            if id.initialized {
                if let Some(npub) = &id.npub {
                    let short_npub = if npub.len() > 20 {
                        format!("{}...{}", &npub[..8], &npub[npub.len() - 8..])
                    } else {
                        npub.clone()
                    };
                    println!("  {}Pubkey: {}{}", colors::DIM, short_npub, colors::RESET);
                }
                if id.has_wallet {
                    println!("  {}Wallet: Available{}", colors::DIM, colors::RESET);
                }
            } else {
                println!(
                    "  {}Not initialized - run 'pylon init'{}",
                    colors::DIM,
                    colors::RESET
                );
            }
        }

        StageDetails::Workspace(ws) => {
            if let Some(name) = &ws.project_name {
                println!("  {}Project: {}{}", colors::DIM, name, colors::RESET);
            }
            if !ws.language_hints.is_empty() {
                println!(
                    "  {}Languages: {}{}",
                    colors::DIM,
                    ws.language_hints.join(", "),
                    colors::RESET
                );
            }
            if ws.open_issues > 0 {
                println!(
                    "  {}Issues: {} open{}",
                    colors::DIM,
                    ws.open_issues,
                    colors::RESET
                );
            }
            if let Some(directive) = &ws.active_directive {
                println!("  {}Active: {}{}", colors::DIM, directive, colors::RESET);
            }
        }

        StageDetails::Summary(sum) => {
            println!(
                "  {}Lane: {}{}",
                colors::DIM,
                sum.recommended_lane,
                colors::RESET
            );
        }

        StageDetails::Issues(issues) => {
            println!(
                "  {}Evaluated: {} issues{}",
                colors::DIM,
                issues.total_evaluated,
                colors::RESET
            );
            if issues.suggestions_found > 0 {
                println!(
                    "  {}Found: {} actionable suggestions{}",
                    colors::DIM,
                    issues.suggestions_found,
                    colors::RESET
                );
            }
            println!(
                "  {}Provider: {}{}",
                colors::DIM,
                issues.provider,
                colors::RESET
            );
        }
    }
}

/// Format for structured logging (non-interactive).
pub fn render_event_structured(event: &BootEvent) -> String {
    match event {
        BootEvent::StageCompleted {
            stage, duration, ..
        } => {
            format!(
                "[BOOT] {} completed in {}ms",
                stage.name(),
                duration.as_millis()
            )
        }
        BootEvent::StageFailed { stage, error, .. } => {
            format!("[BOOT] {} failed: {}", stage.name(), error)
        }
        BootEvent::BootCompleted { total_duration, .. } => {
            format!("[BOOT] Complete in {}ms", total_duration.as_millis())
        }
        _ => String::new(),
    }
}

/// Render all boot events to a formatted string (for testing or non-terminal output).
pub fn render_to_string(events: &[BootEvent]) -> String {
    let mut output = String::new();

    for event in events {
        match event {
            BootEvent::BootStarted { total_stages } => {
                output.push_str(&format!(
                    "=== OpenAgents Boot ===\nStages: {}\n\n",
                    total_stages
                ));
            }
            BootEvent::StageCompleted {
                stage, duration, ..
            } => {
                output.push_str(&format!(
                    "{} {} ({}ms)\n",
                    SUCCESS,
                    stage.name(),
                    duration.as_millis()
                ));
            }
            BootEvent::StageFailed {
                stage,
                duration,
                error,
            } => {
                output.push_str(&format!(
                    "{} {} ({}ms)\n  Error: {}\n",
                    FAILURE,
                    stage.name(),
                    duration.as_millis(),
                    error
                ));
            }
            BootEvent::StageSkipped { stage, reason } => {
                output.push_str(&format!("  {} {} - {}\n", PENDING, stage.name(), reason));
            }
            BootEvent::BootCompleted {
                total_duration,
                summary,
                ..
            } => {
                output.push_str(&format!(
                    "\n=== Boot Complete === ({}ms)\n",
                    total_duration.as_millis()
                ));
                if let Some(s) = summary {
                    output.push_str(&format!("\n{}\n", s));
                }
            }
            _ => {}
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::bootloader::events::{BootStage, HardwareDetails};
    use std::time::Duration;

    #[test]
    fn test_render_to_string() {
        let events = vec![
            BootEvent::BootStarted { total_stages: 3 },
            BootEvent::StageCompleted {
                stage: BootStage::Hardware,
                duration: Duration::from_millis(15),
                details: StageDetails::Hardware(HardwareDetails {
                    cpu_cores: 8,
                    cpu_model: "Apple M2".to_string(),
                    ram_gb: 16.0,
                    apple_silicon: true,
                    gpu_count: 1,
                }),
            },
            BootEvent::BootCompleted {
                manifest: crate::app::bootloader::signatures::BootManifest {
                    hardware: Default::default(),
                    compute: Default::default(),
                    network: crate::app::bootloader::signatures::NetworkProbeOutput::offline(),
                    identity: crate::app::bootloader::signatures::IdentityProbeOutput::unknown(),
                    workspace: None,
                    summary: None,
                    boot_duration: Duration::from_millis(100),
                },
                total_duration: Duration::from_millis(100),
                summary: Some("Ready!".to_string()),
            },
        ];

        let output = render_to_string(&events);
        assert!(output.contains("OpenAgents Boot"));
        assert!(output.contains("Hardware"));
        assert!(output.contains("Boot Complete"));
        assert!(output.contains("Ready!"));
    }
}
