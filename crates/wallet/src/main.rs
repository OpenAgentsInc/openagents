//! OpenAgents Wallet
//!
//! A unified Nostr identity and Bitcoin payment solution that combines:
//! - Nostr identity (NIP-06 key derivation)
//! - Bitcoin/Lightning payments via Spark L2
//! - Both sharing a single BIP39 mnemonic seed
//!
//! # Usage
//!
//! ```bash
//! # Initialize new wallet
//! cargo wallet init
//!
//! # Show wallet info
//! cargo wallet whoami
//!
//! # Check balance
//! cargo wallet balance
//!
//! # Send a payment
//! cargo wallet send <address> <amount>
//! ```

use clap::{Parser, Subcommand};
use colored::Colorize;
use std::process;

mod cli;
mod core;
mod storage;

#[derive(Parser)]
#[command(name = "wallet")]
#[command(version, about = "OpenAgents Wallet - Unified Nostr identity and Bitcoin payments", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,
}

#[derive(Subcommand)]
enum Commands {
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

    /// Nostr identity commands
    #[command(subcommand)]
    Profile(ProfileCommands),

    /// Contact management
    #[command(subcommand)]
    Contacts(ContactsCommands),

    /// Post to Nostr
    Post {
        /// Content to post
        content: String,
    },

    /// Send direct message
    Dm {
        /// Recipient npub
        recipient: String,

        /// Message content
        message: String,
    },

    /// Show Nostr feed
    Feed {
        /// Number of events to display
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Bitcoin/Lightning commands
    #[command(subcommand)]
    Bitcoin(BitcoinCommands),

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

    /// Generate Lightning invoice
    Invoice {
        /// Amount in sats
        amount: u64,

        /// Description
        #[arg(short, long)]
        description: Option<String>,
    },

    /// Pay Lightning invoice
    Pay {
        /// BOLT-11 invoice
        invoice: String,
    },

    /// Transaction history
    History {
        /// Number of transactions to display
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Zap a Nostr note
    Zap {
        /// Note ID or nevent
        note_id: String,

        /// Amount in sats
        amount: u64,
    },

    /// List zaps on a note
    Zaps {
        /// Note ID or nevent
        note_id: String,
    },

    /// Nostr Wallet Connect (NWC) commands
    #[command(subcommand)]
    Nwc(NwcCommands),

    /// Relay management
    #[command(subcommand)]
    Relays(RelayCommands),

    /// Wallet settings
    #[command(subcommand)]
    Settings(SettingsCommands),
}

#[derive(Subcommand)]
enum ProfileCommands {
    /// Show current profile
    Show,

    /// Set profile fields
    Set {
        /// Display name
        #[arg(long)]
        name: Option<String>,

        /// About text
        #[arg(long)]
        about: Option<String>,

        /// Profile picture URL
        #[arg(long)]
        picture: Option<String>,

        /// NIP-05 identifier
        #[arg(long)]
        nip05: Option<String>,
    },
}

#[derive(Subcommand)]
enum ContactsCommands {
    /// List contacts
    List,

    /// Add a contact
    Add {
        /// Contact npub
        npub: String,

        /// Petname (optional)
        #[arg(short, long)]
        name: Option<String>,
    },

    /// Remove a contact
    Remove {
        /// Contact npub
        npub: String,
    },
}

#[derive(Subcommand)]
enum BitcoinCommands {
    /// Show balance breakdown (Spark, Lightning, on-chain)
    Balance,

    /// On-chain deposit address
    Deposit,

    /// Withdraw to on-chain address (cooperative exit)
    Withdraw {
        /// Destination address
        address: String,

        /// Amount in sats
        amount: u64,
    },
}

#[derive(Subcommand)]
enum NwcCommands {
    /// Create new NWC connection
    Create {
        /// Optional name for this connection
        #[arg(short, long)]
        name: Option<String>,
    },

    /// List active NWC connections
    List,

    /// Revoke a connection
    Revoke {
        /// Connection ID
        id: String,
    },
}

#[derive(Subcommand)]
enum RelayCommands {
    /// List configured relays
    List,

    /// Add a relay
    Add {
        /// Relay URL
        url: String,

        /// Marker (read, write, or both)
        #[arg(short, long)]
        marker: Option<String>,
    },

    /// Remove a relay
    Remove {
        /// Relay URL
        url: String,
    },
}

#[derive(Subcommand)]
enum SettingsCommands {
    /// Show current settings
    Show,

    /// Set a configuration value
    Set {
        /// Setting key
        key: String,

        /// Setting value
        value: String,
    },
}

fn main() {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .init();

    // Run command
    if let Err(e) = run(cli.command) {
        eprintln!("{} {}", "Error:".red().bold(), e);
        process::exit(1);
    }
}

fn run(command: Commands) -> anyhow::Result<()> {
    match command {
        Commands::Init { show_mnemonic } => {
            cli::identity::init(show_mnemonic)
        }
        Commands::Import { mnemonic } => {
            cli::identity::import(mnemonic)
        }
        Commands::Export => {
            cli::identity::export()
        }
        Commands::Whoami => {
            cli::identity::whoami()
        }
        Commands::Profile(cmd) => match cmd {
            ProfileCommands::Show => cli::identity::profile_show(),
            ProfileCommands::Set { name, about, picture, nip05 } => {
                cli::identity::profile_set(name, about, picture, nip05)
            }
        },
        Commands::Contacts(cmd) => match cmd {
            ContactsCommands::List => cli::identity::contacts_list(),
            ContactsCommands::Add { npub, name } => cli::identity::contacts_add(npub, name),
            ContactsCommands::Remove { npub } => cli::identity::contacts_remove(npub),
        },
        Commands::Post { content } => {
            cli::identity::post(content)
        }
        Commands::Dm { recipient, message } => {
            cli::identity::dm(recipient, message)
        }
        Commands::Feed { limit } => {
            cli::identity::feed(limit)
        }
        Commands::Bitcoin(cmd) => match cmd {
            BitcoinCommands::Balance => cli::bitcoin::balance_detailed(),
            BitcoinCommands::Deposit => cli::bitcoin::deposit(),
            BitcoinCommands::Withdraw { address, amount } => {
                cli::bitcoin::withdraw(address, amount)
            }
        },
        Commands::Balance => {
            cli::bitcoin::balance()
        }
        Commands::Receive { amount } => {
            cli::bitcoin::receive(amount)
        }
        Commands::Send { address, amount } => {
            cli::bitcoin::send(address, amount)
        }
        Commands::Invoice { amount, description } => {
            cli::bitcoin::invoice(amount, description)
        }
        Commands::Pay { invoice } => {
            cli::bitcoin::pay(invoice)
        }
        Commands::History { limit } => {
            cli::bitcoin::history(limit)
        }
        Commands::Zap { note_id, amount } => {
            cli::bitcoin::zap(note_id, amount)
        }
        Commands::Zaps { note_id } => {
            cli::bitcoin::zaps(note_id)
        }
        Commands::Nwc(cmd) => match cmd {
            NwcCommands::Create { name } => cli::bitcoin::nwc_create(name),
            NwcCommands::List => cli::bitcoin::nwc_list(),
            NwcCommands::Revoke { id } => cli::bitcoin::nwc_revoke(id),
        },
        Commands::Relays(cmd) => match cmd {
            RelayCommands::List => cli::settings::relays_list(),
            RelayCommands::Add { url, marker } => cli::settings::relays_add(url, marker),
            RelayCommands::Remove { url } => cli::settings::relays_remove(url),
        },
        Commands::Settings(cmd) => match cmd {
            SettingsCommands::Show => cli::settings::show(),
            SettingsCommands::Set { key, value } => cli::settings::set(key, value),
        },
    }
}
