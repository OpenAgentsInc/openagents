use clap::{Args, Subcommand};

#[derive(Args)]
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
        None | Some(AutopilotCommands::Gui) => autopilot_app::run(),
        Some(AutopilotCommands::Run) => autopilot_service::cli::run_cli(verbose),
    }
}
