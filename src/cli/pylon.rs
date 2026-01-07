//! Pylon CLI wrapper for unified binary

pub use pylon::cli::Commands as PylonCommands;

/// Run a pylon command
pub fn run(cmd: PylonCommands) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        match cmd {
            PylonCommands::Init(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Init(args),
                })
                .await
            }
            PylonCommands::Start(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Start(args),
                })
                .await
            }
            PylonCommands::Stop(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Stop(args),
                })
                .await
            }
            PylonCommands::Status(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Status(args),
                })
                .await
            }
            PylonCommands::Doctor(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Doctor(args),
                })
                .await
            }
            PylonCommands::Agent(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Agent(args),
                })
                .await
            }
            PylonCommands::Earnings(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Earnings(args),
                })
                .await
            }
            PylonCommands::Compute(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Compute(args),
                })
                .await
            }
            PylonCommands::Connect(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Connect(args),
                })
                .await
            }
            PylonCommands::Wallet(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Wallet(args),
                })
                .await
            }
            PylonCommands::Job(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Job(args),
                })
                .await
            }
            PylonCommands::Api(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Api(args),
                })
                .await
            }
            PylonCommands::Infer(args) => {
                pylon::cli::execute(pylon::cli::PylonCli {
                    command: pylon::cli::Commands::Infer(args),
                })
                .await
            }
        }
    })
}
