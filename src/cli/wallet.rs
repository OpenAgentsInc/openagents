//! Wallet CLI subcommands
//!
//! Wraps wallet crate CLI functions for unified binary.

use clap::Subcommand;

#[derive(Subcommand)]
pub enum WalletCommands {
    /// Launch the wallet GUI
    Gui,

    /// Initialize a new wallet with generated mnemonic
    Init {
        /// Display the mnemonic (WARNING: insecure)
        #[arg(long)]
        show_mnemonic: bool,
    },

    /// Import existing wallet from mnemonic
    Import {
        /// Mnemonic phrase (will prompt if not provided)
        #[arg(long)]
        mnemonic: Option<String>,
    },

    /// Export wallet mnemonic (requires confirmation)
    Export,

    /// Display wallet information (npub, balances, profile)
    Whoami,

    /// Show wallet balance
    Balance,

    /// Generate receiving address
    Receive {
        /// Amount to request (optional)
        #[arg(short, long)]
        amount: Option<u64>,
    },

    /// Send payment
    Send {
        /// Destination address (Bitcoin, Lightning, or Spark)
        address: String,
        /// Amount in sats
        amount: u64,
    },

    /// Transaction history
    History {
        /// Number of transactions to display
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },
}

pub fn run(cmd: WalletCommands) -> anyhow::Result<()> {
    match cmd {
        WalletCommands::Gui => wallet::gui::run_gui(),
        WalletCommands::Init { show_mnemonic } => wallet::cli::identity::init(show_mnemonic),
        WalletCommands::Import { mnemonic } => wallet::cli::identity::import(mnemonic),
        WalletCommands::Export => wallet::cli::identity::export(),
        WalletCommands::Whoami => wallet::cli::identity::whoami(),
        WalletCommands::Balance => wallet::cli::bitcoin::balance(),
        WalletCommands::Receive { amount } => wallet::cli::bitcoin::receive(amount),
        WalletCommands::Send { address, amount } => wallet::cli::bitcoin::send(address, amount),
        WalletCommands::History { limit } => wallet::cli::bitcoin::history(limit),
    }
}
