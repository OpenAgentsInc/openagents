use clap::{Args, Subcommand};
use std::process::Command;

#[derive(Args, Default)]
pub struct AutopilotArgs {
    #[command(subcommand)]
    command: Option<AutopilotCommands>,
}

#[derive(Subcommand)]
pub enum AutopilotCommands {
    /// Launch the Autopilot IDE
    Gui,

    /// Run Autopilot in CLI mode
    Run,
}

pub fn run(args: AutopilotArgs, verbose: bool) -> anyhow::Result<()> {
    match args.command {
        None | Some(AutopilotCommands::Gui) => run_autopilot_gui(),
        Some(AutopilotCommands::Run) => autopilot_service::cli::run_cli(verbose),
    }
}

/// Launch the Autopilot GUI binary
fn run_autopilot_gui() -> anyhow::Result<()> {
    // Find the autopilot binary - check same directory as current executable first
    let current_exe = std::env::current_exe()?;
    let exe_dir = current_exe.parent().unwrap();
    let autopilot_path = exe_dir.join("autopilot");

    if autopilot_path.exists() {
        let status = Command::new(&autopilot_path).status()?;
        if !status.success() {
            anyhow::bail!("Autopilot exited with status: {:?}", status.code());
        }
    } else {
        // Fall back to cargo run during development
        let status = Command::new("cargo")
            .args(["run", "--bin", "autopilot"])
            .status()?;
        if !status.success() {
            anyhow::bail!("Autopilot exited with status: {:?}", status.code());
        }
    }
    Ok(())
}
