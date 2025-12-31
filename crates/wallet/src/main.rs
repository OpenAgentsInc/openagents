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
use wallet::deprecation;

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

    /// Wallet password management
    #[command(subcommand)]
    Password(PasswordCommands),

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

    /// Direct message commands
    #[command(subcommand)]
    Dm(DmCommands),

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

    /// FROSTR threshold signing operations
    #[command(subcommand)]
    Frostr(FrostrCommands),
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
enum DmCommands {
    /// Send encrypted direct message
    Send {
        /// Recipient npub
        recipient: String,

        /// Message content
        message: String,
    },

    /// List received direct messages
    List {
        /// Number of messages to display
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Read a specific direct message
    Read {
        /// Event ID of the message
        event_id: String,
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

    /// Listen for NWC requests
    Listen,

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

#[derive(Subcommand)]
enum PasswordCommands {
    /// Set or change the wallet password
    Set {
        /// New password (will prompt if not provided)
        #[arg(long)]
        password: Option<String>,

        /// Current password (required if already protected)
        #[arg(long)]
        current_password: Option<String>,
    },
}

#[derive(Subcommand)]
enum FrostrCommands {
    /// Generate threshold key shares
    Keygen {
        /// Minimum signers required (threshold k)
        #[arg(short, long)]
        threshold: u16,

        /// Total number of shares (n)
        #[arg(short = 'n', long)]
        total: u16,
    },

    /// Import a FROSTR share credential
    ImportShare {
        /// Share credential (bfshare1...)
        credential: String,
    },

    /// Export the local FROSTR share
    ExportShare,

    /// Sign an event hash using threshold shares
    Sign {
        /// Event hash (64-character hex)
        event_hash: String,
    },

    /// Show FROSTR node status and peer connectivity
    Status,

    /// List available group credentials
    ListGroups,

    /// Manage threshold signing peers
    #[command(subcommand)]
    Peers(PeersCommands),
}

#[derive(Subcommand)]
enum PeersCommands {
    /// Add a threshold signing peer
    Add {
        /// Peer's Nostr public key (npub1... or hex)
        npub: String,

        /// Relay URLs for this peer (can specify multiple times)
        #[arg(short, long)]
        relay: Vec<String>,

        /// Optional name for this peer
        #[arg(short, long)]
        name: Option<String>,
    },

    /// List configured peers
    List,

    /// Remove a peer
    Remove {
        /// Peer's Nostr public key (npub1... or hex)
        npub: String,
    },
}

fn main() {
    eprintln!("{}", deprecation::legacy_warning());

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
        Commands::Init { show_mnemonic } => cli::identity::init(show_mnemonic),
        Commands::Import { mnemonic } => cli::identity::import(mnemonic),
        Commands::Export => cli::identity::export(),
        Commands::Password(cmd) => match cmd {
            PasswordCommands::Set {
                password,
                current_password,
            } => cli::password::set(password, current_password),
        },
        Commands::Whoami => cli::identity::whoami(),
        Commands::Profile(cmd) => match cmd {
            ProfileCommands::Show => cli::identity::profile_show(),
            ProfileCommands::Set {
                name,
                about,
                picture,
                nip05,
            } => cli::identity::profile_set(name, about, picture, nip05),
        },
        Commands::Contacts(cmd) => match cmd {
            ContactsCommands::List => cli::identity::contacts_list(),
            ContactsCommands::Add { npub, name } => cli::identity::contacts_add(npub, name),
            ContactsCommands::Remove { npub } => cli::identity::contacts_remove(npub),
        },
        Commands::Post { content } => cli::identity::post(content),
        Commands::Dm(cmd) => match cmd {
            DmCommands::Send { recipient, message } => cli::identity::dm_send(recipient, message),
            DmCommands::List { limit } => cli::identity::dm_list(limit),
            DmCommands::Read { event_id } => cli::identity::dm_read(event_id),
        },
        Commands::Feed { limit } => cli::identity::feed(limit),
        Commands::Bitcoin(cmd) => match cmd {
            BitcoinCommands::Balance => {
                anyhow::bail!(
                    "Bitcoin balance commands require Spark SDK integration (d-001).\n\
                    The Breez SDK integration is pending. See crates/spark/src/wallet.rs.\n\n\
                    Track progress: directive d-001"
                )
            }
            BitcoinCommands::Deposit => {
                anyhow::bail!(
                    "Deposit commands require Spark SDK integration (d-001).\n\
                    The Breez SDK integration is pending. See crates/spark/src/wallet.rs.\n\n\
                    Track progress: directive d-001"
                )
            }
            BitcoinCommands::Withdraw {
                address: _,
                amount: _,
            } => {
                anyhow::bail!(
                    "Withdraw commands require Spark SDK integration (d-001).\n\
                    The Breez SDK integration is pending. See crates/spark/src/wallet.rs.\n\n\
                    Track progress: directive d-001"
                )
            }
        },
        Commands::Balance => {
            anyhow::bail!(
                "Balance command requires Spark SDK integration (d-001).\n\
                The Breez SDK integration is pending. See crates/spark/src/wallet.rs.\n\n\
                Track progress: directive d-001"
            )
        }
        Commands::Receive { amount: _ } => {
            anyhow::bail!(
                "Receive command requires Spark SDK integration (d-001).\n\
                The Breez SDK integration is pending. See crates/spark/src/wallet.rs.\n\n\
                Track progress: directive d-001"
            )
        }
        Commands::Send {
            address: _,
            amount: _,
        } => {
            anyhow::bail!(
                "Send command requires Spark SDK integration (d-001).\n\
                The Breez SDK integration is pending. See crates/spark/src/wallet.rs.\n\n\
                Track progress: directive d-001"
            )
        }
        Commands::Invoice {
            amount: _,
            description: _,
        } => {
            anyhow::bail!(
                "Invoice command requires Spark SDK integration (d-001).\n\
                The Breez SDK integration is pending. See crates/spark/src/wallet.rs.\n\n\
                Track progress: directive d-001"
            )
        }
        Commands::Pay { invoice: _ } => {
            anyhow::bail!(
                "Pay command requires Spark SDK integration (d-001).\n\
                The Breez SDK integration is pending. See crates/spark/src/wallet.rs.\n\n\
                Track progress: directive d-001"
            )
        }
        Commands::History { limit: _ } => {
            anyhow::bail!(
                "History command requires Spark SDK integration (d-001).\n\
                The Breez SDK integration is pending. See crates/spark/src/wallet.rs.\n\n\
                Track progress: directive d-001"
            )
        }
        Commands::Zap { note_id, amount } => cli::bitcoin::zap(note_id, amount),
        Commands::Zaps { note_id } => cli::bitcoin::zaps(note_id),
        Commands::Nwc(cmd) => match cmd {
            NwcCommands::Create { name } => cli::bitcoin::nwc_create(name),
            NwcCommands::List => cli::bitcoin::nwc_list(),
            NwcCommands::Listen => cli::bitcoin::nwc_listen(),
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
        Commands::Frostr(cmd) => {
            let runtime = tokio::runtime::Runtime::new()?;
            runtime.block_on(async {
                match cmd {
                    FrostrCommands::Keygen { threshold, total } => {
                        cli::frostr::keygen(threshold, total).await
                    }
                    FrostrCommands::ImportShare { credential } => {
                        cli::frostr::import_share(credential).await
                    }
                    FrostrCommands::ExportShare => cli::frostr::export_share().await,
                    FrostrCommands::Sign { event_hash } => cli::frostr::sign(event_hash).await,
                    FrostrCommands::Status => cli::frostr::status().await,
                    FrostrCommands::ListGroups => cli::frostr::list_groups().await,
                    FrostrCommands::Peers(peers_cmd) => match peers_cmd {
                        PeersCommands::Add { npub, relay, name } => {
                            cli::frostr::peers_add(npub, relay, name).await
                        }
                        PeersCommands::List => cli::frostr::peers_list().await,
                        PeersCommands::Remove { npub } => cli::frostr::peers_remove(npub).await,
                    },
                }
            })
        }
    }
}
