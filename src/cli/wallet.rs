//! Wallet CLI subcommands
//!
//! Wraps wallet crate CLI functions for unified binary.

use clap::Subcommand;
use std::path::PathBuf;

#[derive(Subcommand)]
pub enum WalletCommands {
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

    /// Identity management
    #[command(subcommand)]
    Identity(IdentityCommands),

    /// Display wallet information (npub, balances, profile)
    Whoami,

    /// Show wallet balance
    Balance,

    /// Show Spark network status
    Status,

    /// Launch the wallet GUI
    Gui,

    /// Generate receiving address
    Receive {
        /// Amount to request (optional)
        #[arg(short, long)]
        amount: Option<u64>,

        /// Invoice expiry in seconds from now
        #[arg(long)]
        expiry: Option<u64>,

        /// Show a QR code for the invoice or address
        #[arg(long)]
        qr: bool,

        /// Copy the invoice or address to clipboard
        #[arg(long)]
        copy: bool,
    },

    /// Listen for incoming payments and show notifications
    Notify,

    /// Send payment
    Send {
        /// Destination address (Bitcoin, Lightning, or Spark). Use '-' with --qr.
        address: String,
        /// Amount in sats
        amount: u64,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,

        /// Read destination from a QR image file
        #[arg(long)]
        qr: Option<PathBuf>,

        /// Use a saved payee name instead of typing an address
        #[arg(long)]
        payee: Option<String>,
    },

    /// Retry a failed payment using the original invoice details
    Retry {
        /// Payment ID to retry (from history)
        #[arg(value_name = "PAYMENT_ID", required_unless_present = "last")]
        payment_id: Option<String>,

        /// Retry the most recent failed outgoing payment
        #[arg(long, conflicts_with = "payment_id")]
        last: bool,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },

    /// Transaction history
    History {
        /// Number of transactions to display
        #[arg(short, long, default_value = "20")]
        limit: usize,

        /// Output format (table or csv)
        #[arg(long, default_value = "table")]
        format: String,

        /// Optional output file (CSV only)
        #[arg(long)]
        output: Option<PathBuf>,
    },

    /// Nostr profile commands
    #[command(subcommand)]
    Profile(ProfileCommands),

    /// Contact management
    #[command(subcommand)]
    Contacts(ContactsCommands),

    /// Saved payee management
    #[command(subcommand)]
    Payee(PayeeCommands),

    /// Post a note to Nostr
    Post {
        /// Content to post
        content: String,
    },

    /// Direct message commands
    #[command(subcommand)]
    Dm(DmCommands),
}

#[derive(Subcommand)]
pub enum ProfileCommands {
    /// Show profile information
    Show,

    /// Set profile fields
    Set {
        /// Display name
        #[arg(long)]
        name: Option<String>,

        /// About/bio
        #[arg(long)]
        about: Option<String>,

        /// Profile picture URL
        #[arg(long)]
        picture: Option<String>,

        /// NIP-05 identifier (name@domain.com)
        #[arg(long)]
        nip05: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ContactsCommands {
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
pub enum DmCommands {
    /// Send an encrypted DM
    Send {
        /// Recipient npub or nprofile
        recipient: String,

        /// Message content
        message: String,
    },

    /// List received DMs
    List {
        /// Number of messages to display
        #[arg(short, long, default_value = "20")]
        limit: usize,
    },

    /// Read a specific DM by event ID
    Read {
        /// Event ID of the message
        event_id: String,
    },
}

#[derive(Subcommand)]
pub enum PayeeCommands {
    /// List saved payees
    List,

    /// Add a payee
    Add {
        /// Payee name
        name: String,

        /// Payment address or invoice
        address: String,
    },

    /// Remove a payee
    Remove {
        /// Payee name
        name: String,
    },
}

#[derive(Subcommand)]
pub enum PasswordCommands {
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
pub enum IdentityCommands {
    /// List available identities
    List,

    /// Show the active identity
    Current,

    /// Create a new identity
    Create {
        /// Identity name (letters, numbers, '-' or '_')
        name: String,

        /// Display the mnemonic (WARNING: insecure)
        #[arg(long)]
        show_mnemonic: bool,
    },

    /// Import an identity from mnemonic
    Import {
        /// Identity name (letters, numbers, '-' or '_')
        name: String,

        /// Mnemonic phrase (will prompt if not provided)
        #[arg(long)]
        mnemonic: Option<String>,
    },

    /// Switch the active identity
    Use {
        /// Identity name to activate
        name: String,
    },

    /// Remove an identity
    Remove {
        /// Identity name to remove
        name: String,

        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
    },
}

pub fn run(cmd: WalletCommands) -> anyhow::Result<()> {
    match cmd {
        WalletCommands::Init { show_mnemonic } => wallet::cli::identity::init(show_mnemonic),
        WalletCommands::Import { mnemonic } => wallet::cli::identity::import(mnemonic),
        WalletCommands::Export => wallet::cli::identity::export(),
        WalletCommands::Password(cmd) => match cmd {
            PasswordCommands::Set {
                password,
                current_password,
            } => wallet::cli::password::set(password, current_password),
        },
        WalletCommands::Identity(cmd) => match cmd {
            IdentityCommands::List => wallet::cli::identity::identities_list(),
            IdentityCommands::Current => wallet::cli::identity::identity_current(),
            IdentityCommands::Create {
                name,
                show_mnemonic,
            } => wallet::cli::identity::identity_create(name, show_mnemonic),
            IdentityCommands::Import { name, mnemonic } => {
                wallet::cli::identity::identity_import(name, mnemonic)
            }
            IdentityCommands::Use { name } => wallet::cli::identity::identity_use(name),
            IdentityCommands::Remove { name, yes } => {
                wallet::cli::identity::identity_remove(name, yes)
            }
        },
        WalletCommands::Whoami => wallet::cli::identity::whoami(),
        WalletCommands::Balance => wallet::cli::bitcoin::balance(),
        WalletCommands::Status => wallet::cli::bitcoin::status(),
        WalletCommands::Gui => wallet::gui::run_gui(),
        WalletCommands::Receive {
            amount,
            expiry,
            qr,
            copy,
        } => wallet::cli::bitcoin::receive(amount, qr, copy, expiry),
        WalletCommands::Notify => wallet::cli::bitcoin::notify(),
        WalletCommands::Send {
            address,
            amount,
            yes,
            qr,
            payee,
        } => wallet::cli::bitcoin::send(address, amount, yes, qr, payee),
        WalletCommands::Retry {
            payment_id,
            last,
            yes,
        } => wallet::cli::bitcoin::retry(payment_id, last, yes),
        WalletCommands::History {
            limit,
            format,
            output,
        } => {
            let format = wallet::cli::bitcoin::HistoryFormat::parse(&format)?;
            wallet::cli::bitcoin::history(limit, format, output)
        }
        WalletCommands::Profile(cmd) => match cmd {
            ProfileCommands::Show => wallet::cli::identity::profile_show(),
            ProfileCommands::Set {
                name,
                about,
                picture,
                nip05,
            } => wallet::cli::identity::profile_set(name, about, picture, nip05),
        },
        WalletCommands::Contacts(cmd) => match cmd {
            ContactsCommands::List => wallet::cli::identity::contacts_list(),
            ContactsCommands::Add { npub, name } => wallet::cli::identity::contacts_add(npub, name),
            ContactsCommands::Remove { npub } => wallet::cli::identity::contacts_remove(npub),
        },
        WalletCommands::Payee(cmd) => match cmd {
            PayeeCommands::List => wallet::cli::payee::list(),
            PayeeCommands::Add { name, address } => wallet::cli::payee::add(name, address),
            PayeeCommands::Remove { name } => wallet::cli::payee::remove(name),
        },
        WalletCommands::Post { content } => wallet::cli::identity::post(content),
        WalletCommands::Dm(cmd) => match cmd {
            DmCommands::Send { recipient, message } => {
                wallet::cli::identity::dm_send(recipient, message)
            }
            DmCommands::List { limit } => wallet::cli::identity::dm_list(limit),
            DmCommands::Read { event_id } => wallet::cli::identity::dm_read(event_id),
        },
    }
}
